use mlua::{UserDataFields, Value};
use std::sync::Arc;

use crate::game::lua::events::track_yielded_threads;

use super::super::super::*;

pub(super) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
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
}
