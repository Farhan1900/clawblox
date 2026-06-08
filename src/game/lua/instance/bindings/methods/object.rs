use mlua::{UserDataMethods, Value};

use super::super::super::*;

pub(super) fn add_methods<M: UserDataMethods<Instance>>(methods: &mut M) {
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

    methods.add_meta_method(mlua::MetaMethod::Index, |_, this, key: String| {
        Ok(this.find_first_child(&key, false))
    });

    methods.add_meta_method(mlua::MetaMethod::ToString, |_, this, ()| Ok(this.name()));

    methods.add_meta_method(mlua::MetaMethod::Eq, |_, this, other: Instance| {
        Ok(this.id() == other.id())
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
}
