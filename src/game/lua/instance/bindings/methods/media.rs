use mlua::UserDataMethods;

use crate::game::lua::events::track_yielded_threads;

use super::super::super::*;

pub(super) fn add_methods<M: UserDataMethods<Instance>>(methods: &mut M) {
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
}
