use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

use crate::game::instance::GameInstance;
use crate::game::lua::instance::{
    AttributeValue, BillboardGuiData, BodyAngularVelocityData, BodyForceData, BodyVelocityData,
    CFrameValueData, ClassName, GuiObjectData, HumanoidData, Instance, InstanceData, ModelData, ObjectValueData,
    PartData, ScriptData, TextXAlignment, TextYAlignment, UICornerData, WeldConstraintData, WeldData,
};
use crate::game::lua::services::agent_input::AgentInput;
use crate::game::lua::services::remote_event::ReplicatedEvent;
use crate::game::lua::types::{
    CFrame, Color3, HumanoidStateType, Material, PartType, PhysicalProperties, UDim, UDim2,
    Vector3,
};

const SNAPSHOT_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInstanceSnapshot {
    pub schema_version: u32,
    pub game_id: Uuid,
    pub instance_id: Uuid,
    pub tick: u64,
    pub workspace_gravity: f32,
    pub workspace_children: Vec<InstanceSnapshot>,
    pub server_storage_children: Vec<InstanceSnapshot>,
    pub replicated_storage_children: Vec<InstanceSnapshot>,
    pub players: Vec<PlayerSnapshot>,
    #[serde(default)]
    pub pending_inputs: Vec<PendingInputSnapshot>,
    #[serde(default)]
    pub remote_events: Vec<ReplicatedEventSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub agent_id: Uuid,
    pub name: String,
    #[serde(default)]
    pub character_id: Option<u64>,
    #[serde(default)]
    pub humanoid_root_part_id: Option<u64>,
    #[serde(default)]
    pub attributes: HashMap<String, SnapshotAttributeValue>,
    #[serde(default)]
    pub character_attributes: HashMap<String, SnapshotAttributeValue>,
    #[serde(default)]
    pub humanoid_root_part: Option<PlayerPartSnapshot>,
    #[serde(default)]
    pub humanoid: Option<PlayerHumanoidSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPartSnapshot {
    #[serde(default)]
    pub attributes: HashMap<String, SnapshotAttributeValue>,
    pub position: [f32; 3],
    pub cframe: SnapshotCFrame,
    pub velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerHumanoidSnapshot {
    pub health: f32,
    pub max_health: f32,
    pub walk_speed: f32,
    pub jump_power: f32,
    pub jump_height: f32,
    pub auto_rotate: bool,
    pub move_direction: [f32; 3],
    pub move_input_direction: [f32; 3],
    pub running_speed: f32,
    pub state: String,
    #[serde(default)]
    pub move_to_target: Option<[f32; 3]>,
    pub cancel_move_to: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingInputSnapshot {
    pub user_id: u64,
    pub input_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicatedEventSnapshot {
    pub name: String,
    pub payload: serde_json::Value,
    pub reliable: bool,
    #[serde(default)]
    pub target_user_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceSnapshot {
    pub id: u64,
    pub class_name: String,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub attributes: HashMap<String, SnapshotAttributeValue>,
    #[serde(default)]
    pub part: Option<PartSnapshot>,
    #[serde(default)]
    pub model: Option<ModelSnapshot>,
    #[serde(default)]
    pub humanoid: Option<HumanoidSnapshot>,
    #[serde(default)]
    pub script: Option<ScriptSnapshot>,
    #[serde(default)]
    pub int_value: Option<i64>,
    #[serde(default)]
    pub number_value: Option<f64>,
    #[serde(default)]
    pub bool_value: Option<bool>,
    #[serde(default)]
    pub string_value: Option<String>,
    #[serde(default)]
    pub object_value: Option<ObjectRefSnapshot>,
    #[serde(default)]
    pub cframe_value: Option<SnapshotCFrame>,
    #[serde(default)]
    pub body_velocity: Option<BodyVelocitySnapshot>,
    #[serde(default)]
    pub body_force: Option<BodyForceSnapshot>,
    #[serde(default)]
    pub body_angular_velocity: Option<BodyAngularVelocitySnapshot>,
    #[serde(default)]
    pub weld: Option<WeldSnapshot>,
    #[serde(default)]
    pub weld_constraint: Option<WeldConstraintSnapshot>,
    #[serde(default)]
    pub billboard_gui: Option<BillboardGuiSnapshot>,
    #[serde(default)]
    pub ui_corner: Option<UICornerSnapshot>,
    #[serde(default)]
    pub gui: Option<GuiSnapshot>,
    #[serde(default)]
    pub children: Vec<InstanceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartSnapshot {
    pub position: [f32; 3],
    pub cframe: SnapshotCFrame,
    pub size: [f32; 3],
    pub anchored: bool,
    pub can_collide: bool,
    pub can_query: bool,
    pub collision_group: String,
    pub can_touch: bool,
    pub transparency: f32,
    pub color: [f32; 3],
    pub material: String,
    pub top_surface: String,
    pub bottom_surface: String,
    pub velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub massless: bool,
    #[serde(default)]
    pub custom_physical_properties: Option<PhysicalPropertiesSnapshot>,
    pub shape: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSnapshot {
    #[serde(default)]
    pub primary_part_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanoidSnapshot {
    pub health: f32,
    pub max_health: f32,
    pub walk_speed: f32,
    pub jump_power: f32,
    pub jump_height: f32,
    pub auto_rotate: bool,
    pub health_display_distance: f32,
    pub name_display_distance: f32,
    pub hip_height: f32,
    pub move_direction: [f32; 3],
    pub move_input_direction: [f32; 3],
    pub running_speed: f32,
    pub state: String,
    pub jump_requested: bool,
    #[serde(default)]
    pub move_to_target: Option<[f32; 3]>,
    pub cancel_move_to: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSnapshot {
    pub source: String,
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRefSnapshot {
    #[serde(default)]
    pub target_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyVelocitySnapshot {
    pub velocity: [f32; 3],
    pub max_force: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyForceSnapshot {
    pub force: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyAngularVelocitySnapshot {
    pub angular_velocity: [f32; 3],
    pub max_torque: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeldSnapshot {
    #[serde(default)]
    pub part0_id: Option<u64>,
    #[serde(default)]
    pub part1_id: Option<u64>,
    pub c0: SnapshotCFrame,
    pub c1: SnapshotCFrame,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeldConstraintSnapshot {
    #[serde(default)]
    pub part0_id: Option<u64>,
    #[serde(default)]
    pub part1_id: Option<u64>,
    pub enabled: bool,
    #[serde(default)]
    pub offset: Option<[f32; 3]>,
    #[serde(default)]
    pub relative_cframe: Option<SnapshotCFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillboardGuiSnapshot {
    pub size: SnapshotUDim2,
    pub studs_offset: [f32; 3],
    pub always_on_top: bool,
    pub enabled: bool,
    #[serde(default)]
    pub adornee_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UICornerSnapshot {
    pub corner_radius: SnapshotUDim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiSnapshot {
    pub position: SnapshotUDim2,
    pub size: SnapshotUDim2,
    pub anchor_point: [f32; 2],
    pub rotation: f32,
    pub z_index: i32,
    pub layout_order: i32,
    pub visible: bool,
    pub background_color: [f32; 3],
    pub background_transparency: f32,
    pub border_color: [f32; 3],
    pub border_size_pixel: i32,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub text_color: Option<[f32; 3]>,
    #[serde(default)]
    pub text_size: Option<f32>,
    #[serde(default)]
    pub text_transparency: Option<f32>,
    #[serde(default)]
    pub text_stroke_transparency: Option<f32>,
    #[serde(default)]
    pub font: Option<String>,
    pub text_scaled: bool,
    pub text_x_alignment: String,
    pub text_y_alignment: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub image_color: Option<[f32; 3]>,
    #[serde(default)]
    pub image_transparency: Option<f32>,
    pub display_order: i32,
    pub ignore_gui_inset: bool,
    pub enabled: bool,
    pub reset_on_spawn: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotCFrame {
    pub position: [f32; 3],
    pub rotation: [[f32; 3]; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotUDim {
    pub scale: f32,
    pub offset: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotUDim2 {
    pub x: SnapshotUDim,
    pub y: SnapshotUDim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalPropertiesSnapshot {
    pub density: f32,
    pub friction: f32,
    pub elasticity: f32,
    pub friction_weight: f32,
    pub elasticity_weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum SnapshotAttributeValue {
    String(String),
    Number(f64),
    Bool(bool),
    Vector3([f32; 3]),
    Color3([f32; 3]),
    Table(Vec<SnapshotAttributeValue>),
    Nil,
}

enum RefFixupKind {
    ObjectValue(Option<u64>),
    ModelPrimaryPart(Option<u64>),
    WeldPart0(Option<u64>),
    WeldPart1(Option<u64>),
    WeldConstraintPart0(Option<u64>),
    WeldConstraintPart1(Option<u64>),
    BillboardAdornee(Option<u64>),
}

struct RefFixup {
    instance: Instance,
    kind: RefFixupKind,
}

pub fn capture_game_instance(instance: &GameInstance) -> Result<GameInstanceSnapshot, String> {
    let runtime = instance
        .lua_runtime
        .as_ref()
        .ok_or_else(|| "cannot snapshot instance without an initialized Lua runtime".to_string())?;

    let workspace = runtime.workspace();
    let workspace_gravity = workspace.data.lock().unwrap().gravity;
    let workspace_children = workspace
        .get_children()
        .into_iter()
        .map(snapshot_instance)
        .collect::<Result<Vec<_>, _>>()?;
    let server_storage_children = runtime
        .game()
        .server_storage()
        .get_children()
        .into_iter()
        .map(snapshot_instance)
        .collect::<Result<Vec<_>, _>>()?;
    let replicated_storage_children = runtime
        .game()
        .replicated_storage()
        .get_children()
        .into_iter()
        .map(snapshot_instance)
        .collect::<Result<Vec<_>, _>>()?;

    let players = instance
        .players
        .iter()
        .map(|(agent_id, user_id)| capture_player_snapshot(*agent_id, *user_id, instance, runtime))
        .collect::<Result<Vec<_>, _>>()?;

    let pending_inputs = runtime
        .agent_input_service()
        .data
        .lock()
        .unwrap()
        .pending_inputs
        .iter()
        .flat_map(|(user_id, queued)| {
            queued.iter().map(|input| PendingInputSnapshot {
                user_id: *user_id,
                input_type: input.input_type.clone(),
                data: input.data.clone(),
            })
        })
        .collect();

    let remote_events = runtime
        .game()
        .remote_event_service()
        .data
        .lock()
        .unwrap()
        .queued_events
        .iter()
        .map(|evt| ReplicatedEventSnapshot {
            name: evt.name.clone(),
            payload: evt.payload.clone(),
            reliable: evt.reliable,
            target_user_id: evt.target_user_id,
        })
        .collect();

    Ok(GameInstanceSnapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        game_id: instance.game_id,
        instance_id: instance.instance_id,
        tick: instance.tick,
        workspace_gravity,
        workspace_children,
        server_storage_children,
        replicated_storage_children,
        players,
        pending_inputs,
        remote_events,
    })
}

pub fn restore_game_instance(
    instance: &mut GameInstance,
    snapshot: &GameInstanceSnapshot,
) -> Result<(), String> {
    if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported snapshot schema version {}",
            snapshot.schema_version
        ));
    }

    let (workspace, server_storage, replicated_storage, players_service, agent_input_service, remote_event_service) = {
        let runtime = instance
            .lua_runtime
            .as_ref()
            .ok_or_else(|| "cannot restore instance without an initialized Lua runtime".to_string())?;
        (
            runtime.workspace(),
            runtime.game().server_storage(),
            runtime.game().replicated_storage(),
            runtime.players(),
            runtime.agent_input_service(),
            runtime.game().remote_event_service(),
        )
    };

    instance.game_id = snapshot.game_id;
    instance.instance_id = snapshot.instance_id;
    instance.tick = snapshot.tick;

    {
        workspace.data.lock().unwrap().gravity = snapshot.workspace_gravity;
    }

    let mut instance_map = HashMap::new();
    let mut fixups = Vec::new();

    clear_workspace(&workspace);
    clear_instance_children(&server_storage);
    clear_instance_children(&replicated_storage);

    restore_root_children(
        &snapshot.workspace_children,
        &workspace.instance,
        Some(&workspace),
        &mut instance_map,
        &mut fixups,
    )?;
    restore_root_children(
        &snapshot.server_storage_children,
        &server_storage,
        None,
        &mut instance_map,
        &mut fixups,
    )?;
    restore_root_children(
        &snapshot.replicated_storage_children,
        &replicated_storage,
        None,
        &mut instance_map,
        &mut fixups,
    )?;

    apply_ref_fixups(&instance_map, fixups);

    restore_players(instance, snapshot, &players_service, &instance_map)?;

    {
        let mut pending = agent_input_service.data.lock().unwrap();
        pending.pending_inputs.clear();
        for input in &snapshot.pending_inputs {
            pending
                .pending_inputs
                .entry(input.user_id)
                .or_insert_with(Vec::new)
                .push(AgentInput::new(input.input_type.clone(), input.data.clone()));
        }
    }

    {
        let mut queued = remote_event_service.data.lock().unwrap();
        queued.queued_events = snapshot
            .remote_events
            .iter()
            .map(|evt| ReplicatedEvent {
                name: evt.name.clone(),
                payload: evt.payload.clone(),
                reliable: evt.reliable,
                target_user_id: evt.target_user_id,
            })
            .collect();
    }

    Ok(())
}

fn snapshot_instance(instance: Instance) -> Result<InstanceSnapshot, String> {
    let data = instance.data.lock().unwrap();
    let children_refs = data.children.clone();
    let snapshot = InstanceSnapshot {
        id: data.id.0,
        class_name: data.class_name.as_str().to_string(),
        name: data.name.clone(),
        tags: data.tags.iter().cloned().collect(),
        attributes: data
            .attributes
            .iter()
            .map(|(key, value)| (key.clone(), snapshot_attribute_value(value)))
            .collect(),
        part: data.part_data.as_ref().map(snapshot_part_data),
        model: data.model_data.as_ref().map(snapshot_model_data),
        humanoid: data.humanoid_data.as_ref().map(snapshot_humanoid_data),
        script: data.script_data.as_ref().map(snapshot_script_data),
        int_value: data.int_value_data.as_ref().map(|v| v.value),
        number_value: data.number_value_data.as_ref().map(|v| v.value),
        bool_value: data.bool_value_data.as_ref().map(|v| v.value),
        string_value: data.string_value_data.as_ref().map(|v| v.value.clone()),
        object_value: data.object_value_data.as_ref().map(snapshot_object_value_data),
        cframe_value: data.cframe_value_data.as_ref().map(snapshot_cframe_value_data),
        body_velocity: data.body_velocity_data.as_ref().map(snapshot_body_velocity_data),
        body_force: data.body_force_data.as_ref().map(snapshot_body_force_data),
        body_angular_velocity: data
            .body_angular_velocity_data
            .as_ref()
            .map(snapshot_body_angular_velocity_data),
        weld: data.weld_data.as_ref().map(snapshot_weld_data),
        weld_constraint: data.weld_constraint_data.as_ref().map(snapshot_weld_constraint_data),
        billboard_gui: data
            .billboard_gui_data
            .as_ref()
            .map(snapshot_billboard_gui_data),
        ui_corner: data.ui_corner_data.as_ref().map(snapshot_ui_corner_data),
        gui: data.gui_data.as_ref().map(snapshot_gui_data),
        children: Vec::new(),
    };
    drop(data);

    let children = children_refs
        .into_iter()
        .map(Instance::from_ref)
        .map(snapshot_instance)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(InstanceSnapshot {
        children,
        ..snapshot
    })
}

fn capture_player_snapshot(
    agent_id: Uuid,
    user_id: u64,
    instance: &GameInstance,
    runtime: &crate::game::lua::LuaRuntime,
) -> Result<PlayerSnapshot, String> {
    let player = runtime
        .players()
        .get_player_by_user_id(user_id)
        .ok_or_else(|| format!("missing player instance for user_id {}", user_id))?;
    let character = player
        .data
        .lock()
        .unwrap()
        .player_data
        .as_ref()
        .and_then(|pdata| pdata.character.as_ref())
        .and_then(|weak| weak.upgrade())
        .map(Instance::from_ref);

    let mut humanoid_root_part = None;
    let mut humanoid = None;
    let mut character_attributes = HashMap::new();
    let mut character_id = None;
    let mut humanoid_root_part_id = None;

    if let Some(character) = character {
        character_id = Some(character.id().0);
        {
            let char_data = character.data.lock().unwrap();
            character_attributes = char_data
                .attributes
                .iter()
                .map(|(key, value)| (key.clone(), snapshot_attribute_value(value)))
                .collect();
        }

        for child in character.get_children() {
            let data = child.data.lock().unwrap();
            if let Some(part) = &data.part_data {
                if data.name == "HumanoidRootPart" {
                    humanoid_root_part_id = Some(data.id.0);
                    humanoid_root_part = Some(PlayerPartSnapshot {
                        attributes: data
                            .attributes
                            .iter()
                            .map(|(key, value)| (key.clone(), snapshot_attribute_value(value)))
                            .collect(),
                        position: snapshot_vector3(part.position),
                        cframe: snapshot_cframe(part.cframe),
                        velocity: snapshot_vector3(part.velocity),
                        angular_velocity: snapshot_vector3(part.angular_velocity),
                    });
                }
            }
            if let Some(h) = &data.humanoid_data {
                humanoid = Some(PlayerHumanoidSnapshot {
                    health: h.health,
                    max_health: h.max_health,
                    walk_speed: h.walk_speed,
                    jump_power: h.jump_power,
                    jump_height: h.jump_height,
                    auto_rotate: h.auto_rotate,
                    move_direction: snapshot_vector3(h.move_direction),
                    move_input_direction: snapshot_vector3(h.move_input_direction),
                    running_speed: h.running_speed,
                    state: snapshot_humanoid_state(h.state),
                    move_to_target: h.move_to_target.map(snapshot_vector3),
                    cancel_move_to: h.cancel_move_to,
                });
            }
        }
    }

    Ok(PlayerSnapshot {
        agent_id,
        name: instance
            .player_names
            .get(&agent_id)
            .cloned()
            .unwrap_or_else(|| format!("Player-{}", user_id)),
        character_id,
        humanoid_root_part_id,
        attributes: player
            .get_attributes()
            .into_iter()
            .map(|(key, value)| (key, snapshot_attribute_value(&value)))
            .collect(),
        character_attributes,
        humanoid_root_part,
        humanoid,
    })
}

fn restore_players(
    instance: &mut GameInstance,
    snapshot: &GameInstanceSnapshot,
    players_service: &crate::game::lua::services::PlayersService,
    instance_map: &HashMap<u64, Instance>,
) -> Result<(), String> {
    instance.players.clear();
    instance.player_hrp_ids.clear();
    instance.player_names.clear();
    players_service.data.lock().unwrap().players.clear();

    for player in &snapshot.players {
        let user_id = GameInstance::user_id_from_agent_id(player.agent_id);
        instance.players.insert(player.agent_id, user_id);
        instance
            .player_names
            .insert(player.agent_id, player.name.clone());
        instance.record_player_activity(player.agent_id);

        let player_instance = Instance::from_data(InstanceData::new_player(user_id, &player.name));
        let player_gui = Instance::from_data(InstanceData::new_player_gui("PlayerGui"));
        player_gui.set_parent(Some(&player_instance));
        {
            let mut player_data = player_instance.data.lock().unwrap();
            let pdata = player_data
                .player_data
                .as_mut()
                .ok_or_else(|| format!("restored player {} missing player data", player.name))?;
            pdata.player_gui = Some(std::sync::Arc::downgrade(&player_gui.data));
        }
        players_service.add_player(player_instance.clone());

        apply_attributes(&player_instance, &player.attributes);

        let character = match player.character_id {
            Some(character_id) => {
                let character = instance_map.get(&character_id).cloned().ok_or_else(|| {
                    format!(
                        "missing restored character {} for player {}",
                        character_id, player.name
                    )
                })?;
                {
                    let mut player_data = player_instance.data.lock().unwrap();
                    let pdata = player_data.player_data.as_mut().ok_or_else(|| {
                        format!("restored player {} missing player data", player.name)
                    })?;
                    pdata.character = Some(std::sync::Arc::downgrade(&character.data));
                }
                Some(character)
            }
            None => {
                if !player.character_attributes.is_empty()
                    || player.humanoid_root_part.is_some()
                    || player.humanoid.is_some()
                {
                    return Err(format!(
                        "snapshot for player {} is missing character ids; resnapshot with the current engine before resuming",
                        player.name
                    ));
                }
                None
            }
        };

        if let Some(character) = character {
            apply_attributes(&character, &player.character_attributes);

            let mut restored_humanoid = None;
            for child in character.get_children() {
                let name = child.name();
                if name == "Humanoid" {
                    restored_humanoid = Some(child.clone());
                }
            }

            if let Some(expected_hrp_id) = player.humanoid_root_part_id {
                let hrp = instance_map.get(&expected_hrp_id).cloned().ok_or_else(|| {
                    format!(
                        "missing restored HumanoidRootPart {} for player {}",
                        expected_hrp_id, player.name
                    )
                })?;
                if !hrp.is_descendant_of(&character) {
                    return Err(format!(
                        "restored HumanoidRootPart {} is not under character {}",
                        expected_hrp_id, player.name
                    ));
                }
                let hrp_snapshot = player.humanoid_root_part.as_ref().ok_or_else(|| {
                    format!(
                        "missing HumanoidRootPart snapshot data for restored player {}",
                        player.name
                    )
                })?;

                apply_attributes(&hrp, &hrp_snapshot.attributes);
                let hrp_id = hrp.id().0;
                let (radius, height, hrp_id) = {
                    let mut data = hrp.data.lock().unwrap();
                    let part = data.part_data.as_mut().ok_or_else(|| {
                        format!("restored HumanoidRootPart missing part data for {}", player.name)
                    })?;
                    let radius = (part.size.x.max(part.size.z)) * 0.5;
                    let height = part.size.y;
                    part.position = restore_vector3(hrp_snapshot.position);
                    part.cframe = restore_cframe(&hrp_snapshot.cframe);
                    part.velocity = restore_vector3(hrp_snapshot.velocity);
                    part.angular_velocity = restore_vector3(hrp_snapshot.angular_velocity);
                    part.position_dirty = false;
                    part.velocity_dirty = false;
                    part.angular_velocity_dirty = false;
                    (radius, height, hrp_id)
                };

                instance
                    .physics
                    .add_character(hrp_id, hrp_snapshot.position, radius, height);
                instance.player_hrp_ids.insert(player.agent_id, hrp_id);
                instance
                    .physics
                    .set_character_position(hrp_id, hrp_snapshot.position);
                if let Some(handle) = instance.physics.get_handle(hrp_id) {
                    instance.physics.set_velocity(handle, hrp_snapshot.velocity);
                    instance
                        .physics
                        .set_angular_velocity(handle, hrp_snapshot.angular_velocity);
                }
            }

            if let (Some(humanoid_inst), Some(h_snapshot)) =
                (restored_humanoid, player.humanoid.as_ref())
            {
                let mut data = humanoid_inst.data.lock().unwrap();
                if let Some(humanoid) = data.humanoid_data.as_mut() {
                    humanoid.health = h_snapshot.health;
                    humanoid.max_health = h_snapshot.max_health;
                    humanoid.walk_speed = h_snapshot.walk_speed;
                    humanoid.jump_power = h_snapshot.jump_power;
                    humanoid.jump_height = h_snapshot.jump_height;
                    humanoid.auto_rotate = h_snapshot.auto_rotate;
                    humanoid.move_direction = restore_vector3(h_snapshot.move_direction);
                    humanoid.move_input_direction =
                        restore_vector3(h_snapshot.move_input_direction);
                    humanoid.running_speed = h_snapshot.running_speed;
                    humanoid.state = restore_humanoid_state(&h_snapshot.state)?;
                    humanoid.move_to_target =
                        h_snapshot.move_to_target.map(restore_vector3);
                    humanoid.cancel_move_to = h_snapshot.cancel_move_to;
                }
            }
        }
    }

    instance.empty_since = if instance.players.is_empty() {
        Some(Instant::now())
    } else {
        None
    };

    Ok(())
}

fn restore_root_children(
    children: &[InstanceSnapshot],
    root: &Instance,
    workspace: Option<&crate::game::lua::services::WorkspaceService>,
    map: &mut HashMap<u64, Instance>,
    fixups: &mut Vec<RefFixup>,
) -> Result<(), String> {
    for child in children {
        let instance = restore_instance_node(child, map, fixups)?;
        if let Some(workspace) = workspace {
            workspace.add_child(instance);
        } else {
            instance.set_parent(Some(root));
        }
    }
    Ok(())
}

fn restore_instance_node(
    snapshot: &InstanceSnapshot,
    map: &mut HashMap<u64, Instance>,
    fixups: &mut Vec<RefFixup>,
) -> Result<Instance, String> {
    let instance = create_instance_from_class_name(&snapshot.class_name, &snapshot.name)?;
    map.insert(snapshot.id, instance.clone());
    apply_instance_snapshot(&instance, snapshot, fixups)?;
    for child in &snapshot.children {
        let child_instance = restore_instance_node(child, map, fixups)?;
        child_instance.set_parent(Some(&instance));
    }
    Ok(instance)
}

fn apply_instance_snapshot(
    instance: &Instance,
    snapshot: &InstanceSnapshot,
    fixups: &mut Vec<RefFixup>,
) -> Result<(), String> {
    {
        let mut data = instance.data.lock().unwrap();
        data.name = snapshot.name.clone();
        data.tags = snapshot.tags.iter().cloned().collect();
        data.attributes = snapshot
            .attributes
            .iter()
            .map(|(key, value)| (key.clone(), restore_attribute_value(value)))
            .collect();

        if let (Some(part), Some(part_snapshot)) =
            (data.part_data.as_mut(), snapshot.part.as_ref())
        {
            apply_part_snapshot(part, part_snapshot)?;
        }
        if let (Some(model), Some(model_snapshot)) =
            (data.model_data.as_mut(), snapshot.model.as_ref())
        {
            model.primary_part = None;
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::ModelPrimaryPart(model_snapshot.primary_part_id),
            });
        }
        if let (Some(humanoid), Some(h_snapshot)) =
            (data.humanoid_data.as_mut(), snapshot.humanoid.as_ref())
        {
            apply_humanoid_snapshot(humanoid, h_snapshot)?;
        }
        if let (Some(script), Some(script_snapshot)) =
            (data.script_data.as_mut(), snapshot.script.as_ref())
        {
            script.source = script_snapshot.source.clone();
            script.disabled = script_snapshot.disabled;
        }
        if let (Some(value), Some(saved)) = (data.int_value_data.as_mut(), snapshot.int_value) {
            value.value = saved;
        }
        if let (Some(value), Some(saved)) = (data.number_value_data.as_mut(), snapshot.number_value) {
            value.value = saved;
        }
        if let (Some(value), Some(saved)) = (data.bool_value_data.as_mut(), snapshot.bool_value) {
            value.value = saved;
        }
        if let (Some(value), Some(saved)) =
            (data.string_value_data.as_mut(), snapshot.string_value.as_ref())
        {
            value.value = saved.clone();
        }
        if data.object_value_data.is_some() {
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::ObjectValue(
                    snapshot.object_value.as_ref().and_then(|value| value.target_id),
                ),
            });
        }
        if let (Some(value), Some(saved)) = (data.cframe_value_data.as_mut(), snapshot.cframe_value.as_ref()) {
            value.value = restore_cframe(saved);
        }
        if let (Some(body_velocity), Some(saved)) =
            (data.body_velocity_data.as_mut(), snapshot.body_velocity.as_ref())
        {
            body_velocity.velocity = restore_vector3(saved.velocity);
            body_velocity.max_force = restore_vector3(saved.max_force);
        }
        if let (Some(body_force), Some(saved)) =
            (data.body_force_data.as_mut(), snapshot.body_force.as_ref())
        {
            body_force.force = restore_vector3(saved.force);
        }
        if let (Some(body_angular_velocity), Some(saved)) = (
            data.body_angular_velocity_data.as_mut(),
            snapshot.body_angular_velocity.as_ref(),
        ) {
            body_angular_velocity.angular_velocity = restore_vector3(saved.angular_velocity);
            body_angular_velocity.max_torque = restore_vector3(saved.max_torque);
        }
        if let (Some(weld), Some(saved)) = (data.weld_data.as_mut(), snapshot.weld.as_ref()) {
            weld.c0 = restore_cframe(&saved.c0);
            weld.c1 = restore_cframe(&saved.c1);
            weld.enabled = saved.enabled;
            weld.part0 = None;
            weld.part1 = None;
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::WeldPart0(saved.part0_id),
            });
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::WeldPart1(saved.part1_id),
            });
        }
        if let (Some(wc), Some(saved)) = (data.weld_constraint_data.as_mut(), snapshot.weld_constraint.as_ref()) {
            wc.enabled = saved.enabled;
            wc.relative_cframe = saved
                .relative_cframe
                .as_ref()
                .map(restore_cframe)
                .or_else(|| {
                    saved
                        .offset
                        .map(|o| CFrame::from_position(crate::game::lua::types::Vector3::new(o[0], o[1], o[2])))
                });
            wc.part0 = None;
            wc.part1 = None;
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::WeldConstraintPart0(saved.part0_id),
            });
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::WeldConstraintPart1(saved.part1_id),
            });
        }
        if let (Some(billboard), Some(saved)) =
            (data.billboard_gui_data.as_mut(), snapshot.billboard_gui.as_ref())
        {
            billboard.size = restore_udim2(&saved.size);
            billboard.studs_offset = restore_vector3(saved.studs_offset);
            billboard.always_on_top = saved.always_on_top;
            billboard.enabled = saved.enabled;
            billboard.adornee = None;
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::BillboardAdornee(saved.adornee_id),
            });
        }
        if let (Some(ui_corner), Some(saved)) =
            (data.ui_corner_data.as_mut(), snapshot.ui_corner.as_ref())
        {
            ui_corner.corner_radius = restore_udim(&saved.corner_radius);
        }
        if let (Some(gui), Some(saved)) = (data.gui_data.as_mut(), snapshot.gui.as_ref()) {
            apply_gui_snapshot(gui, saved)?;
        }
    }
    Ok(())
}

fn apply_ref_fixups(map: &HashMap<u64, Instance>, fixups: Vec<RefFixup>) {
    for fixup in fixups {
        let target = match &fixup.kind {
            RefFixupKind::ObjectValue(id)
            | RefFixupKind::ModelPrimaryPart(id)
            | RefFixupKind::WeldPart0(id)
            | RefFixupKind::WeldPart1(id)
            | RefFixupKind::WeldConstraintPart0(id)
            | RefFixupKind::WeldConstraintPart1(id)
            | RefFixupKind::BillboardAdornee(id) => id.and_then(|id| map.get(&id).cloned()),
        };

        let mut data = fixup.instance.data.lock().unwrap();
        match fixup.kind {
            RefFixupKind::ObjectValue(_) => {
                if let Some(object_value) = data.object_value_data.as_mut() {
                    object_value.value = target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::ModelPrimaryPart(_) => {
                if let Some(model) = data.model_data.as_mut() {
                    model.primary_part = target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::WeldPart0(_) => {
                if let Some(weld) = data.weld_data.as_mut() {
                    weld.part0 = target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::WeldPart1(_) => {
                if let Some(weld) = data.weld_data.as_mut() {
                    weld.part1 = target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::WeldConstraintPart0(_) => {
                if let Some(wc) = data.weld_constraint_data.as_mut() {
                    wc.part0 = target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::WeldConstraintPart1(_) => {
                if let Some(wc) = data.weld_constraint_data.as_mut() {
                    wc.part1 = target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::BillboardAdornee(_) => {
                if let Some(billboard) = data.billboard_gui_data.as_mut() {
                    billboard.adornee = target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
        }
    }
}

fn clear_workspace(workspace: &crate::game::lua::services::WorkspaceService) {
    for child in workspace.get_children() {
        child.set_parent(None);
    }
    let mut data = workspace.data.lock().unwrap();
    data.children.clear();
    data.current_camera = None;
}

fn clear_instance_children(root: &Instance) {
    for child in root.get_children() {
        child.set_parent(None);
    }
}

fn create_instance_from_class_name(class_name: &str, name: &str) -> Result<Instance, String> {
    let instance = match class_name {
        "Part" => Instance::from_data(crate::game::lua::instance::InstanceData::new_part(name)),
        "Model" => Instance::from_data(crate::game::lua::instance::InstanceData::new_model(name)),
        "Humanoid" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_humanoid(name))
        }
        "Folder" => Instance::new(ClassName::Folder, name),
        "Script" => Instance::from_data(crate::game::lua::instance::InstanceData::new_script(name)),
        "ModuleScript" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_module_script(name))
        }
        "RemoteEvent" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_remote_event(name))
        }
        "IntValue" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_int_value(name))
        }
        "NumberValue" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_number_value(name))
        }
        "BoolValue" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_bool_value(name))
        }
        "StringValue" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_string_value(name))
        }
        "ObjectValue" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_object_value(name))
        }
        "CFrameValue" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_cframe_value(name))
        }
        "BodyVelocity" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_body_velocity(name),
        ),
        "BodyForce" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_body_force(name))
        }
        "BodyAngularVelocity" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_body_angular_velocity(name),
        ),
        "Attachment" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_attachment(name))
        }
        "AlignOrientation" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_align_orientation(name),
        ),
        "Weld" => Instance::from_data(crate::game::lua::instance::InstanceData::new_weld(name)),
        "WeldConstraint" => Instance::from_data(crate::game::lua::instance::InstanceData::new_weld_constraint(name)),
        "BillboardGui" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_billboard_gui(name),
        ),
        "PlayerGui" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_player_gui(name),
        ),
        "ScreenGui" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_screen_gui(name),
        ),
        "Frame" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_frame(name))
        }
        "TextLabel" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_text_label(name),
        ),
        "TextButton" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_text_button(name),
        ),
        "ImageLabel" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_image_label(name),
        ),
        "ImageButton" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_image_button(name),
        ),
        "UICorner" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_uicorner(name),
        ),
        "Sound" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_sound(name))
        }
        other => return Err(format!("unsupported snapshot class '{}'", other)),
    };
    {
        let mut data = instance.data.lock().unwrap();
        data.name = name.to_string();
    }
    Ok(instance)
}

