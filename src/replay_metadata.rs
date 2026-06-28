use crate::recording::RecordingMeta;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayMetadata {
    pub schema_version: u32,
    pub recording: ReplayMetadataRecording,
    #[serde(default)]
    pub behaviors: Vec<ReplayBehavior>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayMetadataRecording {
    pub file: String,
    pub tick_rate: u32,
    pub total_ticks: u64,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub created_at: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub world_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub world_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBehavior {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub occurrences: Vec<ReplayBehaviorOccurrence>,
    #[serde(default)]
    pub attrs: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBehaviorOccurrence {
    pub id: String,
    pub start_tick: u64,
    pub end_tick: u64,
    #[serde(default)]
    pub participants: Vec<ReplayBehaviorParticipant>,
    #[serde(default)]
    pub observations: Vec<ReplayBehaviorObservation>,
    #[serde(default)]
    pub attrs: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBehaviorParticipant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default)]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBehaviorObservation {
    pub tick: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
}

pub fn sidecar_path_for_recording(recording_path: &Path) -> PathBuf {
    recording_path.with_extension("clawmeta.json")
}

pub fn load_for_recording(
    recording_path: &Path,
    recording_meta: &RecordingMeta,
) -> Result<Option<ReplayMetadata>, String> {
    let sidecar_path = sidecar_path_for_recording(recording_path);
    if !sidecar_path.is_file() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&sidecar_path).map_err(|err| {
        format!(
            "failed to read replay metadata '{}': {}",
            sidecar_path.display(),
            err
        )
    })?;
    let metadata: ReplayMetadata = serde_json::from_str(&raw).map_err(|err| {
        format!(
            "failed to parse replay metadata '{}': {}",
            sidecar_path.display(),
            err
        )
    })?;

    validate_metadata(recording_path, recording_meta, &metadata).map(|_| Some(metadata))
}

fn validate_metadata(
    recording_path: &Path,
    recording_meta: &RecordingMeta,
    metadata: &ReplayMetadata,
) -> Result<(), String> {
    let expected_file = recording_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    if !metadata.recording.file.is_empty() && metadata.recording.file != expected_file {
        return Err(format!(
            "replay metadata recording.file '{}' does not match recording '{}'",
            metadata.recording.file, expected_file
        ));
    }
    if metadata.recording.tick_rate != 0 && metadata.recording.tick_rate != recording_meta.tick_rate
    {
        return Err(format!(
            "replay metadata tick_rate {} does not match recording tick_rate {}",
            metadata.recording.tick_rate, recording_meta.tick_rate
        ));
    }
    if metadata.recording.total_ticks != 0
        && metadata.recording.total_ticks != recording_meta.total_ticks
    {
        return Err(format!(
            "replay metadata total_ticks {} does not match recording total_ticks {}",
            metadata.recording.total_ticks, recording_meta.total_ticks
        ));
    }

    Ok(())
}
