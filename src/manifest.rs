use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::workspace_recording::WorkspaceRecordingManifest;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClawbloxManifest {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world: Option<ManifestWorld>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recordings: Vec<ManifestRecording>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<ManifestAgent>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestWorld {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_dir: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub config: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub runtime_mode: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub record_dir: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command_file: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub log_file: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replay_command: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub started_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestRecording {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub format: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestAgent {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub backend: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dir: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub world_session: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub world_agent_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub base_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub internal_base_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub events: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_session: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transcripts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_recording: Option<WorkspaceRecordingManifest>,
}

pub struct WorldStarted<'a> {
    pub source_dir: &'a Path,
    pub runtime_mode: &'a str,
    pub url: &'a str,
    pub port: u16,
    pub record_dir: Option<&'a Path>,
    pub command_file: &'a Path,
    pub log_file: &'a Path,
    pub command: Vec<String>,
    pub replay_command: Vec<String>,
}

pub struct AgentConnected<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub backend: &'a str,
    pub model: &'a str,
    pub agent_dir: &'a Path,
    pub world_session: &'a str,
    pub world_agent_id: &'a str,
    pub base_url: &'a str,
    pub internal_base_url: &'a str,
}

pub fn manifest_path_for_state_root(state_root: &Path) -> PathBuf {
    state_root.join(".clawblox").join("manifest.json")
}

pub fn manifest_state_root(manifest_path: &Path) -> PathBuf {
    manifest_path
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn load(path: &Path) -> Result<ClawbloxManifest, String> {
    if !path.is_file() {
        return Ok(ClawbloxManifest {
            schema_version: 1,
            ..Default::default()
        });
    }
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read manifest '{}': {err}", path.display()))?;
    let mut manifest: ClawbloxManifest = serde_json::from_str(&raw)
        .map_err(|err| format!("failed to parse manifest '{}': {err}", path.display()))?;
    if manifest.schema_version == 0 {
        manifest.schema_version = 1;
    }
    Ok(manifest)
}

pub fn update(
    path: &Path,
    f: impl FnOnce(&mut ClawbloxManifest) -> Result<(), String>,
) -> Result<(), String> {
    let mut manifest = load(path)?;
    if manifest.schema_version == 0 {
        manifest.schema_version = 1;
    }
    f(&mut manifest)?;
    write(path, &manifest)
}

pub fn write(path: &Path, manifest: &ClawbloxManifest) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create manifest directory '{}': {err}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(manifest)
        .map_err(|err| format!("failed to serialize manifest '{}': {err}", path.display()))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, format!("{raw}\n"))
        .map_err(|err| format!("failed to write manifest '{}': {err}", tmp.display()))?;
    fs::rename(&tmp, path)
        .map_err(|err| format!("failed to replace manifest '{}': {err}", path.display()))
}

pub fn record_world_started(path: &Path, started: WorldStarted<'_>) -> Result<(), String> {
    update(path, |manifest| {
        let state_root = manifest_state_root(path);
        manifest.world = Some(ManifestWorld {
            source_dir: path_value(&state_root, started.source_dir),
            config: path_value(&state_root, &started.source_dir.join("world.toml")),
            runtime_mode: started.runtime_mode.to_string(),
            url: started.url.to_string(),
            port: Some(started.port),
            record_dir: started
                .record_dir
                .map(|path| path_value(&state_root, path))
                .unwrap_or_default(),
            command_file: path_value(&state_root, started.command_file),
            log_file: path_value(&state_root, started.log_file),
            command: started.command,
            replay_command: started.replay_command,
            started_at: chrono::Utc::now().to_rfc3339(),
        });
        Ok(())
    })
}

pub fn record_agent_connected(path: &Path, connected: AgentConnected<'_>) -> Result<(), String> {
    update(path, |manifest| {
        let state_root = manifest_state_root(path);
        let id = if connected.id.trim().is_empty() {
            connected.name
        } else {
            connected.id
        };
        let mut agent = agent_from_dir(path, connected.agent_dir, id);
        agent.id = id.to_string();
        agent.name = connected.name.to_string();
        agent.backend = connected.backend.to_string();
        agent.model = connected.model.to_string();
        agent.dir = path_value(&state_root, connected.agent_dir);
        agent.world_session = connected.world_session.to_string();
        agent.world_agent_id = connected.world_agent_id.to_string();
        agent.base_url = connected.base_url.to_string();
        agent.internal_base_url = connected.internal_base_url.to_string();
        upsert_agent(&mut manifest.agents, agent);
        Ok(())
    })
}