fn apply_part_snapshot(part: &mut PartData, snapshot: &PartSnapshot) -> Result<(), String> {
    part.position = restore_vector3(snapshot.position);
    part.cframe = restore_cframe(&snapshot.cframe);
    part.size = restore_vector3(snapshot.size);
    part.anchored = snapshot.anchored;
    part.can_collide = snapshot.can_collide;
    part.can_query = snapshot.can_query;
    part.collision_group = snapshot.collision_group.clone();
    part.can_touch = snapshot.can_touch;
    part.transparency = snapshot.transparency;
    part.color = restore_color3(snapshot.color);
    part.material = restore_material(&snapshot.material)?;
    part.top_surface = snapshot.top_surface.clone();
    part.bottom_surface = snapshot.bottom_surface.clone();
    part.velocity = restore_vector3(snapshot.velocity);
    part.angular_velocity = restore_vector3(snapshot.angular_velocity);
    part.massless = snapshot.massless;
    part.custom_physical_properties = snapshot
        .custom_physical_properties
        .as_ref()
        .map(restore_physical_properties);
    part.shape = restore_part_type(&snapshot.shape)?;
    part.position_dirty = false;
    part.size_dirty = false;
    part.anchored_dirty = false;
    part.can_collide_dirty = false;
    part.velocity_dirty = false;
    part.angular_velocity_dirty = false;
    part.shape_dirty = false;
    part.custom_physical_properties_dirty = false;
    Ok(())
}

