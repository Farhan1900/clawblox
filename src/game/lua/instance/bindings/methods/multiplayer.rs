use mlua::{MultiValue, UserDataMethods, Value};
use std::sync::Arc;

use crate::game::lua::runtime::Game;
use crate::game::lua::types::{Color3, PartType, Vector3};

use super::super::super::*;

pub(super) fn add_methods<M: UserDataMethods<Instance>>(methods: &mut M) {
    methods.add_method("LoadCharacter", |lua, this, ()| {
        let (player_name, old_character, character_added, character_removing) = {
            let data = this.data.lock().unwrap();
            let Some(pdata) = data.player_data.as_ref() else {
                return Ok(());
            };
            (
                data.name.clone(),
                pdata
                    .character
                    .as_ref()
                    .and_then(|w| w.upgrade())
                    .map(Instance::from_ref),
                pdata.character_added.clone(),
                pdata.character_removing.clone(),
            )
        };

        if let Some(old_char) = old_character {
            let threads = character_removing.fire_as_coroutines(
                lua,
                mlua::MultiValue::from_iter([Instance::cached_userdata_value(lua, &old_char)?]),
            )?;
            crate::game::lua::events::track_yielded_threads(lua, threads)?;
            old_char.set_parent(None);
        }

        let character = Instance::from_data(InstanceData::new_model(&player_name));

        let mut hrp_data = InstanceData::new_part("HumanoidRootPart");
        if let Some(part) = &mut hrp_data.part_data {
            part.size = Vector3::new(2.0, 5.0, 2.0);
            part.position = Vector3::new(0.0, 6.0, 0.0);
            part.anchored = false;
            part.shape = PartType::Cylinder;
            part.color = Color3::new(0.9, 0.45, 0.3);
        }
        hrp_data.attributes.insert(
            "ModelUrl".to_string(),
            AttributeValue::String("asset://player.glb".to_string()),
        );
        let hrp = Instance::from_data(hrp_data);
        hrp.set_parent(Some(&character));

        let humanoid = Instance::from_data(InstanceData::new_humanoid("Humanoid"));
        humanoid.set_parent(Some(&character));

        {
            let mut char_data = character.data.lock().unwrap();
            if let Some(model) = &mut char_data.model_data {
                model.primary_part = Some(Arc::downgrade(&hrp.data));
            }
        }

        {
            let mut player_data = this.data.lock().unwrap();
            if let Some(pdata) = &mut player_data.player_data {
                pdata.character = Some(Arc::downgrade(&character.data));
            }
        }

        let game_ud: mlua::AnyUserData = lua.globals().get("__clawblox_game")?;
        let game_ref = game_ud.borrow::<Game>()?;
        game_ref.workspace().add_child(character.clone());

        let threads = character_added.fire_as_coroutines(
            lua,
            mlua::MultiValue::from_iter([Instance::cached_userdata_value(lua, &character)?]),
        )?;
        crate::game::lua::events::track_yielded_threads(lua, threads)?;

        Ok(())
    });

    methods.add_method("Kick", |lua, this, message: Option<String>| {
        // Get user_id from this player instance
        let user_id = {
            let data = this.data.lock().unwrap();
            data.player_data.as_ref().map(|p| p.user_id)
        };

        if let Some(user_id) = user_id {
            // Get game reference from Lua globals and queue kick request
            let game_ud: mlua::Result<mlua::AnyUserData> = lua.globals().get("__clawblox_game");
            if let Ok(ud) = game_ud {
                if let Ok(game) = ud.borrow::<Game>() {
                    game.queue_kick(user_id, message.clone());
                    if let Some(msg) = &message {
                        eprintln!("[Player] Kick requested for user_id={}: {}", user_id, msg);
                    } else {
                        eprintln!("[Player] Kick requested for user_id={}", user_id);
                    }
                } else {
                    eprintln!("[Player] Kick failed: could not borrow Game from UserData");
                }
            } else {
                eprintln!("[Player] Kick failed: game reference not found in Lua globals");
            }
        }
        Ok(())
    });

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