pub fn refresh_agent_from_dir(
    path: &Path,
    agent_dir: &Path,
    id: &str,
    name: &str,
    backend: &str,
    model: &str,
) -> Result<(), String> {
    update(path, |manifest| {
        let mut agent = agent_from_dir(path, agent_dir, id);
        if !name.is_empty() {
            agent.name = name.to_string();
        }
        if !backend.is_empty() {
            agent.backend = backend.to_string();
        }
        if !model.is_empty() {
            agent.model = model.to_string();
        }
        upsert_agent(&mut manifest.agents, agent);
        Ok(())
    })
}

pub fn record_agent_workspace_recording(
    path: &Path,
    agent_dir: &Path,
    id: &str,
    agent_name: &str,
    events_path: &Path,
    blobs_dir: &Path,
) -> Result<(), String> {
    update(path, |manifest| {
        let state_root = manifest_state_root(path);
        let mut agent = agent_from_dir(path, agent_dir, id);
        agent.id = if id.trim().is_empty() {
            agent_name.to_string()
        } else {
            id.to_string()
        };
        if !agent_name.trim().is_empty() {
            agent.name = agent_name.to_string();
        }
        agent.workspace_recording = Some(WorkspaceRecordingManifest {
            events: path_value(&state_root, events_path),
            blobs: path_value(&state_root, blobs_dir),
            format: "clawblox-workspace-v1".to_string(),
        });
        upsert_agent(&mut manifest.agents, agent);
        Ok(())
    })
}

pub fn record_recording(path: &Path, recording_path: &Path, format: &str) -> Result<(), String> {
    update(path, |manifest| {
        let state_root = manifest_state_root(path);
        let rel = path_value(&state_root, recording_path);
        if manifest.recordings.iter().any(|item| item.path == rel) {
            return Ok(());
        }
        let events = sibling_events_path(recording_path)
            .filter(|path| path.is_file())
            .map(|path| path_value(&state_root, &path));
        manifest.recordings.push(ManifestRecording {
            path: rel,
            events,
            format: format.to_string(),
        });
        Ok(())
    })
}

fn upsert_agent(agents: &mut Vec<ManifestAgent>, agent: ManifestAgent) {
    if let Some(existing) = agents.iter_mut().find(|item| item.id == agent.id) {
        merge_agent(existing, agent);
    } else {
        agents.push(agent);
    }
}

fn merge_agent(existing: &mut ManifestAgent, next: ManifestAgent) {
    if !next.name.is_empty() {
        existing.name = next.name;
    }
    if !next.backend.is_empty() {
        existing.backend = next.backend;
    }
    if !next.model.is_empty() {
        existing.model = next.model;
    }
    if !next.dir.is_empty() {
        existing.dir = next.dir;
    }
    if !next.world_session.is_empty() {
        existing.world_session = next.world_session;
    }
    if !next.world_agent_id.is_empty() {
        existing.world_agent_id = next.world_agent_id;
    }
    if !next.base_url.is_empty() {
        existing.base_url = next.base_url;
    }
    if !next.internal_base_url.is_empty() {
        existing.internal_base_url = next.internal_base_url;
    }
    if !next.events.is_empty() {
        existing.events = next.events;
    }
    if !next.provider_session.is_empty() {
        existing.provider_session = next.provider_session;
    }
    for transcript in next.transcripts {
        if !existing.transcripts.iter().any(|item| item == &transcript) {
            existing.transcripts.push(transcript);
        }
    }
    if next.workspace_recording.is_some() {
        existing.workspace_recording = next.workspace_recording;
    }
}

fn agent_from_dir(manifest_path: &Path, agent_dir: &Path, id: &str) -> ManifestAgent {
    let state_root = manifest_state_root(manifest_path);
    ManifestAgent {
        id: id.to_string(),
        name: id.to_string(),
        dir: path_value(&state_root, agent_dir),
        world_session: read_trimmed(&agent_dir.join("world_session.txt")).unwrap_or_default(),
        events: path_value(&state_root, &agent_dir.join("events.jsonl")),
        provider_session: provider_session(agent_dir).unwrap_or_default(),
        transcripts: transcript_paths(agent_dir)
            .into_iter()
            .map(|path| path_value(&state_root, &path))
            .collect(),
        ..Default::default()
    }
}

fn path_value(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn provider_session(agent_dir: &Path) -> Option<String> {
    read_trimmed(&agent_dir.join("claude_session_id.txt"))
        .or_else(|| read_trimmed(&agent_dir.join("codex_session_id.txt")))
}

fn transcript_paths(agent_dir: &Path) -> Vec<PathBuf> {
    let dir = agent_dir.join("session");
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn sibling_events_path(recording_path: &Path) -> Option<PathBuf> {
    let parent = recording_path.parent()?;
    let stem = recording_path.file_stem()?.to_str()?;
    Some(parent.join(format!("{stem}.events.jsonl")))
}
