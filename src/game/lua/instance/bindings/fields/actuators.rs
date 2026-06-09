use mlua::{UserDataFields, Value};
use std::sync::Arc;

use crate::game::lua::types::{ActuatorRelativeTo, Vector3};

use super::super::super::*;

pub(super) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
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
                return Err(mlua::Error::runtime(
                    "BodyVelocity.MaxForce expects Vector3",
                ));
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

    fields.add_field_method_get("P", |_, this| {
        let data = this.data.lock().unwrap();
        if let Some(bv) = &data.body_velocity_data {
            return Ok(Some(bv.p));
        }
        if let Some(bav) = &data.body_angular_velocity_data {
            return Ok(Some(bav.p));
        }
        Ok(None)
    });
    fields.add_field_method_set("P", |_, this, value: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(bv) = &mut data.body_velocity_data {
            bv.p = value;
        }
        if let Some(bav) = &mut data.body_angular_velocity_data {
            bav.p = value;
        }
        Ok(())
    });

    fields.add_field_method_get("Force", |_, this| {
        let data = this.data.lock().unwrap();
        if let Some(bf) = &data.body_force_data {
            return Ok(Some(bf.force));
        }
        Ok(data.vector_force_data.as_ref().map(|vf| vf.force))
    });
    fields.add_field_method_set("Force", |_, this, force: Vector3| {
        let mut data = this.data.lock().unwrap();
        if let Some(bf) = &mut data.body_force_data {
            bf.force = force;
        }
        if let Some(vf) = &mut data.vector_force_data {
            vf.force = force;
        }
        Ok(())
    });

    fields.add_field_method_get("RelativeTo", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.vector_force_data.as_ref().map(|vf| vf.relative_to))
    });
    fields.add_field_method_set("RelativeTo", |_, this, relative_to: ActuatorRelativeTo| {
        let mut data = this.data.lock().unwrap();
        if let Some(vf) = &mut data.vector_force_data {
            vf.relative_to = relative_to;
        }
        Ok(())
    });

    fields.add_field_method_get("ApplyAtCenterOfMass", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .vector_force_data
            .as_ref()
            .map(|vf| vf.apply_at_center_of_mass))
    });
    fields.add_field_method_set("ApplyAtCenterOfMass", |_, this, enabled: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(vf) = &mut data.vector_force_data {
            vf.apply_at_center_of_mass = enabled;
        }
        Ok(())
    });

    fields.add_field_method_get("Enabled", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.vector_force_data.as_ref().map(|vf| vf.enabled))
    });
    fields.add_field_method_set("Enabled", |_, this, enabled: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(vf) = &mut data.vector_force_data {
            vf.enabled = enabled;
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
            .vector_force_data
            .as_ref()
            .and_then(|a| a.attachment0.as_ref())
            .or_else(|| {
                data.align_position_data
                    .as_ref()
                    .and_then(|a| a.attachment0.as_ref())
            })
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
        if let Some(vector_force) = &mut data.vector_force_data {
            vector_force.attachment0 = attachment.as_ref().map(|a| Arc::downgrade(&a.data));
        }
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
            .vector_force_data
            .as_ref()
            .and_then(|a| a.attachment1.as_ref())
            .or_else(|| {
                data.align_position_data
                    .as_ref()
                    .and_then(|a| a.attachment1.as_ref())
            })
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
        if let Some(vector_force) = &mut data.vector_force_data {
            vector_force.attachment1 = attachment.as_ref().map(|a| Arc::downgrade(&a.data));
        }
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
}
