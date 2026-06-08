use super::*;

/// Round a float to 2 decimal places (reduces JSON payload size)
#[inline]
pub(super) fn round_f32(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

/// Round a position array to 2 decimal places
#[inline]
pub(super) fn round_position(pos: [f32; 3]) -> [f32; 3] {
    [round_f32(pos[0]), round_f32(pos[1]), round_f32(pos[2])]
}

#[inline]
fn mat_mul_vec3(rot: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        rot[0][0] * v[0] + rot[0][1] * v[1] + rot[0][2] * v[2],
        rot[1][0] * v[0] + rot[1][1] * v[1] + rot[1][2] * v[2],
        rot[2][0] * v[0] + rot[2][1] * v[1] + rot[2][2] * v[2],
    ]
}

pub(super) fn compute_part_bounds(
    center: [f32; 3],
    size: [f32; 3],
    rotation: [[f32; 3]; 3],
) -> (EntityAabb, EntityObb) {
    let half = [
        size[0].abs() * 0.5,
        size[1].abs() * 0.5,
        size[2].abs() * 0.5,
    ];
    let local_corners = [
        [-half[0], -half[1], -half[2]],
        [half[0], -half[1], -half[2]],
        [-half[0], half[1], -half[2]],
        [half[0], half[1], -half[2]],
        [-half[0], -half[1], half[2]],
        [half[0], -half[1], half[2]],
        [-half[0], half[1], half[2]],
        [half[0], half[1], half[2]],
    ];

    let mut raw_corners = [[0.0; 3]; 8];
    for (i, corner) in local_corners.iter().enumerate() {
        let rotated = mat_mul_vec3(&rotation, *corner);
        raw_corners[i] = [
            center[0] + rotated[0],
            center[1] + rotated[1],
            center[2] + rotated[2],
        ];
    }

    let mut min = raw_corners[0];
    let mut max = raw_corners[0];
    for corner in raw_corners.iter().skip(1) {
        min[0] = min[0].min(corner[0]);
        min[1] = min[1].min(corner[1]);
        min[2] = min[2].min(corner[2]);
        max[0] = max[0].max(corner[0]);
        max[1] = max[1].max(corner[1]);
        max[2] = max[2].max(corner[2]);
    }

    let mut rounded_corners = [[0.0; 3]; 8];
    for (i, corner) in raw_corners.iter().enumerate() {
        rounded_corners[i] = round_position(*corner);
    }

    (
        EntityAabb {
            min: round_position(min),
            max: round_position(max),
        },
        EntityObb {
            center: round_position(center),
            half_extents: round_position(half),
            corners: rounded_corners,
        },
    )
}

#[inline]
fn sq_distance_point_to_obb(
    point: [f32; 3],
    center: [f32; 3],
    half_extents: [f32; 3],
    rotation: [[f32; 3]; 3],
) -> f32 {
    let rel = [
        point[0] - center[0],
        point[1] - center[1],
        point[2] - center[2],
    ];
    // Rotation is local->world, so transpose it to evaluate the point in
    // the part's local frame.
    let local = [
        rotation[0][0] * rel[0] + rotation[1][0] * rel[1] + rotation[2][0] * rel[2],
        rotation[0][1] * rel[0] + rotation[1][1] * rel[1] + rotation[2][1] * rel[2],
        rotation[0][2] * rel[0] + rotation[1][2] * rel[1] + rotation[2][2] * rel[2],
    ];

    let mut sq_distance = 0.0;
    for axis in 0..3 {
        let overflow = (local[axis].abs() - half_extents[axis]).max(0.0);
        sq_distance += overflow * overflow;
    }
    sq_distance
}

#[inline]
pub(super) fn part_intersects_observation_radius(
    observer_pos: [f32; 3],
    center: [f32; 3],
    size: [f32; 3],
    rotation: [[f32; 3]; 3],
    radius: f32,
) -> bool {
    let half_extents = [
        size[0].abs() * 0.5,
        size[1].abs() * 0.5,
        size[2].abs() * 0.5,
    ];
    sq_distance_point_to_obb(observer_pos, center, half_extents, rotation) <= radius * radius
}

