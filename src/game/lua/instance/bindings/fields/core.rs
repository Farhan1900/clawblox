use mlua::{UserDataFields, Value};

use super::super::super::constructors::parent_instance_from_lua_value;
use super::super::super::*;

pub(super) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
    fields.add_field_method_get("Name", |_, this| Ok(this.name()));
    fields.add_field_method_set("Name", |lua, this, name: String| {
        this.set_name(&name);
        this.fire_property_changed_if_listening(lua, "Name")?;
        Ok(())
    });

    fields.add_field_method_get("ClassName", |_, this| {
        Ok(this.class_name().as_str().to_string())
    });

    fields.add_field_method_get("Parent", |_, this| Ok(this.parent()));
    fields.add_field_method_set("Parent", |lua, this, parent: Value| {
        let parent = parent_instance_from_lua_value(parent)?;
        this.set_parent_with_lua(parent.as_ref(), lua)?;
        Ok(())
    });

    fields.add_field_method_get("ChildAdded", |_, this| {
        Ok(this.data.lock().unwrap().child_added.clone())
    });
    fields.add_field_method_get("ChildRemoved", |_, this| {
        Ok(this.data.lock().unwrap().child_removed.clone())
    });
    fields.add_field_method_get("AncestryChanged", |_, this| {
        Ok(this.data.lock().unwrap().ancestry_changed.clone())
    });
    fields.add_field_method_get("Destroying", |_, this| {
        Ok(this.data.lock().unwrap().destroying.clone())
    });
    fields.add_field_method_get("AttributeChanged", |_, this| {
        Ok(this.data.lock().unwrap().attribute_changed.clone())
    });
    fields.add_field_method_get("OnServerEvent", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .remote_event_data
            .as_ref()
            .map(|r| r.on_server_event.clone()))
    });

    fields.add_field_method_get("Source", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.script_data.as_ref().map(|s| s.source.clone()))
    });
    fields.add_field_method_set("Source", |lua, this, source: String| {
        let mut data = this.data.lock().unwrap();
        if let Some(script) = &mut data.script_data {
            script.source = source;
            drop(data);
            this.fire_property_changed_if_listening(lua, "Source")?;
        }
        Ok(())
    });

    fields.add_field_method_get("Disabled", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.script_data.as_ref().map(|s| s.disabled))
    });
    fields.add_field_method_set("Disabled", |lua, this, disabled: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(script) = &mut data.script_data {
            script.disabled = disabled;
            drop(data);
            this.fire_property_changed_if_listening(lua, "Disabled")?;
        }
        Ok(())
    });
}