fn apply_humanoid_snapshot(
    humanoid: &mut HumanoidData,
    snapshot: &HumanoidSnapshot,
) -> Result<(), String> {
    humanoid.health = snapshot.health;
    humanoid.max_health = snapshot.max_health;
    humanoid.walk_speed = snapshot.walk_speed;
    humanoid.jump_power = snapshot.jump_power;
    humanoid.jump_height = snapshot.jump_height;
    humanoid.auto_rotate = snapshot.auto_rotate;
    humanoid.health_display_distance = snapshot.health_display_distance;
    humanoid.name_display_distance = snapshot.name_display_distance;
    humanoid.hip_height = snapshot.hip_height;
    humanoid.move_direction = restore_vector3(snapshot.move_direction);
    humanoid.move_input_direction = restore_vector3(snapshot.move_input_direction);
    humanoid.running_speed = snapshot.running_speed;
    humanoid.state = restore_humanoid_state(&snapshot.state)?;
    humanoid.jump_requested = snapshot.jump_requested;
    humanoid.move_to_target = snapshot.move_to_target.map(restore_vector3);
    humanoid.cancel_move_to = snapshot.cancel_move_to;
    Ok(())
}

fn apply_gui_snapshot(gui: &mut GuiObjectData, snapshot: &GuiSnapshot) -> Result<(), String> {
    gui.position = restore_udim2(&snapshot.position);
    gui.size = restore_udim2(&snapshot.size);
    gui.anchor_point = (snapshot.anchor_point[0], snapshot.anchor_point[1]);
    gui.rotation = snapshot.rotation;
    gui.z_index = snapshot.z_index;
    gui.layout_order = snapshot.layout_order;
    gui.visible = snapshot.visible;
    gui.background_color = restore_color3(snapshot.background_color);
    gui.background_transparency = snapshot.background_transparency;
    gui.border_color = restore_color3(snapshot.border_color);
    gui.border_size_pixel = snapshot.border_size_pixel;
    gui.text = snapshot.text.clone();
    gui.text_color = snapshot.text_color.map(restore_color3);
    gui.text_size = snapshot.text_size;
    gui.text_transparency = snapshot.text_transparency;
    gui.text_stroke_transparency = snapshot.text_stroke_transparency;
    gui.font = snapshot.font.clone();
    gui.text_scaled = snapshot.text_scaled;
    gui.text_x_alignment = restore_text_x_alignment(&snapshot.text_x_alignment)?;
    gui.text_y_alignment = restore_text_y_alignment(&snapshot.text_y_alignment)?;
    gui.image = snapshot.image.clone();
    gui.image_color = snapshot.image_color.map(restore_color3);
    gui.image_transparency = snapshot.image_transparency;
    gui.display_order = snapshot.display_order;
    gui.ignore_gui_inset = snapshot.ignore_gui_inset;
    gui.enabled = snapshot.enabled;
    gui.reset_on_spawn = snapshot.reset_on_spawn;
    Ok(())
}

