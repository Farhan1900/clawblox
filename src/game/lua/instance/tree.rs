use std::collections::{HashMap, HashSet};

use mlua::MultiValue;

use crate::game::lua::events::{create_signal, RBXScriptSignal};
use crate::game::lua::types::CFrame;

use super::*;

impl Instance {
    fn top_ancestor(&self) -> Instance {
        let mut current = self.clone();
        while let Some(parent) = current.parent() {
            current = parent;
        }
        current
    }

    pub(super) fn connected_parts_from_graph(&self, recursive: bool) -> Vec<Instance> {
        if self.data.lock().unwrap().part_data.is_none() {
            return Vec::new();
        }

        let root = self.top_ancestor();
        let mut adjacency: HashMap<u64, Vec<Instance>> = HashMap::new();

        let mut scan_instances = root.get_descendants();
        scan_instances.push(root);

        for inst in scan_instances {
            let data = inst.data.lock().unwrap();

            if let Some(weld) = &data.weld_data {
                if !weld.enabled {
                    continue;
                }
                let Some(part0) = weld.part0.as_ref().and_then(|w| w.upgrade()) else {
                    continue;
                };
                let Some(part1) = weld.part1.as_ref().and_then(|w| w.upgrade()) else {
                    continue;
                };
                let part0 = Instance::from_ref(part0);
                let part1 = Instance::from_ref(part1);
                if part0.data.lock().unwrap().part_data.is_some()
                    && part1.data.lock().unwrap().part_data.is_some()
                {
                    adjacency
                        .entry(part0.id().0)
                        .or_default()
                        .push(part1.clone());
                    adjacency.entry(part1.id().0).or_default().push(part0);
                }
            }

            if let Some(weld) = &data.weld_constraint_data {
                if !weld.enabled {
                    continue;
                }
                let Some(part0) = weld.part0.as_ref().and_then(|w| w.upgrade()) else {
                    continue;
                };
                let Some(part1) = weld.part1.as_ref().and_then(|w| w.upgrade()) else {
                    continue;
                };
                let part0 = Instance::from_ref(part0);
                let part1 = Instance::from_ref(part1);
                if part0.data.lock().unwrap().part_data.is_some()
                    && part1.data.lock().unwrap().part_data.is_some()
                {
                    adjacency
                        .entry(part0.id().0)
                        .or_default()
                        .push(part1.clone());
                    adjacency.entry(part1.id().0).or_default().push(part0);
                }
            }
        }

        if !recursive {
            let mut seen = HashSet::new();
            return adjacency
                .remove(&self.id().0)
                .unwrap_or_default()
                .into_iter()
                .filter(|part| seen.insert(part.id().0))
                .collect();
        }

        let mut result = Vec::new();
        let mut stack = adjacency.remove(&self.id().0).unwrap_or_default();
        let mut visited = HashSet::from([self.id().0]);

        while let Some(part) = stack.pop() {
            if !visited.insert(part.id().0) {
                continue;
            }
            if let Some(neighbors) = adjacency.get(&part.id().0) {
                for neighbor in neighbors {
                    if !visited.contains(&neighbor.id().0) {
                        stack.push(neighbor.clone());
                    }
                }
            }
            result.push(part);
        }

        result
    }

    pub(super) fn assembly_parts_from_graph(&self) -> Vec<Instance> {
        if self.data.lock().unwrap().part_data.is_none() {
            return Vec::new();
        }
        let mut parts = vec![self.clone()];
        parts.extend(self.connected_parts_from_graph(true));
        parts
    }

    fn model_descendant_parts(&self) -> Vec<Instance> {
        self.get_descendants()
            .into_iter()
            .filter(|inst| inst.data.lock().unwrap().part_data.is_some())
            .collect()
    }

    pub fn attachment_parent_part(&self) -> Option<Instance> {
        let parent = self.parent()?;
        if parent.data.lock().unwrap().part_data.is_some() {
            Some(parent)
        } else {
            None
        }
    }

