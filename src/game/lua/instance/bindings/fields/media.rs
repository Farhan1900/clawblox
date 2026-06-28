use mlua::UserDataFields;
use std::sync::Arc;

use super::super::super::*;

pub(super) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
    fields.add_field_method_get("AnimationId", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.animation_data.as_ref().map(|a| a.animation_id.clone()))
    });
    fields.add_field_method_set("AnimationId", |lua, this, animation_id: String| {
        let mut data = this.data.lock().unwrap();
        if let Some(animation) = &mut data.animation_data {
            animation.animation_id = animation_id;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    // Sound properties
    fields.add_field_method_get("StreamId", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.sound_data.as_ref().map(|s| s.stream_id.clone()))
    });
    fields.add_field_method_set("StreamId", |lua, this, stream_id: String| {
        let mut data = this.data.lock().unwrap();
        if let Some(sound) = &mut data.sound_data {
            sound.stream_id = stream_id;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });
    fields.add_field_method_get("Volume", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.sound_data.as_ref().map(|s| s.volume))
    });
    fields.add_field_method_set("Volume", |lua, this, volume: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(sound) = &mut data.sound_data {
            sound.volume = volume.clamp(0.0, 1.0);
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });
    fields.add_field_method_get("PlaybackSpeed", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.sound_data.as_ref().map(|s| s.playback_speed))
    });
    fields.add_field_method_set("PlaybackSpeed", |lua, this, speed: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(sound) = &mut data.sound_data {
            sound.playback_speed = speed;
            drop(data);
            this.mark_replication_dirty(lua);
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

    fields.add_field_method_get("PrimaryPart", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .model_data
            .as_ref()
            .and_then(|m| m.primary_part.as_ref())
            .and_then(|w| w.upgrade())
            .map(Instance::from_ref))
    });
    fields.add_field_method_set("PrimaryPart", |lua, this, part: Option<Instance>| {
        let mut data = this.data.lock().unwrap();
        if let Some(model) = &mut data.model_data {
            model.primary_part = part.map(|p| Arc::downgrade(&p.data));
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });
}
