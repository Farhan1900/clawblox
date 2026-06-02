use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const DEFAULT_POLL_MS: u64 = 500;
const MAX_BLOB_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct WorkspaceRecorderConfig {
    pub agent_id: String,
    pub agent_name: String,
    pub workspace_dir: PathBuf,
    pub events_file: PathBuf,
    pub blobs_dir: PathBuf,
    pub poll_interval: Duration,
}

#[derive(Debug)]
pub struct WorkspaceRecorderHandle {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceRecordingManifest {
    pub events: String,
    pub blobs: String,
    pub format: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub wall_time: f64,
    pub time: String,
    pub agent_id: String,
    pub agent_name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub initial: bool,
    pub changes: Vec<WorkspaceChange>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceChange {
    pub op: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtime_ns: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileState {
    size: u64,
    mode: u32,
    mtime_ns: u128,
    sha256: String,
    omitted: Option<String>,
}

impl WorkspaceRecorderConfig {
    pub fn for_agent(agent_id: &str, agent_name: &str, agent_dir: &Path) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            agent_name: agent_name.to_string(),
            workspace_dir: agent_dir.join("workspace"),
            events_file: agent_dir.join("workspace.events.jsonl"),
            blobs_dir: agent_dir.join("workspace-blobs"),
            poll_interval: Duration::from_millis(
                std::env::var("CLAWBLOX_WORKSPACE_RECORD_POLL_MS")
                    .ok()
                    .and_then(|value| value.parse::<u64>().ok())
                    .filter(|value| *value > 0)
                    .unwrap_or(DEFAULT_POLL_MS),
            ),
        }
    }
}

impl WorkspaceRecorderHandle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for WorkspaceRecorderHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

pub fn start_workspace_recorder(
    config: WorkspaceRecorderConfig,
) -> Result<WorkspaceRecorderHandle, String> {
    if !recording_enabled() {
        return Ok(WorkspaceRecorderHandle {
            stop: Arc::new(AtomicBool::new(true)),
            thread: None,
        });
    }
    fs::create_dir_all(&config.workspace_dir).map_err(|err| {
        format!(
            "failed to create workspace recorder source dir '{}': {err}",
            config.workspace_dir.display()
        )
    })?;
    if let Some(parent) = config.events_file.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create workspace recorder event dir '{}': {err}",
                parent.display()
            )
        })?;
    }
    fs::create_dir_all(&config.blobs_dir).map_err(|err| {
        format!(
            "failed to create workspace recorder blob dir '{}': {err}",
            config.blobs_dir.display()
        )
    })?;

    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let thread = thread::Builder::new()
        .name(format!("clawblox-workspace-{}", config.agent_name))
        .spawn(move || run_recorder(config, thread_stop))
        .map_err(|err| format!("failed to start workspace recorder: {err}"))?;

    Ok(WorkspaceRecorderHandle {
        stop,
        thread: Some(thread),
    })
}

pub fn load_workspace_events(path: &Path) -> Result<Vec<WorkspaceEvent>, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read workspace events '{}': {err}", path.display()))?;
    let mut events = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        events.push(serde_json::from_str::<WorkspaceEvent>(line).map_err(|err| {
            format!(
                "failed to parse workspace event in '{}': {err}",
                path.display()
            )
        })?);
    }
    Ok(events)
}

fn run_recorder(config: WorkspaceRecorderConfig, stop: Arc<AtomicBool>) {
    let mut previous = match scan_workspace(&config) {
        Ok(current) => {
            let changes = current
                .iter()
                .map(|(path, state)| upsert_change(path, state))
                .collect::<Vec<_>>();
            let _ = append_event(&config, true, changes);
            current
        }
        Err(err) => {
            eprintln!("Warning: workspace recorder initial scan failed: {err}");
            BTreeMap::new()
        }
    };

    while !stop.load(Ordering::SeqCst) {
        thread::sleep(config.poll_interval);
        let Ok(current) = scan_workspace(&config) else {
            continue;
        };
        let changes = diff_states(&previous, &current);
        if !changes.is_empty() {
            let _ = append_event(&config, false, changes);
        }
        previous = current;
    }

    if let Ok(current) = scan_workspace(&config) {
        let changes = diff_states(&previous, &current);
        if !changes.is_empty() {
            let _ = append_event(&config, false, changes);
        }
    }
}

fn scan_workspace(config: &WorkspaceRecorderConfig) -> Result<BTreeMap<String, FileState>, String> {
    let mut out = BTreeMap::new();
    scan_dir(config, &config.workspace_dir, &mut out)?;
    Ok(out)
}

