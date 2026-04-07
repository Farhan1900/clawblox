use super::*;

#[derive(Clone)]
struct AppState {
    project_root: PathBuf,
    base_path: String,
    sessions: Arc<DashMap<String, ReplayState>>,
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
    let playback = replay::ReplayPlayback::new(
        first_tick,
        meta.total_ticks,
        meta.tick_rate,
        meta.duration_ms,
        speed,
    )?;

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

    let agent_timeline = if let Some(path) = agent_log_file.as_ref() {
        match load_agent_timeline(path, meta.tick_rate) {
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

    Ok(ReplayState {
        reader: Arc::new(reader),
        playback: Arc::new(tokio::sync::Mutex::new(playback)),
        base_path: normalize_base_path(base_path),
        renderer_manifest,
        agent_timeline,
        world_dir,
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
                .route("/sessions/{id}/replay/state", get(app_replay_state_at))
                .route(
                    "/sessions/{id}/replay/agent",
                    get(app_replay_agent_messages),
                )
                .route("/sessions/{id}/replay/seek", post(app_replay_seek))
                .route("/sessions/{id}/replay/speed", post(app_replay_speed))
                .route("/sessions/{id}/replay/pause", post(app_replay_pause))
                .route("/sessions/{id}/replay/play", post(app_replay_play))
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
        if path.extension().and_then(|ext| ext.to_str()) != Some("clawrec") {
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

fn get_app_replay_state(
    state: &AppState,
    id: &str,
) -> Result<ReplayState, (axum::http::StatusCode, String)> {
    state.sessions.get(id).map(|entry| entry.clone()).ok_or((
        axum::http::StatusCode::NOT_FOUND,
        format!("unknown replay session '{}'", id),
    ))
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
            format!("recording not found: {}", recording_path.display()),
        ));
    }
    if recording_path.extension().and_then(|ext| ext.to_str()) != Some("clawrec") {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "selected file is not a .clawrec recording".to_string(),
        ));
    }

    let session_id = Uuid::new_v4().to_string();
    let session_base = app_session_base_path(&state, &session_id);
    let replay_state = build_replay_state(recording_path.clone(), 1.0, &session_base, None, None)
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e))?;
    let world_dir = replay_state.world_dir.display().to_string();
    state.sessions.insert(session_id.clone(), replay_state);

    Ok(Json(AppOpenReplayResponse {
        session_id,
        url: session_base,
        recording_path: recording_path.display().to_string(),
        world_dir,
    }))
}

async fn app_session_ui_index(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    get_app_replay_state(&state, &id)?;
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
) -> Result<Json<replay::ReplayInfo>, (axum::http::StatusCode, String)> {
    let replay_state = get_app_replay_state(&state, &id)?;
    let pb = replay_state.playback.lock().await;
    Ok(Json(pb.info()))
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

async fn app_replay_seek(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
    Query(q): Query<SeekQuery>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
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
    let replay_state = get_app_replay_state(&state, &id)?;
    let mut pb = replay_state.playback.lock().await;
    pb.set_speed(q.value);
    Ok(axum::http::StatusCode::OK)
}

async fn app_replay_pause(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    let replay_state = get_app_replay_state(&state, &id)?;
    let mut pb = replay_state.playback.lock().await;
    pb.set_paused(true);
    Ok(axum::http::StatusCode::OK)
}

async fn app_replay_play(
    AxumPath(id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    let replay_state = get_app_replay_state(&state, &id)?;
    let mut pb = replay_state.playback.lock().await;
    pb.set_paused(false);
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
