use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use crate::game::lua::events::{create_signal, RBXScriptSignal};
use crate::game::lua::types::{Color3, Vector3};

use super::class_data::*;
use super::class_name::ClassName;

static INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

pub type InstanceRef = Arc<Mutex<InstanceData>>;
pub type WeakInstanceRef = Weak<Mutex<InstanceData>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceId(pub u64);

impl InstanceId {
    pub fn new() -> Self {
        Self(INSTANCE_ID.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for InstanceId {
    fn default() -> Self {
        Self::new()
    }
}

pub struct InstanceData {
    pub id: InstanceId,
    pub name: String,
    pub class_name: ClassName,
    pub parent: Option<WeakInstanceRef>,
    pub children: Vec<InstanceRef>,
    pub attributes: HashMap<String, AttributeValue>,
    pub tags: HashSet<String>,

    pub child_added: RBXScriptSignal,
    pub child_removed: RBXScriptSignal,
    pub descendant_added: RBXScriptSignal,
    pub descendant_removing: RBXScriptSignal,
    pub ancestry_changed: RBXScriptSignal,
    pub destroying: RBXScriptSignal,
    pub attribute_changed: RBXScriptSignal,

    pub part_data: Option<PartData>,
    pub humanoid_data: Option<HumanoidData>,
    pub player_data: Option<PlayerData>,
    pub animation_data: Option<AnimationData>,
    pub animator_data: Option<AnimatorData>,
    pub model_data: Option<ModelData>,
    pub terrain_data: Option<TerrainData>,
    pub gui_data: Option<GuiObjectData>,
    pub weld_data: Option<WeldData>,
    pub weld_constraint_data: Option<WeldConstraintData>,
    pub billboard_gui_data: Option<BillboardGuiData>,
    pub ui_corner_data: Option<UICornerData>,
    pub script_data: Option<ScriptData>,
    pub int_value_data: Option<IntValueData>,
    pub number_value_data: Option<NumberValueData>,
    pub bool_value_data: Option<BoolValueData>,
    pub string_value_data: Option<StringValueData>,
    pub object_value_data: Option<ObjectValueData>,
    pub cframe_value_data: Option<CFrameValueData>,
    pub remote_event_data: Option<RemoteEventData>,
    pub body_velocity_data: Option<BodyVelocityData>,
    pub body_force_data: Option<BodyForceData>,
    pub body_angular_velocity_data: Option<BodyAngularVelocityData>,
    pub attachment_data: Option<AttachmentData>,
    pub vector_force_data: Option<VectorForceData>,
    pub align_position_data: Option<AlignPositionData>,
    pub align_orientation_data: Option<AlignOrientationData>,
    pub sound_data: Option<SoundData>,
    pub property_changed_signals: HashMap<String, RBXScriptSignal>,
    pub attribute_changed_signals: HashMap<String, RBXScriptSignal>,

    pub(super) destroyed: bool,
}

#[derive(Debug, Clone)]
pub enum AttributeValue {
    String(String),
    Number(f64),
    Bool(bool),
    Vector3(Vector3),
    Color3(Color3),
    Table(Vec<AttributeValue>),
    Nil,
}

impl AttributeValue {
    /// Convert to JSON value for API responses
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            AttributeValue::String(s) => serde_json::Value::String(s.clone()),
            AttributeValue::Number(n) => serde_json::json!(*n),
            AttributeValue::Bool(b) => serde_json::Value::Bool(*b),
            AttributeValue::Vector3(v) => serde_json::json!([v.x, v.y, v.z]),
            AttributeValue::Color3(c) => serde_json::json!([c.r, c.g, c.b]),
            AttributeValue::Table(items) => {
                serde_json::Value::Array(items.iter().map(|v| v.to_json()).collect())
            }
            AttributeValue::Nil => serde_json::Value::Null,
        }
    }
}

/// Convert a HashMap of AttributeValue to JSON-serializable HashMap
pub fn attributes_to_json(
    attrs: &std::collections::HashMap<String, AttributeValue>,
) -> std::collections::HashMap<String, serde_json::Value> {
    attrs
        .iter()
        .map(|(k, v)| (k.clone(), v.to_json()))
        .collect()
}

impl InstanceData {
    pub fn new(class_name: ClassName, name: &str) -> Self {
        Self {
            id: InstanceId::new(),
            name: name.to_string(),
            class_name,
            parent: None,
            children: Vec::new(),
            attributes: HashMap::new(),
            tags: HashSet::new(),
            child_added: create_signal("ChildAdded"),
            child_removed: create_signal("ChildRemoved"),
            descendant_added: create_signal("DescendantAdded"),
            descendant_removing: create_signal("DescendantRemoving"),
            ancestry_changed: create_signal("AncestryChanged"),
            destroying: create_signal("Destroying"),
            attribute_changed: create_signal("AttributeChanged"),
            part_data: None,
            humanoid_data: None,
            player_data: None,
            animation_data: None,
            animator_data: None,
            model_data: None,
            terrain_data: None,
            gui_data: None,
            weld_data: None,
            weld_constraint_data: None,
            billboard_gui_data: None,
            ui_corner_data: None,
            script_data: None,
            int_value_data: None,
            number_value_data: None,
            bool_value_data: None,
            string_value_data: None,
            object_value_data: None,
            cframe_value_data: None,
            remote_event_data: None,
            body_velocity_data: None,
            body_force_data: None,
            body_angular_velocity_data: None,
            attachment_data: None,
            vector_force_data: None,
            align_position_data: None,
            align_orientation_data: None,
            sound_data: None,
            property_changed_signals: HashMap::new(),
            attribute_changed_signals: HashMap::new(),
            destroyed: false,
        }
    }

