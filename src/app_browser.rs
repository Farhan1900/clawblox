use super::*;
use clawblox::agent::build_agent;
use clawblox::replay_agent_log::{
    detect_transcript_parser, load_agent_timeline_with_parser, TranscriptLineParser,
};
use clawblox::workspace_recording::{load_workspace_events, WorkspaceChange};

#[derive(Clone)]
struct AppState {
    project_root: PathBuf,
    base_path: String,
    sessions: Arc<DashMap<String, ReplayState>>,
    external_sessions: Arc<DashMap<String, ExternalReplaySession>>,
}

#[derive(Debug, Clone, Serialize)]
struct AppTreeNode {
    name: String,
    path: String,
    kind: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<AppTreeNode>,
}

#[derive(Debug, Clone)]
struct ReplayListEntry {
    rel_path: String,
}

#[derive(Debug, Deserialize)]
struct ExternalReplayManifest {
    #[serde(default)]
    version: Option<u32>,
    #[serde(default)]
    format: Option<String>,
    recording: String,
    #[serde(default)]
    events: Option<String>,
    command: Vec<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    agents: Vec<ExternalReplayAgentManifest>,
}

#[derive(Debug, Deserialize)]
struct ExternalReplayAgentManifest {
    #[serde(default)]
    transcript: Option<String>,
}

#[derive(Clone)]
struct ExternalReplaySession {
    _process: Arc<ExternalReplayProcess>,
    url: String,
    recording_path: PathBuf,
    manifest_path: PathBuf,
    format: Option<String>,
    agent_timeline: Option<Arc<ReplayAgentTimeline>>,
    workspace_timeline: Option<Arc<ReplayWorkspaceTimeline>>,
}

#[derive(Clone, Serialize)]
struct ReplayWorkspaceEvent {
    pub event_type: String,
    pub wall_time: f64,
    pub time: String,
    pub agent_id: String,
    pub agent_name: String,
    pub initial: bool,
    pub tick: f64,
    pub approx: bool,
    pub changes: Vec<WorkspaceChange>,
}

#[derive(Clone)]
struct ReplayWorkspaceTimeline {
    events: Vec<ReplayWorkspaceEvent>,
}

#[derive(Deserialize)]
struct ReplayWorkspaceQuery {
    tick: Option<u64>,
}

#[derive(Serialize)]
struct ReplayWorkspaceEventsResponse {
    enabled: bool,
    current_tick: u64,
    start_tick: u64,
    end_tick: u64,
    events: Vec<ReplayWorkspaceEvent>,
}

struct ExternalReplayProcess {
    child: Mutex<std::process::Child>,
}

impl Drop for ExternalReplayProcess {
    fn drop(&mut self) {
        let Ok(mut child) = self.child.lock() else {
            return;
        };
        let Ok(None) = child.try_wait() else {
            return;
        };
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[derive(Serialize)]
struct AppTreeResponse {
    root_name: String,
    root_path: String,
    tree: Vec<AppTreeNode>,
}

#[derive(Deserialize)]
struct AppOpenReplayRequest {
    path: String,
}

#[derive(Serialize)]
struct AppOpenReplayResponse {
    session_id: String,
    url: String,
    recording_path: String,
    world_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    replay_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_log: Option<String>,
}

#[derive(Serialize)]
struct ExternalReplayViewResponse {
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[derive(Deserialize)]
struct ExternalReplayProxyPath {
    id: String,
    path: String,
}

async fn build_replay_state(
    file: PathBuf,
    speed: f32,
    base_path: &str,
    world_dir_override: Option<PathBuf>,
    agent_log_file: Option<PathBuf>,
) -> Result<ReplayState, String> {
    let reader = recording::RecordingReader::open(&file)
        .await
        .map_err(|e| format!("failed to open recording '{}': {}", file.display(), e))?;
    let meta = reader.meta().await.map_err(|e| {
        format!(
            "failed to read recording metadata '{}': {}",
            file.display(),
            e
        )
    })?;
    let active_ranges = reader.active_tick_ranges().await.map_err(|e| {
        format!(
            "failed to analyze replay activity '{}': {}",
            file.display(),
            e
        )
    })?;

    let world_dir = world_dir_override
        .or_else(|| {
            let path = PathBuf::from(&meta.world_dir);
            if path.is_dir() {
                Some(path)
            } else {
                None
            }
        })
        .or_else(|| find_world_dir_near_recording(&file))
        .unwrap_or_else(|| PathBuf::from(&meta.world_dir));

    let first_tick = reader
        .first_tick()
        .ok_or_else(|| "recording has no checkpoints to replay".to_string())?;
    let mut playback = replay::ReplayPlayback::new(
        first_tick,
        meta.total_ticks,
        meta.tick_rate,
        meta.duration_ms,
        speed,
        active_ranges,
    )?;
    playback.set_paused(true);

    let renderer_manifest = {
        let world_toml_path = world_dir.join("world.toml");
        if world_toml_path.exists() {
            let cfg = clawblox::config::WorldConfig::from_game_dir(&world_dir).ok();
            cfg.map(|c| maybe_build_renderer_package(&world_dir, &c.renderer, base_path).ok())
                .flatten()
                .unwrap_or_else(default_renderer_manifest)
        } else {
            default_renderer_manifest()
        }
    };

    let agent_log_file = agent_log_file.or_else(|| find_agent_log_near_recording(&file));

    let agent_timeline = if let Some(path) = agent_log_file.as_ref() {
        match load_agent_timeline(path, meta.tick_rate, first_tick) {
            Ok(timeline) => Some(Arc::new(timeline)),
            Err(err) => {
                eprintln!(
                    "Warning: failed to load agent log '{}': {}",
                    path.display(),
                    err
                );
                None
            }
        }
    } else {
        None
    };

    let replay_metadata = match replay_metadata::load_for_recording(&file, &meta) {
        Ok(metadata) => metadata.map(Arc::new),
        Err(err) => {
            eprintln!(
                "Warning: failed to load replay metadata '{}': {}",
                file.display(),
                err
            );
            None
        }
    };

    Ok(ReplayState {
        reader: Arc::new(reader),
        playback: Arc::new(tokio::sync::Mutex::new(playback)),
        base_path: normalize_base_path(base_path),
        asset_base_path: normalize_base_path(base_path),
        renderer_manifest,
        agent_timeline,
        replay_metadata,
        world_dir,
        camera: Arc::new(Mutex::new(ReplayCameraBridge::default())),
    })
}

pub fn run_app(path: PathBuf, port: u16, base_path: &str) {
    let base_path = normalize_base_path(base_path);
    let project_root = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let state = AppState {
        project_root: project_root.clone(),
        base_path: base_path.clone(),
        sessions: Arc::new(DashMap::new()),
        external_sessions: Arc::new(DashMap::new()),
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async move {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = mount_router_at_base_path(
            Router::new()
                .route("/", get(app_ui_index))
                .route("/ui", get(app_ui_index))
                .route("/ui/{*path}", get(local_ui_file))
                .route("/api/tree", get(app_tree))
                .route("/api/open-replay", post(app_open_replay))
                .route("/sessions/{id}", get(app_session_ui_index))
                .route("/sessions/{id}/ui", get(app_session_ui_index))
                .route("/sessions/{id}/ui/{*path}", get(session_ui_file))
                .route("/sessions/{id}/spectate/ws", get(app_replay_spectate_ws))
                .route(
                    "/sessions/{id}/renderer/manifest",
                    get(app_replay_renderer_manifest),
                )
                .route("/sessions/{id}/replay/info", get(app_replay_info))
                .route(
                    "/sessions/{id}/replay/external-view",
                    get(app_replay_external_view),
                )
                .route(
                    "/sessions/{id}/replay/external",
                    get(app_external_replay_proxy_root),
                )
                .route(
                    "/sessions/{id}/replay/external/{*path}",
                    get(app_external_replay_proxy_path),
                )
                .route("/sessions/{id}/replay/state", get(app_replay_state_at))
                .route(
                    "/sessions/{id}/replay/agent",
                    get(app_replay_agent_messages),
                )
                .route(
                    "/sessions/{id}/replay/workspace",
                    get(app_replay_workspace_events),
                )
                .route("/sessions/{id}/replay/metadata", get(app_replay_metadata))
                .route("/sessions/{id}/replay/seek", post(app_replay_seek))
                .route("/sessions/{id}/replay/speed", post(app_replay_speed))
                .route(
                    "/sessions/{id}/replay/skip-idle",
                    post(app_replay_skip_idle),
                )
                .route("/sessions/{id}/replay/pause", post(app_replay_pause))
                .route("/sessions/{id}/replay/play", post(app_replay_play))
                .route(
                    "/sessions/{id}/replay/play-range",
                    post(app_replay_play_range),
                )
                .route(
                    "/sessions/{id}/renderer-files/{*path}",
                    get(app_replay_world_file),
                )
                .route("/sessions/{id}/assets/{*path}", get(app_replay_world_file))
                .route("/sessions/{id}/static/{*path}", get(app_replay_world_file))
                .with_state(state)
                .layer(cors),
            &base_path,
        );

        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        println!(
            "App server: http://localhost:{}{}",
            port,
            join_base_path(&base_path, "/")
        );
        println!("Project root: {}", project_root.display());

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .unwrap_or_else(|e| {
                eprintln!("Error: Port {} is already in use. Try --port <PORT>", port);
                eprintln!("Details: {}", e);
                std::process::exit(1);
            });

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown_signal().await;
                eprintln!("\nApp server stopped.");
            })
            .await
            .unwrap_or_else(|e| {
                eprintln!("Server error: {}", e);
            });
    });
}

fn should_hide_project_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "__pycache__" | "target" | ".venv" | ".clawblox"
    )
}