    pub fn attachment_local_cframe(&self) -> Option<CFrame> {
        let data = self.data.lock().unwrap();
        data.attachment_data
            .as_ref()
            .map(|attachment| attachment.cframe)
    }

    pub fn attachment_world_cframe(&self) -> Option<CFrame> {
        let parent = self.attachment_parent_part()?;
        let parent_cframe = {
            let data = parent.data.lock().unwrap();
            data.part_data.as_ref().map(|part| part.cframe)?
        };
        let local_cframe = self.attachment_local_cframe()?;
        Some(parent_cframe.multiply(&local_cframe))
    }

    pub(super) fn get_pivot_cframe(&self) -> Option<CFrame> {
        let data = self.data.lock().unwrap();
        if let Some(part) = &data.part_data {
            return Some(part.cframe);
        }

        let Some(model) = &data.model_data else {
            return None;
        };

        if let Some(primary) = &model.primary_part {
            if let Some(primary_ref) = primary.upgrade() {
                let primary_data = primary_ref.lock().unwrap();
                if let Some(part) = &primary_data.part_data {
                    return Some(part.cframe);
                }
            }
        }

        if let Some(world_pivot) = model.world_pivot {
            return Some(world_pivot);
        }
        drop(data);

        let parts = self.model_descendant_parts();
        if parts.is_empty() {
            return Some(CFrame::identity());
        }

        let first_cframe = {
            let first = parts[0].data.lock().unwrap();
            first.part_data.as_ref().map(|part| part.cframe)?
        };
        Some(first_cframe)
    }

    pub(super) fn pivot_to_cframe(&self, target: CFrame) {
        let target = target.orthonormalized();
        if self.data.lock().unwrap().part_data.is_some() {
            let mut data = self.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.cframe = target;
                part.position = target.position;
                part.position_dirty = true;
                part.cframe_dirty = true;
            }
            return;
        }

        let Some(old_pivot) = self.get_pivot_cframe() else {
            return;
        };
        let offset = old_pivot.inverse().multiply(&target);
        let parts = self.model_descendant_parts();

        for part in &parts {
            let mut data = part.data.lock().unwrap();
            if let Some(part_data) = &mut data.part_data {
                part_data.cframe = part_data.cframe.multiply(&offset);
                part_data.position = part_data.cframe.position;
                part_data.position_dirty = true;
                part_data.cframe_dirty = true;
            }
        }

