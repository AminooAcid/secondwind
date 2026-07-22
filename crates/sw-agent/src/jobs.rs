//! Node job offload (v0.5).
//!
//! Presets are Docker invocations defined in the node's own preset file;
//! the host only ever names a `preset_id` plus an input. Inputs are
//! validated hard: share paths must be relative with no traversal, URLs
//! must be http(s). Containers see exactly one bind mount — the host
//! share — so jobs operate on shared paths with no copying.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use sw_core::{JobInfo, JobInput, JobState};

pub const JOBS_FILE_ENV: &str = "SECONDWIND_JOBS_FILE";
pub const SHARE_MOUNTPOINT_ENV: &str = "SECONDWIND_SHARE_MOUNTPOINT";
const DEFAULT_SHARE_MOUNTPOINT: &str = "/mnt/secondwind-host";
/// Where the share is mounted inside job containers.
const CONTAINER_DATA_DIR: &str = "/data";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobPreset {
    pub preset_id: String,
    pub display_name: String,
    /// Container image (pinned in the preset file, pulled at image build
    /// or first use).
    pub image: String,
    /// Command; `{input}` is replaced by the container path of the input
    /// (share paths) or the URL (url presets).
    pub command: Vec<String>,
    /// Which input kind this preset accepts.
    pub accepts: JobInputKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobInputKind {
    SharePath,
    Url,
}

pub fn default_presets() -> Vec<JobPreset> {
    vec![
        JobPreset {
            preset_id: "convert-video".to_string(),
            display_name: "Convert to MP4 on node".to_string(),
            image: "linuxserver/ffmpeg".to_string(),
            command: vec![
                "-i".to_string(),
                "{input}".to_string(),
                "{input}.mp4".to_string(),
            ],
            accepts: JobInputKind::SharePath,
        },
        JobPreset {
            preset_id: "compress".to_string(),
            display_name: "Compress on node".to_string(),
            image: "alpine".to_string(),
            command: vec![
                "tar".to_string(),
                "-czf".to_string(),
                "{input}.tar.gz".to_string(),
                "-C".to_string(),
                CONTAINER_DATA_DIR.to_string(),
                "{input_relative}".to_string(),
            ],
            accepts: JobInputKind::SharePath,
        },
        JobPreset {
            preset_id: "download".to_string(),
            display_name: "Download on node".to_string(),
            image: "curlimages/curl".to_string(),
            command: vec![
                "-L".to_string(),
                "--output-dir".to_string(),
                format!("{CONTAINER_DATA_DIR}/Downloads"),
                "--create-dirs".to_string(),
                "-O".to_string(),
                "{input}".to_string(),
            ],
            accepts: JobInputKind::Url,
        },
    ]
}

pub fn load_presets(presets_file: &Path) -> Vec<JobPreset> {
    fs::read_to_string(presets_file)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_else(default_presets)
}

/// A share-relative path is safe when it is relative, non-empty, and never
/// steps out of the share.
pub fn is_safe_share_path(path: &str) -> bool {
    if path.trim().is_empty() || path.len() > 4096 {
        return false;
    }
    let normalized = path.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains("//") || normalized.contains('\0') {
        return false;
    }
    !normalized.split('/').any(|segment| segment == "..") && !normalized.contains(':')
}

pub fn is_safe_url(url: &str) -> bool {
    (url.starts_with("http://") || url.starts_with("https://"))
        && !url.contains(|c: char| c.is_whitespace() || c == '\0')
        && url.len() <= 4096
}

/// Builds the full `docker run` argument list for a preset + input.
pub fn docker_args(
    preset: &JobPreset,
    input: &JobInput,
    share_mountpoint: &str,
) -> Result<Vec<String>, JobsError> {
    let (container_input, relative) = match (&preset.accepts, input) {
        (JobInputKind::SharePath, JobInput::SharePath { path }) => {
            if !is_safe_share_path(path) {
                return Err(JobsError::InvalidInput);
            }
            let normalized = path.replace('\\', "/");
            (format!("{CONTAINER_DATA_DIR}/{normalized}"), normalized)
        }
        (JobInputKind::Url, JobInput::Url { url }) => {
            if !is_safe_url(url) {
                return Err(JobsError::InvalidInput);
            }
            (url.clone(), String::new())
        }
        _ => return Err(JobsError::InvalidInput),
    };

    let mut args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "--network".to_string(),
        // Only download presets need the network.
        match preset.accepts {
            JobInputKind::Url => "bridge".to_string(),
            JobInputKind::SharePath => "none".to_string(),
        },
        "-v".to_string(),
        format!("{share_mountpoint}:{CONTAINER_DATA_DIR}"),
        preset.image.clone(),
    ];
    args.extend(preset.command.iter().map(|part| {
        part.replace("{input}", &container_input)
            .replace("{input_relative}", &relative)
    }));

    Ok(args)
}

