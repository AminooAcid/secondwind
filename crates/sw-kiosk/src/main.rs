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
    let mut last_painted: Option<KioskState> = None;
    let mut last_failure_at: Option<Instant> = None;

    loop {
        let state = read_kiosk_state(&config.state_file).unwrap_or(KioskState::Starting);

        match supervisor.decide(&state) {
            KioskAction::ShowScreen => {
                stop_client(&mut client);
                if last_painted.as_ref() != Some(&state) {
                    paint(&mut stdout, &screens::render(&state))?;
                    last_painted = Some(state.clone());
                }
            }
            KioskAction::EnsureStreaming {
                host_address,
                pair_first,
            } => {
                // Reap a finished client, count the failure for backoff.
                if let Some(child) = client.as_mut() {
                    if child.try_wait()?.is_some() {
                        client = None;
                        supervisor.record_failure();
                        last_failure_at = Some(Instant::now());
                    }
                }

                let backoff_over = last_failure_at
                    .map(|at| at.elapsed() >= supervisor.restart_backoff())
                    .unwrap_or(true);

                if client.is_none() && backoff_over {
                    if last_painted.as_ref() != Some(&state) {
                        paint(&mut stdout, &screens::render(&state))?;
                        last_painted = Some(state.clone());
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
            if let event::Event::Key(key) = event::read()? {
                if key.code == event::KeyCode::Char('q') {
                    stop_client(&mut client);
                    break;
                }
            }
        } else if !config.allow_exit_key {
            std::thread::sleep(config.poll_interval);
        }
    }

    Ok(())
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
