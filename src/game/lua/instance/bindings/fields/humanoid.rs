use mlua::{UserDataFields, Value};

use super::super::super::*;

pub(super) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
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
}