pub fn share_mountpoint() -> String {
    std::env::var(SHARE_MOUNTPOINT_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SHARE_MOUNTPOINT.to_string())
}

pub trait JobsController: Send + Sync + std::fmt::Debug {
    fn presets(&self) -> Vec<JobPreset>;
    fn submit(
        &self,
        preset: &JobPreset,
        input: &JobInput,
        idempotency_key: Option<&str>,
    ) -> Result<String, JobsError>;
    fn jobs(&self) -> Vec<JobInfo>;
}

pub type SharedJobsController = Arc<dyn JobsController>;

pub const JOB_TIMEOUT_SECS_ENV: &str = "SECONDWIND_JOB_TIMEOUT_SECS";
const DEFAULT_JOB_TIMEOUT_SECS: u64 = 3600;
/// How often the background reaper sweeps.
const REAPER_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug)]
struct RunningJob {
    preset_id: String,
    child: Option<Child>,
    state: JobState,
    started_at: std::time::Instant,
    idempotency_key: Option<String>,
}

/// Finished jobs kept for status queries; older ones are evicted FIFO so
/// a long-running node never grows its job table unbounded (BUG-010).
const MAX_FINISHED_JOBS: usize = 50;

/// Production controller: presets from the node's file, jobs as `docker
/// run` children with timeout enforcement, reaped by the background
/// sweeper and on every status call.
#[derive(Debug)]
pub struct DockerJobsController {
    presets_file: PathBuf,
    share_mountpoint: String,
    timeout: std::time::Duration,
    jobs: Mutex<JobTable>,
}

impl DockerJobsController {
    pub fn sweep(&self) {
        self.jobs
            .lock()
            .expect("jobs lock")
            .reap_and_prune(self.timeout);
    }
}

#[derive(Debug, Default)]
struct JobTable {
    jobs: HashMap<String, RunningJob>,
    /// Completion order for FIFO eviction of finished entries.
    finished_order: std::collections::VecDeque<String>,
}

impl JobTable {
    /// Reaps finished children, kills jobs past `timeout`, and evicts the
    /// oldest finished entries beyond the cap.
    fn reap_and_prune(&mut self, timeout: std::time::Duration) {
        for (job_id, job) in self.jobs.iter_mut() {
            let Some(child) = job.child.as_mut() else {
                continue;
            };

            if let Ok(Some(status)) = child.try_wait() {
                job.state = if status.success() {
                    JobState::Succeeded
                } else {
                    JobState::Failed
                };
                job.child = None;
                self.finished_order.push_back(job_id.clone());
            } else if job.started_at.elapsed() >= timeout {
                // Runaway job: kill it and record the failure.
                let _ = child.kill();
                let _ = child.wait();
                job.state = JobState::Failed;
                job.child = None;
                self.finished_order.push_back(job_id.clone());
                tracing::warn!(job = %job_id, preset = %job.preset_id, "job timed out");
            }
        }

        while self.finished_order.len() > MAX_FINISHED_JOBS {
            if let Some(oldest) = self.finished_order.pop_front() {
                self.jobs.remove(&oldest);
            }
        }
    }