fn attr_string(
    attrs: &std::collections::HashMap<String, AttributeValue>,
    key: &str,
) -> Option<String> {
    match attrs.get(key) {
        Some(AttributeValue::String(v)) => Some(v.clone()),
        _ => None,
    }
}

fn attr_bool(attrs: &std::collections::HashMap<String, AttributeValue>, key: &str) -> Option<bool> {
    match attrs.get(key) {
        Some(AttributeValue::Bool(v)) => Some(*v),
        _ => None,
    }
}

fn attr_color(
    attrs: &std::collections::HashMap<String, AttributeValue>,
    key: &str,
) -> Option<[f32; 3]> {
    match attrs.get(key) {
        Some(AttributeValue::Color3(c)) => Some([c.r, c.g, c.b]),
        Some(AttributeValue::Vector3(v)) => Some([v.x, v.y, v.z]),
        _ => None,
    }
}

fn build_render(
    data: &crate::game::lua::instance::InstanceData,
    part_data: &crate::game::lua::instance::PartData,
) -> SpectatorRender {
    let role =
        attr_string(&data.attributes, "RenderRole").unwrap_or_else(|| "unspecified".to_string());
    let preset_id = attr_string(&data.attributes, "RenderPresetId");
    let kind = if preset_id.is_some() {
        "preset"
    } else {
        "primitive"
    }
    .to_string();
    let primitive = attr_string(&data.attributes, "RenderPrimitive")
        .unwrap_or_else(|| part_data.shape.name().to_string().to_lowercase());
    let material = attr_string(&data.attributes, "RenderMaterial")
        .unwrap_or_else(|| part_data.material.name().to_string());
    let color = attr_color(&data.attributes, "RenderColor").unwrap_or([
        part_data.color.r,
        part_data.color.g,
        part_data.color.b,
    ]);
    let is_static = attr_bool(&data.attributes, "RenderStatic")
        .unwrap_or(data.tags.contains("Static") || part_data.anchored);
    let visible =
        attr_bool(&data.attributes, "RenderVisible").unwrap_or(part_data.transparency < 1.0);
    let casts_shadow = attr_bool(&data.attributes, "RenderCastsShadow").unwrap_or(true);
    let receives_shadow = attr_bool(&data.attributes, "RenderReceivesShadow").unwrap_or(true);

    SpectatorRender {
        kind,
        role,
        preset_id,
        primitive,
        material,
        color,
        is_static,
        casts_shadow,
        receives_shadow,
        visible,
        double_sided: false,
        transparency: if part_data.transparency != 0.0 {
            Some(part_data.transparency)
        } else {
            None
        },
    }
}

