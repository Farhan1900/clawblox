use mlua::{UserDataMethods, Value};

use crate::game::lua::types::{HumanoidStateType, Vector3};

use super::super::super::*;

pub(super) fn add_methods<M: UserDataMethods<Instance>>(methods: &mut M) {
    methods.add_method("TakeDamage", |lua, this, amount: f32| {
        let (old_health, new_health, health_changed, died) = {
            let mut data = this.data.lock().unwrap();
            if let Some(humanoid) = &mut data.humanoid_data {
                let old = humanoid.health;
                humanoid.health = (humanoid.health - amount).max(0.0);
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

    methods.add_method(
        "Move",
        |_, this, (direction, _relative): (Vector3, Option<bool>)| {
            let mut data = this.data.lock().unwrap();
            if let Some(humanoid) = &mut data.humanoid_data {
                humanoid.move_input_direction = direction;
            }
            Ok(())
        },
    );

    methods.add_method(
        "MoveTo",
        |_, this, (position, _part): (Vector3, Option<Instance>)| {
            let mut data = this.data.lock().unwrap();
            let id = data.id.0;
            if let Some(humanoid) = &mut data.humanoid_data {
                let _ = id;
                humanoid.move_to_target = Some(position);
                humanoid.cancel_move_to = false;
            } else {
                // No-op: MoveTo on non-humanoid instance.
            }
            Ok(())
        },
    );

    methods.add_method("CancelMoveTo", |_, this, ()| {
        let mut data = this.data.lock().unwrap();
        if let Some(humanoid) = &mut data.humanoid_data {
            humanoid.move_to_target = None;
            humanoid.cancel_move_to = true; // Signal to clear physics target
        }
        Ok(())
    });

    methods.add_method("Jump", |_, this, ()| {
        let mut data = this.data.lock().unwrap();
        if let Some(humanoid) = &mut data.humanoid_data {
            humanoid.jump_requested = true;
        }
        Ok(())
    });

    methods.add_method("GetState", |_, this, ()| {
        let data = this.data.lock().unwrap();
        Ok(data.humanoid_data.as_ref().map(|h| h.state))
    });

    methods.add_method("ChangeState", |_, this, state: HumanoidStateType| {
        let mut data = this.data.lock().unwrap();
        if let Some(humanoid) = &mut data.humanoid_data {
            humanoid.state = state;
        }
        Ok(())
    });
}
