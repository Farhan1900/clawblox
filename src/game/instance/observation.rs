use super::*;
use crate::game::profiling::{record_values, ProfileFrame};

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

fn build_spectator_entity_from_data(
    data: &crate::game::lua::instance::InstanceData,
) -> Option<SpectatorEntity> {
    let part_data = data.part_data.as_ref()?;
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

    let billboard_gui = GameInstance::collect_billboard_gui(&data.children);
    let attrs = attributes_to_json(&data.attributes);

    Some(SpectatorEntity {
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
        render: build_render(data, part_data),
        health: None,
        pickup_type: None,
        model_url: GameInstance::extract_model_url(&data.attributes),
        model_yaw_offset_deg: GameInstance::extract_model_yaw_offset_deg(&data.attributes),
        attributes: if attrs.is_empty() { None } else { Some(attrs) },
        billboard_gui,
    })
}

fn replication_entity_instance(instance: Instance) -> Option<Instance> {
    let mut current = Some(instance);
    while let Some(inst) = current {
        if inst.data.lock().unwrap().part_data.is_some() {
            return Some(inst);
        }
        current = inst.parent();
    }
    None
}

pub(super) fn build_player_observation(
    instance: &GameInstance,
    agent_id: Uuid,
) -> Option<PlayerObservation> {
    let mut profile = ProfileFrame::new("observation.player");
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
    if let Some(profile) = profile.as_mut() {
        profile.mark("player_state");
    }

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
    if let Some(profile) = profile.as_mut() {
        profile.mark("player_attributes");
    }

    let observation_radius = instance.effective_observation_radius(agent_id);

    // Get other players (with LOS filtering)
    let other_players = instance.get_other_players(agent_id, observation_radius);
    if let Some(profile) = profile.as_mut() {
        profile.mark("other_players");
    }

    // Get dynamic world entities only (static entities fetched via /map endpoint)
    let world = instance.get_dynamic_world_info(position, observation_radius);
    if let Some(profile) = profile.as_mut() {
        profile.mark("dynamic_world");
    }

    if let Some(profile) = profile {
        profile.finish();
    }

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
    let mut profile = ProfileFrame::new("observation.spectator");
    let mut entities = Vec::new();
    let mut players = Vec::new();
    let mut sounds = Vec::new();
    let mut workspace_descendants = 0usize;
    let mut part_entities = 0usize;
    let mut sound_instances = 0usize;

    if let Some(runtime) = &instance.lua_runtime {
        // Collect all parts and sounds from Workspace
        let descendants = runtime.workspace().get_descendants();
        workspace_descendants = descendants.len();
        if let Some(profile) = profile.as_mut() {
            profile.mark("get_descendants");
        }
        for part in descendants {
            let data = part.data.lock().unwrap();

            // Collect playing Sound instances
            if let Some(sound_data) = &data.sound_data {
                sound_instances += 1;
                if sound_data.is_playing {
                    sounds.push(SpectatorSound {
                        id: data.id.0 as u32,
                        stream_id: sound_data.stream_id.clone(),
                        volume: sound_data.volume,
                        is_playing: true,
                    });
                }
            }

            if data.part_data.is_some() {
                part_entities += 1;
                if let Some(entity) = build_spectator_entity_from_data(&data) {
                    entities.push(entity);
                }
            }
        }
        if let Some(profile) = profile.as_mut() {
            profile.mark("workspace_entities");
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
        if let Some(profile) = profile.as_mut() {
            profile.mark("players");
        }
    }

    record_values(
        "observation.spectator",
        &[
            ("workspace_descendants", workspace_descendants as f64),
            ("part_entities", part_entities as f64),
            ("players", players.len() as f64),
            ("sounds", sound_instances as f64),
            ("playing_sounds", sounds.len() as f64),
            ("emitted_entities", entities.len() as f64),
        ],
    );
    if let Some(profile) = profile {
        profile.finish();
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

impl GameInstance {
    /// Gets the observation for a specific player.
    pub fn get_player_observation(&self, agent_id: Uuid) -> Option<PlayerObservation> {
        build_player_observation(self, agent_id)
    }

    /// Get static map geometry (entities with "Static" tag).
    pub fn get_map_info(&self) -> MapInfo {
        build_map_info(self)
    }

    /// Gets the spectator observation (full world state from Lua Workspace).
    pub fn get_spectator_observation(&self) -> SpectatorObservation {
        build_spectator_observation(self)
    }

    pub fn replication_revision(&self) -> u64 {
        let Some(runtime) = &self.lua_runtime else {
            return 0;
        };
        runtime
            .lua()
            .app_data_ref::<crate::game::replication::SharedWorldReplicationState>()
            .map(|shared| shared.lock().unwrap().latest_revision())
            .unwrap_or(0)
    }

    pub fn get_replication_observation_update(
        &self,
        known_entity_ids: &HashSet<u32>,
        since_revision: u64,
    ) -> ReplicationObservationUpdate {
        let mut observation = self.get_spectator_player_observation();
        let mut entities = Vec::new();
        let mut removed_entity_ids = Vec::new();
        let mut emitted_entity_ids = HashSet::new();
        let mut dirty_instance_count = 0usize;
        let mut latest_revision = since_revision;

        if let Some(runtime) = &self.lua_runtime {
            let dirty = runtime
                .lua()
                .app_data_ref::<crate::game::replication::SharedWorldReplicationState>()
                .map(|shared| {
                    let state = shared.lock().unwrap();
                    latest_revision = state.latest_revision();
                    state.changes_since(since_revision)
                })
                .unwrap_or_default();
            dirty_instance_count = dirty.len();
            let workspace = runtime.workspace().instance;

            for dirty in dirty {
                let Some(instance_ref) = dirty.instance.upgrade() else {
                    if let Ok(entity_id) = u32::try_from(dirty.id) {
                        if known_entity_ids.contains(&entity_id) {
                            removed_entity_ids.push(entity_id);
                        }
                    }
                    continue;
                };

                let instance = Instance::from_ref(instance_ref);
                let Some(entity_instance) = replication_entity_instance(instance) else {
                    if let Ok(entity_id) = u32::try_from(dirty.id) {
                        if known_entity_ids.contains(&entity_id) {
                            removed_entity_ids.push(entity_id);
                        }
                    }
                    continue;
                };

                let entity_id = entity_instance.id().0 as u32;
                if !entity_instance.is_descendant_of(&workspace) {
                    if known_entity_ids.contains(&entity_id) {
                        removed_entity_ids.push(entity_id);
                    }
                    continue;
                }
                if !emitted_entity_ids.insert(entity_id) {
                    continue;
                }

                let data = entity_instance.data.lock().unwrap();
                if let Some(entity) = build_spectator_entity_from_data(&data) {
                    entities.push(entity);
                } else if known_entity_ids.contains(&entity_id) {
                    removed_entity_ids.push(entity_id);
                }
            }
        }

        removed_entity_ids.sort_unstable();
        removed_entity_ids.dedup();
        observation.entities = entities;

        record_values(
            "observation.replication_update",
            &[
                ("dirty_instances", dirty_instance_count as f64),
                ("emitted_entities", observation.entities.len() as f64),
                ("removed_entities", removed_entity_ids.len() as f64),
            ],
        );

        ReplicationObservationUpdate {
            observation,
            removed_entity_ids,
            dirty_instance_count,
            latest_revision,
        }
    }

    /// Gets high-rate spectator state that does not scan Workspace entities.
    pub fn get_spectator_player_observation(&self) -> SpectatorObservation {
        let mut players = Vec::new();

        if let Some(runtime) = &self.lua_runtime {
            for (&agent_id, &user_id) in &self.players {
                if let Some(player) = runtime.players().get_player_by_user_id(user_id) {
                    let player_data = player.data.lock().unwrap();
                    let (position, health, humanoid_state, root_part_id, animator_id) = player_data
                        .player_data
                        .as_ref()
                        .and_then(|pd| pd.character.as_ref())
                        .and_then(|weak| weak.upgrade())
                        .map(|char_ref| {
                            let char = char_ref.lock().unwrap();
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
                                    animator_id =
                                        child_data.children.iter().find_map(|anim_child| {
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

                    let name = self
                        .player_names
                        .get(&agent_id)
                        .cloned()
                        .unwrap_or_else(|| format!("Player_{}", agent_id.as_simple()));

                    let mut attrs = attributes_to_json(&player_data.attributes);
                    if let Some(origin) = self.get_player_view_origin(agent_id) {
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
                    if let Some(forward) = self.get_player_view_forward(agent_id) {
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
                        serde_json::Value::from(round_f32(self.player_view_fov_deg())),
                    );
                    let attributes = if attrs.is_empty() {
                        None
                    } else {
                        Some(serde_json::to_value(&attrs).unwrap_or(serde_json::Value::Null))
                    };

                    let gui = player_data
                        .player_data
                        .as_ref()
                        .and_then(|pd| pd.player_gui.as_ref())
                        .and_then(|weak| weak.upgrade())
                        .map(|player_gui_ref| {
                            let player_gui = Instance::from_ref(player_gui_ref);
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
                }
            }
        }

        SpectatorObservation {
            instance_id: self.instance_id,
            tick: self.tick,
            server_time_ms: self.elapsed_ms(),
            game_status: if self.halted_error.is_some() {
                "failed".to_string()
            } else {
                match self.status {
                    GameStatus::Waiting => "waiting".to_string(),
                    GameStatus::Playing => "playing".to_string(),
                    GameStatus::Finished => "finished".to_string(),
                }
            },
            players,
            entities: Vec::new(),
            sounds: Vec::new(),
            recent_events: self.recent_public_replicated_events(),
        }
    }

    /// Get the game status from Lua if available.
    fn get_game_status_from_lua(&self) -> String {
        if self.halted_error.is_some() {
            return "failed".to_string();
        }
        match self.status {
            GameStatus::Waiting => "waiting".to_string(),
            GameStatus::Playing => "active".to_string(),
            GameStatus::Finished => "finished".to_string(),
        }
    }

    fn effective_observation_radius(&self, agent_id: Uuid) -> f32 {
        let default_radius = self.observation_radius.max(0.0);
        let Some(runtime) = &self.lua_runtime else {
            return default_radius;
        };
        let Some(user_id) = self.players.get(&agent_id).copied() else {
            return default_radius;
        };
        let Some(player) = runtime.players().get_player_by_user_id(user_id) else {
            return default_radius;
        };

        let player_data = player.data.lock().unwrap();
        match player_data.attributes.get("ObservationRadius") {
            Some(AttributeValue::Number(radius)) if radius.is_finite() => {
                (*radius as f32).clamp(0.0, default_radius)
            }
            _ => default_radius,
        }
    }

    /// Get dynamic world info (entities WITHOUT "Static" tag + folders with attributes).
    fn get_dynamic_world_info(&self, observer_pos: [f32; 3], observation_radius: f32) -> WorldInfo {
        let mut profile = ProfileFrame::new("observation.dynamic_world");
        let mut entities = Vec::new();
        let mut workspace_descendants = 0usize;
        let mut part_entities = 0usize;
        let mut in_radius_parts = 0usize;
        let mut folder_entities = 0usize;

        if let Some(runtime) = &self.lua_runtime {
            let descendants = runtime.workspace().get_descendants();
            workspace_descendants = descendants.len();
            if let Some(profile) = profile.as_mut() {
                profile.mark("get_descendants");
            }
            for part in descendants {
                let data = part.data.lock().unwrap();
                let is_static = data.tags.contains("Static");

                if let Some(part_data) = &data.part_data {
                    if !is_static {
                        part_entities += 1;
                        let raw_position = [
                            part_data.position.x,
                            part_data.position.y,
                            part_data.position.z,
                        ];
                        let raw_size = [part_data.size.x, part_data.size.y, part_data.size.z];
                        if !part_intersects_observation_radius(
                            observer_pos,
                            raw_position,
                            raw_size,
                            part_data.cframe.rotation,
                            observation_radius,
                        ) {
                            continue;
                        }
                        in_radius_parts += 1;
                        let (aabb, obb) =
                            compute_part_bounds(raw_position, raw_size, part_data.cframe.rotation);
                        let attrs = attributes_to_json(&data.attributes);
                        entities.push(WorldEntity {
                            id: data.id.0,
                            name: data.name.clone(),
                            entity_type: Some("part".to_string()),
                            position: round_position(raw_position),
                            size: round_position(raw_size),
                            rotation: Some(part_data.cframe.rotation),
                            aabb: Some(aabb),
                            obb: Some(obb),
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
                } else if data.class_name == ClassName::Folder && !is_static {
                    let attrs = attributes_to_json(&data.attributes);
                    if !attrs.is_empty() {
                        folder_entities += 1;
                        entities.push(WorldEntity {
                            id: data.id.0,
                            name: data.name.clone(),
                            entity_type: Some("folder".to_string()),
                            position: [0.0, 0.0, 0.0],
                            size: [0.0, 0.0, 0.0],
                            rotation: None,
                            aabb: None,
                            obb: None,
                            color: None,
                            material: None,
                            shape: None,
                            transparency: None,
                            anchored: true,
                            attributes: Some(attrs),
                        });
                    }
                }
            }
            if let Some(profile) = profile.as_mut() {
                profile.mark("scan_entities");
            }
        }

        record_values(
            "observation.dynamic_world",
            &[
                ("workspace_descendants", workspace_descendants as f64),
                ("dynamic_part_entities", part_entities as f64),
                ("in_radius_parts", in_radius_parts as f64),
                ("folder_entities", folder_entities as f64),
                ("emitted_entities", entities.len() as f64),
            ],
        );
        if let Some(profile) = profile {
            profile.finish();
        }

        WorldInfo { entities }
    }

    /// Get world info (all visible parts and folders from Workspace).
    #[allow(dead_code)]
    fn get_world_info(&self) -> WorldInfo {
        let mut entities = Vec::new();

        if let Some(runtime) = &self.lua_runtime {
            for part in runtime.workspace().get_descendants() {
                let data = part.data.lock().unwrap();

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
                } else if data.class_name == ClassName::Folder {
                    let attrs = attributes_to_json(&data.attributes);
                    if !attrs.is_empty() {
                        entities.push(WorldEntity {
                            id: data.id.0,
                            name: data.name.clone(),
                            entity_type: Some("folder".to_string()),
                            position: [0.0, 0.0, 0.0],
                            size: [0.0, 0.0, 0.0],
                            rotation: None,
                            aabb: None,
                            obb: None,
                            color: None,
                            material: None,
                            shape: None,
                            transparency: None,
                            anchored: true,
                            attributes: Some(attrs),
                        });
                    }
                }
            }
        }

        WorldInfo { entities }
    }

    fn get_player_view_origin(&self, agent_id: Uuid) -> Option<[f32; 3]> {
        let pos = self.get_player_position(agent_id)?;
        Some([pos[0], pos[1] + 2.0, pos[2]])
    }

    fn get_player_view_forward(&self, agent_id: Uuid) -> Option<[f32; 3]> {
        let hrp_id = *self.player_hrp_ids.get(&agent_id)?;
        let state = self.physics.get_character_state(hrp_id)?;
        let rot = self.physics.get_rotation_matrix(state.body_handle)?;
        // Roblox/CFrame look vector convention: local -Z points forward.
        let mut fx = -rot[0][2];
        let mut fz = -rot[2][2];
        let mag = (fx * fx + fz * fz).sqrt();
        if mag <= 1e-4 {
            return Some([0.0, 0.0, -1.0]);
        }
        fx /= mag;
        fz /= mag;
        Some([fx, 0.0, fz])
    }

    fn player_view_fov_deg(&self) -> f32 {
        // Matches spectator camera intent (75deg vertical perspective on widescreen ~= ~105deg horizontal).
        105.0
    }

    /// Get info about other players with distance, line-of-sight and view-cone filtering.
    fn get_other_players(
        &self,
        exclude_agent_id: Uuid,
        observation_radius: f32,
    ) -> Vec<OtherPlayerInfo> {
        let mut others = Vec::new();
        let observer_pos = self
            .get_player_position(exclude_agent_id)
            .unwrap_or([0.0, 0.0, 0.0]);
        let observer_eye = self.get_player_view_origin(exclude_agent_id).unwrap_or([
            observer_pos[0],
            observer_pos[1] + 2.0,
            observer_pos[2],
        ]);
        let observer_body = self
            .player_hrp_ids
            .get(&exclude_agent_id)
            .and_then(|&hrp_id| self.physics.get_character_state(hrp_id))
            .map(|state| state.body_handle);

        for (&agent_id, &user_id) in &self.players {
            if agent_id == exclude_agent_id {
                continue;
            }

            let position = self
                .get_player_position(agent_id)
                .unwrap_or([0.0, 0.0, 0.0]);

            let dx = position[0] - observer_pos[0];
            let dy = position[1] - observer_pos[1];
            let dz = position[2] - observer_pos[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();

            if distance > observation_radius {
                continue;
            }

            let target_eye = [position[0], position[1] + 2.0, position[2]];
            let target_body = self
                .player_hrp_ids
                .get(&agent_id)
                .and_then(|&hrp_id| self.physics.get_character_state(hrp_id))
                .map(|state| state.body_handle);
            if !self
                .physics
                .has_line_of_sight(observer_eye, target_eye, observer_body, target_body)
            {
                continue;
            }

            let health = self.get_player_health(agent_id).unwrap_or(100);

            let attributes = if let Some(runtime) = &self.lua_runtime {
                if let Some(player) = runtime.players().get_player_by_user_id(user_id) {
                    let data = player.data.lock().unwrap();
                    attributes_to_json(&data.attributes)
                } else {
                    std::collections::HashMap::new()
                }
            } else {
                std::collections::HashMap::new()
            };

            others.push(OtherPlayerInfo {
                id: agent_id,
                name: self.player_names.get(&agent_id).cloned(),
                position: round_position(position),
                distance: round_f32(distance),
                health,
                attributes,
            });
        }

        others
    }

    /// Serializes a GUI instance tree to GuiElement for frontend rendering.
    fn serialize_gui_tree(instance: &Instance) -> Option<GuiElement> {
        let data = instance.data.lock().unwrap();

        let element_type = match data.class_name {
            ClassName::ScreenGui => "ScreenGui",
            ClassName::Frame => "Frame",
            ClassName::TextLabel => "TextLabel",
            ClassName::TextButton => "TextButton",
            ClassName::ImageLabel => "ImageLabel",
            ClassName::ImageButton => "ImageButton",
            _ => return None,
        };

        let gui_data = data.gui_data.as_ref();

        let mut element = GuiElement {
            id: data.id.0,
            element_type: element_type.to_string(),
            name: data.name.clone(),
            position: gui_data.map(|g| UDim2Json {
                x_scale: g.position.x.scale,
                x_offset: g.position.x.offset,
                y_scale: g.position.y.scale,
                y_offset: g.position.y.offset,
            }),
            size: gui_data.map(|g| UDim2Json {
                x_scale: g.size.x.scale,
                x_offset: g.size.x.offset,
                y_scale: g.size.y.scale,
                y_offset: g.size.y.offset,
            }),
            anchor_point: gui_data.map(|g| [g.anchor_point.0, g.anchor_point.1]),
            rotation: gui_data.map(|g| g.rotation),
            z_index: gui_data.map(|g| g.z_index),
            visible: gui_data.map(|g| g.visible),
            background_color: gui_data.map(|g| {
                [
                    g.background_color.r,
                    g.background_color.g,
                    g.background_color.b,
                ]
            }),
            background_transparency: gui_data.map(|g| g.background_transparency),
            border_color: gui_data.map(|g| [g.border_color.r, g.border_color.g, g.border_color.b]),
            border_size_pixel: gui_data.map(|g| g.border_size_pixel),
            text: gui_data.and_then(|g| g.text.clone()),
            text_color: gui_data.and_then(|g| g.text_color.map(|c| [c.r, c.g, c.b])),
            text_size: gui_data.and_then(|g| g.text_size),
            text_transparency: gui_data.and_then(|g| g.text_transparency),
            text_x_alignment: gui_data.map(|g| match g.text_x_alignment {
                TextXAlignment::Left => "Left".to_string(),
                TextXAlignment::Center => "Center".to_string(),
                TextXAlignment::Right => "Right".to_string(),
            }),
            text_y_alignment: gui_data.map(|g| match g.text_y_alignment {
                TextYAlignment::Top => "Top".to_string(),
                TextYAlignment::Center => "Center".to_string(),
                TextYAlignment::Bottom => "Bottom".to_string(),
            }),
            image: gui_data.and_then(|g| g.image.clone()),
            image_color: gui_data.and_then(|g| g.image_color.map(|c| [c.r, c.g, c.b])),
            image_transparency: gui_data.and_then(|g| g.image_transparency),
            display_order: gui_data.map(|g| g.display_order),
            enabled: gui_data.map(|g| g.enabled),
            children: Vec::new(),
        };

        let children_refs: Vec<_> = data.children.iter().cloned().collect();
        drop(data);

        for child_ref in children_refs {
            let child = Instance::from_ref(child_ref);
            if let Some(child_element) = Self::serialize_gui_tree(&child) {
                element.children.push(child_element);
            }
        }

        Some(element)
    }

    /// Collects BillboardGui data from a part's children.
    fn collect_billboard_gui(
        children: &[crate::game::lua::instance::InstanceRef],
    ) -> Option<BillboardGuiJson> {
        for child_ref in children {
            let child_data = child_ref.lock().unwrap();

            if child_data.class_name == ClassName::BillboardGui {
                if let Some(billboard_data) = &child_data.billboard_gui_data {
                    let mut labels = Vec::new();

                    for label_ref in &child_data.children {
                        let label_data = label_ref.lock().unwrap();

                        if label_data.class_name == ClassName::TextLabel {
                            if let Some(gui_data) = &label_data.gui_data {
                                labels.push(BillboardLabelJson {
                                    text: gui_data.text.clone().unwrap_or_default(),
                                    color: [
                                        gui_data.text_color.map(|c| c.r).unwrap_or(1.0),
                                        gui_data.text_color.map(|c| c.g).unwrap_or(1.0),
                                        gui_data.text_color.map(|c| c.b).unwrap_or(1.0),
                                    ],
                                    size: gui_data.text_size.unwrap_or(14.0) as f32,
                                });
                            }
                        }
                    }

                    return Some(BillboardGuiJson {
                        studs_offset: [
                            billboard_data.studs_offset.x,
                            billboard_data.studs_offset.y,
                            billboard_data.studs_offset.z,
                        ],
                        always_on_top: billboard_data.always_on_top,
                        labels,
                    });
                }
            }
        }

        None
    }

    /// Reads a model URL from instance attributes.
    fn extract_model_url(
        attributes: &std::collections::HashMap<String, AttributeValue>,
    ) -> Option<String> {
        if let Some(AttributeValue::String(url)) = attributes.get("ModelUrl") {
            let trimmed = url.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    /// Reads model local yaw correction (degrees) from instance attributes.
    fn extract_model_yaw_offset_deg(
        attributes: &std::collections::HashMap<String, AttributeValue>,
    ) -> Option<f32> {
        if let Some(AttributeValue::Number(value)) = attributes.get("ModelYawOffsetDeg") {
            if value.is_finite() {
                return Some(*value as f32);
            }
        }
        None
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

#[derive(Debug, Clone)]
pub struct ReplicationObservationUpdate {
    pub observation: SpectatorObservation,
    pub removed_entity_ids: Vec<u32>,
    pub dirty_instance_count: usize,
    pub latest_revision: u64,
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