    /// A still-known job with this idempotency key, if any.
    fn job_id_for_key(&self, key: &str) -> Option<String> {
        self.jobs
            .iter()
            .find(|(_, job)| job.idempotency_key.as_deref() == Some(key))
            .map(|(job_id, _)| job_id.clone())
    }
}

fn job_timeout() -> std::time::Duration {
    let secs = std::env::var(JOB_TIMEOUT_SECS_ENV)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(DEFAULT_JOB_TIMEOUT_SECS);
    std::time::Duration::from_secs(secs)
}

/// Background sweeper so zombies are reaped and timeouts enforced even
/// when nobody polls `GET /v1/jobs`. Stops when the controller is dropped.
pub fn spawn_reaper(controller: &Arc<DockerJobsController>) {
    let weak = Arc::downgrade(controller);
    std::thread::spawn(move || {
        while let Some(controller) = weak.upgrade() {
            controller.sweep();
            drop(controller);
            std::thread::sleep(REAPER_INTERVAL);
        }
    });
}

impl DockerJobsController {
    pub fn from_env() -> Option<Self> {
        let presets_file = std::env::var_os(JOBS_FILE_ENV)
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())?;

        Some(Self {
            presets_file,
            share_mountpoint: share_mountpoint(),
            timeout: job_timeout(),
            jobs: Mutex::new(JobTable::default()),
        })
    }

    fn random_job_id() -> Result<String, JobsError> {
        let mut bytes = [0_u8; 6];
        getrandom::getrandom(&mut bytes).map_err(|_| JobsError::Randomness)?;
        Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }
}

impl JobsController for DockerJobsController {
    fn presets(&self) -> Vec<JobPreset> {
        load_presets(&self.presets_file)
    }

    fn submit(
        &self,
        preset: &JobPreset,
        input: &JobInput,
        idempotency_key: Option<&str>,
    ) -> Result<String, JobsError> {
        // Validate before taking the lock.
        let args = docker_args(preset, input, &self.share_mountpoint)?;

        let mut table = self.jobs.lock().expect("jobs lock");
        if let Some(key) = idempotency_key
            && let Some(existing) = table.job_id_for_key(key)
        {
            return Ok(existing);
        }

        let child = Command::new("docker")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|source| JobsError::Spawn { source })?;

        let job_id = Self::random_job_id()?;
        table.jobs.insert(
            job_id.clone(),
            RunningJob {
                preset_id: preset.preset_id.clone(),
                child: Some(child),
                state: JobState::Running,
                started_at: std::time::Instant::now(),
                idempotency_key: idempotency_key.map(|key| key.to_string()),
            },
        );
        Ok(job_id)
    }

    fn jobs(&self) -> Vec<JobInfo> {
        let mut table = self.jobs.lock().expect("jobs lock");
        table.reap_and_prune(self.timeout);
        table
            .jobs
            .iter()
            .map(|(job_id, job)| JobInfo {
                job_id: job_id.clone(),
                preset_id: job.preset_id.clone(),
                state: job.state.clone(),
            })
            .collect()
    }
}

#[derive(Debug)]
pub enum JobsError {
    InvalidInput,
    UnknownPreset,
    Randomness,
    Spawn { source: std::io::Error },
}

impl std::fmt::Display for JobsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput => write!(formatter, "that input can't be used for this job"),
            Self::UnknownPreset => write!(formatter, "that job type is not on this node"),
            Self::Randomness => write!(formatter, "could not create a job id"),
            Self::Spawn { .. } => write!(formatter, "the node could not start the job"),
        }
    }
}

impl std::error::Error for JobsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
pub mod test_support {
    use super::*;

    #[derive(Debug, Default)]
    pub struct FakeJobsController {
        pub submitted: Mutex<Vec<(String, JobInput)>>,
    }

    impl JobsController for FakeJobsController {
        fn presets(&self) -> Vec<JobPreset> {
            default_presets()
        }