fn scan_recordings_recursive(
    root: &Path,
    dir: &Path,
    out: &mut Vec<ReplayListEntry>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("failed to read directory '{}': {}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read directory entry: {}", e))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if should_hide_project_dir(&name) {
                continue;
            }
            scan_recordings_recursive(root, &path, out)?;
            continue;
        }
        if !is_replay_entry_path(&path) {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push(ReplayListEntry { rel_path: rel });
    }
    Ok(())
}

fn list_project_recordings(root: &Path) -> Result<Vec<ReplayListEntry>, String> {
    let mut entries = Vec::new();
    scan_recordings_recursive(root, root, &mut entries)?;
    entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(entries)
}

fn is_replay_entry_path(path: &Path) -> bool {
    if path.extension().and_then(|ext| ext.to_str()) == Some("clawrec") {
        return true;
    }
    if path.extension().and_then(|ext| ext.to_str()) == Some("h5") {
        return find_clawblox_manifest_near(path)
            .and_then(|manifest_path| manifest::load(&manifest_path).ok())
            .and_then(|manifest| manifest.world)
            .is_some_and(|world| !world.replay_command.is_empty());
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".replay.toml"))
}

fn is_external_replay_manifest_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".replay.toml"))
}

fn insert_tree_path(nodes: &mut Vec<AppTreeNode>, segments: &[&str], full_path: &str) {
    if segments.is_empty() {
        return;
    }
    let name = segments[0];
    let is_leaf = segments.len() == 1;
    if let Some(existing) = nodes.iter_mut().find(|node| node.name == name) {
        if !is_leaf {
            insert_tree_path(&mut existing.children, &segments[1..], full_path);
        }
        return;
    }

    let mut node = AppTreeNode {
        name: name.to_string(),
        path: if is_leaf {
            full_path.to_string()
        } else {
            segments[..1].join("/")
        },
        kind: if is_leaf {
            "recording".to_string()
        } else {
            "directory".to_string()
        },
        children: Vec::new(),
    };
    if !is_leaf {
        insert_tree_path(&mut node.children, &segments[1..], full_path);
    }
    nodes.push(node);
    nodes.sort_by(|a, b| match (a.kind.as_str(), b.kind.as_str()) {
        ("directory", "recording") => std::cmp::Ordering::Less,
        ("recording", "directory") => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });
}

fn build_tree_from_recordings(entries: &[ReplayListEntry]) -> Vec<AppTreeNode> {
    let mut tree = Vec::new();
    for entry in entries {
        let parts = entry
            .rel_path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        insert_tree_path(&mut tree, &parts, &entry.rel_path);
    }
    tree
}

fn find_world_dir_near_recording(file: &Path) -> Option<PathBuf> {
    let mut current = file.parent();
    while let Some(dir) = current {
        if dir.join("world.toml").is_file() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

fn find_world_instance_dir_near_recording(file: &Path) -> Option<PathBuf> {
    let mut current = file.parent();
    while let Some(dir) = current {
        if dir.join("agents").is_dir() && dir.join("recordings").is_dir() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

fn find_clawblox_manifest_near(file: &Path) -> Option<PathBuf> {
    let mut current = file.parent();
    while let Some(dir) = current {
        let manifest_path = dir.join(".clawblox").join("manifest.json");
        if manifest_path.is_file() {
            return Some(manifest_path);
        }
        current = dir.parent();
    }
    None
}

fn resolve_clawblox_manifest_path(manifest_path: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        manifest::manifest_state_root(manifest_path).join(path)
    }
}

fn collect_session_transcripts_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_session_transcripts_recursive(&path, out);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(parent) = path.parent() else {
            continue;
        };
        if parent.file_name().and_then(|name| name.to_str()) == Some("session") {
            out.push(path);
        }
    }
}

fn timestamp_key_from_recording(path: &Path) -> Option<u64> {
    let stem = path.file_stem()?.to_str()?;
    let digits = stem
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.len() < 14 {
        return None;
    }
    digits[..14].parse().ok()
}

fn timestamp_key_from_transcript(path: &Path) -> Option<u64> {
    let name = path.file_name()?.to_str()?;
    let prefix = name.split('_').next().unwrap_or(name);
    let digits = prefix
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.len() < 14 {
        return None;
    }
    digits[..14].parse().ok()
}

fn parse_transcript_path_from_session_json(path: &Path, agent_dir: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    let transcript = value.get("transcript_path")?.as_str()?.trim();
    if transcript.is_empty() {
        return None;
    }
    Some(map_runtime_transcript_path(agent_dir, transcript))
}

fn parse_transcript_path_from_hook_env(path: &Path, agent_dir: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(path).ok()?;
    for line in raw.lines() {
        let line = line.trim();
        let Some(value) = line.strip_prefix("TRANSCRIPT_PATH=") else {
            continue;
        };
        let value = shell_unescape_basic(value.trim());
        if value.is_empty() {
            return None;
        }
        return Some(map_runtime_transcript_path(agent_dir, &value));
    }
    None
}

fn shell_unescape_basic(value: &str) -> String {
    let trimmed = value.trim_matches('"').trim_matches('\'');
    let mut out = String::with_capacity(trimmed.len());
    let mut chars = trimmed.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
            continue;
        }
        out.push(ch);
    }
    out
}