fn apply_attributes(instance: &Instance, attributes: &HashMap<String, SnapshotAttributeValue>) {
    let mut data = instance.data.lock().unwrap();
    data.attributes = attributes
        .iter()
        .map(|(key, value)| (key.clone(), restore_attribute_value(value)))
        .collect();
}

fn snapshot_part_data(part: &PartData) -> PartSnapshot {
    PartSnapshot {
        position: snapshot_vector3(part.position),
        cframe: snapshot_cframe(part.cframe),
        size: snapshot_vector3(part.size),
        anchored: part.anchored,
        can_collide: part.can_collide,
        can_query: part.can_query,
        collision_group: part.collision_group.clone(),
        can_touch: part.can_touch,
        transparency: part.transparency,
        color: snapshot_color3(part.color),
        material: snapshot_material(part.material),
        top_surface: part.top_surface.clone(),
        bottom_surface: part.bottom_surface.clone(),
        velocity: snapshot_vector3(part.velocity),
        angular_velocity: snapshot_vector3(part.angular_velocity),
        massless: part.massless,
        custom_physical_properties: part
            .custom_physical_properties
            .map(snapshot_physical_properties),
        shape: snapshot_part_type(part.shape),
    }
}

fn snapshot_model_data(model: &ModelData) -> ModelSnapshot {
    ModelSnapshot {
        primary_part_id: model
            .primary_part
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
    }
}