pub(super) fn build_player_observation(
    instance: &GameInstance,
    agent_id: Uuid,
) -> Option<PlayerObservation> {
    let user_id = *instance.players.get(&agent_id)?;

    let runtime = instance.lua_runtime.as_ref()?;
    let player = runtime.players().get_player_by_user_id(user_id)?;

    // Get position from character's HumanoidRootPart (rounded to reduce payload size)
    let position = round_position(
        instance
            .get_player_position(agent_id)
            .unwrap_or([0.0, 3.0, 0.0]),
    );

    {
        let mut counts = instance.observation_log_counts.lock().unwrap();
        let count = counts.entry(agent_id).or_insert(0);
        if *count < 5 {
            let name = instance
                .player_names
                .get(&agent_id)
                .cloned()
                .unwrap_or_default();
            eprintln!(
                "[Obs] tick={} agent={} name={} pos=({:.2},{:.2},{:.2})",
                instance.tick, agent_id, name, position[0], position[1], position[2]
            );
            *count += 1;
        }
    }

    // Get health from humanoid
    let health = instance.get_player_health(agent_id).unwrap_or(100);

    // Read all player attributes generically and convert to JSON
    let player_data = player.data.lock().unwrap();
    let mut attributes = attributes_to_json(&player_data.attributes);
    if let Some(origin) = instance.get_player_view_origin(agent_id) {
        attributes.insert(
            "ViewOriginX".to_string(),
            serde_json::Value::from(round_f32(origin[0])),
        );
        attributes.insert(
            "ViewOriginY".to_string(),
            serde_json::Value::from(round_f32(origin[1])),
        );
        attributes.insert(
            "ViewOriginZ".to_string(),
            serde_json::Value::from(round_f32(origin[2])),
        );
    }
    if let Some(forward) = instance.get_player_view_forward(agent_id) {
        attributes.insert(
            "ViewForwardX".to_string(),
            serde_json::Value::from(round_f32(forward[0])),
        );
        attributes.insert(
            "ViewForwardY".to_string(),
            serde_json::Value::from(round_f32(forward[1])),
        );
        attributes.insert(
            "ViewForwardZ".to_string(),
            serde_json::Value::from(round_f32(forward[2])),
        );
    }
    attributes.insert(
        "ViewFovDeg".to_string(),
        serde_json::Value::from(round_f32(instance.player_view_fov_deg())),
    );
    drop(player_data);

    // Get other players (with LOS filtering)
    let other_players = instance.get_other_players(agent_id);

    // Get dynamic world entities only (static entities fetched via /map endpoint)
    let world = instance.get_dynamic_world_info(position);

    Some(PlayerObservation {
        tick: instance.tick,
        game_status: instance.get_game_status_from_lua(),
        player: PlayerInfo {
            id: agent_id,
            position,
            health,
            attributes,
        },
        other_players,
        world,
        events: Vec::new(),
        recent_events: instance.recent_replicated_events_for_user(user_id),
    })
}

pub(super) fn build_map_info(instance: &GameInstance) -> MapInfo {
    let mut entities = Vec::new();

    if let Some(runtime) = &instance.lua_runtime {
        for part in runtime.workspace().get_descendants() {
            let data = part.data.lock().unwrap();

            // Only include entities with "Static" tag
            if !data.tags.contains("Static") {
                continue;
            }

            if let Some(part_data) = &data.part_data {
                let attrs = attributes_to_json(&data.attributes);
                entities.push(WorldEntity {
                    id: data.id.0,
                    name: data.name.clone(),
                    entity_type: Some("part".to_string()),
                    position: [
                        part_data.position.x,
                        part_data.position.y,
                        part_data.position.z,
                    ],
                    size: [part_data.size.x, part_data.size.y, part_data.size.z],
                    rotation: Some(part_data.cframe.rotation),
                    aabb: None,
                    obb: None,
                    color: Some([part_data.color.r, part_data.color.g, part_data.color.b]),
                    material: Some(part_data.material.name().to_string()),
                    shape: Some(part_data.shape.name().to_string()),
                    transparency: if part_data.transparency != 0.0 {
                        Some(part_data.transparency)
                    } else {
                        None
                    },
                    anchored: part_data.anchored,
                    attributes: if attrs.is_empty() { None } else { Some(attrs) },
                });
            }
        }
    }

    MapInfo { entities }
}