fn map_runtime_transcript_path(agent_dir: &Path, transcript_path: &str) -> PathBuf {
    if let Some(rel) = transcript_path.strip_prefix("/home/agent/") {
        return agent_dir.join("sandbox-home").join(rel);
    }
    PathBuf::from(transcript_path)
}

fn collect_transcript_candidates_for_agent(agent_dir: &Path, out: &mut Vec<PathBuf>) {
    let session_dir = agent_dir.join("session");
    if session_dir.is_dir() {
        collect_session_transcripts_recursive(&session_dir, out);
    }

    let logs_dir = agent_dir.join("logs");
    if let Ok(entries) = std::fs::read_dir(&logs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.starts_with("session_start") || !name.ends_with(".json") {
                continue;
            }
            if let Some(transcript_path) = parse_transcript_path_from_session_json(&path, agent_dir)
            {
                out.push(transcript_path);
            }
        }
    }

    let hook_env = agent_dir.join("runtime").join("hook_session.env");
    if hook_env.is_file() {
        if let Some(transcript_path) = parse_transcript_path_from_hook_env(&hook_env, agent_dir) {
            out.push(transcript_path);
        }
    }
}

fn find_agent_log_near_recording(file: &Path) -> Option<PathBuf> {
    if let Some(manifest_path) = find_clawblox_manifest_near(file) {
        if let Ok(manifest) = manifest::load(&manifest_path) {
            for agent in &manifest.agents {
                for transcript in &agent.transcripts {
                    let path = resolve_clawblox_manifest_path(&manifest_path, transcript);
                    if path.is_file() {
                        return Some(path);
                    }
                }
            }
        }
    }

    let world_instance_dir = find_world_instance_dir_near_recording(file)?;
    let agents_dir = world_instance_dir.join("agents");
    if !agents_dir.is_dir() {
        return None;
    }

    let mut candidates = Vec::new();
    let Ok(agent_entries) = std::fs::read_dir(&agents_dir) else {
        return None;
    };
    for entry in agent_entries.flatten() {
        let agent_dir = entry.path();
        if !agent_dir.is_dir() {
            continue;
        }
        collect_transcript_candidates_for_agent(&agent_dir, &mut candidates);
    }
    if candidates.is_empty() {
        return None;
    }

    let mut deduped = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in candidates {
        let key = path.to_string_lossy().to_string();
        if seen.insert(key) {
            deduped.push(path);
        }
    }

    let recording_key = timestamp_key_from_recording(file);
    let recording_modified = std::fs::metadata(file)
        .and_then(|meta| meta.modified())
        .ok();
    let mut ranked = deduped
        .into_iter()
        .map(|path| {
            let diff = match (recording_key, timestamp_key_from_transcript(&path)) {
                (Some(recording), Some(agent)) => recording.abs_diff(agent),
                _ => u64::MAX,
            };
            let modified = std::fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .ok();
            let modified_diff = match (recording_modified, modified) {
                (Some(recording), Some(candidate)) => {
                    if let Ok(delta) = recording.duration_since(candidate) {
                        delta.as_secs()
                    } else if let Ok(delta) = candidate.duration_since(recording) {
                        delta.as_secs()
                    } else {
                        u64::MAX
                    }
                }
                _ => u64::MAX,
            };
            let exists = path.is_file();
            (diff, modified_diff, !exists, modified, path)
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| b.3.cmp(&a.3))
            .then_with(|| a.4.cmp(&b.4))
    });

    ranked
        .into_iter()
        .find_map(|(_, _, missing, _, path)| if missing { None } else { Some(path) })
}

fn sanitize_relative_path(value: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in Path::new(value).components() {
        match component {
            std::path::Component::Normal(part) => out.push(part),
            _ => return None,
        }
    }
    Some(out)
}

fn app_session_base_path(state: &AppState, id: &str) -> String {
    join_base_path(&state.base_path, &format!("/sessions/{}", id))
}

fn resolve_manifest_path(manifest_dir: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        manifest_dir.join(path)
    }
}

fn substitute_external_replay_arg(
    value: &str,
    recording_path: &Path,
    manifest_path: &Path,
    manifest_dir: &Path,
    port: u16,
) -> String {
    value
        .replace("{recording}", &recording_path.to_string_lossy())
        .replace("{manifest}", &manifest_path.to_string_lossy())
        .replace("{manifest_dir}", &manifest_dir.to_string_lossy())
        .replace("{port}", &port.to_string())
}

fn choose_external_replay_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("failed to allocate replay port: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("failed to read replay port: {}", e))?
        .port();
    drop(listener);
    Ok(port)
}

async fn wait_for_external_replay(
    url: &str,
    process: &ExternalReplayProcess,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let client = reqwest::Client::new();
    loop {
        if let Ok(mut child) = process.child.lock() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    return Err(format!(
                        "external replay command exited early with {}",
                        status
                    ));
                }
                Ok(None) => {}
                Err(err) => {
                    return Err(format!(
                        "failed to inspect external replay command: {}",
                        err
                    ));
                }
            }
        }

        if let Ok(resp) = client.get(url).send().await {
            if resp.status().is_success() || resp.status().is_redirection() {
                return Ok(());
            }
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "external replay server did not become ready at {}",
                url
            ));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn external_replay_info(url: &str) -> Result<serde_json::Value, String> {
    let info_url = join_external_replay_url(url, "/replay/info");
    let resp = reqwest::Client::new()
        .get(&info_url)
        .send()
        .await
        .map_err(|e| format!("external replay info request failed '{}': {}", info_url, e))?;
    if !resp.status().is_success() {
        return Err(format!(
            "external replay info request '{}' returned {}",
            info_url,
            resp.status()
        ));
    }
    resp.json::<serde_json::Value>().await.map_err(|e| {
        format!(
            "external replay info response was not JSON '{}': {}",
            info_url, e
        )
    })
}

