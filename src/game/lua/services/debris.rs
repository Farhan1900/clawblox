use mlua::{Lua, UserData, UserDataFields, UserDataMethods, Value};
use std::sync::{Arc, Mutex};

use crate::game::lua::instance::Instance;

#[derive(Clone)]
pub struct DebrisService {
    data: Arc<Mutex<DebrisServiceData>>,
}

#[derive(Clone)]
struct DebrisEntry {
    instance: Instance,
    destroy_at_secs: f64,
}

#[derive(Clone)]
pub struct DebrisEntrySnapshot {
    pub instance: Instance,
    pub destroy_at_secs: f64,
}

#[derive(Default)]
struct DebrisServiceData {
    queue: Vec<DebrisEntry>,
}

impl DebrisService {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(DebrisServiceData::default())),
        }
    }

    pub fn add_item(&self, instance: Instance, lifetime_secs: f64, now_secs: f64) {
        let destroy_at_secs = now_secs + lifetime_secs.max(0.0);
        self.data.lock().unwrap().queue.push(DebrisEntry {
            instance,
            destroy_at_secs,
        });
    }

    pub fn process_expired(&self, lua: &Lua, now_secs: f64) -> mlua::Result<()> {
        let ready = {
            let mut data = self.data.lock().unwrap();
            let mut ready = Vec::new();
            let mut pending = Vec::with_capacity(data.queue.len());
            for entry in data.queue.drain(..) {
                if now_secs >= entry.destroy_at_secs {
                    ready.push(entry);
                } else {
                    pending.push(entry);
                }
            }
            data.queue = pending;
            ready
        };

        for entry in ready {
            entry.instance.destroy(lua)?;
        }
        Ok(())
    }

    pub fn snapshot_queue(&self) -> Vec<DebrisEntrySnapshot> {
        self.data
            .lock()
            .unwrap()
            .queue
            .iter()
            .map(|entry| DebrisEntrySnapshot {
                instance: entry.instance.clone(),
                destroy_at_secs: entry.destroy_at_secs,
            })
            .collect()
    }

    pub fn restore_queue(&self, entries: Vec<DebrisEntrySnapshot>) {
        self.data.lock().unwrap().queue = entries
            .into_iter()
            .map(|entry| DebrisEntry {
                instance: entry.instance,
                destroy_at_secs: entry.destroy_at_secs,
            })
            .collect();
    }
}

impl Default for DebrisService {
    fn default() -> Self {
        Self::new()
    }
}

impl UserData for DebrisService {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Name", |_, _| Ok("Debris".to_string()));
        fields.add_field_method_get("ClassName", |_, _| Ok("Debris".to_string()));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "AddItem",
            |lua, this, (item, lifetime): (Value, Option<f64>)| {
                if let Value::UserData(ud) = item {
                    if let Ok(instance) = ud.borrow::<Instance>() {
                        let now_secs = lua
                            .globals()
                            .get::<mlua::Function>("__clawblox_now")
                            .ok()
                            .and_then(|f| f.call::<f64>(()).ok())
                            .unwrap_or(0.0);
                        this.add_item(instance.clone(), lifetime.unwrap_or(10.0), now_secs);
                    }
                }
                Ok(())
            },
        );
    }
}
