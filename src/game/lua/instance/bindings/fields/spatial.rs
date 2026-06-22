use mlua::{UserDataFields, Value};

use crate::game::lua::types::{CFrame, UDim2, Vector3};

use super::super::super::*;

pub(super) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
    // Position: Vector3 for Parts, UDim2 for GUI objects
    fields.add_field_method_get("Position", |lua, this| {
        let data = this.data.lock().unwrap();
        if let Some(align) = &data.align_position_data {
            Ok(Value::UserData(lua.create_userdata(align.position)?))
        } else if let Some(part) = &data.part_data {
            Ok(Value::UserData(lua.create_userdata(part.position)?))
        } else if let Some(attachment) = &data.attachment_data {
            Ok(Value::UserData(
                lua.create_userdata(attachment.cframe.position)?,
            ))
        } else if let Some(gui) = &data.gui_data {
            Ok(Value::UserData(lua.create_userdata(gui.position)?))
        } else {
            Ok(Value::Nil)
        }
    });
    fields.add_field_method_set("Position", |lua, this, value: Value| {
        let mut changed = false;
        let mut data = this.data.lock().unwrap();
        match value {
            Value::UserData(ud) => {
                if let Ok(pos) = ud.borrow::<Vector3>() {
                    if let Some(align) = &mut data.align_position_data {
                        align.position = *pos;
                        changed = true;
                    } else if let Some(part) = &mut data.part_data {
                        part.position = *pos;
                        part.cframe.position = *pos;
                        part.position_dirty = true;
                        part.cframe_dirty = false;
                        changed = true;
                    } else if let Some(attachment) = &mut data.attachment_data {
                        attachment.cframe.position = *pos;
                        changed = true;
                    }
                } else if let Ok(pos) = ud.borrow::<UDim2>() {
                    if let Some(gui) = &mut data.gui_data {
                        gui.position = *pos;
                        changed = true;
                    }
                }
            }
            _ => {}
        }
        drop(data);
        if changed {
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("CFrame", |_, this| {
        let data = this.data.lock().unwrap();
        if let Some(part) = &data.part_data {
            Ok(Some(part.cframe))
        } else {
            Ok(data
                .attachment_data
                .as_ref()
                .map(|attachment| attachment.cframe))
        }
    });
    fields.add_field_method_set("CFrame", |lua, this, cf: CFrame| {
        let cf = cf.orthonormalized();
        let mut changed = false;
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.cframe = cf;
            part.position = cf.position;
            part.position_dirty = true;
            part.cframe_dirty = true;
            changed = true;
        } else if let Some(attachment) = &mut data.attachment_data {
            attachment.cframe = cf;
            changed = true;
        }
        drop(data);
        if changed {
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("WorldPosition", |_, this| {
        Ok(this.attachment_world_cframe().map(|cf| cf.position))
    });
    fields.add_field_method_get("WorldCFrame", |_, this| Ok(this.attachment_world_cframe()));

    // Size: Vector3 for Parts, UDim2 for GUI objects
    fields.add_field_method_get("Size", |lua, this| {
        let data = this.data.lock().unwrap();
        if let Some(part) = &data.part_data {
            Ok(Value::UserData(lua.create_userdata(part.size)?))
        } else if let Some(gui) = &data.gui_data {
            Ok(Value::UserData(lua.create_userdata(gui.size)?))
        } else {
            Ok(Value::Nil)
        }
    });
    fields.add_field_method_set("Size", |lua, this, value: Value| {
        let mut changed = false;
        let mut data = this.data.lock().unwrap();
        match value {
            Value::UserData(ud) => {
                if let Ok(size) = ud.borrow::<Vector3>() {
                    if let Some(part) = &mut data.part_data {
                        part.size = *size;
                        part.size_dirty = true;
                        changed = true;
                    }
                } else if let Ok(size) = ud.borrow::<UDim2>() {
                    if let Some(gui) = &mut data.gui_data {
                        gui.size = *size;
                        changed = true;
                    }
                }
            }
            _ => {}
        }
        drop(data);
        if changed {
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });
}