fn load_agent_timeline_from_manifest_paths(
    transcript_paths: &[(PathBuf, Box<dyn TranscriptLineParser>)],
    info: &serde_json::Value,
    anchors: &[ReplayTickAnchor],
) -> Option<Arc<ReplayAgentTimeline>> {
    let tick_rate = info
        .get("tick_rate")
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(60);
    let first_tick = info
        .get("first_tick")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let mut messages = Vec::new();
    let mut anchor_count = 0usize;

    for (path, parser) in transcript_paths {
        match load_agent_timeline_with_parser(path, tick_rate, first_tick, anchors, parser.as_ref())
        {
            Ok(timeline) => {
                anchor_count += timeline.anchor_count;
                messages.extend(timeline.messages);
            }
            Err(err) => {
                eprintln!(
                    "Warning: failed to load manifest transcript '{}': {}",
                    path.display(),
                    err
                );
            }
        }
    }

    if messages.is_empty() {
        return None;
    }
    messages.sort_by(|a, b| {
        a.tick
            .partial_cmp(&b.tick)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.timestamp_ms.cmp(&b.timestamp_ms))
    });
    Some(Arc::new(ReplayAgentTimeline {
        messages,
        anchor_count,
    }))
}

fn load_workspace_timeline_from_manifest_paths(
    event_paths: &[PathBuf],
    info: &serde_json::Value,
    anchors: &[ReplayTickAnchor],
) -> Option<Arc<ReplayWorkspaceTimeline>> {
    let tick_rate = info
        .get("tick_rate")
        .and_then(|value| value.as_u64())
        .unwrap_or(60) as f64;
    let first_tick = info
        .get("first_tick")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as f64;
    let ticks_per_ms = tick_rate / 1000.0;
    let mut events = Vec::new();

    for path in event_paths {
        match load_workspace_events(path) {
            Ok(workspace_events) => {
                for event in workspace_events {
                    let timestamp_ms = (event.wall_time * 1000.0).round() as i64;
                    let (tick, approx) =
                        infer_external_replay_tick(timestamp_ms, anchors, ticks_per_ms, first_tick);
                    events.push(ReplayWorkspaceEvent {
                        event_type: event.event_type,
                        wall_time: event.wall_time,
                        time: event.time,
                        agent_id: event.agent_id,
                        agent_name: event.agent_name,
                        initial: event.initial,
                        tick,
                        approx,
                        changes: event.changes,
                    });
                }
            }
            Err(err) => {
                eprintln!(
                    "Warning: failed to load workspace recording '{}': {}",
                    path.display(),
                    err
                );
            }
        }
    }

    if events.is_empty() {
        return None;
    }
    events.sort_by(|a, b| {
        a.tick
            .partial_cmp(&b.tick)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.time.cmp(&b.time))
    });
    Some(Arc::new(ReplayWorkspaceTimeline { events }))
}

fn infer_external_replay_tick(
    timestamp_ms: i64,
    anchors: &[ReplayTickAnchor],
    fallback_ticks_per_ms: f64,
    first_tick: f64,
) -> (f64, bool) {
    if anchors.is_empty() {
        return (first_tick.max(0.0), true);
    }
    match anchors.binary_search_by_key(&timestamp_ms, |anchor| anchor.timestamp_ms) {
        Ok(index) => (anchors[index].tick as f64, false),
        Err(0) => {
            let slope = if anchors.len() >= 2 {
                external_replay_slope(&anchors[0], &anchors[1], fallback_ticks_per_ms)
            } else {
                fallback_ticks_per_ms
            };
            let dt = (timestamp_ms - anchors[0].timestamp_ms) as f64;
            ((anchors[0].tick as f64 + dt * slope).max(0.0), true)
        }
        Err(index) if index >= anchors.len() => {
            let last = anchors.len() - 1;
            let slope = if anchors.len() >= 2 {
                external_replay_slope(&anchors[last - 1], &anchors[last], fallback_ticks_per_ms)
            } else {
                fallback_ticks_per_ms
            };
            let dt = (timestamp_ms - anchors[last].timestamp_ms) as f64;
            ((anchors[last].tick as f64 + dt * slope).max(0.0), true)
        }
        Err(index) => {
            let a = &anchors[index - 1];
            let b = &anchors[index];
            let dt = (b.timestamp_ms - a.timestamp_ms) as f64;
            if dt.abs() < f64::EPSILON {
                return (a.tick as f64, true);
            }
            let t = (timestamp_ms - a.timestamp_ms) as f64 / dt;
            let tick = a.tick as f64 + (b.tick as f64 - a.tick as f64) * t;
            (tick.max(0.0), true)
        }
    }
}

fn external_replay_slope(
    a: &ReplayTickAnchor,
    b: &ReplayTickAnchor,
    fallback_ticks_per_ms: f64,
) -> f64 {
    let dt = (b.timestamp_ms - a.timestamp_ms) as f64;
    if dt.abs() < f64::EPSILON {
        fallback_ticks_per_ms
    } else {
        (b.tick as f64 - a.tick as f64) / dt
    }
}

fn load_replay_tick_anchors(paths: &[PathBuf]) -> Vec<ReplayTickAnchor> {
    let mut anchors = Vec::new();
    for path in paths {
        load_replay_tick_anchors_from_jsonl(path, &mut anchors);
    }
    anchors.sort_by_key(|anchor| (anchor.timestamp_ms, anchor.tick));
    anchors.dedup_by_key(|anchor| (anchor.timestamp_ms, anchor.tick));
    anchors
}

fn load_replay_tick_anchors_from_jsonl(path: &Path, out: &mut Vec<ReplayTickAnchor>) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return;
    };
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(tick) = value.get("tick").and_then(|value| value.as_u64()) else {
            continue;
        };
        let Some(timestamp_ms) = value
            .get("wall_time")
            .and_then(|value| value.as_f64())
            .map(|seconds| (seconds * 1000.0).round() as i64)
        else {
            continue;
        };
        out.push(ReplayTickAnchor { timestamp_ms, tick });
    }
}

