use mlua::{MultiValue, UserDataFields, Value};
use std::sync::Arc;

use crate::game::lua::events::track_yielded_threads;
use crate::game::lua::types::{
    CFrame, Color3, Material, PartType, PhysicalProperties, UDim, UDim2, Vector3,
};

use super::super::constructors::parent_instance_from_lua_value;
use super::super::*;

/// Compute and cache the relative transform for a WeldConstraint when both parts are set.
fn compute_weld_constraint_relative_cframe(data: &mut InstanceData) {
    let wc = match &data.weld_constraint_data {
        Some(wc) => wc,
        None => return,
    };
    let part0_ref = wc.part0.as_ref().and_then(|w| w.upgrade());
    let part1_ref = wc.part1.as_ref().and_then(|w| w.upgrade());
    if let (Some(p0), Some(p1)) = (part0_ref, part1_ref) {
        let p0_data = p0.lock().unwrap();
        let p1_data = p1.lock().unwrap();
        if let (Some(p0_part), Some(p1_part)) = (&p0_data.part_data, &p1_data.part_data) {
            let relative_cframe = p0_part.cframe.inverse().multiply(&p1_part.cframe);
            if let Some(wc) = &mut data.weld_constraint_data {
                wc.relative_cframe = Some(relative_cframe);
            }
        }
    }
}

