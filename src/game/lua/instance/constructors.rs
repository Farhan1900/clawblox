use mlua::{Lua, Result, Value};

use crate::game::lua::services::{PlayersService, WorkspaceService};

use super::{ClassName, Instance, InstanceData};

pub(super) fn parent_instance_from_lua_value(parent: Value) -> Result<Option<Instance>> {
    match parent {
        Value::Nil => Ok(None),
        Value::UserData(ud) => {
            if let Ok(inst) = ud.borrow::<Instance>() {
                Ok(Some(inst.clone()))
            } else if let Ok(ws) = ud.borrow::<WorkspaceService>() {
                Ok(Some(ws.instance.clone()))
            } else if let Ok(players) = ud.borrow::<PlayersService>() {
                Ok(Some(players.instance.clone()))
            } else {
                Err(mlua::Error::runtime(
                    "Parent must be an Instance, supported Service, or nil",
                ))
            }
        }
        _ => Err(mlua::Error::runtime(
            "Parent must be an Instance, supported Service, or nil",
        )),
    }
}

pub fn register_instance(lua: &Lua) -> Result<()> {
    let instance_table = lua.create_table()?;

    instance_table.set(
        "new",
        lua.create_function(
            |lua, (class_name, parent): (String, Value)| -> Result<Instance> {
                let instance = match class_name.as_str() {
                    "Part" => Instance::from_data(InstanceData::new_part("Part")),
                    "Model" => Instance::from_data(InstanceData::new_model("Model")),
                    "Humanoid" => Instance::from_data(InstanceData::new_humanoid("Humanoid")),
                    "Player" => Instance::from_data(InstanceData::new_player(0, "Player")),
                    "Animation" => Instance::from_data(InstanceData::new_animation("Animation")),
                    "Animator" => Instance::from_data(InstanceData::new_animator("Animator")),
                    "Folder" => Instance::new(ClassName::Folder, "Folder"),
                    "Terrain" => Instance::from_data(InstanceData::new_terrain("Terrain")),
                    "Script" => Instance::from_data(InstanceData::new_script("Script")),
                    "ModuleScript" => {
                        Instance::from_data(InstanceData::new_module_script("ModuleScript"))
                    }
                    "ServerScriptService" => {
                        Instance::from_data(InstanceData::new_server_script_service())
                    }
                    "ServerStorage" => Instance::from_data(InstanceData::new_server_storage()),
                    "RemoteEvent" => {
                        Instance::from_data(InstanceData::new_remote_event("RemoteEvent"))
                    }
                    "IntValue" => Instance::from_data(InstanceData::new_int_value("IntValue")),
                    "NumberValue" => {
                        Instance::from_data(InstanceData::new_number_value("NumberValue"))
                    }
                    "BoolValue" => Instance::from_data(InstanceData::new_bool_value("BoolValue")),
                    "StringValue" => {
                        Instance::from_data(InstanceData::new_string_value("StringValue"))
                    }
                    "ObjectValue" => {
                        Instance::from_data(InstanceData::new_object_value("ObjectValue"))
                    }
                    "CFrameValue" => {
                        Instance::from_data(InstanceData::new_cframe_value("CFrameValue"))
                    }
                    "BodyVelocity" => {
                        Instance::from_data(InstanceData::new_body_velocity("BodyVelocity"))
                    }
                    "BodyForce" => Instance::from_data(InstanceData::new_body_force("BodyForce")),
                    "BodyAngularVelocity" => Instance::from_data(
                        InstanceData::new_body_angular_velocity("BodyAngularVelocity"),
                    ),
                    "Attachment" => Instance::from_data(InstanceData::new_attachment("Attachment")),
                    "VectorForce" => {
                        Instance::from_data(InstanceData::new_vector_force("VectorForce"))
                    }
                    "AlignPosition" => {
                        Instance::from_data(InstanceData::new_align_position("AlignPosition"))
                    }
                    "AlignOrientation" => {
                        Instance::from_data(InstanceData::new_align_orientation("AlignOrientation"))
                    }
                    // Constraints
                    "Weld" => Instance::from_data(InstanceData::new_weld("Weld")),
                    "WeldConstraint" => {
                        Instance::from_data(InstanceData::new_weld_constraint("WeldConstraint"))
                    }
                    // 3D GUI
                    "BillboardGui" => {
                        Instance::from_data(InstanceData::new_billboard_gui("BillboardGui"))
                    }
                    // GUI classes
                    "ScreenGui" => Instance::from_data(InstanceData::new_screen_gui("ScreenGui")),
                    "Frame" => Instance::from_data(InstanceData::new_frame("Frame")),
                    "TextLabel" => Instance::from_data(InstanceData::new_text_label("TextLabel")),
                    "TextButton" => {
                        Instance::from_data(InstanceData::new_text_button("TextButton"))
                    }
                    "ImageLabel" => {
                        Instance::from_data(InstanceData::new_image_label("ImageLabel"))
                    }
                    "ImageButton" => {
                        Instance::from_data(InstanceData::new_image_button("ImageButton"))
                    }
                    "UICorner" => Instance::from_data(InstanceData::new_uicorner("UICorner")),
                    "Sound" => Instance::from_data(InstanceData::new_sound("Sound")),
                    _ => {
                        return Err(mlua::Error::runtime(format!(
                            "Unknown class '{}'",
                            class_name
                        )))
                    }
                };

                let parent = parent_instance_from_lua_value(parent)?;
                instance.set_parent_with_lua(parent.as_ref(), lua)?;

                Ok(instance)
            },
        )?,
    )?;

    lua.globals().set("Instance", instance_table)?;

    Ok(())
}