async fn open_external_replay_command(
    manifest_path: PathBuf,
    manifest_dir: PathBuf,
    recording_path: PathBuf,
    command_template: Vec<String>,
    cwd: PathBuf,
    format: Option<String>,
    transcript_paths: Vec<(PathBuf, Box<dyn TranscriptLineParser>)>,
    event_paths: Vec<PathBuf>,
    workspace_event_paths: Vec<PathBuf>,
) -> Result<ExternalReplaySession, String> {
    if command_template.is_empty() {
        return Err(format!(
            "manifest '{}' does not declare a replay command",
            manifest_path.display()
        ));
    }
    if !recording_path.is_file() {
        return Err(format!("recording not found: {}", recording_path.display()));
    }
    if !cwd.is_dir() {
        return Err(format!("replay cwd not found: {}", cwd.display()));
    }

    let port = choose_external_replay_port()?;
    let command = command_template
        .iter()
        .map(|arg| {
            substitute_external_replay_arg(
                arg,
                &recording_path,
                &manifest_path,
                &manifest_dir,
                port,
            )
        })
        .collect::<Vec<_>>();
    let executable = command[0].clone();
    let args = command.get(1..).unwrap_or(&[]);
    let child = Command::new(&executable)
        .args(args)
        .current_dir(&cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| {
            format!(
                "failed to start external replay command '{}': {}",
                executable, e
            )
        })?;
    let process = Arc::new(ExternalReplayProcess {
        child: Mutex::new(child),
    });
    let url = format!("http://127.0.0.1:{}/", port);
    wait_for_external_replay(&url, &process).await?;
    let info = external_replay_info(&url).await?;
    let anchors = load_replay_tick_anchors(&event_paths);
    let agent_timeline =
        load_agent_timeline_from_manifest_paths(&transcript_paths, &info, &anchors);
    let workspace_timeline =
        load_workspace_timeline_from_manifest_paths(&workspace_event_paths, &info, &anchors);

    Ok(ExternalReplaySession {
        _process: process,
        url,
        recording_path,
        manifest_path,
        format,
        agent_timeline,
        workspace_timeline,
    })
}

async fn open_external_replay_manifest(
    manifest_path: PathBuf,
) -> Result<ExternalReplaySession, String> {
    let raw = std::fs::read_to_string(&manifest_path).map_err(|e| {
        format!(
            "failed to read replay manifest '{}': {}",
            manifest_path.display(),
            e
        )
    })?;
    let manifest: ExternalReplayManifest = toml::from_str(&raw).map_err(|e| {
        format!(
            "failed to parse replay manifest '{}': {}",
            manifest_path.display(),
            e
        )
    })?;
    let version = manifest.version.unwrap_or(1);
    if version != 1 {
        return Err(format!(
            "unsupported replay manifest version {} in '{}'",
            version,
            manifest_path.display()
        ));
    }
    if manifest.command.is_empty() {
        return Err(format!(
            "replay manifest '{}' must include a non-empty command",
            manifest_path.display()
        ));
    }

    let manifest_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let recording_path = resolve_manifest_path(&manifest_dir, &manifest.recording);
    if !recording_path.is_file() {
        return Err(format!(
            "manifest recording not found: {}",
            recording_path.display()
        ));
    }
    let cwd = manifest
        .cwd
        .as_deref()
        .map(|value| resolve_manifest_path(&manifest_dir, value))
        .unwrap_or_else(|| manifest_dir.clone());
    if !cwd.is_dir() {
        return Err(format!("manifest cwd not found: {}", cwd.display()));
    }

    let transcript_paths: Vec<(PathBuf, Box<dyn TranscriptLineParser>)> = manifest
        .agents
        .iter()
        .filter_map(|agent| agent.transcript.as_deref())
        .map(|path| {
            let resolved = resolve_manifest_path(&manifest_dir, path);
            let parser = detect_transcript_parser(&resolved);
            (resolved, parser)
        })
        .filter(|(path, _)| path.is_file())
        .collect();
    let event_paths = manifest
        .events
        .as_deref()
        .map(|path| resolve_manifest_path(&manifest_dir, path))
        .filter(|path| path.is_file())
        .into_iter()
        .collect::<Vec<_>>();

    open_external_replay_command(
        manifest_path,
        manifest_dir,
        recording_path,
        manifest.command,
        cwd,
        manifest.format,
        transcript_paths,
        event_paths,
        Vec::new(),
    )
    .await
}

async fn open_external_replay_from_clawblox_manifest(
    recording_path: PathBuf,
) -> Result<ExternalReplaySession, String> {
    let manifest_path = find_clawblox_manifest_near(&recording_path).ok_or_else(|| {
        format!(
            "no .clawblox/manifest.json found near recording '{}'",
            recording_path.display()
        )
    })?;
    let manifest = manifest::load(&manifest_path)?;
    let world = manifest.world.ok_or_else(|| {
        format!(
            "manifest '{}' does not contain world metadata",
            manifest_path.display()
        )
    })?;
    if world.replay_command.is_empty() {
        return Err(format!(
            "manifest '{}' does not contain a replay command",
            manifest_path.display()
        ));
    }

    let state_root = manifest::manifest_state_root(&manifest_path);
    let cwd = if world.source_dir.is_empty() {
        state_root.clone()
    } else {
        resolve_clawblox_manifest_path(&manifest_path, &world.source_dir)
    };
    let transcript_paths: Vec<(PathBuf, Box<dyn TranscriptLineParser>)> = manifest
        .agents
        .iter()
        .flat_map(|agent| {
            let backend = agent.backend.clone();
            let mp = manifest_path.clone();
            agent.transcripts.iter().map(move |path| {
                let resolved = resolve_clawblox_manifest_path(&mp, path);
                let parser: Box<dyn TranscriptLineParser> = build_agent(&backend, None, false)
                    .map(|a| a.transcript_parser())
                    .unwrap_or_else(|_| detect_transcript_parser(&resolved));
                (resolved, parser)
            })
        })
        .filter(|(path, _)| path.is_file())
        .collect();
    let event_paths = manifest
        .recordings
        .iter()
        .filter(|recording| {
            resolve_clawblox_manifest_path(&manifest_path, &recording.path) == recording_path
        })
        .filter_map(|recording| recording.events.as_deref())
        .map(|path| resolve_clawblox_manifest_path(&manifest_path, path))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    let workspace_event_paths = manifest
        .agents
        .iter()
        .filter_map(|agent| agent.workspace_recording.as_ref())
        .map(|recording| resolve_clawblox_manifest_path(&manifest_path, &recording.events))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    let format = recording_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(ToString::to_string);

    open_external_replay_command(
        manifest_path,
        state_root.clone(),
        recording_path,
        world.replay_command,
        cwd,
        format,
        transcript_paths,
        event_paths,
        workspace_event_paths,
    )
    .await
}

fn get_app_replay_state(
    state: &AppState,
    id: &str,
) -> Result<ReplayState, (axum::http::StatusCode, String)> {
    state.sessions.get(id).map(|entry| entry.clone()).ok_or((
        axum::http::StatusCode::NOT_FOUND,
        format!("unknown replay session '{}'", id),
    ))
}

fn get_app_external_session(
    state: &AppState,
    id: &str,
) -> Result<ExternalReplaySession, (axum::http::StatusCode, String)> {
    state
        .external_sessions
        .get(id)
        .map(|entry| entry.clone())
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            format!("unknown external replay session '{}'", id),
        ))
}

fn get_app_session_exists(
    state: &AppState,
    id: &str,
) -> Result<(), (axum::http::StatusCode, String)> {
    if state.sessions.contains_key(id) || state.external_sessions.contains_key(id) {
        Ok(())
    } else {
        Err((
            axum::http::StatusCode::NOT_FOUND,
            format!("unknown replay session '{}'", id),
        ))
    }
}

fn join_external_replay_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };
    format!("{}{}", base, path)
}

fn app_external_replay_base_path(state: &AppState, id: &str) -> String {
    join_base_path(&app_session_base_path(state, id), "/replay/external")
}