pub(super) fn build_spectator_observation(instance: &GameInstance) -> SpectatorObservation {
    let mut entities = Vec::new();
    let mut players = Vec::new();
    let mut sounds = Vec::new();

    if let Some(runtime) = &instance.lua_runtime {
        // Collect all parts and sounds from Workspace
        for part in runtime.workspace().get_descendants() {
            let data = part.data.lock().unwrap();

            // Collect playing Sound instances
            if let Some(sound_data) = &data.sound_data {
                if sound_data.is_playing {
                    sounds.push(SpectatorSound {
                        id: data.id.0 as u32,
                        stream_id: sound_data.stream_id.clone(),
                        volume: sound_data.volume,
                        is_playing: true,
                    });
                }
            }

            if let Some(part_data) = &data.part_data {
                // Only include rotation if it's not identity
                let rot = part_data.cframe.rotation;
                let is_identity = (rot[0][0] - 1.0).abs() < 0.001
                    && rot[0][1].abs() < 0.001
                    && rot[0][2].abs() < 0.001
                    && rot[1][0].abs() < 0.001
                    && (rot[1][1] - 1.0).abs() < 0.001
                    && rot[1][2].abs() < 0.001
                    && rot[2][0].abs() < 0.001
                    && rot[2][1].abs() < 0.001
                    && (rot[2][2] - 1.0).abs() < 0.001;

                // Check for BillboardGui children
                let billboard_gui = GameInstance::collect_billboard_gui(&data.children);
                let attrs = attributes_to_json(&data.attributes);

                entities.push(SpectatorEntity {
                    id: data.id.0 as u32,
                    entity_type: "part".to_string(),
                    position: round_position([
                        part_data.position.x,
                        part_data.position.y,
                        part_data.position.z,
                    ]),
                    rotation: if is_identity { None } else { Some(rot) },
                    size: Some(round_position([
                        part_data.size.x,
                        part_data.size.y,
                        part_data.size.z,
                    ])),
                    shape: Some(part_data.shape.name().to_string()),
                    render: build_render(&data, part_data),
                    health: None,
                    pickup_type: None,
                    model_url: GameInstance::extract_model_url(&data.attributes),
                    model_yaw_offset_deg: GameInstance::extract_model_yaw_offset_deg(
                        &data.attributes,
                    ),
                    attributes: if attrs.is_empty() { None } else { Some(attrs) },
                    billboard_gui,
                });
            }
        }

        // Collect player info
        for (&agent_id, &user_id) in &instance.players {
            if let Some(player) = runtime.players().get_player_by_user_id(user_id) {
                let player_data = player.data.lock().unwrap();

                // Get position and health in one pass (avoid redundant locking)
                let (position, health, humanoid_state, root_part_id, animator_id) = player_data
                    .player_data
                    .as_ref()
                    .and_then(|pd| pd.character.as_ref())
                    .and_then(|weak| weak.upgrade())
                    .map(|char_ref| {
                        let char = char_ref.lock().unwrap();

                        // Get position from HumanoidRootPart
                        let primary_part = char
                            .model_data
                            .as_ref()
                            .and_then(|m| m.primary_part.as_ref())
                            .and_then(|weak| weak.upgrade());
                        let (pos, root_part_id) = primary_part
                            .as_ref()
                            .and_then(|hrp_data| {
                                let hrp = hrp_data.lock().unwrap();
                                hrp.part_data.as_ref().map(|p| {
                                    (
                                        [p.position.x, p.position.y, p.position.z],
                                        Some(hrp.id.0 as u32),
                                    )
                                })
                            })
                            .unwrap_or(([0.0, 3.0, 0.0], None));

                        // Get health from Humanoid (while we have character locked)
                        let mut hp = 100;
                        let mut animator_id: Option<u64> = None;
                        let mut humanoid_state: Option<String> = None;
                        for child in &char.children {
                            let child_data = child.lock().unwrap();
                            if child_data.name == "Humanoid" {
                                if let Some(humanoid) = &child_data.humanoid_data {
                                    hp = humanoid.health as i32;
                                    humanoid_state = Some(humanoid.state.name().to_string());
                                }
                                animator_id = child_data.children.iter().find_map(|anim_child| {
                                    let anim_data = anim_child.lock().unwrap();
                                    if anim_data.class_name == ClassName::Animator {
                                        Some(anim_data.id.0)
                                    } else {
                                        None
                                    }
                                });
                                break;
                            }
                        }

                        (pos, hp, humanoid_state, root_part_id, animator_id)
                    })
                    .unwrap_or(([0.0, 3.0, 0.0], 100, None, None, None));

                let active_animations = animator_id.and_then(|id| {
                    runtime
                        .lua()
                        .app_data_ref::<crate::game::lua::animation::AnimationScheduler>()
                        .map(|scheduler| {
                            let tracks = scheduler.active_tracks_for_animator(id);
                            tracks
                                .into_iter()
                                .map(|track| SpectatorPlayerAnimation {
                                    track_id: track.track_id,
                                    animation_id: track.animation_id,
                                    length: round_f32(track.length),
                                    priority: track.priority,
                                    time_position: round_f32(track.time_position),
                                    speed: round_f32(track.speed),
                                    looped: track.looped,
                                    is_playing: track.is_playing,
                                    is_stopping: track.is_stopping,
                                    weight_current: round_f32(track.weight_current),
                                    weight_target: round_f32(track.weight_target),
                                    effective_weight: round_f32(track.effective_weight),
                                })
                                .collect::<Vec<_>>()
                        })
                        .filter(|tracks| !tracks.is_empty())
                });

                // Get player name from our cache, or fall back to Player_<uuid>
                let name = instance
                    .player_names
                    .get(&agent_id)
                    .cloned()
                    .unwrap_or_else(|| format!("Player_{}", agent_id.as_simple()));

                // Get all attributes and convert to JSON Value
                let mut attrs = attributes_to_json(&player_data.attributes);
                if let Some(origin) = instance.get_player_view_origin(agent_id) {
                    attrs.insert(
                        "ViewOriginX".to_string(),
                        serde_json::Value::from(round_f32(origin[0])),
                    );
                    attrs.insert(
                        "ViewOriginY".to_string(),
                        serde_json::Value::from(round_f32(origin[1])),
                    );
                    attrs.insert(
                        "ViewOriginZ".to_string(),
                        serde_json::Value::from(round_f32(origin[2])),
                    );
                }
                if let Some(forward) = instance.get_player_view_forward(agent_id) {
                    attrs.insert(
                        "ViewForwardX".to_string(),
                        serde_json::Value::from(round_f32(forward[0])),
                    );
                    attrs.insert(
                        "ViewForwardY".to_string(),
                        serde_json::Value::from(round_f32(forward[1])),
                    );
                    attrs.insert(
                        "ViewForwardZ".to_string(),
                        serde_json::Value::from(round_f32(forward[2])),
                    );
                }
                attrs.insert(
                    "ViewFovDeg".to_string(),
                    serde_json::Value::from(round_f32(instance.player_view_fov_deg())),
                );
                let attributes = if attrs.is_empty() {
                    None
                } else {
                    Some(serde_json::to_value(&attrs).unwrap_or(serde_json::Value::Null))
                };

                // Serialize PlayerGui tree
                let gui = player_data
                    .player_data
                    .as_ref()
                    .and_then(|pd| pd.player_gui.as_ref())
                    .and_then(|weak| weak.upgrade())
                    .map(|player_gui_ref| {
                        let player_gui = Instance::from_ref(player_gui_ref);
                        // Get all ScreenGui children
                        player_gui
                            .get_children()
                            .iter()
                            .filter_map(|child| GameInstance::serialize_gui_tree(child))
                            .collect::<Vec<_>>()
                    })
                    .filter(|v: &Vec<GuiElement>| !v.is_empty());

                players.push(SpectatorPlayerInfo {
                    id: agent_id,
                    name,
                    position: round_position(position),
                    root_part_id,
                    health,
                    humanoid_state,
                    attributes,
                    gui,
                    active_animations,
                });
                drop(player_data);
            }
        }
    }

    SpectatorObservation {
        instance_id: instance.instance_id,
        tick: instance.tick,
        server_time_ms: instance.elapsed_ms(),
        game_status: if instance.halted_error.is_some() {
            "failed".to_string()
        } else {
            match instance.status {
                GameStatus::Waiting => "waiting".to_string(),
                GameStatus::Playing => "playing".to_string(),
                GameStatus::Finished => "finished".to_string(),
            }
        },
        players,
        entities,
        sounds,
        // Spectator feed is public-only: client-targeted FireClient events are excluded.
        recent_events: instance.recent_public_replicated_events(),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerObservation {
    pub tick: u64,
    pub game_status: String,
    pub player: PlayerInfo,
    pub other_players: Vec<OtherPlayerInfo>,
    pub world: WorldInfo,
    pub events: Vec<GameEvent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_events: Vec<SpectatorReplicatedEvent>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorldInfo {
    pub entities: Vec<WorldEntity>,
}

/// Static map geometry (anchored entities that never change)
#[derive(Debug, Clone, serde::Serialize)]
pub struct MapInfo {
    pub entities: Vec<WorldEntity>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorldEntity {
    pub id: u64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    pub position: [f32; 3],
    pub size: [f32; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<[[f32; 3]; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aabb: Option<EntityAabb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obb: Option<EntityObb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparency: Option<f32>,
    pub anchored: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EntityAabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EntityObb {
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
    pub corners: [[f32; 3]; 8],
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerInfo {
    pub id: Uuid,
    pub position: [f32; 3],
    pub health: i32,
    /// Game-specific attributes set by Lua scripts
    pub attributes: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OtherPlayerInfo {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub position: [f32; 3],
    pub distance: f32,
    pub health: i32,
    /// Game-specific attributes set by Lua scripts
    pub attributes: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VisibleEntity {
    pub id: u32,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub position: [f32; 3],
    pub distance: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pickup_type: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GameEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpectatorObservation {
    pub instance_id: Uuid,
    pub tick: u64,
    /// Milliseconds since game instance was created (for client clock synchronization)
    pub server_time_ms: u64,
    pub game_status: String,
    pub players: Vec<SpectatorPlayerInfo>,
    pub entities: Vec<SpectatorEntity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sounds: Vec<SpectatorSound>,
    #[serde(default)]
    pub recent_events: Vec<SpectatorReplicatedEvent>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpectatorSound {
    pub id: u32,
    pub stream_id: String,
    pub volume: f32,
    pub is_playing: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpectatorReplicatedEvent {
    pub id: u64,
    pub tick: u64,
    pub server_time_ms: u64,
    pub name: String,
    pub reliable: bool,
    pub payload: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_user_id: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpectatorPlayerInfo {
    pub id: Uuid,
    pub name: String,
    pub position: [f32; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_part_id: Option<u32>,
    pub health: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub humanoid_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gui: Option<Vec<GuiElement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_animations: Option<Vec<SpectatorPlayerAnimation>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpectatorPlayerAnimation {
    pub track_id: u64,
    pub animation_id: String,
    pub length: f32,
    pub priority: i32,
    pub time_position: f32,
    pub speed: f32,
    pub looped: bool,
    pub is_playing: bool,
    pub is_stopping: bool,
    pub weight_current: f32,
    pub weight_target: f32,
    pub effective_weight: f32,
}

/// Serialized GUI element for frontend rendering
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GuiElement {
    pub id: u64,
    #[serde(rename = "type")]
    pub element_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<UDim2Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<UDim2Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor_point: Option<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_transparency: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_color: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_size_pixel: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_transparency: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_x_alignment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_y_alignment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_color: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_transparency: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_order: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub children: Vec<GuiElement>,
}

/// UDim2 serialization format
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UDim2Json {
    pub x_scale: f32,
    pub x_offset: i32,
    pub y_scale: f32,
    pub y_offset: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpectatorEntity {
    pub id: u32,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub position: [f32; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<[[f32; 3]; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    pub render: SpectatorRender,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pickup_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_yaw_offset_deg: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<std::collections::HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billboard_gui: Option<BillboardGuiJson>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpectatorRender {
    pub kind: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<String>,
    pub primitive: String,
    pub material: String,
    pub color: [f32; 3],
    #[serde(rename = "static")]
    pub is_static: bool,
    pub casts_shadow: bool,
    pub receives_shadow: bool,
    pub visible: bool,
    pub double_sided: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparency: Option<f32>,
}

/// BillboardGui serialization for 3D floating labels
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BillboardGuiJson {
    pub studs_offset: [f32; 3],
    pub always_on_top: bool,
    pub labels: Vec<BillboardLabelJson>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BillboardLabelJson {
    pub text: String,
    pub color: [f32; 3],
    pub size: f32,
}
