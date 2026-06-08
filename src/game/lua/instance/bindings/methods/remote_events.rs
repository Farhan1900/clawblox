use mlua::{MultiValue, UserDataMethods, Value};

use crate::game::lua::runtime::Game;

use super::super::super::*;

pub(super) fn add_methods<M: UserDataMethods<Instance>>(methods: &mut M) {
    methods.add_method("FireAllClients", |lua, this, args: MultiValue| {
        if this.class_name() != ClassName::RemoteEvent {
            return Ok(());
        }

        let args_vec: Vec<Value> = args.into_iter().collect();
        let payload =
            crate::game::lua::services::remote_event::lua_args_to_payload(lua, &args_vec)?;
        let game_ud: mlua::AnyUserData = lua.globals().get("__clawblox_game")?;
        let game = game_ud.borrow::<Game>()?;
        game.remote_event_service()
            .fire_all_clients(this.name(), payload, true);
        Ok(())
    });

    methods.add_method("FireClient", |lua, this, args: MultiValue| {
        if this.class_name() != ClassName::RemoteEvent {
            return Ok(());
        }

        let mut iter = args.into_iter();
        let Some(player_value) = iter.next() else {
            return Ok(());
        };
        let player: Instance = mlua::FromLua::from_lua(player_value, lua)?;
        let target_user_id = {
            let data = player.data.lock().unwrap();
            data.player_data.as_ref().map(|p| p.user_id)
        };
        let Some(target_user_id) = target_user_id else {
            return Ok(());
        };

        let rest: Vec<Value> = iter.collect();
        let payload = crate::game::lua::services::remote_event::lua_args_to_payload(lua, &rest)?;
        let game_ud: mlua::AnyUserData = lua.globals().get("__clawblox_game")?;
        let game = game_ud.borrow::<Game>()?;
        game.remote_event_service()
            .fire_client(this.name(), payload, true, target_user_id);
        Ok(())
    });
}