fn app_external_replay_path(state: &AppState, id: &str, path: &str) -> String {
    join_base_path(&app_external_replay_base_path(state, id), path)
}

fn external_replay_target_path(path: &str, original_uri: &axum::http::Uri) -> String {
    let mut target = if path.is_empty() {
        "/".to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };
    if let Some(query) = original_uri.query() {
        target.push('?');
        target.push_str(query);
    }
    target
}

fn external_replay_ws_url(base: &str, path: &str) -> Result<String, String> {
    let url = join_external_replay_url(base, path);
    if let Some(rest) = url.strip_prefix("http://") {
        Ok(format!("ws://{}", rest))
    } else if let Some(rest) = url.strip_prefix("https://") {
        Ok(format!("wss://{}", rest))
    } else {
        Err(format!("unsupported external replay URL '{}'", url))
    }
}

fn selected_ws_protocols(headers: &axum::http::HeaderMap) -> Vec<String> {
    headers
        .get(axum::http::header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn is_ws_upgrade(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(axum::http::header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
}

fn map_ws_message_to_tungstenite(message: Message) -> Option<tungstenite::Message> {
    match message {
        Message::Text(value) => Some(tungstenite::Message::Text(value.to_string().into())),
        Message::Binary(value) => Some(tungstenite::Message::Binary(value.to_vec().into())),
        Message::Ping(value) => Some(tungstenite::Message::Ping(value.to_vec().into())),
        Message::Pong(value) => Some(tungstenite::Message::Pong(value.to_vec().into())),
        Message::Close(frame) => Some(tungstenite::Message::Close(frame.map(|frame| {
            tungstenite::protocol::CloseFrame {
                code: tungstenite::protocol::frame::coding::CloseCode::from(u16::from(frame.code)),
                reason: frame.reason.to_string().into(),
            }
        }))),
    }
}

fn map_tungstenite_message_to_ws(message: tungstenite::Message) -> Option<Message> {
    match message {
        tungstenite::Message::Text(value) => Some(Message::Text(value.to_string().into())),
        tungstenite::Message::Binary(value) => Some(Message::Binary(value.to_vec().into())),
        tungstenite::Message::Ping(value) => Some(Message::Ping(value.to_vec().into())),
        tungstenite::Message::Pong(value) => Some(Message::Pong(value.to_vec().into())),
        tungstenite::Message::Close(frame) => Some(Message::Close(frame.map(|frame| {
            axum::extract::ws::CloseFrame {
                code: u16::from(frame.code),
                reason: frame.reason.to_string().into(),
            }
        }))),
        tungstenite::Message::Frame(_) => None,
    }
}

async fn proxy_external_replay_json(
    session: &ExternalReplaySession,
    path: &str,
) -> Result<serde_json::Value, (axum::http::StatusCode, String)> {
    let url = join_external_replay_url(&session.url, path);
    let resp = reqwest::Client::new().get(&url).send().await.map_err(|e| {
        (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("external replay request failed '{}': {}", url, e),
        )
    })?;
    let status = resp.status();
    if !status.is_success() {
        return Err((
            axum::http::StatusCode::BAD_GATEWAY,
            format!("external replay request '{}' returned {}", url, status),
        ));
    }
    resp.json::<serde_json::Value>().await.map_err(|e| {
        (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("external replay response was not JSON '{}': {}", url, e),
        )
    })
}

async fn proxy_external_replay_http(
    session: &ExternalReplaySession,
    path: &str,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    let url = join_external_replay_url(&session.url, path);
    let resp = reqwest::Client::new().get(&url).send().await.map_err(|e| {
        (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("external replay request failed '{}': {}", url, e),
        )
    })?;
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.bytes().await.map_err(|e| {
        (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("external replay response read failed '{}': {}", url, e),
        )
    })?;

    let mut builder = axum::response::Response::builder().status(status);
    for name in [
        axum::http::header::CONTENT_TYPE,
        axum::http::header::CACHE_CONTROL,
        axum::http::header::ETAG,
        axum::http::header::LAST_MODIFIED,
    ] {
        if let Some(value) = headers.get(&name) {
            builder = builder.header(name, value);
        }
    }
    builder.body(axum::body::Body::from(bytes)).map_err(|e| {
        (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("failed to build external replay response: {}", e),
        )
    })
}

async fn proxy_external_replay_ws(socket: WebSocket, target_url: String, protocols: Vec<String>) {
    let mut request = match tungstenite::client::IntoClientRequest::into_client_request(&target_url)
    {
        Ok(request) => request,
        Err(_) => return,
    };
    if !protocols.is_empty() {
        if let Ok(value) = axum::http::HeaderValue::from_str(&protocols.join(", ")) {
            request
                .headers_mut()
                .insert(axum::http::header::SEC_WEBSOCKET_PROTOCOL, value);
        }
    }

    let Ok((upstream, _)) = tokio_tungstenite::connect_async(request).await else {
        return;
    };

    let (mut browser_tx, mut browser_rx) = socket.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();

    let browser_to_upstream = async {
        while let Some(result) = browser_rx.next().await {
            let Ok(message) = result else {
                break;
            };
            let Some(message) = map_ws_message_to_tungstenite(message) else {
                continue;
            };
            if upstream_tx.send(message).await.is_err() {
                break;
            }
        }
    };

    let upstream_to_browser = async {
        while let Some(result) = upstream_rx.next().await {
            let Ok(message) = result else {
                break;
            };
            let Some(message) = map_tungstenite_message_to_ws(message) else {
                continue;
            };
            if browser_tx.send(message).await.is_err() {
                break;
            }
        }
    };

    tokio::select! {
        _ = browser_to_upstream => {},
        _ = upstream_to_browser => {},
    }
}

async fn app_external_replay_proxy(
    state: AppState,
    id: String,
    path: String,
    original_uri: axum::http::Uri,
    headers: axum::http::HeaderMap,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    let session = get_app_external_session(&state, &id)?;
    let target_path = external_replay_target_path(&path, &original_uri);
    if is_ws_upgrade(&headers) {
        let ws = ws.map_err(|_| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                "invalid websocket upgrade request".to_string(),
            )
        })?;
        let protocols = selected_ws_protocols(&headers);
        let target_url = external_replay_ws_url(&session.url, &target_path)
            .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, e))?;
        let response = if protocols.is_empty() {
            ws.on_upgrade(move |socket| proxy_external_replay_ws(socket, target_url, protocols))
        } else {
            ws.protocols(protocols.clone())
                .on_upgrade(move |socket| proxy_external_replay_ws(socket, target_url, protocols))
        };
        return Ok(response.into_response());
    }
    proxy_external_replay_http(&session, &target_path).await
}

