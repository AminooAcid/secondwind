//! Kiosk binary: watches the agent's kiosk state file, paints SecondWind
//! screens fullscreen, and supervises the streaming client.

use std::{
    io::Write,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use crossterm::{
    cursor, event, execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType},
};
use sw_core::{KioskState, kiosk::read_kiosk_state};
use sw_kiosk::{
    screens,
    supervise::{
        CommandSpec, KioskAction, KioskRuntimeConfig, Supervisor, pair_command, stream_command,
    },
};

const PAIR_TIMEOUT: Duration = Duration::from_secs(60);

fn main() {
    let config = match KioskRuntimeConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("sw-kiosk: {error}");
            std::process::exit(2);
        }
    };

    if let Err(error) = run(&config) {
        let _ = restore_terminal();
        eprintln!("sw-kiosk: {error}");
        std::process::exit(1);
    }

    let _ = restore_terminal();
}

fn run(config: &KioskRuntimeConfig) -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let mut supervisor = Supervisor::default();
    let mut client: Option<Child> = None;
    let mut last_painted: Option<(KioskState, Option<screens::AmbientStats>)> = None;
    let mut last_failure_at: Option<Instant> = None;

    loop {
        let state = read_kiosk_state(&config.state_file).unwrap_or(KioskState::Starting);

        match supervisor.decide(&state) {
            KioskAction::ShowScreen => {
                stop_client(&mut client);
                // Idle gets the ambient extras (clock + light stats).
                let stats = match &state {
                    KioskState::Idle { .. } => ambient_stats(),
                    _ => None,
                };
                let key = (state.clone(), stats.clone());
                if last_painted.as_ref() != Some(&key) {
                    paint(
                        &mut stdout,
                        &screens::render_with_stats(&state, stats.as_ref()),
                    )?;
                    last_painted = Some(key);
                }
            }
            KioskAction::EnsureStreaming {
                host_address,
                pair_first,
            } => {
                // Reap a finished client, count the failure for backoff.
                if let Some(child) = client.as_mut()
                    && child.try_wait()?.is_some()
                {
                    client = None;
                    supervisor.record_failure();
                    last_failure_at = Some(Instant::now());
                }

                let backoff_over = last_failure_at
                    .map(|at| at.elapsed() >= supervisor.restart_backoff())
                    .unwrap_or(true);

                if client.is_none() && backoff_over {
                    let key = (state.clone(), None);
                    if last_painted.as_ref() != Some(&key) {
                        paint(&mut stdout, &screens::render(&state))?;
                        last_painted = Some(key);
                    }

                    if let Some(pin) = &pair_first {
                        // One-shot inner pairing; success or failure, never
                        // retried with the same PIN (the host arms a new one
                        // on the next connect if needed).
                        let paired = run_to_completion(
                            &pair_command(config, &host_address, pin),
                            PAIR_TIMEOUT,
                        );
                        supervisor.mark_pair_completed(pin);
                        if !paired {
                            supervisor.record_failure();
                            last_failure_at = Some(Instant::now());
                            continue;
                        }
                    }

                    match spawn(&stream_command(config, &host_address)) {
                        Ok(child_process) => {
                            client = Some(child_process);
                            supervisor.record_healthy();
                        }
                        Err(_) => {
                            supervisor.record_failure();
                            last_failure_at = Some(Instant::now());
                        }
                    }
                }
            }
        }

        // Development escape hatch only; disabled in the product image.
        if config.allow_exit_key && event::poll(config.poll_interval)? {
            if let event::Event::Key(key) = event::read()?
                && key.code == event::KeyCode::Char('q')
            {
                stop_client(&mut client);
                break;
            }
        } else if !config.allow_exit_key {
            std::thread::sleep(config.poll_interval);
        }
    }

    Ok(())
}

/// Clock + memory line for the ambient idle screen, read from the running
/// system. Minute resolution so repaints are rare.
fn ambient_stats() -> Option<screens::AmbientStats> {
    // In-process local time: no subprocess in the render loop (a stalled
    // spawn would freeze the kiosk).
    let clock = chrono::Local::now().format("%H:%M").to_string();

    let stats_line = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|meminfo| {
            let field = |name: &str| {
                meminfo
                    .lines()
                    .find(|line| line.starts_with(name))
                    .and_then(|line| line.split_whitespace().nth(1))
                    .and_then(|kb| kb.parse::<u64>().ok())
            };
            let total = field("MemTotal:")? / 1024;
            let available = field("MemAvailable:")? / 1024;
            Some(format!(
                "Memory: {} MB used of {} MB",
                total - available,
                total
            ))
        })
        .unwrap_or_default();

    if clock.is_empty() && stats_line.is_empty() {
        None
    } else {
        Some(screens::AmbientStats { clock, stats_line })
    }
}

fn spawn(spec: &CommandSpec) -> std::io::Result<Child> {
    Command::new(&spec.program)
        .args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

fn run_to_completion(spec: &CommandSpec, timeout: Duration) -> bool {
    let Ok(mut child) = spawn(spec) else {
        return false;
    };
    let deadline = Instant::now() + timeout;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(200));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

fn stop_client(client: &mut Option<Child>) {
    if let Some(mut child) = client.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn paint(stdout: &mut std::io::Stdout, screen: &screens::Screen) -> std::io::Result<()> {
    let (columns, rows) = terminal::size().unwrap_or((80, 24));
    queue!(stdout, Clear(ClearType::All))?;

    let content_height = screen.lines.len() as u16;
    let top = rows.saturating_sub(content_height) / 2;

    for (index, line) in screen.lines.iter().enumerate() {
        let width = line.chars().count() as u16;
        let left = columns.saturating_sub(width) / 2;
        queue!(
            stdout,
            cursor::MoveTo(left, top + index as u16),
            Print(line)
        )?;
    }

    stdout.flush()
}

fn restore_terminal() -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()
}