fn snapshot_humanoid_data(humanoid: &HumanoidData) -> HumanoidSnapshot {
    HumanoidSnapshot {
        health: humanoid.health,
        max_health: humanoid.max_health,
        walk_speed: humanoid.walk_speed,
        jump_power: humanoid.jump_power,
        jump_height: humanoid.jump_height,
        auto_rotate: humanoid.auto_rotate,
        health_display_distance: humanoid.health_display_distance,
        name_display_distance: humanoid.name_display_distance,
        hip_height: humanoid.hip_height,
        move_direction: snapshot_vector3(humanoid.move_direction),
        move_input_direction: snapshot_vector3(humanoid.move_input_direction),
        running_speed: humanoid.running_speed,
        state: snapshot_humanoid_state(humanoid.state),
        jump_requested: humanoid.jump_requested,
        move_to_target: humanoid.move_to_target.map(snapshot_vector3),
        cancel_move_to: humanoid.cancel_move_to,
    }
}

fn snapshot_script_data(script: &ScriptData) -> ScriptSnapshot {
    ScriptSnapshot {
        source: script.source.clone(),
        disabled: script.disabled,
    }
}

fn snapshot_object_value_data(value: &ObjectValueData) -> ObjectRefSnapshot {
    ObjectRefSnapshot {
        target_id: value
            .value
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
    }
}

