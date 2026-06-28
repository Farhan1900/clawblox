use mlua::{UserDataMethods, Value};

use crate::game::lua::runtime::{Game, PhysicsCommand};
use crate::game::lua::types::{CFrame, Vector3};

use super::super::super::*;

pub(super) fn add_methods<M: UserDataMethods<Instance>>(methods: &mut M) {
    methods.add_method("GetPrimaryPartCFrame", |_, this, ()| {
        Ok(this.get_pivot_cframe())
    });

    methods.add_method("SetPrimaryPartCFrame", |_, this, cframe: CFrame| {
        this.pivot_to_cframe(cframe);
        Ok(())
    });

    methods.add_method("GetPivot", |_, this, ()| {
        if let Some(pivot) = this.get_pivot_cframe() {
            Ok(pivot)
        } else {
            Err(mlua::Error::runtime(
                "GetPivot is only supported on PVInstance",
            ))
        }
    });

    methods.add_method("PivotTo", |_, this, cframe: CFrame| {
        if this.data.lock().unwrap().part_data.is_none()
            && this.data.lock().unwrap().model_data.is_none()
        {
            return Err(mlua::Error::runtime(
                "PivotTo is only supported on PVInstance",
            ));
        }
        this.pivot_to_cframe(cframe);
        Ok(())
    });

    methods.add_method(
        "GetConnectedParts",
        |_, this, recursive: Option<bool>| -> mlua::Result<Vec<Instance>> {
            if this.data.lock().unwrap().part_data.is_none() {
                return Err(mlua::Error::runtime(
                    "GetConnectedParts is only supported on BasePart",
                ));
            }
            Ok(this.connected_parts_from_graph(recursive.unwrap_or(false)))
        },
    );

    methods.add_method("GetMass", |_, this, ()| {
        let data = this.data.lock().unwrap();
        let Some(part) = &data.part_data else {
            return Err(mlua::Error::runtime(
                "GetMass is only supported on BasePart",
            ));
        };

        Ok(part.mass())
    });

    methods.add_method("ApplyImpulse", |lua, this, impulse: Vector3| {
        if this.data.lock().unwrap().part_data.is_none() {
            return Err(mlua::Error::runtime(
                "ApplyImpulse is only supported on BasePart",
            ));
        }
        let game_ud: mlua::AnyUserData = lua.globals().get("__clawblox_game")?;
        let game_ref = game_ud.borrow::<Game>()?;
        game_ref.queue_physics_command(PhysicsCommand::ApplyImpulse {
            part_id: this.id().0,
            impulse,
        });
        Ok(())
    });

    methods.add_method(
        "ApplyImpulseAtPosition",
        |lua, this, (impulse, position): (Vector3, Vector3)| {
            if this.data.lock().unwrap().part_data.is_none() {
                return Err(mlua::Error::runtime(
                    "ApplyImpulseAtPosition is only supported on BasePart",
                ));
            }
            let game_ud: mlua::AnyUserData = lua.globals().get("__clawblox_game")?;
            let game_ref = game_ud.borrow::<Game>()?;
            game_ref.queue_physics_command(PhysicsCommand::ApplyImpulseAtPosition {
                part_id: this.id().0,
                impulse,
                position,
            });
            Ok(())
        },
    );

    methods.add_method("ApplyAngularImpulse", |lua, this, impulse: Vector3| {
        if this.data.lock().unwrap().part_data.is_none() {
            return Err(mlua::Error::runtime(
                "ApplyAngularImpulse is only supported on BasePart",
            ));
        }
        let game_ud: mlua::AnyUserData = lua.globals().get("__clawblox_game")?;
        let game_ref = game_ud.borrow::<Game>()?;
        game_ref.queue_physics_command(PhysicsCommand::ApplyAngularImpulse {
            part_id: this.id().0,
            impulse,
        });
        Ok(())
    });

    methods.add_method("SetNetworkOwner", |_, this, _owner: Value| {
        if this.data.lock().unwrap().part_data.is_none() {
            return Err(mlua::Error::runtime(
                "SetNetworkOwner is only supported on BasePart",
            ));
        }
        // Roblox parity: server scripts often force server ownership with nil.
        // Clawblox runs authoritative server simulation, so ownership is a no-op.
        Ok(())
    });

    methods.add_method(
        "SetNetworkOwnershipAuto",
        |_, this, _enabled: Option<bool>| {
            if this.data.lock().unwrap().part_data.is_none() {
                return Err(mlua::Error::runtime(
                    "SetNetworkOwnershipAuto is only supported on BasePart",
                ));
            }
            // Same parity rationale as SetNetworkOwner: network ownership is not modeled yet.
            Ok(())
        },
    );
}
