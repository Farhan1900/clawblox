use mlua::{MultiValue, UserDataFields, Value};
use std::sync::Arc;

use crate::game::lua::types::CFrame;

use super::super::super::*;

pub(super) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
    fields.add_field_method_get("Changed", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .int_value_data
            .as_ref()
            .map(|v| v.changed.clone())
            .or_else(|| data.number_value_data.as_ref().map(|v| v.changed.clone()))
            .or_else(|| data.bool_value_data.as_ref().map(|v| v.changed.clone()))
            .or_else(|| data.string_value_data.as_ref().map(|v| v.changed.clone()))
            .or_else(|| data.object_value_data.as_ref().map(|v| v.changed.clone()))
            .or_else(|| data.cframe_value_data.as_ref().map(|v| v.changed.clone())))
    });

    fields.add_field_method_get("Value", |lua, this| {
        let data = this.data.lock().unwrap();
        if let Some(v) = &data.int_value_data {
            return Ok(Value::Integer(v.value));
        }
        if let Some(v) = &data.number_value_data {
            return Ok(Value::Number(v.value));
        }
        if let Some(v) = &data.bool_value_data {
            return Ok(Value::Boolean(v.value));
        }
        if let Some(v) = &data.string_value_data {
            return Ok(Value::String(lua.create_string(&v.value)?));
        }
        if let Some(v) = &data.object_value_data {
            if let Some(value_ref) = &v.value {
                if let Some(strong) = value_ref.upgrade() {
                    let inst = Instance::from_ref(strong);
                    return Ok(Value::UserData(Instance::cached_userdata(lua, &inst)?));
                }
            }
            return Ok(Value::Nil);
        }
        if let Some(v) = &data.cframe_value_data {
            return Ok(Value::UserData(lua.create_userdata(v.value)?));
        }
        Ok(Value::Nil)
    });
    fields.add_field_method_set("Value", |lua, this, value: Value| {
        let changed_signal = {
            let mut data = this.data.lock().unwrap();
            if let Some(v) = &mut data.int_value_data {
                v.value = match &value {
                    Value::Integer(i) => *i,
                    Value::Number(n) => *n as i64,
                    _ => v.value,
                };
                Some(v.changed.clone())
            } else if let Some(v) = &mut data.number_value_data {
                v.value = match &value {
                    Value::Integer(i) => *i as f64,
                    Value::Number(n) => *n,
                    _ => v.value,
                };
                Some(v.changed.clone())
            } else if let Some(v) = &mut data.bool_value_data {
                v.value = match &value {
                    Value::Boolean(b) => *b,
                    _ => v.value,
                };
                Some(v.changed.clone())
            } else if let Some(v) = &mut data.string_value_data {
                if let Value::String(s) = &value {
                    if let Ok(text) = s.to_str() {
                        v.value = text.to_string();
                    }
                }
                Some(v.changed.clone())
            } else if let Some(v) = &mut data.object_value_data {
                match &value {
                    Value::Nil => {
                        v.value = None;
                    }
                    Value::UserData(ud) => {
                        if let Ok(inst) = ud.borrow::<Instance>() {
                            v.value = Some(Arc::downgrade(&inst.data));
                        } else {
                            return Err(mlua::Error::runtime(
                                "ObjectValue.Value expects Instance or nil",
                            ));
                        }
                    }
                    _ => {
                        return Err(mlua::Error::runtime(
                            "ObjectValue.Value expects Instance or nil",
                        ));
                    }
                }
                Some(v.changed.clone())
            } else if let Some(v) = &mut data.cframe_value_data {
                match &value {
                    Value::UserData(ud) => {
                        if let Ok(cf) = ud.borrow::<CFrame>() {
                            v.value = cf.orthonormalized();
                        } else {
                            return Err(mlua::Error::runtime("CFrameValue.Value expects CFrame"));
                        }
                    }
                    _ => {
                        return Err(mlua::Error::runtime("CFrameValue.Value expects CFrame"));
                    }
                }
                Some(v.changed.clone())
            } else {
                None
            }
        };
        if let Some(signal) = changed_signal {
            let threads = signal.fire_as_coroutines(lua, MultiValue::from_iter([value]))?;
            crate::game::lua::events::track_yielded_threads(lua, threads)?;
        }
        Ok(())
    });
}