async fn post_external_replay(
    session: &ExternalReplaySession,
    path: &str,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    let url = join_external_replay_url(&session.url, path);
    let resp = reqwest::Client::new()
        .post(&url)
        .send()
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::BAD_GATEWAY,
                format!("external replay request failed '{}': {}", url, e),
            )
        })?;
    if resp.status().is_success() {
        Ok(axum::http::StatusCode::OK)
    } else {
        Err((
            axum::http::StatusCode::BAD_GATEWAY,
            format!(
                "external replay request '{}' returned {}",
                url,
                resp.status()
            ),
        ))
    }
}

async fn app_ui_index(State(state): State<AppState>) -> impl IntoResponse {
    embedded_index_response("app/index.html", &state.base_path)
}

async fn app_tree(
    State(state): State<AppState>,
) -> Result<Json<AppTreeResponse>, (axum::http::StatusCode, String)> {
    let entries = list_project_recordings(&state.project_root)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let root_name = state
        .project_root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| state.project_root.display().to_string());
    Ok(Json(AppTreeResponse {
        root_name,
        root_path: state.project_root.display().to_string(),
        tree: build_tree_from_recordings(&entries),
    }))
}

async fn app_open_replay(
    State(state): State<AppState>,
    Json(body): Json<AppOpenReplayRequest>,
) -> Result<Json<AppOpenReplayResponse>, (axum::http::StatusCode, String)> {
    let rel = sanitize_relative_path(&body.path).ok_or((
        axum::http::StatusCode::BAD_REQUEST,
        "invalid relative recording path".to_string(),
    ))?;
    let recording_path = state.project_root.join(&rel);
    if !recording_path.is_file() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            format!("replay entry not found: {}", recording_path.display()),
        ));
    }

    if is_external_replay_manifest_path(&recording_path) {
        let session_id = Uuid::new_v4().to_string();
        let session = open_external_replay_manifest(recording_path.clone())
            .await
            .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e))?;
        let resolved_recording_path = session.recording_path.display().to_string();
        let world_dir = session
            .manifest_path
            .parent()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let replay_format = session.format.clone();
        state.external_sessions.insert(session_id.clone(), session);
        let session_url = app_session_base_path(&state, &session_id);

        return Ok(Json(AppOpenReplayResponse {
            session_id,
            url: session_url,
            recording_path: resolved_recording_path,
            world_dir,
            replay_format,
            agent_log: None,
        }));
    }

    if recording_path.extension().and_then(|ext| ext.to_str()) == Some("h5") {
        let session_id = Uuid::new_v4().to_string();
        let session = open_external_replay_from_clawblox_manifest(recording_path.clone())
            .await
            .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e))?;
        let resolved_recording_path = session.recording_path.display().to_string();
        let world_dir = manifest::manifest_state_root(&session.manifest_path)
            .display()
            .to_string();
        let replay_format = session.format.clone();
        state.external_sessions.insert(session_id.clone(), session);
        let session_url = app_session_base_path(&state, &session_id);

        return Ok(Json(AppOpenReplayResponse {
            session_id,
            url: session_url,
            recording_path: resolved_recording_path,
            world_dir,
            replay_format,
            agent_log: None,
        }));
    }

    if recording_path.extension().and_then(|ext| ext.to_str()) != Some("clawrec") {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "selected file is not a .clawrec recording, .h5 manifest recording, or .replay.toml manifest".to_string(),
        ));
    }

    let session_id = Uuid::new_v4().to_string();
    let session_base = app_session_base_path(&state, &session_id);
    let agent_log = find_agent_log_near_recording(&recording_path);
    let replay_state = build_replay_state(
        recording_path.clone(),
        1.0,
        &session_base,
        None,
        agent_log.clone(),
    )
    .await
    .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e))?;
    let world_dir = replay_state.world_dir.display().to_string();
    state.sessions.insert(session_id.clone(), replay_state);

    Ok(Json(AppOpenReplayResponse {
        session_id,
        url: session_base,
        recording_path: recording_path.display().to_string(),
        world_dir,
        replay_format: Some("clawrec".to_string()),
        agent_log: agent_log.map(|path| path.display().to_string()),
    }))
}

async fn app_session_ui_index(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    get_app_session_exists(&state, &id)?;
    Ok(embedded_ui_index_response(&app_session_base_path(
        &state, &id,
    )))
}

async fn app_replay_spectate_ws(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    let replay_state = get_app_replay_state(&state, &id)?;
    Ok(ws.on_upgrade(move |socket| handle_replay_ws(socket, replay_state)))
}

async fn app_replay_renderer_manifest(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<LocalRendererManifest>, (axum::http::StatusCode, String)> {
    let replay_state = get_app_replay_state(&state, &id)?;
    Ok(Json(replay_state.renderer_manifest))
}

async fn app_replay_info(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    if let Ok(session) = get_app_external_session(&state, &id) {
        return proxy_external_replay_json(&session, "/replay/info")
            .await
            .map(Json);
    }
    let replay_state = get_app_replay_state(&state, &id)?;
    let pb = replay_state.playback.lock().await;
    serde_json::to_value(pb.info()).map(Json).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to serialize replay info: {}", e),
        )
    })
}

async fn app_replay_external_view(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<ExternalReplayViewResponse>, (axum::http::StatusCode, String)> {
    if let Ok(session) = get_app_external_session(&state, &id) {
        let _ = session;
        return Ok(Json(ExternalReplayViewResponse {
            enabled: true,
            url: Some(app_external_replay_path(&state, &id, "/replay/view")),
        }));
    }
    get_app_replay_state(&state, &id)?;
    Ok(Json(ExternalReplayViewResponse {
        enabled: false,
        url: None,
    }))
}

async fn app_external_replay_proxy_root(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    OriginalUri(original_uri): OriginalUri,
    headers: axum::http::HeaderMap,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    app_external_replay_proxy(state, id, String::new(), original_uri, headers, ws).await
}

async fn app_external_replay_proxy_path(
    AxumPath(params): AxumPath<ExternalReplayProxyPath>,
    State(state): State<AppState>,
    OriginalUri(original_uri): OriginalUri,
    headers: axum::http::HeaderMap,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    app_external_replay_proxy(state, params.id, params.path, original_uri, headers, ws).await
}

async fn app_replay_state_at(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    Query(query): Query<ReplayStateQuery>,
) -> Result<Json<SpectatorObservation>, (axum::http::StatusCode, String)> {
    let replay_state = get_app_replay_state(&state, &id)?;
    let tick = if let Some(tick) = query.tick {
        tick
    } else {
        let pb = replay_state.playback.lock().await;
        pb.current_tick()
    };
    replay_state
        .reader
        .get_state_at(tick)
        .await
        .map(Json)
        .map_err(|err| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                format!("Failed to load replay state at tick {}: {}", tick, err),
            )
        })
}

