use mlua::{UserDataMethods, Value};

use crate::game::lua::types::{Color3, Vector3};

use super::super::super::*;

pub(super) fn add_methods<M: UserDataMethods<Instance>>(methods: &mut M) {
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
}