pub(crate) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
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
        fields.add_field_method_set("Position", |_, this, value: Value| {
            let mut data = this.data.lock().unwrap();
            match value {
                Value::UserData(ud) => {
                    if let Ok(pos) = ud.borrow::<Vector3>() {
                        if let Some(align) = &mut data.align_position_data {
                            align.position = *pos;
                        } else if let Some(part) = &mut data.part_data {
                            part.position = *pos;
                            part.cframe.position = *pos;
                            part.position_dirty = true;
                            part.cframe_dirty = false;
                        } else if let Some(attachment) = &mut data.attachment_data {
                            attachment.cframe.position = *pos;
                        }
                    } else if let Ok(pos) = ud.borrow::<UDim2>() {
                        if let Some(gui) = &mut data.gui_data {
                            gui.position = *pos;
                        }
                    }
                }
                _ => {}
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
        fields.add_field_method_set("CFrame", |_, this, cf: CFrame| {
            let cf = cf.orthonormalized();
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.cframe = cf;
                part.position = cf.position;
                part.position_dirty = true;
                part.cframe_dirty = true;
            } else if let Some(attachment) = &mut data.attachment_data {
                attachment.cframe = cf;
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
        fields.add_field_method_set("Size", |_, this, value: Value| {
            let mut data = this.data.lock().unwrap();
            match value {
                Value::UserData(ud) => {
                    if let Ok(size) = ud.borrow::<Vector3>() {
                        if let Some(part) = &mut data.part_data {
                            part.size = *size;
                            part.size_dirty = true;
                        }
                    } else if let Ok(size) = ud.borrow::<UDim2>() {
                        if let Some(gui) = &mut data.gui_data {
                            gui.size = *size;
                        }
                    }
                }
                _ => {}
            }
            Ok(())
        });

        fields.add_field_method_get("Anchored", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.anchored))
        });
        fields.add_field_method_set("Anchored", |_, this, anchored: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.anchored = anchored;
                part.anchored_dirty = true;
            }
            Ok(())
        });

        fields.add_field_method_get("CanCollide", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.can_collide))
        });
        fields.add_field_method_set("CanCollide", |_, this, can_collide: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.can_collide = can_collide;
                part.can_collide_dirty = true;
                part.interaction_dirty = true;
            }
            Ok(())
        });
        fields.add_field_method_get("Massless", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.massless))
        });
        fields.add_field_method_set("Massless", |_, this, massless: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.massless = massless;
                part.custom_physical_properties_dirty = true;
            }
            Ok(())
        });

        fields.add_field_method_get("CanQuery", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.can_query))
        });
        fields.add_field_method_set("CanQuery", |_, this, can_query: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.can_query = can_query;
                part.interaction_dirty = true;
            }
            Ok(())
        });

        fields.add_field_method_get("CollisionGroup", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.collision_group.clone()))
        });
        fields.add_field_method_set("CollisionGroup", |_, this, collision_group: String| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.collision_group = collision_group;
            }
            Ok(())
        });

        fields.add_field_method_get("CanTouch", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.can_touch))
        });
        fields.add_field_method_set("CanTouch", |_, this, can_touch: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.can_touch = can_touch;
                part.interaction_dirty = true;
            }
            Ok(())
        });

        fields.add_field_method_get("Transparency", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.transparency))
        });
        fields.add_field_method_set("Transparency", |_, this, transparency: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.transparency = transparency.clamp(0.0, 1.0);
            }
            Ok(())
        });

        fields.add_field_method_get("Color", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.color))
        });
        fields.add_field_method_set("Color", |_, this, color: Color3| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.color = color;
            }
            Ok(())
        });

        fields.add_field_method_get("Material", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.material))
        });
        fields.add_field_method_set("Material", |_, this, material: Material| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.material = material;
                // Keep CurrentPhysicalProperties in sync with material defaults when not overridden.
                if part.custom_physical_properties.is_none() {
                    part.custom_physical_properties_dirty = true;
                }
            }
            Ok(())
        });

        fields.add_field_method_get("TopSurface", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.top_surface.clone()))
        });
        fields.add_field_method_set("TopSurface", |_, this, surface: String| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.top_surface = surface;
            }
            Ok(())
        });

        fields.add_field_method_get("BottomSurface", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.bottom_surface.clone()))
        });
        fields.add_field_method_set("BottomSurface", |_, this, surface: String| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.bottom_surface = surface;
            }
            Ok(())
        });

        fields.add_field_method_get("BrickColor", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.color))
        });
        fields.add_field_method_set("BrickColor", |_, this, color: Color3| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.color = color;
            }
            Ok(())
        });

        fields.add_field_method_get("Velocity", |_, this| {
            let data = this.data.lock().unwrap();
            if let Some(bv) = &data.body_velocity_data {
                Ok(Some(bv.velocity))
            } else {
                Ok(data.part_data.as_ref().map(|p| p.velocity))
            }
        });
        fields.add_field_method_set("Velocity", |_, this, velocity: Vector3| {
            let parent_part = {
                let mut data = this.data.lock().unwrap();
                if let Some(bv) = &mut data.body_velocity_data {
                    bv.velocity = velocity;
                    data.parent.as_ref().and_then(|w| w.upgrade())
                } else {
                    if let Some(part) = &mut data.part_data {
                        part.velocity = velocity;
                        part.velocity_dirty = true;
                    }
                    None
                }
            };
            if let Some(parent_ref) = parent_part {
                let mut parent_data = parent_ref.lock().unwrap();
                if let Some(part) = &mut parent_data.part_data {
                    part.velocity = velocity;
                    part.velocity_dirty = true;
                }
            }
            Ok(())
        });

        fields.add_field_method_get("CustomPhysicalProperties", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .part_data
                .as_ref()
                .and_then(|p| p.custom_physical_properties))
        });
        fields.add_field_method_get("CurrentPhysicalProperties", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .part_data
                .as_ref()
                .map(|p| p.current_physical_properties()))
        });
        fields.add_field_method_set(
            "CustomPhysicalProperties",
            |_, this, value: Option<PhysicalProperties>| {
                let mut data = this.data.lock().unwrap();
                if let Some(part) = &mut data.part_data {
                    part.custom_physical_properties = value;
                    part.custom_physical_properties_dirty = true;
                }
                Ok(())
            },
        );
        fields.add_field_method_get("Density", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .part_data
                .as_ref()
                .map(|p| p.current_physical_properties().density))
        });
        fields.add_field_method_set("Density", |_, this, density: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                let props = part.current_physical_properties();
                part.custom_physical_properties = Some(PhysicalProperties::new(
                    density,
                    props.friction,
                    props.elasticity,
                    props.friction_weight,
                    props.elasticity_weight,
                ));
                part.custom_physical_properties_dirty = true;
            }
            Ok(())
        });
        fields.add_field_method_get("Friction", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .part_data
                .as_ref()
                .map(|p| p.current_physical_properties().friction))
        });
        fields.add_field_method_set("Friction", |_, this, friction: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                let props = part.current_physical_properties();
                part.custom_physical_properties = Some(PhysicalProperties::new(
                    props.density,
                    friction,
                    props.elasticity,
                    props.friction_weight,
                    props.elasticity_weight,
                ));
                part.custom_physical_properties_dirty = true;
            }
            Ok(())
        });
        fields.add_field_method_get("Elasticity", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .part_data
                .as_ref()
                .map(|p| p.current_physical_properties().elasticity))
        });
        fields.add_field_method_set("Elasticity", |_, this, elasticity: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                let props = part.current_physical_properties();
                part.custom_physical_properties = Some(PhysicalProperties::new(
                    props.density,
                    props.friction,
                    elasticity,
                    props.friction_weight,
                    props.elasticity_weight,
                ));
                part.custom_physical_properties_dirty = true;
            }
            Ok(())
        });
        fields.add_field_method_get("FrictionWeight", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .part_data
                .as_ref()
                .map(|p| p.current_physical_properties().friction_weight))
        });
        fields.add_field_method_set("FrictionWeight", |_, this, friction_weight: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                let props = part.current_physical_properties();
                part.custom_physical_properties = Some(PhysicalProperties::new(
                    props.density,
                    props.friction,
                    props.elasticity,
                    friction_weight,
                    props.elasticity_weight,
                ));
                part.custom_physical_properties_dirty = true;
            }
            Ok(())
        });
        fields.add_field_method_get("ElasticityWeight", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .part_data
                .as_ref()
                .map(|p| p.current_physical_properties().elasticity_weight))
        });
        fields.add_field_method_set("ElasticityWeight", |_, this, elasticity_weight: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                let props = part.current_physical_properties();
                part.custom_physical_properties = Some(PhysicalProperties::new(
                    props.density,
                    props.friction,
                    props.elasticity,
                    props.friction_weight,
                    elasticity_weight,
                ));
                part.custom_physical_properties_dirty = true;
            }
            Ok(())
        });

        fields.add_field_method_get("MaxForce", |lua, this| {
            let data = this.data.lock().unwrap();
            if let Some(bv) = &data.body_velocity_data {
                return Ok(Value::UserData(lua.create_userdata(bv.max_force)?));
            }
            if let Some(align) = &data.align_position_data {
                return Ok(Value::Number(align.max_force as f64));
            }
            Ok(Value::Nil)
        });
        fields.add_field_method_set("MaxForce", |_, this, value: Value| {
            let mut data = this.data.lock().unwrap();
            if let Some(bv) = &mut data.body_velocity_data {
                let Value::UserData(ud) = value else {
                    return Err(mlua::Error::runtime("BodyVelocity.MaxForce expects Vector3"));
                };
                let max_force = ud.borrow::<Vector3>()?;
                bv.max_force = *max_force;
                return Ok(());
            }
            if let Some(align) = &mut data.align_position_data {
                align.max_force = match value {
                    Value::Integer(n) => n as f32,
                    Value::Number(n) => n as f32,
                    _ => {
                        return Err(mlua::Error::runtime(
                            "AlignPosition.MaxForce expects number",
                        ))
                    }
                };
            }
            Ok(())
        });

        fields.add_field_method_get("Force", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.body_force_data.as_ref().map(|bf| bf.force))
        });
        fields.add_field_method_set("Force", |_, this, force: Vector3| {
            let mut data = this.data.lock().unwrap();
            if let Some(bf) = &mut data.body_force_data {
                bf.force = force;
            }
            Ok(())
        });

        fields.add_field_method_get("AngularVelocity", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .body_angular_velocity_data
                .as_ref()
                .map(|bav| bav.angular_velocity))
        });
        fields.add_field_method_set("AngularVelocity", |_, this, angular_velocity: Vector3| {
            let mut data = this.data.lock().unwrap();
            if let Some(bav) = &mut data.body_angular_velocity_data {
                bav.angular_velocity = angular_velocity;
            }
            Ok(())
        });

        fields.add_field_method_get("MaxTorque", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .body_angular_velocity_data
                .as_ref()
                .map(|bav| bav.max_torque))
        });
        fields.add_field_method_set("MaxTorque", |_, this, max_torque: Vector3| {
            let mut data = this.data.lock().unwrap();
            if let Some(bav) = &mut data.body_angular_velocity_data {
                bav.max_torque = max_torque;
            }
            Ok(())
        });

        fields.add_field_method_get("Attachment0", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .align_position_data
                .as_ref()
                .and_then(|a| a.attachment0.as_ref())
                .or_else(|| {
                    data.align_orientation_data
                        .as_ref()
                        .and_then(|a| a.attachment0.as_ref())
                })
                .and_then(|w| w.upgrade())
                .map(Instance::from_ref))
        });
        fields.add_field_method_set("Attachment0", |_, this, attachment: Option<Instance>| {
            let mut data = this.data.lock().unwrap();
            if let Some(align) = &mut data.align_position_data {
                align.attachment0 = attachment.as_ref().map(|a| Arc::downgrade(&a.data));
            }
            if let Some(align) = &mut data.align_orientation_data {
                align.attachment0 = attachment.as_ref().map(|a| Arc::downgrade(&a.data));
            }
            Ok(())
        });

        fields.add_field_method_get("Attachment1", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .align_position_data
                .as_ref()
                .and_then(|a| a.attachment1.as_ref())
                .or_else(|| {
                    data.align_orientation_data
                        .as_ref()
                        .and_then(|a| a.attachment1.as_ref())
                })
                .and_then(|w| w.upgrade())
                .map(Instance::from_ref))
        });
        fields.add_field_method_set("Attachment1", |_, this, attachment: Option<Instance>| {
            let mut data = this.data.lock().unwrap();
            if let Some(align) = &mut data.align_position_data {
                align.attachment1 = attachment.as_ref().map(|a| Arc::downgrade(&a.data));
            }
            if let Some(align) = &mut data.align_orientation_data {
                align.attachment1 = attachment.as_ref().map(|a| Arc::downgrade(&a.data));
            }
            Ok(())
        });

        fields.add_field_method_get("Responsiveness", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .align_position_data
                .as_ref()
                .map(|a| a.responsiveness)
                .or_else(|| {
                    data.align_orientation_data
                        .as_ref()
                        .map(|a| a.responsiveness)
                }))
        });
        fields.add_field_method_set("Responsiveness", |_, this, responsiveness: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(align) = &mut data.align_position_data {
                align.responsiveness = responsiveness;
            }
            if let Some(align) = &mut data.align_orientation_data {
                align.responsiveness = responsiveness;
            }
            Ok(())
        });

        fields.add_field_method_get("RigidityEnabled", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .align_position_data
                .as_ref()
                .map(|a| a.rigidity_enabled)
                .or_else(|| {
                    data.align_orientation_data
                        .as_ref()
                        .map(|a| a.rigidity_enabled)
                }))
        });
        fields.add_field_method_set("RigidityEnabled", |_, this, enabled: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(align) = &mut data.align_position_data {
                align.rigidity_enabled = enabled;
            }
            if let Some(align) = &mut data.align_orientation_data {
                align.rigidity_enabled = enabled;
            }
            Ok(())
        });

        fields.add_field_method_get("AssemblyLinearVelocity", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.velocity))
        });
        fields.add_field_method_set("AssemblyLinearVelocity", |_, this, velocity: Vector3| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.velocity = velocity;
                part.velocity_dirty = true;
            }
            Ok(())
        });
        fields.add_field_method_get("AssemblyAngularVelocity", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.angular_velocity))
        });
        fields.add_field_method_set(
            "AssemblyAngularVelocity",
            |_, this, angular_velocity: Vector3| {
                let mut data = this.data.lock().unwrap();
                if let Some(part) = &mut data.part_data {
                    part.angular_velocity = angular_velocity;
                    part.angular_velocity_dirty = true;
                }
                Ok(())
            },
        );

        fields.add_field_method_get("Shape", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.shape))
        });
        fields.add_field_method_set("Shape", |_, this, shape: PartType| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.shape = shape;
                part.shape_dirty = true;
            }
            Ok(())
        });

        fields.add_field_method_get("Touched", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.touched.clone()))
        });

        fields.add_field_method_get("TouchEnded", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.part_data.as_ref().map(|p| p.touch_ended.clone()))
        });

        fields.add_field_method_get("OnServerEvent", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .remote_event_data
                .as_ref()
                .map(|r| r.on_server_event.clone()))
        });

        fields.add_field_method_get("Changed", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .int_value_data
                .as_ref()
                .map(|v| v.changed.clone())
                .or_else(|| data.number_value_data.as_ref().map(|v| v.changed.clone()))
                .or_else(|| data.bool_value_data.as_ref().map(|v| v.changed.clone()))
                .or_else(|| data.string_value_data.as_ref().map(|v| v.changed.clone()))
                .or_else(|| data.object_value_data.as_ref().map(|v| v.changed.clone()))
                .or_else(|| data.cframe_value_data.as_ref().map(|v| v.changed.clone())))
        });

        fields.add_field_method_get("Value", |lua, this| {
            let data = this.data.lock().unwrap();
            if let Some(v) = &data.int_value_data {
                return Ok(Value::Integer(v.value));
            }
            if let Some(v) = &data.number_value_data {
                return Ok(Value::Number(v.value));
            }
            if let Some(v) = &data.bool_value_data {
                return Ok(Value::Boolean(v.value));
            }
            if let Some(v) = &data.string_value_data {
                return Ok(Value::String(lua.create_string(&v.value)?));
            }
            if let Some(v) = &data.object_value_data {
                if let Some(value_ref) = &v.value {
                    if let Some(strong) = value_ref.upgrade() {
                        let inst = Instance::from_ref(strong);
                        return Ok(Value::UserData(Instance::cached_userdata(lua, &inst)?));
                    }
                }
                return Ok(Value::Nil);
            }
            if let Some(v) = &data.cframe_value_data {
                return Ok(Value::UserData(lua.create_userdata(v.value)?));
            }
            Ok(Value::Nil)
        });
        fields.add_field_method_set("Value", |lua, this, value: Value| {
            let changed_signal = {
                let mut data = this.data.lock().unwrap();
                if let Some(v) = &mut data.int_value_data {
                    v.value = match &value {
                        Value::Integer(i) => *i,
                        Value::Number(n) => *n as i64,
                        _ => v.value,
                    };
                    Some(v.changed.clone())
                } else if let Some(v) = &mut data.number_value_data {
                    v.value = match &value {
                        Value::Integer(i) => *i as f64,
                        Value::Number(n) => *n,
                        _ => v.value,
                    };
                    Some(v.changed.clone())
                } else if let Some(v) = &mut data.bool_value_data {
                    v.value = match &value {
                        Value::Boolean(b) => *b,
                        _ => v.value,
                    };
                    Some(v.changed.clone())
                } else if let Some(v) = &mut data.string_value_data {
                    if let Value::String(s) = &value {
                        if let Ok(text) = s.to_str() {
                            v.value = text.to_string();
                        }
                    }
                    Some(v.changed.clone())
                } else if let Some(v) = &mut data.object_value_data {
                    match &value {
                        Value::Nil => {
                            v.value = None;
                        }
                        Value::UserData(ud) => {
                            if let Ok(inst) = ud.borrow::<Instance>() {
                                v.value = Some(Arc::downgrade(&inst.data));
                            } else {
                                return Err(mlua::Error::runtime(
                                    "ObjectValue.Value expects Instance or nil",
                                ));
                            }
                        }
                        _ => {
                            return Err(mlua::Error::runtime(
                                "ObjectValue.Value expects Instance or nil",
                            ));
                        }
                    }
                    Some(v.changed.clone())
                } else if let Some(v) = &mut data.cframe_value_data {
                    match &value {
                        Value::UserData(ud) => {
                            if let Ok(cf) = ud.borrow::<CFrame>() {
                                v.value = cf.orthonormalized();
                            } else {
                                return Err(mlua::Error::runtime(
                                    "CFrameValue.Value expects CFrame",
                                ));
                            }
                        }
                        _ => {
                            return Err(mlua::Error::runtime("CFrameValue.Value expects CFrame"));
                        }
                    }
                    Some(v.changed.clone())
                } else {
                    None
                }
            };
            if let Some(signal) = changed_signal {
                let threads = signal.fire_as_coroutines(lua, MultiValue::from_iter([value]))?;
                crate::game::lua::events::track_yielded_threads(lua, threads)?;
            }
            Ok(())
        });

        fields.add_field_method_get("Health", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.health))
        });
        fields.add_field_method_set("Health", |lua, this, health: f32| {
            let (old_health, new_health, health_changed, died) = {
                let mut data = this.data.lock().unwrap();
                if let Some(humanoid) = &mut data.humanoid_data {
                    let old = humanoid.health;
                    humanoid.health = health.max(0.0).min(humanoid.max_health);
                    (
                        old,
                        humanoid.health,
                        humanoid.health_changed.clone(),
                        humanoid.died.clone(),
                    )
                } else {
                    return Ok(());
                }
            };

            if old_health != new_health {
                let threads = health_changed.fire_as_coroutines(
                    lua,
                    mlua::MultiValue::from_iter([Value::Number(new_health as f64)]),
                )?;
                crate::game::lua::events::track_yielded_threads(lua, threads)?;
                if new_health <= 0.0 && old_health > 0.0 {
                    let threads = died.fire_as_coroutines(lua, mlua::MultiValue::new())?;
                    crate::game::lua::events::track_yielded_threads(lua, threads)?;
                }
            }
            Ok(())
        });

        fields.add_field_method_get("MaxHealth", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.max_health))
        });
        fields.add_field_method_set("MaxHealth", |_, this, max_health: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(humanoid) = &mut data.humanoid_data {
                humanoid.max_health = max_health.max(0.0);
            }
            Ok(())
        });

        fields.add_field_method_get("WalkSpeed", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.walk_speed))
        });
        fields.add_field_method_set("WalkSpeed", |_, this, walk_speed: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(humanoid) = &mut data.humanoid_data {
                humanoid.walk_speed = walk_speed.max(0.0);
            }
            Ok(())
        });

        fields.add_field_method_get("JumpPower", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.jump_power))
        });
        fields.add_field_method_set("JumpPower", |_, this, jump_power: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(humanoid) = &mut data.humanoid_data {
                humanoid.jump_power = jump_power.max(0.0);
            }
            Ok(())
        });

        fields.add_field_method_get("JumpHeight", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.jump_height))
        });
        fields.add_field_method_set("JumpHeight", |_, this, jump_height: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(humanoid) = &mut data.humanoid_data {
                humanoid.jump_height = jump_height.max(0.0);
            }
            Ok(())
        });

        fields.add_field_method_get("Jump", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.jump_requested))
        });
        fields.add_field_method_set("Jump", |_, this, jump: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(humanoid) = &mut data.humanoid_data {
                // Roblox semantics: setting Jump true requests one jump.
                if jump {
                    humanoid.jump_requested = true;
                }
            }
            Ok(())
        });

        fields.add_field_method_get("AutoRotate", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.auto_rotate))
        });
        fields.add_field_method_set("AutoRotate", |_, this, auto_rotate: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(humanoid) = &mut data.humanoid_data {
                humanoid.auto_rotate = auto_rotate;
            }
            Ok(())
        });

        fields.add_field_method_get("HealthDisplayDistance", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .humanoid_data
                .as_ref()
                .map(|h| h.health_display_distance))
        });
        fields.add_field_method_set("HealthDisplayDistance", |_, this, value: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(humanoid) = &mut data.humanoid_data {
                humanoid.health_display_distance = value.max(0.0);
            }
            Ok(())
        });

        fields.add_field_method_get("NameDisplayDistance", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.name_display_distance))
        });
        fields.add_field_method_set("NameDisplayDistance", |_, this, value: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(humanoid) = &mut data.humanoid_data {
                humanoid.name_display_distance = value.max(0.0);
            }
            Ok(())
        });

        fields.add_field_method_get("HipHeight", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.hip_height))
        });
        fields.add_field_method_set("HipHeight", |_, this, hip_height: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(humanoid) = &mut data.humanoid_data {
                humanoid.hip_height = hip_height;
            }
            Ok(())
        });

        fields.add_field_method_get("MoveDirection", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.move_direction))
        });

        fields.add_field_method_get("Died", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.died.clone()))
        });

        fields.add_field_method_get("HealthChanged", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .humanoid_data
                .as_ref()
                .map(|h| h.health_changed.clone()))
        });

        fields.add_field_method_get("Running", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.running.clone()))
        });

        fields.add_field_method_get("MoveToFinished", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .humanoid_data
                .as_ref()
                .map(|h| h.move_to_finished.clone()))
        });

        fields.add_field_method_get("StateChanged", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.humanoid_data.as_ref().map(|h| h.state_changed.clone()))
        });

        fields.add_field_method_get("UserId", |_, this| {
            let data = this.data.lock().unwrap();
            // Return as f64 to ensure it's a Lua number type (not Integer) for table key compatibility
            Ok(data.player_data.as_ref().map(|p| p.user_id as f64))
        });

        fields.add_field_method_get("DisplayName", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.player_data.as_ref().map(|p| p.display_name.clone()))
        });

        fields.add_field_method_get("Character", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .player_data
                .as_ref()
                .and_then(|p| p.character.as_ref())
                .and_then(|w| w.upgrade())
                .map(Instance::from_ref))
        });
        fields.add_field_method_set("Character", |lua, this, character: Option<Instance>| {
            // Roblox parity: Player.Character is writable from Lua and emits
            // CharacterRemoving(old) followed by CharacterAdded(new) on transitions.
            let (old_character, new_character, character_added, character_removing) = {
                let mut data = this.data.lock().unwrap();
                let Some(player_data) = &mut data.player_data else {
                    return Ok(());
                };

                let old_character = player_data
                    .character
                    .as_ref()
                    .and_then(|w| w.upgrade())
                    .map(Instance::from_ref);
                let new_character = character.clone();

                player_data.character = character.as_ref().map(|c| Arc::downgrade(&c.data));

                (
                    old_character,
                    new_character,
                    player_data.character_added.clone(),
                    player_data.character_removing.clone(),
                )
            };

            let same_character = match (&old_character, &new_character) {
                (Some(old_char), Some(new_char)) => {
                    old_char.data.lock().unwrap().id == new_char.data.lock().unwrap().id
                }
                (None, None) => true,
                _ => false,
            };

            if same_character {
                return Ok(());
            }

            if let Some(old_char) = old_character {
                let threads = character_removing.fire_as_coroutines(
                    lua,
                    mlua::MultiValue::from_iter([Value::UserData(lua.create_userdata(old_char)?)]),
                )?;
                track_yielded_threads(lua, threads)?;
            }

            if let Some(new_char) = new_character {
                let threads = character_added.fire_as_coroutines(
                    lua,
                    mlua::MultiValue::from_iter([Value::UserData(lua.create_userdata(new_char)?)]),
                )?;
                track_yielded_threads(lua, threads)?;
            }

            Ok(())
        });

        fields.add_field_method_get("AnimationId", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.animation_data.as_ref().map(|a| a.animation_id.clone()))
        });
        fields.add_field_method_set("AnimationId", |_, this, animation_id: String| {
            let mut data = this.data.lock().unwrap();
            if let Some(animation) = &mut data.animation_data {
                animation.animation_id = animation_id;
            }
            Ok(())
        });

        // Sound properties
        fields.add_field_method_get("StreamId", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.sound_data.as_ref().map(|s| s.stream_id.clone()))
        });
        fields.add_field_method_set("StreamId", |_, this, stream_id: String| {
            let mut data = this.data.lock().unwrap();
            if let Some(sound) = &mut data.sound_data {
                sound.stream_id = stream_id;
            }
            Ok(())
        });
        fields.add_field_method_get("Volume", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.sound_data.as_ref().map(|s| s.volume))
        });
        fields.add_field_method_set("Volume", |_, this, volume: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(sound) = &mut data.sound_data {
                sound.volume = volume.clamp(0.0, 1.0);
            }
            Ok(())
        });
        fields.add_field_method_get("PlaybackSpeed", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.sound_data.as_ref().map(|s| s.playback_speed))
        });
        fields.add_field_method_set("PlaybackSpeed", |_, this, speed: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(sound) = &mut data.sound_data {
                sound.playback_speed = speed;
            }
            Ok(())
        });
        fields.add_field_method_get("IsPlaying", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.sound_data.as_ref().map(|s| s.is_playing))
        });
        fields.add_field_method_get("Ended", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.sound_data.as_ref().map(|s| s.ended.clone()))
        });
        fields.add_field_method_get("Stopped", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.sound_data.as_ref().map(|s| s.stopped.clone()))
        });

        fields.add_field_method_get("CharacterAdded", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.player_data.as_ref().map(|p| p.character_added.clone()))
        });

        fields.add_field_method_get("CharacterRemoving", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .player_data
                .as_ref()
                .map(|p| p.character_removing.clone()))
        });

        fields.add_field_method_get("PlayerGui", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .player_data
                .as_ref()
                .and_then(|p| p.player_gui.as_ref())
                .and_then(|w| w.upgrade())
                .map(Instance::from_ref))
        });

        fields.add_field_method_get("PrimaryPart", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .model_data
                .as_ref()
                .and_then(|m| m.primary_part.as_ref())
                .and_then(|w| w.upgrade())
                .map(Instance::from_ref))
        });
        fields.add_field_method_set("PrimaryPart", |_, this, part: Option<Instance>| {
            let mut data = this.data.lock().unwrap();
            if let Some(model) = &mut data.model_data {
                model.primary_part = part.map(|p| Arc::downgrade(&p.data));
            }
            Ok(())
        });

        // ========== Weld Properties ==========

        fields.add_field_method_get("Part0", |_, this| {
            let data = this.data.lock().unwrap();
            let from_weld = data
                .weld_data
                .as_ref()
                .and_then(|w| w.part0.as_ref())
                .and_then(|w| w.upgrade())
                .map(Instance::from_ref);
            if from_weld.is_some() {
                return Ok(from_weld);
            }
            Ok(data
                .weld_constraint_data
                .as_ref()
                .and_then(|w| w.part0.as_ref())
                .and_then(|w| w.upgrade())
                .map(Instance::from_ref))
        });
        fields.add_field_method_set("Part0", |_, this, part: Option<Instance>| {
            let mut data = this.data.lock().unwrap();
            let weak = part.map(|p| Arc::downgrade(&p.data));
            if let Some(weld) = &mut data.weld_data {
                weld.part0 = weak.clone();
            }
            if let Some(wc) = &mut data.weld_constraint_data {
                wc.part0 = weak;
                compute_weld_constraint_relative_cframe(&mut data);
            }
            Ok(())
        });

        fields.add_field_method_get("Part1", |_, this| {
            let data = this.data.lock().unwrap();
            let from_weld = data
                .weld_data
                .as_ref()
                .and_then(|w| w.part1.as_ref())
                .and_then(|w| w.upgrade())
                .map(Instance::from_ref);
            if from_weld.is_some() {
                return Ok(from_weld);
            }
            Ok(data
                .weld_constraint_data
                .as_ref()
                .and_then(|w| w.part1.as_ref())
                .and_then(|w| w.upgrade())
                .map(Instance::from_ref))
        });
        fields.add_field_method_set("Part1", |_, this, part: Option<Instance>| {
            let mut data = this.data.lock().unwrap();
            let weak = part.map(|p| Arc::downgrade(&p.data));
            if let Some(weld) = &mut data.weld_data {
                weld.part1 = weak.clone();
            }
            if let Some(wc) = &mut data.weld_constraint_data {
                wc.part1 = weak;
                compute_weld_constraint_relative_cframe(&mut data);
            }
            Ok(())
        });

        fields.add_field_method_get("C0", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.weld_data.as_ref().map(|w| w.c0))
        });
        fields.add_field_method_set("C0", |_, this, cframe: CFrame| {
            let mut data = this.data.lock().unwrap();
            if let Some(weld) = &mut data.weld_data {
                weld.c0 = cframe.orthonormalized();
            }
            Ok(())
        });

        fields.add_field_method_get("C1", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.weld_data.as_ref().map(|w| w.c1))
        });
        fields.add_field_method_set("C1", |_, this, cframe: CFrame| {
            let mut data = this.data.lock().unwrap();
            if let Some(weld) = &mut data.weld_data {
                weld.c1 = cframe.orthonormalized();
            }
            Ok(())
        });

        fields.add_field_method_get("Enabled", |_, this| {
            let data = this.data.lock().unwrap();
            // Return Enabled for Welds, WeldConstraints, BillboardGui, or existing gui_data.enabled
            if let Some(weld) = &data.weld_data {
                return Ok(Some(weld.enabled));
            }
            if let Some(wc) = &data.weld_constraint_data {
                return Ok(Some(wc.enabled));
            }
            if let Some(billboard) = &data.billboard_gui_data {
                return Ok(Some(billboard.enabled));
            }
            if let Some(gui) = &data.gui_data {
                return Ok(Some(gui.enabled));
            }
            Ok(None)
        });
        fields.add_field_method_set("Enabled", |_, this, enabled: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(weld) = &mut data.weld_data {
                weld.enabled = enabled;
            }
            if let Some(wc) = &mut data.weld_constraint_data {
                wc.enabled = enabled;
            }
            if let Some(billboard) = &mut data.billboard_gui_data {
                billboard.enabled = enabled;
            }
            if let Some(gui) = &mut data.gui_data {
                gui.enabled = enabled;
            }
            Ok(())
        });

        // ========== BillboardGui Properties ==========

        fields.add_field_method_get("StudsOffset", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.billboard_gui_data.as_ref().map(|b| b.studs_offset))
        });
        fields.add_field_method_set("StudsOffset", |_, this, offset: Vector3| {
            let mut data = this.data.lock().unwrap();
            if let Some(billboard) = &mut data.billboard_gui_data {
                billboard.studs_offset = offset;
            }
            Ok(())
        });

        fields.add_field_method_get("AlwaysOnTop", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.billboard_gui_data.as_ref().map(|b| b.always_on_top))
        });
        fields.add_field_method_set("AlwaysOnTop", |_, this, value: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(billboard) = &mut data.billboard_gui_data {
                billboard.always_on_top = value;
            }
            Ok(())
        });

        fields.add_field_method_get("Adornee", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .billboard_gui_data
                .as_ref()
                .and_then(|b| b.adornee.as_ref())
                .and_then(|w| w.upgrade())
                .map(Instance::from_ref))
        });
        fields.add_field_method_set("Adornee", |_, this, part: Option<Instance>| {
            let mut data = this.data.lock().unwrap();
            if let Some(billboard) = &mut data.billboard_gui_data {
                billboard.adornee = part.map(|p| Arc::downgrade(&p.data));
            }
            Ok(())
        });

        // ========== GUI Properties ==========

        // AnchorPoint (GuiObject)
        fields.add_field_method_get("AnchorPoint", |lua, this| {
            let data = this.data.lock().unwrap();
            if let Some(gui) = &data.gui_data {
                let table = lua.create_table()?;
                table.set("X", gui.anchor_point.0)?;
                table.set("Y", gui.anchor_point.1)?;
                Ok(Value::Table(table))
            } else {
                Ok(Value::Nil)
            }
        });
        fields.add_field_method_set("AnchorPoint", |_, this, value: Value| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                if let Value::Table(t) = value {
                    let x: f32 = t.get("X").unwrap_or(0.0);
                    let y: f32 = t.get("Y").unwrap_or(0.0);
                    gui.anchor_point = (x, y);
                }
            }
            Ok(())
        });

        // Rotation (GuiObject) - in degrees
        fields.add_field_method_get("Rotation", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.rotation))
        });
        fields.add_field_method_set("Rotation", |_, this, rotation: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.rotation = rotation;
            }
            Ok(())
        });

        // ZIndex (GuiObject)
        fields.add_field_method_get("ZIndex", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.z_index))
        });
        fields.add_field_method_set("ZIndex", |_, this, z_index: i32| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.z_index = z_index;
            }
            Ok(())
        });

        // LayoutOrder (GuiObject)
        fields.add_field_method_get("LayoutOrder", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.layout_order))
        });
        fields.add_field_method_set("LayoutOrder", |_, this, layout_order: i32| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.layout_order = layout_order;
            }
            Ok(())
        });

        // Visible (GuiObject)
        fields.add_field_method_get("Visible", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.visible))
        });
        fields.add_field_method_set("Visible", |_, this, visible: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.visible = visible;
            }
            Ok(())
        });

        // BackgroundColor3 (GuiObject)
        fields.add_field_method_get("BackgroundColor3", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.background_color))
        });
        fields.add_field_method_set("BackgroundColor3", |_, this, color: Color3| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.background_color = color;
            }
            Ok(())
        });

        // BackgroundTransparency (GuiObject)
        fields.add_field_method_get("BackgroundTransparency", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.background_transparency))
        });
        fields.add_field_method_set("BackgroundTransparency", |_, this, transparency: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.background_transparency = transparency.clamp(0.0, 1.0);
            }
            Ok(())
        });

        // BorderColor3 (GuiObject)
        fields.add_field_method_get("BorderColor3", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.border_color))
        });
        fields.add_field_method_set("BorderColor3", |_, this, color: Color3| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.border_color = color;
            }
            Ok(())
        });

        // BorderSizePixel (GuiObject)
        fields.add_field_method_get("BorderSizePixel", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.border_size_pixel))
        });
        fields.add_field_method_set("BorderSizePixel", |_, this, size: i32| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.border_size_pixel = size.max(0);
            }
            Ok(())
        });

        // Text (TextLabel, TextButton)
        fields.add_field_method_get("Text", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().and_then(|g| g.text.clone()))
        });
        fields.add_field_method_set("Text", |_, this, text: String| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.text = Some(text);
            }
            Ok(())
        });

        // TextColor3 (TextLabel, TextButton)
        fields.add_field_method_get("TextColor3", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().and_then(|g| g.text_color))
        });
        fields.add_field_method_set("TextColor3", |_, this, color: Color3| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.text_color = Some(color);
            }
            Ok(())
        });

        // TextSize (TextLabel, TextButton)
        fields.add_field_method_get("TextSize", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().and_then(|g| g.text_size))
        });
        fields.add_field_method_set("TextSize", |_, this, size: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.text_size = Some(size.max(1.0));
            }
            Ok(())
        });

        // TextTransparency (TextLabel, TextButton)
        fields.add_field_method_get("TextTransparency", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().and_then(|g| g.text_transparency))
        });
        fields.add_field_method_set("TextTransparency", |_, this, transparency: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.text_transparency = Some(transparency.clamp(0.0, 1.0));
            }
            Ok(())
        });

        // TextStrokeTransparency (TextLabel, TextButton)
        fields.add_field_method_get("TextStrokeTransparency", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .gui_data
                .as_ref()
                .and_then(|g| g.text_stroke_transparency))
        });
        fields.add_field_method_set("TextStrokeTransparency", |_, this, transparency: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.text_stroke_transparency = Some(transparency.clamp(0.0, 1.0));
            }
            Ok(())
        });

        // Font (TextLabel, TextButton)
        fields.add_field_method_get("Font", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().and_then(|g| g.font.clone()))
        });
        fields.add_field_method_set("Font", |_, this, font: Value| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                if let Value::String(s) = font {
                    if let Ok(font_name) = s.to_str() {
                        gui.font = Some(font_name.to_string());
                    }
                }
            }
            Ok(())
        });

        // TextScaled (TextLabel, TextButton)
        fields.add_field_method_get("TextScaled", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.text_scaled))
        });
        fields.add_field_method_set("TextScaled", |_, this, scaled: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.text_scaled = scaled;
            }
            Ok(())
        });

        // TextXAlignment (TextLabel, TextButton)
        fields.add_field_method_get("TextXAlignment", |lua, this| {
            let data = this.data.lock().unwrap();
            if let Some(gui) = &data.gui_data {
                let alignment = match gui.text_x_alignment {
                    TextXAlignment::Left => "Left",
                    TextXAlignment::Center => "Center",
                    TextXAlignment::Right => "Right",
                };
                Ok(Value::String(lua.create_string(alignment)?))
            } else {
                Ok(Value::Nil)
            }
        });
        fields.add_field_method_set("TextXAlignment", |_, this, alignment: Value| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                if let Value::String(s) = alignment {
                    if let Ok(str_val) = s.to_str() {
                        gui.text_x_alignment = match str_val.as_ref() {
                            "Left" => TextXAlignment::Left,
                            "Right" => TextXAlignment::Right,
                            _ => TextXAlignment::Center,
                        };
                    }
                }
            }
            Ok(())
        });

        // TextYAlignment (TextLabel, TextButton)
        fields.add_field_method_get("TextYAlignment", |lua, this| {
            let data = this.data.lock().unwrap();
            if let Some(gui) = &data.gui_data {
                let alignment = match gui.text_y_alignment {
                    TextYAlignment::Top => "Top",
                    TextYAlignment::Center => "Center",
                    TextYAlignment::Bottom => "Bottom",
                };
                Ok(Value::String(lua.create_string(alignment)?))
            } else {
                Ok(Value::Nil)
            }
        });
        fields.add_field_method_set("TextYAlignment", |_, this, alignment: Value| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                if let Value::String(s) = alignment {
                    if let Ok(str_val) = s.to_str() {
                        gui.text_y_alignment = match str_val.as_ref() {
                            "Top" => TextYAlignment::Top,
                            "Bottom" => TextYAlignment::Bottom,
                            _ => TextYAlignment::Center,
                        };
                    }
                }
            }
            Ok(())
        });

        // Image (ImageLabel, ImageButton)
        fields.add_field_method_get("Image", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().and_then(|g| g.image.clone()))
        });
        fields.add_field_method_set("Image", |_, this, image: String| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.image = Some(image);
            }
            Ok(())
        });

        // ImageColor3 (ImageLabel, ImageButton)
        fields.add_field_method_get("ImageColor3", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().and_then(|g| g.image_color))
        });
        fields.add_field_method_set("ImageColor3", |_, this, color: Color3| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.image_color = Some(color);
            }
            Ok(())
        });

        // ImageTransparency (ImageLabel, ImageButton)
        fields.add_field_method_get("ImageTransparency", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().and_then(|g| g.image_transparency))
        });
        fields.add_field_method_set("ImageTransparency", |_, this, transparency: f32| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.image_transparency = Some(transparency.clamp(0.0, 1.0));
            }
            Ok(())
        });

        // DisplayOrder (ScreenGui)
        fields.add_field_method_get("DisplayOrder", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.display_order))
        });
        fields.add_field_method_set("DisplayOrder", |_, this, order: i32| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.display_order = order;
            }
            Ok(())
        });

        // IgnoreGuiInset (ScreenGui)
        fields.add_field_method_get("IgnoreGuiInset", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.ignore_gui_inset))
        });
        fields.add_field_method_set("IgnoreGuiInset", |_, this, ignore: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.ignore_gui_inset = ignore;
            }
            Ok(())
        });

        // ResetOnSpawn (ScreenGui)
        fields.add_field_method_get("ResetOnSpawn", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().map(|g| g.reset_on_spawn))
        });
        fields.add_field_method_set("ResetOnSpawn", |_, this, reset_on_spawn: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(gui) = &mut data.gui_data {
                gui.reset_on_spawn = reset_on_spawn;
            }
            Ok(())
        });

        // CornerRadius (UICorner)
        fields.add_field_method_get("CornerRadius", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.ui_corner_data.as_ref().map(|c| c.corner_radius))
        });
        fields.add_field_method_set("CornerRadius", |_, this, corner_radius: UDim| {
            let mut data = this.data.lock().unwrap();
            if let Some(ui_corner) = &mut data.ui_corner_data {
                ui_corner.corner_radius = corner_radius;
            }
            Ok(())
        });

        // MouseButton1Click (GuiButton)
        fields.add_field_method_get("MouseButton1Click", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .gui_data
                .as_ref()
                .and_then(|g| g.mouse_button1_click.clone()))
        });

        // MouseButton1Down (GuiButton)
        fields.add_field_method_get("MouseButton1Down", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .gui_data
                .as_ref()
                .and_then(|g| g.mouse_button1_down.clone()))
        });

        // MouseButton1Up (GuiButton)
        fields.add_field_method_get("MouseButton1Up", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data
                .gui_data
                .as_ref()
                .and_then(|g| g.mouse_button1_up.clone()))
        });

        // MouseEnter (GuiButton)
        fields.add_field_method_get("MouseEnter", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().and_then(|g| g.mouse_enter.clone()))
        });

        // MouseLeave (GuiButton)
        fields.add_field_method_get("MouseLeave", |_, this| {
            let data = this.data.lock().unwrap();
            Ok(data.gui_data.as_ref().and_then(|g| g.mouse_leave.clone()))
        });
    
}