fn snapshot_cframe_value_data(value: &CFrameValueData) -> SnapshotCFrame {
    snapshot_cframe(value.value)
}

fn snapshot_body_velocity_data(value: &BodyVelocityData) -> BodyVelocitySnapshot {
    BodyVelocitySnapshot {
        velocity: snapshot_vector3(value.velocity),
        max_force: snapshot_vector3(value.max_force),
    }
}

fn snapshot_body_force_data(value: &BodyForceData) -> BodyForceSnapshot {
    BodyForceSnapshot {
        force: snapshot_vector3(value.force),
    }
}

fn snapshot_body_angular_velocity_data(
    value: &BodyAngularVelocityData,
) -> BodyAngularVelocitySnapshot {
    BodyAngularVelocitySnapshot {
        angular_velocity: snapshot_vector3(value.angular_velocity),
        max_torque: snapshot_vector3(value.max_torque),
    }
}

fn snapshot_weld_data(weld: &WeldData) -> WeldSnapshot {
    WeldSnapshot {
        part0_id: weld
            .part0
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        part1_id: weld
            .part1
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        c0: snapshot_cframe(weld.c0),
        c1: snapshot_cframe(weld.c1),
        enabled: weld.enabled,
    }
}

fn snapshot_weld_constraint_data(wc: &WeldConstraintData) -> WeldConstraintSnapshot {
    WeldConstraintSnapshot {
        part0_id: wc
            .part0
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        part1_id: wc
            .part1
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        enabled: wc.enabled,
        offset: wc.relative_cframe.map(|cf| [cf.position.x, cf.position.y, cf.position.z]),
        relative_cframe: wc.relative_cframe.map(snapshot_cframe),
    }
}

fn snapshot_billboard_gui_data(data: &BillboardGuiData) -> BillboardGuiSnapshot {
    BillboardGuiSnapshot {
        size: snapshot_udim2(data.size),
        studs_offset: snapshot_vector3(data.studs_offset),
        always_on_top: data.always_on_top,
        enabled: data.enabled,
        adornee_id: data
            .adornee
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
    }
}