        let mut data = self.data.lock().unwrap();
        if let Some(model) = &mut data.model_data {
            model.world_pivot = Some(target);
        }
    }

    pub(crate) fn cached_userdata(lua: &Lua, instance: &Instance) -> Result<AnyUserData> {
        const CACHE_KEY: &str = "__clawblox_instance_userdata_cache";
        let cache = match lua.named_registry_value::<Table>(CACHE_KEY) {
            Ok(table) => table,
            Err(_) => {
                let table = lua.create_table()?;
                lua.set_named_registry_value(CACHE_KEY, table.clone())?;
                table
            }
        };

        let id_key = instance.id().0.to_string();
        if let Ok(existing) = cache.get::<AnyUserData>(id_key.as_str()) {
            return Ok(existing);
        }

        let created = lua.create_userdata(instance.clone())?;
        cache.set(id_key.as_str(), created.clone())?;
        Ok(created)
    }

    pub fn new(class_name: ClassName, name: &str) -> Self {
        Self {
            data: Arc::new(Mutex::new(InstanceData::new(class_name, name))),
        }
    }

    pub fn from_data(data: InstanceData) -> Self {
        Self {
            data: Arc::new(Mutex::new(data)),
        }
    }

    pub fn from_ref(data: InstanceRef) -> Self {
        Self { data }
    }

    pub fn id(&self) -> InstanceId {
        self.data.lock().unwrap().id
    }

    pub fn name(&self) -> String {
        self.data.lock().unwrap().name.clone()
    }

    pub fn set_name(&self, name: &str) {
        self.data.lock().unwrap().name = name.to_string();
    }

    pub fn fire_remote_on_server_event(
        &self,
        lua: &Lua,
        player: &Instance,
        args: Vec<Value>,
    ) -> Result<()> {
        let signal = {
            let data = self.data.lock().unwrap();
            data.remote_event_data
                .as_ref()
                .map(|r| r.on_server_event.clone())
        };
        let Some(signal) = signal else {
            return Ok(());
        };

        let mut values = Vec::with_capacity(args.len() + 1);
        values.push(Value::UserData(Instance::cached_userdata(lua, player)?));
        values.extend(args);
        let threads = signal.fire_as_coroutines(lua, MultiValue::from_iter(values))?;
        crate::game::lua::events::track_yielded_threads(lua, threads)?;
        Ok(())
    }

    fn fire_signal(lua: &Lua, signal: &RBXScriptSignal, args: MultiValue) -> Result<()> {
        let threads = signal.fire_as_coroutines(lua, args)?;
        crate::game::lua::events::track_yielded_threads(lua, threads)?;
        Ok(())
    }

    pub(super) fn fire_property_changed_if_listening(
        &self,
        lua: &Lua,
        property_name: &str,
    ) -> Result<()> {
        let signal = self
            .data
            .lock()
            .unwrap()
            .property_changed_signals
            .get(property_name)
            .cloned();
        if let Some(signal) = signal {
            Self::fire_signal(lua, &signal, MultiValue::new())?;
        }
        Ok(())
    }

    pub(super) fn ensure_animator_child_for_humanoid(&self) -> Instance {
        if let Some(existing) = self
            .get_children()
            .into_iter()
            .find(|child| child.class_name() == ClassName::Animator)
        {
            return existing;
        }

        let animator = Instance::from_data(InstanceData::new_animator("Animator"));
        animator.set_parent(Some(self));
        animator
    }

    pub(super) fn load_animation_track_for_animator(
        &self,
        lua: &Lua,
        animation: &Instance,
    ) -> Result<AnimationTrack> {
        let animation_id = {
            let data = animation.data.lock().unwrap();
            if data.class_name != ClassName::Animation {
                return Err(mlua::Error::runtime(
                    "LoadAnimation expects an Animation instance",
                ));
            }
            data.animation_data
                .as_ref()
                .map(|a| a.animation_id.clone())
                .unwrap_or_default()
        };

        if animation_id.is_empty() {
            return Err(mlua::Error::runtime("AnimationId is empty"));
        }

        let length = default_animation_length_seconds(&animation_id);
        let track = AnimationTrack::new(animation_id, self.id().0, length);

        if let Some(scheduler) = lua.app_data_ref::<AnimationScheduler>() {
            scheduler.register(track.clone());
        }

        {
            let mut data = self.data.lock().unwrap();
            if let Some(animator_data) = &mut data.animator_data {
                animator_data.loaded_tracks += 1;
            }
        }

        Ok(track)
    }

    pub fn class_name(&self) -> ClassName {
        self.data.lock().unwrap().class_name
    }

    pub fn is_a(&self, class_name: &str) -> bool {
        self.data.lock().unwrap().class_name.is_a(class_name)
    }

    pub fn parent(&self) -> Option<Instance> {
        self.data
            .lock()
            .unwrap()
            .parent
            .as_ref()
            .and_then(|w| w.upgrade())
            .map(Instance::from_ref)
    }

    pub fn set_parent(&self, parent: Option<&Instance>) {
        if let Some(old_parent) = self.parent() {
            let my_id = self.id();
            old_parent
                .data
                .lock()
                .unwrap()
                .children
                .retain(|c| c.lock().unwrap().id != my_id);
        }

        if let Some(new_parent) = parent {
            self.data.lock().unwrap().parent = Some(Arc::downgrade(&new_parent.data));
            new_parent
                .data
                .lock()
                .unwrap()
                .children
                .push(Arc::clone(&self.data));
        } else {
            self.data.lock().unwrap().parent = None;
        }
    }

    pub fn set_parent_with_lua(&self, parent: Option<&Instance>, lua: &Lua) -> Result<()> {
        let old_parent = self.parent();
        if let Some(old_parent) = &old_parent {
            self.fire_descendant_removing_from_ancestors(lua, old_parent)?;
        }
        self.set_parent(parent);

        if let Some(old_parent) = old_parent {
            let signal = old_parent.data.lock().unwrap().child_removed.clone();
            let child_ud = lua.create_userdata(self.clone())?;
            Self::fire_signal(
                lua,
                &signal,
                MultiValue::from_iter([Value::UserData(child_ud)]),
            )?;
        }

        if let Some(new_parent) = parent {
            let signal = new_parent.data.lock().unwrap().child_added.clone();
            let child_ud = lua.create_userdata(self.clone())?;
            Self::fire_signal(
                lua,
                &signal,
                MultiValue::from_iter([Value::UserData(child_ud)]),
            )?;

            self.fire_descendant_added_from_ancestors(lua, new_parent)?;
        }

        self.fire_ancestry_changed_recursive(lua)?;
        self.fire_property_changed_if_listening(lua, "Parent")?;
        Ok(())
    }

    fn self_and_descendants(&self) -> Vec<Instance> {
        let mut instances = vec![self.clone()];
        instances.extend(self.get_descendants());
        instances
    }

    fn ancestor_chain_from(&self, ancestor: &Instance) -> Vec<Instance> {
        let mut ancestors = vec![ancestor.clone()];
        let mut current = ancestor.parent();
        while let Some(parent) = current {
            current = parent.parent();
            ancestors.push(parent);
        }
        ancestors
    }

    fn fire_descendant_added_from_ancestors(&self, lua: &Lua, ancestor: &Instance) -> Result<()> {
        let descendants = self.self_and_descendants();
        let ancestors = self.ancestor_chain_from(ancestor);
        for descendant in descendants {
            for ancestor in &ancestors {
                let signal = ancestor.data.lock().unwrap().descendant_added.clone();
                let descendant_ud = lua.create_userdata(descendant.clone())?;
                Self::fire_signal(
                    lua,
                    &signal,
                    MultiValue::from_iter([Value::UserData(descendant_ud)]),
                )?;
            }
        }
        Ok(())
    }

    fn fire_descendant_removing_from_ancestors(
        &self,
        lua: &Lua,
        ancestor: &Instance,
    ) -> Result<()> {
        let descendants = self.self_and_descendants();
        let ancestors = self.ancestor_chain_from(ancestor);
        for descendant in descendants {
            for ancestor in &ancestors {
                let signal = ancestor.data.lock().unwrap().descendant_removing.clone();
                let descendant_ud = lua.create_userdata(descendant.clone())?;
                Self::fire_signal(
                    lua,
                    &signal,
                    MultiValue::from_iter([Value::UserData(descendant_ud)]),
                )?;
            }
        }
        Ok(())
    }

    fn fire_ancestry_changed_recursive(&self, lua: &Lua) -> Result<()> {
        let ancestry_signal = self.data.lock().unwrap().ancestry_changed.clone();
        let this_ud = lua.create_userdata(self.clone())?;
        let parent_value = match self.parent() {
            Some(parent) => Value::UserData(lua.create_userdata(parent)?),
            None => Value::Nil,
        };
        Self::fire_signal(
            lua,
            &ancestry_signal,
            MultiValue::from_iter([Value::UserData(this_ud), parent_value]),
        )?;

        for child in self.get_children() {
            child.fire_ancestry_changed_recursive(lua)?;
        }

        Ok(())
    }

    pub fn get_children(&self) -> Vec<Instance> {
        self.data
            .lock()
            .unwrap()
            .children
            .iter()
            .map(|c| Instance::from_ref(Arc::clone(c)))
            .collect()
    }

    pub fn get_descendants(&self) -> Vec<Instance> {
        let mut result = Vec::new();
        for child in self.get_children() {
            result.push(child.clone());
            result.extend(child.get_descendants());
        }
        result
    }

    pub fn find_first_child(&self, name: &str, recursive: bool) -> Option<Instance> {
        for child in self.get_children() {
            if child.name() == name {
                return Some(child);
            }
            if recursive {
                if let Some(found) = child.find_first_child(name, true) {
                    return Some(found);
                }
            }
        }
        None
    }

    pub fn find_first_child_of_class(&self, class_name: &str) -> Option<Instance> {
        for child in self.get_children() {
            if child.is_a(class_name) {
                return Some(child);
            }
        }
        None
    }

    pub fn is_descendant_of(&self, ancestor: &Instance) -> bool {
        let mut current = self.parent();
        while let Some(p) = current {
            if p.id() == ancestor.id() {
                return true;
            }
            current = p.parent();
        }
        false
    }

    pub fn destroy(&self, lua: &Lua) -> Result<()> {
        {
            let mut data = self.data.lock().unwrap();
            if data.destroyed {
                return Ok(());
            }
            data.destroyed = true;
        }

        let destroying = self.data.lock().unwrap().destroying.clone();
        let threads = destroying.fire_as_coroutines(lua, mlua::MultiValue::new())?;
        crate::game::lua::events::track_yielded_threads(lua, threads)?;

        for child in self.get_children() {
            child.destroy(lua)?;
        }

        self.set_parent_with_lua(None, lua)?;
        Ok(())
    }

    pub fn clone_instance(&self) -> Instance {
        let data = self.data.lock().unwrap();
        let mut new_data = InstanceData::new(data.class_name, &data.name);
        new_data.attributes = data.attributes.clone();
        new_data.tags = data.tags.clone();
        new_data.part_data = data.part_data.clone();
        new_data.humanoid_data = data.humanoid_data.clone();
        new_data.model_data = data.model_data.clone();
        if let Some(int_data) = &data.int_value_data {
            new_data.int_value_data = Some(IntValueData {
                value: int_data.value,
                changed: create_signal("Changed"),
            });
        }
        if let Some(number_data) = &data.number_value_data {
            new_data.number_value_data = Some(NumberValueData {
                value: number_data.value,
                changed: create_signal("Changed"),
            });
        }
        if let Some(bool_data) = &data.bool_value_data {
            new_data.bool_value_data = Some(BoolValueData {
                value: bool_data.value,
                changed: create_signal("Changed"),
            });
        }
        if let Some(string_data) = &data.string_value_data {
            new_data.string_value_data = Some(StringValueData {
                value: string_data.value.clone(),
                changed: create_signal("Changed"),
            });
        }
        if let Some(object_data) = &data.object_value_data {
            new_data.object_value_data = Some(ObjectValueData {
                value: object_data.value.clone(),
                changed: create_signal("Changed"),
            });
        }
        if let Some(cframe_data) = &data.cframe_value_data {
            new_data.cframe_value_data = Some(CFrameValueData {
                value: cframe_data.value,
                changed: create_signal("Changed"),
            });
        }
        if data.remote_event_data.is_some() {
            new_data.remote_event_data = Some(RemoteEventData::default());
        }
        new_data.body_velocity_data = data.body_velocity_data.clone();
        new_data.body_force_data = data.body_force_data;
        new_data.body_angular_velocity_data = data.body_angular_velocity_data;
        new_data.attachment_data = data.attachment_data.clone();
        new_data.vector_force_data = data.vector_force_data.clone();
        new_data.align_position_data = data.align_position_data.clone();
        new_data.align_orientation_data = data.align_orientation_data.clone();
        new_data.terrain_data = data.terrain_data.clone();

        // Clone GUI data but create fresh signals to avoid sharing handlers
        if let Some(gui) = &data.gui_data {
            let mut cloned_gui = gui.clone();
            // Create new signals for the cloned instance
            if cloned_gui.mouse_button1_click.is_some() {
                cloned_gui.mouse_button1_click = Some(create_signal("MouseButton1Click"));
                cloned_gui.mouse_button1_down = Some(create_signal("MouseButton1Down"));
                cloned_gui.mouse_button1_up = Some(create_signal("MouseButton1Up"));
                cloned_gui.mouse_enter = Some(create_signal("MouseEnter"));
                cloned_gui.mouse_leave = Some(create_signal("MouseLeave"));
            }
            new_data.gui_data = Some(cloned_gui);
        }

        drop(data);

        let new_instance = Instance::from_data(new_data);

        for child in self.get_children() {
            let cloned_child = child.clone_instance();
            cloned_child.set_parent(Some(&new_instance));
        }

        new_instance
    }

    pub fn set_attribute(&self, name: &str, value: AttributeValue) {
        self.data
            .lock()
            .unwrap()
            .attributes
            .insert(name.to_string(), value);
    }

    pub fn set_attribute_with_lua(
        &self,
        lua: &Lua,
        name: &str,
        value: AttributeValue,
    ) -> Result<()> {
        self.set_attribute(name, value);

        let (attribute_changed, specific_signal) = {
            let mut data = self.data.lock().unwrap();
            let specific = data
                .attribute_changed_signals
                .entry(name.to_string())
                .or_insert_with(|| create_signal(&format!("AttributeChanged_{}", name)))
                .clone();
            (data.attribute_changed.clone(), specific)
        };

        let attr_name = Value::String(lua.create_string(name)?);
        Self::fire_signal(lua, &attribute_changed, MultiValue::from_iter([attr_name]))?;
        Self::fire_signal(lua, &specific_signal, MultiValue::new())?;
        Ok(())
    }

    pub fn ensure_property_signal(&self, property_name: &str) -> RBXScriptSignal {
        let mut data = self.data.lock().unwrap();
        data.property_changed_signals
            .entry(property_name.to_string())
            .or_insert_with(|| create_signal(&format!("{}Changed", property_name)))
            .clone()
    }

    pub fn ensure_attribute_signal(&self, attr_name: &str) -> RBXScriptSignal {
        let mut data = self.data.lock().unwrap();
        data.attribute_changed_signals
            .entry(attr_name.to_string())
            .or_insert_with(|| create_signal(&format!("AttributeChanged_{}", attr_name)))
            .clone()
    }

    pub fn get_attribute(&self, name: &str) -> Option<AttributeValue> {
        self.data.lock().unwrap().attributes.get(name).cloned()
    }

    pub fn get_attributes(&self) -> HashMap<String, AttributeValue> {
        self.data.lock().unwrap().attributes.clone()
    }

    pub fn add_tag(&self, tag: &str) {
        self.data.lock().unwrap().tags.insert(tag.to_string());
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.data.lock().unwrap().tags.contains(tag)
    }

    pub fn remove_tag(&self, tag: &str) {
        self.data.lock().unwrap().tags.remove(tag);
    }

    pub fn get_tags(&self) -> HashSet<String> {
        self.data.lock().unwrap().tags.clone()
    }

    pub fn weak_ref(&self) -> WeakInstanceRef {
        Arc::downgrade(&self.data)
    }
}

impl FromLua for Instance {
    fn from_lua(value: Value, _lua: &Lua) -> Result<Self> {
        match value {
            Value::UserData(ud) => ud.borrow::<Instance>().map(|v| v.clone()),
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Instance".to_string(),
                message: Some("expected Instance".to_string()),
            }),
        }
    }
}