    pub fn new_weld(name: &str) -> Self {
        let mut inst = Self::new(ClassName::Weld, name);
        inst.weld_data = Some(WeldData::default());
        inst
    }

    pub fn new_weld_constraint(name: &str) -> Self {
        let mut inst = Self::new(ClassName::WeldConstraint, name);
        inst.weld_constraint_data = Some(WeldConstraintData::default());
        inst
    }

    pub fn new_billboard_gui(name: &str) -> Self {
        let mut inst = Self::new(ClassName::BillboardGui, name);
        inst.billboard_gui_data = Some(BillboardGuiData::default());
        inst
    }

    pub fn new_part(name: &str) -> Self {
        let mut inst = Self::new(ClassName::Part, name);
        inst.part_data = Some(PartData::default());
        inst
    }

    pub fn new_model(name: &str) -> Self {
        let mut inst = Self::new(ClassName::Model, name);
        inst.model_data = Some(ModelData::default());
        inst
    }

    pub fn new_humanoid(name: &str) -> Self {
        let mut inst = Self::new(ClassName::Humanoid, name);
        inst.humanoid_data = Some(HumanoidData::default());
        inst
    }

    pub fn new_player(user_id: u64, name: &str) -> Self {
        let mut inst = Self::new(ClassName::Player, name);
        inst.player_data = Some(PlayerData::new(user_id, name));
        inst
    }

    pub fn new_animation(name: &str) -> Self {
        let mut inst = Self::new(ClassName::Animation, name);
        inst.animation_data = Some(AnimationData::default());
        inst
    }

    pub fn new_animator(name: &str) -> Self {
        let mut inst = Self::new(ClassName::Animator, name);
        inst.animator_data = Some(AnimatorData::default());
        inst
    }

    pub fn new_script(name: &str) -> Self {
        let mut inst = Self::new(ClassName::Script, name);
        inst.script_data = Some(ScriptData::default());
        inst
    }

    pub fn new_module_script(name: &str) -> Self {
        let mut inst = Self::new(ClassName::ModuleScript, name);
        inst.script_data = Some(ScriptData::default());
        inst
    }

    pub fn new_server_script_service() -> Self {
        Self::new(ClassName::ServerScriptService, "ServerScriptService")
    }

    pub fn new_server_storage() -> Self {
        Self::new(ClassName::ServerStorage, "ServerStorage")
    }

    pub fn new_replicated_storage() -> Self {
        Self::new(ClassName::ReplicatedStorage, "ReplicatedStorage")
    }

    pub fn new_remote_event(name: &str) -> Self {
        let mut inst = Self::new(ClassName::RemoteEvent, name);
        inst.remote_event_data = Some(RemoteEventData::default());
        inst
    }

    pub fn new_int_value(name: &str) -> Self {
        let mut inst = Self::new(ClassName::IntValue, name);
        inst.int_value_data = Some(IntValueData::default());
        inst
    }

    pub fn new_number_value(name: &str) -> Self {
        let mut inst = Self::new(ClassName::NumberValue, name);
        inst.number_value_data = Some(NumberValueData::default());
        inst
    }

