use mlua::{MultiValue, UserDataMethods, Value};
use std::sync::Arc;

use crate::game::lua::events::track_yielded_threads;
use crate::game::lua::runtime::{Game, PhysicsCommand};
use crate::game::lua::types::{CFrame, Color3, HumanoidStateType, PartType, Vector3};

use super::super::*;

pub(crate) fn add_methods<M: UserDataMethods<Instance>>(methods: &mut M) {
        methods.add_method("Clone", |_, this, ()| Ok(this.clone_instance()));

        methods.add_method("Destroy", |lua, this, ()| this.destroy(lua));

        methods.add_method("ClearAllChildren", |lua, this, ()| {
            // Roblox parity: ClearAllChildren destroys all direct children.
            // `destroy` recursively destroys descendants and updates parent/child signals.
            for child in this.get_children() {
                child.destroy(lua)?;
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
            let payload =
                crate::game::lua::services::remote_event::lua_args_to_payload(lua, &rest)?;
            let game_ud: mlua::AnyUserData = lua.globals().get("__clawblox_game")?;
            let game = game_ud.borrow::<Game>()?;
            game.remote_event_service()
                .fire_client(this.name(), payload, true, target_user_id);
            Ok(())
        });

        methods.add_method(
            "FindFirstChild",
            |_, this, (name, recursive): (String, Option<bool>)| {
                Ok(this.find_first_child(&name, recursive.unwrap_or(false)))
            },
        );

        methods.add_method("FindFirstChildOfClass", |_, this, class_name: String| {
            Ok(this.find_first_child_of_class(&class_name))
        });

        methods.add_method(
            "FindFirstChildWhichIsA",
            |_, this, (class_name, recursive): (String, Option<bool>)| {
                if recursive.unwrap_or(false) {
                    Ok(this
                        .get_descendants()
                        .into_iter()
                        .find(|child| child.is_a(&class_name)))
                } else {
                    Ok(this
                        .get_children()
                        .into_iter()
                        .find(|child| child.is_a(&class_name)))
                }
            },
        );

        methods.add_method("GetChildren", |_, this, ()| Ok(this.get_children()));

        methods.add_method("GetDescendants", |_, this, ()| Ok(this.get_descendants()));

        methods.add_method("GetDebugId", |_, this, _scope_length: Option<u32>| {
            Ok(format!("RBX{:016x}", this.id().0))
        });

        methods.add_method("IsA", |_, this, class_name: String| {
            Ok(this.is_a(&class_name))
        });

        methods.add_method("IsDescendantOf", |_, this, ancestor: Value| {
            let ancestor = match ancestor {
                Value::UserData(ud) => match ud.borrow::<Instance>() {
                    Ok(inst) => inst.clone(),
                    Err(_) => return Ok(false),
                },
                _ => return Ok(false),
            };
            Ok(this.is_descendant_of(&ancestor))
        });

        methods.add_method(
            "SetAttribute",
            |lua, this, (name, value): (String, Value)| {
                fn lua_value_to_attr(value: &Value) -> mlua::Result<AttributeValue> {
                    match value {
                        Value::Nil => Ok(AttributeValue::Nil),
                        Value::Boolean(b) => Ok(AttributeValue::Bool(*b)),
                        Value::Integer(n) => Ok(AttributeValue::Number(*n as f64)),
                        Value::Number(n) => Ok(AttributeValue::Number(*n)),
                        Value::String(s) => Ok(AttributeValue::String(s.to_str()?.to_string())),
                        Value::UserData(ud) => {
                            if let Ok(v) = ud.borrow::<Vector3>() {
                                Ok(AttributeValue::Vector3(*v))
                            } else if let Ok(c) = ud.borrow::<Color3>() {
                                Ok(AttributeValue::Color3(*c))
                            } else {
                                Err(mlua::Error::runtime("Unsupported attribute type"))
                            }
                        }
                        Value::Table(t) => {
                            let mut items = Vec::new();
                            for pair in t.clone().sequence_values::<Value>() {
                                items.push(lua_value_to_attr(&pair?)?);
                            }
                            Ok(AttributeValue::Table(items))
                        }
                        _ => Err(mlua::Error::runtime("Unsupported attribute type")),
                    }
                }
                let attr_value = lua_value_to_attr(&value)?;
                this.set_attribute_with_lua(lua, &name, attr_value)?;
                Ok(())
            },
        );

        methods.add_method("GetAttributeChangedSignal", |_, this, name: String| {
            Ok(this.ensure_attribute_signal(&name))
        });

        methods.add_method("GetPropertyChangedSignal", |_, this, name: String| {
            Ok(this.ensure_property_signal(&name))
        });

        methods.add_async_method(
            "WaitForChild",
            |lua, this, (name, timeout): (String, Option<f64>)| {
                let this = this.clone();
                async move {
                    let start = std::time::Instant::now();
                    let timeout = timeout.map(|t| t.max(0.0));
                    let name_for_poll = name.clone();

                    let child = futures_util::future::poll_fn(move |_cx| {
                        if let Some(child) = this.find_first_child(&name_for_poll, false) {
                            return std::task::Poll::Ready(Some(child));
                        }

                        if let Some(t) = timeout {
                            if start.elapsed().as_secs_f64() >= t {
                                return std::task::Poll::Ready(None);
                            }
                        }

                        std::task::Poll::Pending
                    })
                    .await;

                    match child {
                        Some(child) => Ok(Value::UserData(lua.create_userdata(child)?)),
                        None => Ok(Value::Nil),
                    }
                }
            },
        );

        methods.add_method("GetAttribute", |lua, this, name: String| {
            fn attr_to_lua(lua: &mlua::Lua, attr: AttributeValue) -> mlua::Result<Value> {
                match attr {
                    AttributeValue::Nil => Ok(Value::Nil),
                    AttributeValue::Bool(b) => Ok(Value::Boolean(b)),
                    AttributeValue::Number(n) => Ok(Value::Number(n)),
                    AttributeValue::String(s) => Ok(Value::String(lua.create_string(&s)?)),
                    AttributeValue::Vector3(v) => Ok(Value::UserData(lua.create_userdata(v)?)),
                    AttributeValue::Color3(c) => Ok(Value::UserData(lua.create_userdata(c)?)),
                    AttributeValue::Table(items) => {
                        let t = lua.create_table()?;
                        for (i, item) in items.into_iter().enumerate() {
                            t.set(i + 1, attr_to_lua(lua, item)?)?;
                        }
                        Ok(Value::Table(t))
                    }
                }
            }
            match this.get_attribute(&name) {
                Some(attr) => attr_to_lua(lua, attr),
                None => Ok(Value::Nil),
            }
        });

        methods.add_method("GetAttributes", |lua, this, ()| {
            fn attr_to_lua_val(lua: &mlua::Lua, attr: AttributeValue) -> mlua::Result<Value> {
                match attr {
                    AttributeValue::Nil => Ok(Value::Nil),
                    AttributeValue::Bool(b) => Ok(Value::Boolean(b)),
                    AttributeValue::Number(n) => Ok(Value::Number(n)),
                    AttributeValue::String(s) => Ok(Value::String(lua.create_string(&s)?)),
                    AttributeValue::Vector3(v) => Ok(Value::UserData(lua.create_userdata(v)?)),
                    AttributeValue::Color3(c) => Ok(Value::UserData(lua.create_userdata(c)?)),
                    AttributeValue::Table(items) => {
                        let t = lua.create_table()?;
                        for (i, item) in items.into_iter().enumerate() {
                            t.set(i + 1, attr_to_lua_val(lua, item)?)?;
                        }
                        Ok(Value::Table(t))
                    }
                }
            }
            let table = lua.create_table()?;
            for (key, value) in this.get_attributes() {
                table.set(key, attr_to_lua_val(lua, value)?)?;
            }
            Ok(table)
        });

        // Tag methods (Roblox CollectionService-style tags)
        methods.add_method("AddTag", |_, this, tag: String| {
            this.add_tag(&tag);
            Ok(())
        });

        methods.add_method("HasTag", |_, this, tag: String| Ok(this.has_tag(&tag)));

        methods.add_method("RemoveTag", |_, this, tag: String| {
            this.remove_tag(&tag);
            Ok(())
        });

        methods.add_method("GetTags", |lua, this, ()| {
            let tags = this.get_tags();
            let table = lua.create_table()?;
            for (i, tag) in tags.iter().enumerate() {
                table.set(i + 1, tag.clone())?;
            }
            Ok(table)
        });

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

        methods.add_method("LoadAnimation", |lua, this, animation: Instance| {
            let class_name = this.class_name();
            if class_name == ClassName::Animator {
                return this.load_animation_track_for_animator(lua, &animation);
            }
            if class_name == ClassName::Humanoid {
                let animator = this.ensure_animator_child_for_humanoid();
                return animator.load_animation_track_for_animator(lua, &animation);
            }
            Err(mlua::Error::runtime(
                "LoadAnimation is supported on Humanoid and Animator",
            ))
        });

        methods.add_method("GetPlayingAnimationTracks", |lua, this, ()| {
            if this.class_name() != ClassName::Animator {
                return Err(mlua::Error::runtime(
                    "GetPlayingAnimationTracks is supported on Animator",
                ));
            }
            let animator_id = this.id().0;
            let tracks = lua
                .app_data_ref::<AnimationScheduler>()
                .map(|scheduler| scheduler.playing_tracks_for_animator(animator_id))
                .unwrap_or_default();
            lua.create_sequence_from(tracks)
        });

        // Sound methods
        methods.add_method("Play", |_, this, ()| {
            let class_name = this.class_name();
            if class_name != ClassName::Sound {
                return Err(mlua::Error::runtime("Play is only supported on Sound"));
            }
            let mut data = this.data.lock().unwrap();
            if let Some(sound) = &mut data.sound_data {
                if sound.stream_id.is_empty() {
                    return Err(mlua::Error::runtime(
                        "Sound.Play: StreamId must be set before calling Play()",
                    ));
                }
                sound.is_playing = true;
            }
            Ok(())
        });

        methods.add_method("Stop", |lua, this, ()| {
            let class_name = this.class_name();
            if class_name != ClassName::Sound {
                return Err(mlua::Error::runtime("Stop is only supported on Sound"));
            }
            let stopped_signal = {
                let mut data = this.data.lock().unwrap();
                if let Some(sound) = &mut data.sound_data {
                    if !sound.is_playing {
                        eprintln!("[Sound] Stop called on already-stopped Sound");
                        return Ok(());
                    }
                    sound.is_playing = false;
                    Some(sound.stopped.clone())
                } else {
                    None
                }
            };
            if let Some(stopped) = stopped_signal {
                let threads = stopped.fire_as_coroutines(lua, mlua::MultiValue::new())?;
                track_yielded_threads(lua, threads)?;
            }
            Ok(())
        });

        methods.add_method("MarkEnded", |lua, this, ()| {
            let class_name = this.class_name();
            if class_name != ClassName::Sound {
                return Err(mlua::Error::runtime("MarkEnded is only supported on Sound"));
            }
            let signals = {
                let mut data = this.data.lock().unwrap();
                if let Some(sound) = &mut data.sound_data {
                    if !sound.is_playing {
                        eprintln!("[Sound] MarkEnded called on already-stopped Sound");
                        return Ok(());
                    }
                    sound.is_playing = false;
                    Some((sound.ended.clone(), sound.stopped.clone()))
                } else {
                    None
                }
            };
            if let Some((ended, stopped)) = signals {
                let threads = ended.fire_as_coroutines(lua, mlua::MultiValue::new())?;
                track_yielded_threads(lua, threads)?;
                let threads = stopped.fire_as_coroutines(lua, mlua::MultiValue::new())?;
                track_yielded_threads(lua, threads)?;
            }
            Ok(())
        });

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

            let density = part.effective_physical_properties().density;

            let sx = part.size.x.max(0.0);
            let sy = part.size.y.max(0.0);
            let sz = part.size.z.max(0.0);
            let volume = match part.shape {
                PartType::Block => sx * sy * sz,
                PartType::Wedge => 0.5 * sx * sy * sz,
                PartType::Ball => {
                    let r = sx * 0.5;
                    (4.0 / 3.0) * std::f32::consts::PI * r * r * r
                }
                PartType::Cylinder => {
                    let r = sx * 0.5;
                    std::f32::consts::PI * r * r * sy
                }
                PartType::Ring => {
                    let outer_radius = 0.5 * sx.min(sz);
                    let tube_radius = (sy * 0.5).min(outer_radius * 0.45);
                    let major_radius = (outer_radius - tube_radius).max(0.0);
                    2.0 * std::f32::consts::PI * std::f32::consts::PI * major_radius * tube_radius * tube_radius
                }
            };
            Ok(volume * density)
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
                    mlua::MultiValue::from_iter([Value::UserData(
                        lua.create_userdata(old_char.clone())?,
                    )]),
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
                mlua::MultiValue::from_iter([Value::UserData(
                    lua.create_userdata(character.clone())?,
                )]),
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

        methods.add_meta_method(mlua::MetaMethod::Index, |_, this, key: String| {
            Ok(this.find_first_child(&key, false))
        });

        methods.add_meta_method(mlua::MetaMethod::ToString, |_, this, ()| Ok(this.name()));

        methods.add_meta_method(mlua::MetaMethod::Eq, |_, this, other: Instance| {
            Ok(this.id() == other.id())
        });
    
}