async fn app_replay_agent_messages(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    Query(query): Query<ReplayAgentQuery>,
) -> Result<Json<ReplayAgentMessagesResponse>, (axum::http::StatusCode, String)> {
    if let Ok(session) = get_app_external_session(&state, &id) {
        let info = proxy_external_replay_json(&session, "/replay/info")
            .await
            .ok();
        let current_tick = query
            .tick
            .or_else(|| info.as_ref()?.get("current_tick")?.as_u64())
            .unwrap_or(0);
        let Some(timeline) = session.agent_timeline.as_ref() else {
            return Ok(Json(ReplayAgentMessagesResponse {
                enabled: false,
                current_tick,
                start_tick: 0,
                end_tick: current_tick,
                messages: Vec::new(),
            }));
        };
        return Ok(Json(ReplayAgentMessagesResponse {
            enabled: true,
            current_tick,
            start_tick: 0,
            end_tick: current_tick,
            messages: timeline.messages.clone(),
        }));
    }
    let replay_state = get_app_replay_state(&state, &id)?;
    let current_tick = if let Some(tick) = query.tick {
        tick
    } else {
        let pb = replay_state.playback.lock().await;
        pb.current_tick()
    };
    let start_tick = 0;
    let end_tick = current_tick;
    let Some(timeline) = replay_state.agent_timeline.as_ref() else {
        return Ok(Json(ReplayAgentMessagesResponse {
            enabled: false,
            current_tick,
            start_tick,
            end_tick,
            messages: Vec::new(),
        }));
    };
    Ok(Json(ReplayAgentMessagesResponse {
        enabled: true,
        current_tick,
        start_tick,
        end_tick,
        messages: timeline.messages.clone(),
    }))
}

async fn app_replay_workspace_events(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    Query(query): Query<ReplayWorkspaceQuery>,
) -> Result<Json<ReplayWorkspaceEventsResponse>, (axum::http::StatusCode, String)> {
    if let Ok(session) = get_app_external_session(&state, &id) {
        let info = proxy_external_replay_json(&session, "/replay/info")
            .await
            .ok();
        let current_tick = query
            .tick
            .or_else(|| info.as_ref()?.get("current_tick")?.as_u64())
            .unwrap_or(0);
        let Some(timeline) = session.workspace_timeline.as_ref() else {
            return Ok(Json(ReplayWorkspaceEventsResponse {
                enabled: false,
                current_tick,
                start_tick: 0,
                end_tick: current_tick,
                events: Vec::new(),
            }));
        };
        return Ok(Json(ReplayWorkspaceEventsResponse {
            enabled: true,
            current_tick,
            start_tick: 0,
            end_tick: current_tick,
            events: timeline.events.clone(),
        }));
    }
    let replay_state = get_app_replay_state(&state, &id)?;
    let current_tick = if let Some(tick) = query.tick {
        tick
    } else {
        let pb = replay_state.playback.lock().await;
        pb.current_tick()
    };
    Ok(Json(ReplayWorkspaceEventsResponse {
        enabled: false,
        current_tick,
        start_tick: 0,
        end_tick: current_tick,
        events: Vec::new(),
    }))
}

async fn app_replay_metadata(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<ReplayMetadataResponse>, (axum::http::StatusCode, String)> {
    if get_app_external_session(&state, &id).is_ok() {
        return Ok(Json(ReplayMetadataResponse {
            enabled: false,
            metadata: None,
        }));
    }
    let replay_state = get_app_replay_state(&state, &id)?;
    Ok(Json(ReplayMetadataResponse {
        enabled: replay_state.replay_metadata.is_some(),
        metadata: replay_state
            .replay_metadata
            .as_ref()
            .map(|metadata| (**metadata).clone()),
    }))
}

async fn app_replay_seek(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    Query(q): Query<SeekQuery>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    if let Ok(session) = get_app_external_session(&state, &id) {
        return post_external_replay(&session, &format!("/replay/seek?tick={}", q.tick)).await;
    }
    let replay_state = get_app_replay_state(&state, &id)?;
    let mut pb = replay_state.playback.lock().await;
    pb.seek(q.tick);
    Ok(axum::http::StatusCode::OK)
}

async fn app_replay_speed(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    Query(q): Query<SpeedQuery>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    if let Ok(session) = get_app_external_session(&state, &id) {
        return post_external_replay(&session, &format!("/replay/speed?value={}", q.value)).await;
    }
    let replay_state = get_app_replay_state(&state, &id)?;
    let mut pb = replay_state.playback.lock().await;
    pb.set_speed(q.value);
    Ok(axum::http::StatusCode::OK)
}

async fn app_replay_skip_idle(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    Query(q): Query<SkipIdleQuery>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    if let Ok(session) = get_app_external_session(&state, &id) {
        return post_external_replay(
            &session,
            &format!("/replay/skip-idle?enabled={}", q.enabled),
        )
        .await;
    }
    let replay_state = get_app_replay_state(&state, &id)?;
    let mut pb = replay_state.playback.lock().await;
    pb.set_skip_idle(q.enabled);
    Ok(axum::http::StatusCode::OK)
}

async fn app_replay_pause(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    if let Ok(session) = get_app_external_session(&state, &id) {
        return post_external_replay(&session, "/replay/pause").await;
    }
    let replay_state = get_app_replay_state(&state, &id)?;
    let mut pb = replay_state.playback.lock().await;
    pb.set_paused(true);
    Ok(axum::http::StatusCode::OK)
}

async fn app_replay_play(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    if let Ok(session) = get_app_external_session(&state, &id) {
        return post_external_replay(&session, "/replay/play").await;
    }
    let replay_state = get_app_replay_state(&state, &id)?;
    let mut pb = replay_state.playback.lock().await;
    pb.play();
    Ok(axum::http::StatusCode::OK)
}

async fn app_replay_play_range(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    Query(q): Query<PlayRangeQuery>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    if let Ok(session) = get_app_external_session(&state, &id) {
        return post_external_replay(
            &session,
            &format!(
                "/replay/play-range?start_tick={}&end_tick={}",
                q.start_tick, q.end_tick
            ),
        )
        .await;
    }
    let replay_state = get_app_replay_state(&state, &id)?;
    let mut pb = replay_state.playback.lock().await;
    pb.play_range(q.start_tick, q.end_tick);
    Ok(axum::http::StatusCode::OK)
}

async fn app_replay_world_file(
    AxumPath(params): AxumPath<SessionWorldFilePath>,
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    let replay_state = get_app_replay_state(&state, &params.id)?;
    let rel = sanitize_relative_path(&params.path).ok_or((
        axum::http::StatusCode::BAD_REQUEST,
        "invalid session file path".to_string(),
    ))?;
    let uri_path = uri.path();
    let root = if uri_path.contains("/renderer-files/") {
        replay_state.world_dir.join("renderer")
    } else if uri_path.contains("/assets/") {
        replay_state.world_dir.join("assets")
    } else {
        replay_state.world_dir.join("static")
    };
    let file_path = root.join(rel);
    let bytes = std::fs::read(&file_path).map_err(|e| {
        (
            axum::http::StatusCode::NOT_FOUND,
            format!("failed to read '{}': {}", file_path.display(), e),
        )
    })?;
    let content_type = content_type_for_path(file_path.to_string_lossy().as_ref());
    Ok((
        axum::http::StatusCode::OK,
        [("content-type", content_type)],
        bytes,
    )
        .into_response())
}