fn scan_dir(
    config: &WorkspaceRecorderConfig,
    dir: &Path,
    out: &mut BTreeMap<String, FileState>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|err| format!("failed to read workspace dir '{}': {err}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if should_ignore_name(&name) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            scan_dir(config, &path, out)?;
        } else if metadata.is_file() {
            let Some(rel) = relative_workspace_path(&config.workspace_dir, &path) else {
                continue;
            };
            if should_ignore_path(&rel) {
                continue;
            }
            if let Some(state) = file_state(&path, &metadata, &config.blobs_dir) {
                out.insert(rel, state);
            }
        }
    }
    Ok(())
}

fn file_state(path: &Path, metadata: &fs::Metadata, blobs_dir: &Path) -> Option<FileState> {
    let size = metadata.len();
    let mode = file_mode(metadata);
    let mtime_ns = metadata
        .modified()
        .ok()
        .and_then(system_time_ns)
        .unwrap_or(0);
    if size > MAX_BLOB_BYTES {
        return Some(FileState {
            size,
            mode,
            mtime_ns,
            sha256: String::new(),
            omitted: Some(format!("file exceeds {} bytes", MAX_BLOB_BYTES)),
        });
    }
    let bytes = fs::read(path).ok()?;
    let sha256 = hex_sha256(&bytes);
    let _ = write_blob(blobs_dir, &sha256, &bytes);
    Some(FileState {
        size,
        mode,
        mtime_ns,
        sha256,
        omitted: None,
    })
}

fn diff_states(
    previous: &BTreeMap<String, FileState>,
    current: &BTreeMap<String, FileState>,
) -> Vec<WorkspaceChange> {
    let mut paths = BTreeSet::new();
    paths.extend(previous.keys().cloned());
    paths.extend(current.keys().cloned());
    let mut changes = Vec::new();
    for path in paths {
        match (previous.get(&path), current.get(&path)) {
            (None, Some(state)) => changes.push(upsert_change(&path, state)),
            (Some(_), None) => changes.push(WorkspaceChange {
                op: "delete".to_string(),
                path,
                sha256: String::new(),
                size: None,
                mode: None,
                mtime_ns: None,
                omitted: None,
            }),
            (Some(old), Some(new)) if old != new => changes.push(upsert_change(&path, new)),
            _ => {}
        }
    }
    changes
}

fn upsert_change(path: &str, state: &FileState) -> WorkspaceChange {
    WorkspaceChange {
        op: "upsert".to_string(),
        path: path.to_string(),
        sha256: state.sha256.clone(),
        size: Some(state.size),
        mode: Some(state.mode),
        mtime_ns: Some(state.mtime_ns),
        omitted: state.omitted.clone(),
    }
}

fn append_event(
    config: &WorkspaceRecorderConfig,
    initial: bool,
    changes: Vec<WorkspaceChange>,
) -> std::io::Result<()> {
    if changes.is_empty() {
        return Ok(());
    }
    let event = WorkspaceEvent {
        event_type: "WorkspaceChange".to_string(),
        wall_time: now_epoch_f64(),
        time: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        agent_id: config.agent_id.clone(),
        agent_name: config.agent_name.clone(),
        initial,
        changes,
    };
    if let Some(parent) = config.events_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.events_file)?;
    let line = serde_json::to_string(&event)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn write_blob(blobs_dir: &Path, sha256: &str, bytes: &[u8]) -> std::io::Result<()> {
    if sha256.len() < 4 {
        return Ok(());
    }
    let path = blobs_dir.join(&sha256[0..2]).join(&sha256[2..4]).join(sha256);
    if path.is_file() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn relative_workspace_path(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

fn should_ignore_name(name: &str) -> bool {
    matches!(name, ".venv" | "__pycache__" | ".pytest_cache" | ".git")
        || name.ends_with(".pyc")
}

fn should_ignore_path(path: &str) -> bool {
    path.starts_with(".venv/")
        || path.starts_with("__pycache__/")
        || path.starts_with(".pytest_cache/")
        || path.starts_with(".git/")
        || path.ends_with(".pyc")
}

fn recording_enabled() -> bool {
    std::env::var("CLAWBLOX_WORKSPACE_RECORD")
        .map(|value| !matches!(value.trim().to_ascii_lowercase().as_str(), "0" | "false" | "no"))
        .unwrap_or(true)
}

fn now_epoch_f64() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

fn system_time_ns(value: SystemTime) -> Option<u128> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
}

#[cfg(unix)]
fn file_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode()
}

#[cfg(not(unix))]
fn file_mode(_metadata: &fs::Metadata) -> u32 {
    0
}