fn snapshot_ui_corner_data(data: &UICornerData) -> UICornerSnapshot {
    UICornerSnapshot {
        corner_radius: snapshot_udim(data.corner_radius),
    }
}

fn snapshot_gui_data(data: &GuiObjectData) -> GuiSnapshot {
    GuiSnapshot {
        position: snapshot_udim2(data.position),
        size: snapshot_udim2(data.size),
        anchor_point: [data.anchor_point.0, data.anchor_point.1],
        rotation: data.rotation,
        z_index: data.z_index,
        layout_order: data.layout_order,
        visible: data.visible,
        background_color: snapshot_color3(data.background_color),
        background_transparency: data.background_transparency,
        border_color: snapshot_color3(data.border_color),
        border_size_pixel: data.border_size_pixel,
        text: data.text.clone(),
        text_color: data.text_color.map(snapshot_color3),
        text_size: data.text_size,
        text_transparency: data.text_transparency,
        text_stroke_transparency: data.text_stroke_transparency,
        font: data.font.clone(),
        text_scaled: data.text_scaled,
        text_x_alignment: snapshot_text_x_alignment(data.text_x_alignment),
        text_y_alignment: snapshot_text_y_alignment(data.text_y_alignment),
        image: data.image.clone(),
        image_color: data.image_color.map(snapshot_color3),
        image_transparency: data.image_transparency,
        display_order: data.display_order,
        ignore_gui_inset: data.ignore_gui_inset,
        enabled: data.enabled,
        reset_on_spawn: data.reset_on_spawn,
    }
}

fn snapshot_attribute_value(value: &AttributeValue) -> SnapshotAttributeValue {
    match value {
        AttributeValue::String(v) => SnapshotAttributeValue::String(v.clone()),
        AttributeValue::Number(v) => SnapshotAttributeValue::Number(*v),
        AttributeValue::Bool(v) => SnapshotAttributeValue::Bool(*v),
        AttributeValue::Vector3(v) => SnapshotAttributeValue::Vector3(snapshot_vector3(*v)),
        AttributeValue::Color3(v) => SnapshotAttributeValue::Color3(snapshot_color3(*v)),
        AttributeValue::Table(items) => {
            SnapshotAttributeValue::Table(items.iter().map(snapshot_attribute_value).collect())
        }
        AttributeValue::Nil => SnapshotAttributeValue::Nil,
    }
}

fn restore_attribute_value(value: &SnapshotAttributeValue) -> AttributeValue {
    match value {
        SnapshotAttributeValue::String(v) => AttributeValue::String(v.clone()),
        SnapshotAttributeValue::Number(v) => AttributeValue::Number(*v),
        SnapshotAttributeValue::Bool(v) => AttributeValue::Bool(*v),
        SnapshotAttributeValue::Vector3(v) => AttributeValue::Vector3(restore_vector3(*v)),
        SnapshotAttributeValue::Color3(v) => AttributeValue::Color3(restore_color3(*v)),
        SnapshotAttributeValue::Table(items) => {
            AttributeValue::Table(items.iter().map(restore_attribute_value).collect())
        }
        SnapshotAttributeValue::Nil => AttributeValue::Nil,
    }
}