    pub fn new_bool_value(name: &str) -> Self {
        let mut inst = Self::new(ClassName::BoolValue, name);
        inst.bool_value_data = Some(BoolValueData::default());
        inst
    }

    pub fn new_string_value(name: &str) -> Self {
        let mut inst = Self::new(ClassName::StringValue, name);
        inst.string_value_data = Some(StringValueData::default());
        inst
    }

    pub fn new_object_value(name: &str) -> Self {
        let mut inst = Self::new(ClassName::ObjectValue, name);
        inst.object_value_data = Some(ObjectValueData::default());
        inst
    }

    pub fn new_cframe_value(name: &str) -> Self {
        let mut inst = Self::new(ClassName::CFrameValue, name);
        inst.cframe_value_data = Some(CFrameValueData::default());
        inst
    }

    pub fn new_terrain(name: &str) -> Self {
        let mut inst = Self::new(ClassName::Terrain, name);
        inst.terrain_data = Some(TerrainData::default());
        inst
    }

    pub fn new_body_velocity(name: &str) -> Self {
        let mut inst = Self::new(ClassName::BodyVelocity, name);
        inst.body_velocity_data = Some(BodyVelocityData::default());
        inst
    }

    pub fn new_body_force(name: &str) -> Self {
        let mut inst = Self::new(ClassName::BodyForce, name);
        inst.body_force_data = Some(BodyForceData::default());
        inst
    }

    pub fn new_body_angular_velocity(name: &str) -> Self {
        let mut inst = Self::new(ClassName::BodyAngularVelocity, name);
        inst.body_angular_velocity_data = Some(BodyAngularVelocityData::default());
        inst
    }

    pub fn new_attachment(name: &str) -> Self {
        let mut inst = Self::new(ClassName::Attachment, name);
        inst.attachment_data = Some(AttachmentData::default());
        inst
    }

    pub fn new_vector_force(name: &str) -> Self {
        let mut inst = Self::new(ClassName::VectorForce, name);
        inst.vector_force_data = Some(VectorForceData::default());
        inst
    }

    pub fn new_align_position(name: &str) -> Self {
        let mut inst = Self::new(ClassName::AlignPosition, name);
        inst.align_position_data = Some(AlignPositionData::default());
        inst
    }

    pub fn new_align_orientation(name: &str) -> Self {
        let mut inst = Self::new(ClassName::AlignOrientation, name);
        inst.align_orientation_data = Some(AlignOrientationData::default());
        inst
    }

    pub fn new_player_gui(name: &str) -> Self {
        Self::new(ClassName::PlayerGui, name)
    }

    pub fn new_screen_gui(name: &str) -> Self {
        let mut inst = Self::new(ClassName::ScreenGui, name);
        inst.gui_data = Some(GuiObjectData::new_screen_gui());
        inst
    }

    pub fn new_uicorner(name: &str) -> Self {
        let mut inst = Self::new(ClassName::UICorner, name);
        inst.ui_corner_data = Some(UICornerData::default());
        inst
    }

    pub fn new_frame(name: &str) -> Self {
        let mut inst = Self::new(ClassName::Frame, name);
        inst.gui_data = Some(GuiObjectData::new_frame());
        inst
    }

    pub fn new_text_label(name: &str) -> Self {
        let mut inst = Self::new(ClassName::TextLabel, name);
        inst.gui_data = Some(GuiObjectData::new_text_label());
        inst
    }

    pub fn new_text_button(name: &str) -> Self {
        let mut inst = Self::new(ClassName::TextButton, name);
        inst.gui_data = Some(GuiObjectData::new_text_button());
        inst
    }

    pub fn new_image_label(name: &str) -> Self {
        let mut inst = Self::new(ClassName::ImageLabel, name);
        inst.gui_data = Some(GuiObjectData::new_image_label());
        inst
    }

    pub fn new_image_button(name: &str) -> Self {
        let mut inst = Self::new(ClassName::ImageButton, name);
        inst.gui_data = Some(GuiObjectData::new_image_button());
        inst
    }

    pub fn new_sound(name: &str) -> Self {
        let mut inst = Self::new(ClassName::Sound, name);
        inst.sound_data = Some(SoundData::default());
        inst
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    pub(crate) fn set_destroyed_for_restore(&mut self, destroyed: bool) {
        self.destroyed = destroyed;
    }
}