        fn submit(
            &self,
            preset: &JobPreset,
            input: &JobInput,
            _idempotency_key: Option<&str>,
        ) -> Result<String, JobsError> {
            // Same validation path as production.
            docker_args(preset, input, "/mnt/share")?;
            self.submitted
                .lock()
                .expect("submitted lock")
                .push((preset.preset_id.clone(), input.clone()));
            Ok("job-1".to_string())
        }

        fn jobs(&self) -> Vec<JobInfo> {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_path_validation_blocks_traversal_and_absolutes() {
        assert!(is_safe_share_path("Videos/movie.mkv"));
        assert!(is_safe_share_path("file.txt"));
        assert!(!is_safe_share_path("../etc/passwd"));
        assert!(!is_safe_share_path("a/../../b"));
        assert!(!is_safe_share_path("/etc/passwd"));
        assert!(!is_safe_share_path("C:\\Windows"));
        assert!(!is_safe_share_path(""));
    }

    #[test]
    fn url_validation_requires_http() {
        assert!(is_safe_url("https://example.org/file.iso"));
        assert!(!is_safe_url("file:///etc/passwd"));
        assert!(!is_safe_url("https://a b"));
    }

    #[test]
    fn docker_args_isolate_share_jobs_from_the_network() {
        let presets = default_presets();
        let convert = presets
            .iter()
            .find(|preset| preset.preset_id == "convert-video")
            .expect("convert preset");

        let args = docker_args(
            convert,
            &JobInput::SharePath {
                path: "Videos/movie.mkv".to_string(),
            },
            "/mnt/secondwind-host",
        )
        .expect("args");

        assert!(args.contains(&"--network".to_string()));
        assert!(args.contains(&"none".to_string()));
        assert!(args.contains(&"/mnt/secondwind-host:/data".to_string()));
        assert!(args.contains(&"/data/Videos/movie.mkv".to_string()));
    }

    #[test]
    fn download_preset_takes_urls_only() {
        let presets = default_presets();
        let download = presets
            .iter()
            .find(|preset| preset.preset_id == "download")
            .expect("download preset");

        let ok = docker_args(
            download,
            &JobInput::Url {
                url: "https://example.org/f.iso".to_string(),
            },
            "/mnt/share",
        );
        assert!(ok.is_ok());

        let wrong_kind = docker_args(
            download,
            &JobInput::SharePath {
                path: "f.iso".to_string(),
            },
            "/mnt/share",
        );
        assert!(matches!(wrong_kind, Err(JobsError::InvalidInput)));
    }

    #[test]
    fn finished_jobs_are_evicted_fifo_beyond_the_cap() {
        let mut table = JobTable::default();
        for index in 0..(MAX_FINISHED_JOBS + 10) {
            let job_id = format!("job-{index}");
            table
                .jobs
                .insert(job_id.clone(), finished_job("compress", None));
            table.finished_order.push_back(job_id);
        }

        table.reap_and_prune(std::time::Duration::from_secs(3600));

        assert_eq!(table.jobs.len(), MAX_FINISHED_JOBS);
        // Oldest entries evicted first.
        assert!(!table.jobs.contains_key("job-0"));
        assert!(
            table
                .jobs
                .contains_key(&format!("job-{}", MAX_FINISHED_JOBS + 9))
        );
    }

    fn finished_job(preset_id: &str, key: Option<&str>) -> RunningJob {
        RunningJob {
            preset_id: preset_id.to_string(),
            child: None,
            state: JobState::Succeeded,
            started_at: std::time::Instant::now(),
            idempotency_key: key.map(|key| key.to_string()),
        }
    }

    #[test]
    fn idempotency_key_finds_the_existing_job() {
        let mut table = JobTable::default();
        table
            .jobs
            .insert("job-a".to_string(), finished_job("compress", Some("key-1")));

        assert_eq!(table.job_id_for_key("key-1").as_deref(), Some("job-a"));
        assert!(table.job_id_for_key("key-2").is_none());
    }

    #[test]
    fn presets_round_trip_as_json() {
        let presets = default_presets();
        let json = serde_json::to_string(&presets).expect("serialize");
        let decoded: Vec<JobPreset> = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded, presets);
    }
}