fn snapshot_vector3(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

fn restore_vector3(v: [f32; 3]) -> Vector3 {
    Vector3::new(v[0], v[1], v[2])
}

fn snapshot_color3(v: Color3) -> [f32; 3] {
    [v.r, v.g, v.b]
}

fn restore_color3(v: [f32; 3]) -> Color3 {
    Color3::new(v[0], v[1], v[2])
}

fn snapshot_cframe(v: CFrame) -> SnapshotCFrame {
    SnapshotCFrame {
        position: snapshot_vector3(v.position),
        rotation: v.rotation,
    }
}

fn restore_cframe(v: &SnapshotCFrame) -> CFrame {
    CFrame {
        position: restore_vector3(v.position),
        rotation: v.rotation,
    }
}

fn snapshot_udim(v: UDim) -> SnapshotUDim {
    SnapshotUDim {
        scale: v.scale,
        offset: v.offset,
    }
}

fn restore_udim(v: &SnapshotUDim) -> UDim {
    UDim::new(v.scale, v.offset)
}

fn snapshot_udim2(v: UDim2) -> SnapshotUDim2 {
    SnapshotUDim2 {
        x: snapshot_udim(v.x),
        y: snapshot_udim(v.y),
    }
}

fn restore_udim2(v: &SnapshotUDim2) -> UDim2 {
    UDim2::from_udims(restore_udim(&v.x), restore_udim(&v.y))
}

fn snapshot_physical_properties(v: PhysicalProperties) -> PhysicalPropertiesSnapshot {
    PhysicalPropertiesSnapshot {
        density: v.density,
        friction: v.friction,
        elasticity: v.elasticity,
        friction_weight: v.friction_weight,
        elasticity_weight: v.elasticity_weight,
    }
}

fn restore_physical_properties(v: &PhysicalPropertiesSnapshot) -> PhysicalProperties {
    PhysicalProperties::new(
        v.density,
        v.friction,
        v.elasticity,
        v.friction_weight,
        v.elasticity_weight,
    )
}

fn snapshot_part_type(v: PartType) -> String {
    v.name().to_string()
}

fn restore_part_type(v: &str) -> Result<PartType, String> {
    match v {
        "Ball" => Ok(PartType::Ball),
        "Block" => Ok(PartType::Block),
        "Cylinder" => Ok(PartType::Cylinder),
        "Wedge" => Ok(PartType::Wedge),
        other => Err(format!("unsupported PartType '{}'", other)),
    }
}

fn snapshot_material(v: Material) -> String {
    v.name().to_string()
}

fn restore_material(v: &str) -> Result<Material, String> {
    match v {
        "Plastic" => Ok(Material::Plastic),
        "Wood" => Ok(Material::Wood),
        "Metal" => Ok(Material::Metal),
        "Glass" => Ok(Material::Glass),
        "Neon" => Ok(Material::Neon),
        "Concrete" => Ok(Material::Concrete),
        "Brick" => Ok(Material::Brick),
        "Granite" => Ok(Material::Granite),
        "Grass" => Ok(Material::Grass),
        "Ice" => Ok(Material::Ice),
        "Sand" => Ok(Material::Sand),
        "Fabric" => Ok(Material::Fabric),
        "Marble" => Ok(Material::Marble),
        "Slate" => Ok(Material::Slate),
        "SmoothPlastic" => Ok(Material::SmoothPlastic),
        "ForceField" => Ok(Material::ForceField),
        other => Err(format!("unsupported Material '{}'", other)),
    }
}

fn snapshot_humanoid_state(v: HumanoidStateType) -> String {
    v.name().to_string()
}

fn restore_humanoid_state(v: &str) -> Result<HumanoidStateType, String> {
    match v {
        "Running" => Ok(HumanoidStateType::Running),
        "Landed" => Ok(HumanoidStateType::Landed),
        "Jumping" => Ok(HumanoidStateType::Jumping),
        "Freefall" => Ok(HumanoidStateType::Freefall),
        "Dead" => Ok(HumanoidStateType::Dead),
        "Physics" => Ok(HumanoidStateType::Physics),
        "None" => Ok(HumanoidStateType::None),
        other => Err(format!("unsupported HumanoidStateType '{}'", other)),
    }
}

fn snapshot_text_x_alignment(v: TextXAlignment) -> String {
    match v {
        TextXAlignment::Left => "Left",
        TextXAlignment::Center => "Center",
        TextXAlignment::Right => "Right",
    }
    .to_string()
}

fn restore_text_x_alignment(v: &str) -> Result<TextXAlignment, String> {
    match v {
        "Left" => Ok(TextXAlignment::Left),
        "Center" => Ok(TextXAlignment::Center),
        "Right" => Ok(TextXAlignment::Right),
        other => Err(format!("unsupported TextXAlignment '{}'", other)),
    }
}

fn snapshot_text_y_alignment(v: TextYAlignment) -> String {
    match v {
        TextYAlignment::Top => "Top",
        TextYAlignment::Center => "Center",
        TextYAlignment::Bottom => "Bottom",
    }
    .to_string()
}

fn restore_text_y_alignment(v: &str) -> Result<TextYAlignment, String> {
    match v {
        "Top" => Ok(TextYAlignment::Top),
        "Center" => Ok(TextYAlignment::Center),
        "Bottom" => Ok(TextYAlignment::Bottom),
        other => Err(format!("unsupported TextYAlignment '{}'", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::instance::GameInstance;
    use crate::game::lua::instance::InstanceData;

    #[test]
    fn roundtrip_restores_workspace_server_storage_and_players() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.init_runtime();

        let workspace = instance.lua_runtime.as_ref().unwrap().workspace();
        let server_storage = instance.lua_runtime.as_ref().unwrap().game().server_storage();

        let part = Instance::from_data(InstanceData::new_part("TrackedPart"));
        {
            let mut data = part.data.lock().unwrap();
            let part_data = data.part_data.as_mut().unwrap();
            part_data.position = Vector3::new(12.0, 3.0, -4.0);
            part_data.cframe = CFrame::new(12.0, 3.0, -4.0);
            part_data.color = Color3::from_rgb(10, 20, 30);
            data.attributes.insert(
                "Marker".to_string(),
                AttributeValue::String("alpha".to_string()),
            );
        }
        workspace.add_child(part.clone());

        let folder = Instance::new(ClassName::Folder, "HiddenState");
        folder.set_parent(Some(&server_storage));
        let object_value = Instance::from_data(InstanceData::new_object_value("TrackedRef"));
        {
            let mut data = object_value.data.lock().unwrap();
            data.object_value_data.as_mut().unwrap().value = Some(std::sync::Arc::downgrade(&part.data));
        }
        object_value.set_parent(Some(&folder));
        let cframe_value = Instance::from_data(InstanceData::new_cframe_value("TrackedCFrame"));
        let expected_cframe = CFrame::new(1.0, 2.0, 3.0)
            .multiply(&CFrame::from_orientation(0.2, 0.4, 0.1));
        {
            let mut data = cframe_value.data.lock().unwrap();
            data.cframe_value_data.as_mut().unwrap().value = expected_cframe;
        }
        cframe_value.set_parent(Some(&folder));
        folder.set_attribute("InternalCounter", AttributeValue::Number(42.0));

        let agent_id = Uuid::new_v4();
        assert!(instance.add_player(agent_id, "Tester"));
        let user_id = *instance.players.get(&agent_id).unwrap();
        let player = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .players()
            .get_player_by_user_id(user_id)
            .unwrap();
        player.set_attribute("PlayerMemo", AttributeValue::String("keep".to_string()));
        let character = player
            .data
            .lock()
            .unwrap()
            .player_data
            .as_ref()
            .and_then(|pdata| pdata.character.as_ref())
            .and_then(|weak| weak.upgrade())
            .map(Instance::from_ref)
            .unwrap();
        let hrp = character.find_first_child("HumanoidRootPart", false).unwrap();
        {
            let mut data = hrp.data.lock().unwrap();
            let part_data = data.part_data.as_mut().unwrap();
            part_data.position = Vector3::new(5.0, 6.5, 7.5);
            part_data.cframe = CFrame::new(5.0, 6.5, 7.5);
            part_data.velocity = Vector3::new(1.0, 2.0, 3.0);
        }

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let restored_runtime = restored.lua_runtime.as_ref().unwrap();
        let restored_part = restored_runtime
            .workspace()
            .instance
            .find_first_child("TrackedPart", true)
            .unwrap();
        assert!(matches!(
            restored_part.get_attribute("Marker"),
            Some(AttributeValue::String(value)) if value == "alpha"
        ));
        let restored_part_data = restored_part.data.lock().unwrap();
        let restored_part_pos = restored_part_data.part_data.as_ref().unwrap().position;
        assert_eq!(snapshot_vector3(restored_part_pos), [12.0, 3.0, -4.0]);
        drop(restored_part_data);

        let restored_folder = restored_runtime
            .game()
            .server_storage()
            .find_first_child("HiddenState", true)
            .unwrap();
        assert!(matches!(
            restored_folder.get_attribute("InternalCounter"),
            Some(AttributeValue::Number(value)) if (value - 42.0).abs() < f64::EPSILON
        ));
        let restored_ref = restored_folder.find_first_child("TrackedRef", false).unwrap();
        let target_name = restored_ref
            .data
            .lock()
            .unwrap()
            .object_value_data
            .as_ref()
            .and_then(|value| value.value.as_ref())
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().name.clone());
        assert_eq!(target_name.as_deref(), Some("TrackedPart"));
        let restored_cframe = restored_folder.find_first_child("TrackedCFrame", false).unwrap();
        let restored_value = restored_cframe
            .data
            .lock()
            .unwrap()
            .cframe_value_data
            .as_ref()
            .map(|value| value.value)
            .unwrap();
        assert_eq!(snapshot_cframe(restored_value).position, [1.0, 2.0, 3.0]);
        assert_eq!(
            snapshot_cframe(restored_value).rotation,
            snapshot_cframe(expected_cframe).rotation
        );

        let restored_user_id = *restored.players.get(&agent_id).unwrap();
        let restored_player = restored_runtime
            .players()
            .get_player_by_user_id(restored_user_id)
            .unwrap();
        assert!(matches!(
            restored_player.get_attribute("PlayerMemo"),
            Some(AttributeValue::String(value)) if value == "keep"
        ));
        let restored_character = restored_player
            .data
            .lock()
            .unwrap()
            .player_data
            .as_ref()
            .and_then(|pdata| pdata.character.as_ref())
            .and_then(|weak| weak.upgrade())
            .map(Instance::from_ref)
            .unwrap();
        let restored_hrp = restored_character
            .find_first_child("HumanoidRootPart", false)
            .unwrap();
        let restored_hrp_data = restored_hrp.data.lock().unwrap();
        let restored_hrp_part = restored_hrp_data.part_data.as_ref().unwrap();
        assert_eq!(snapshot_vector3(restored_hrp_part.position), [5.0, 6.5, 7.5]);
        assert_eq!(snapshot_vector3(restored_hrp_part.velocity), [1.0, 2.0, 3.0]);
        drop(restored_hrp_data);

        let restored_character_models = restored_runtime
            .workspace()
            .get_children()
            .into_iter()
            .filter(|child| child.is_a("Model") && child.name() == "Tester")
            .count();
        assert_eq!(restored_character_models, 1);
    }
}
