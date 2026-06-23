use mlua::{
    Function, Lua, MultiValue, RegistryKey, Result, Thread, UserData, UserDataFields,
    UserDataMethods, Value,
};
use std::fmt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::game::instance::ErrorMode;
use crate::game::profiling::{enabled as profiling_enabled, record_values};

static CONNECTION_ID: AtomicU64 = AtomicU64::new(1);
static TOUCH_SIGNAL_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct RBXScriptConnection {
    id: u64,
    connected: Arc<Mutex<bool>>,
    signal: Arc<Mutex<SignalInner>>,
}

impl RBXScriptConnection {
    pub fn new(signal: Arc<Mutex<SignalInner>>) -> Self {
        Self {
            id: CONNECTION_ID.fetch_add(1, Ordering::SeqCst),
            connected: Arc::new(Mutex::new(true)),
            signal,
        }
    }

    pub fn restored(id: u64, signal: &RBXScriptSignal, connected: bool) -> Self {
        ensure_connection_id_above(id);
        Self {
            id,
            connected: Arc::new(Mutex::new(connected)),
            signal: Arc::clone(&signal.inner),
        }
    }

    pub fn disconnect(&self) {
        *self.connected.lock().unwrap() = false;
        self.signal.lock().unwrap().remove_connection(self.id);
    }

    pub fn is_connected(&self) -> bool {
        *self.connected.lock().unwrap()
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn is_for_signal(&self, signal: &RBXScriptSignal) -> bool {
        Arc::ptr_eq(&self.signal, &signal.inner)
    }
}

impl UserData for RBXScriptConnection {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Connected", |_, this| Ok(this.is_connected()));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Disconnect", |_, this, ()| {
            this.disconnect();
            Ok(())
        });
    }
}

struct ConnectionEntry {
    id: u64,
    callback: RegistryKey,
    once: bool,
}

pub struct SignalConnectionSnapshot {
    pub id: u64,
    pub callback: Function,
    pub once: bool,
}

pub struct SignalWaiterSnapshot {
    pub thread: Thread,
}

pub struct SignalInner {
    connections: Vec<ConnectionEntry>,
    waiters: Vec<RegistryKey>,
    touch_interest: bool,
}

impl SignalInner {
    fn new(touch_interest: bool) -> Self {
        Self {
            connections: Vec::new(),
            waiters: Vec::new(),
            touch_interest,
        }
    }

    fn remove_connection(&mut self, id: u64) {
        let before = self.connections.len();
        self.connections.retain(|c| c.id != id);
        let removed = before.saturating_sub(self.connections.len());
        if self.touch_interest && removed > 0 {
            TOUCH_SIGNAL_CONNECTIONS.fetch_sub(removed, Ordering::SeqCst);
        }
    }

    fn clear_connections(&mut self) {
        if self.touch_interest && !self.connections.is_empty() {
            TOUCH_SIGNAL_CONNECTIONS.fetch_sub(self.connections.len(), Ordering::SeqCst);
        }
        self.connections.clear();
    }

    fn clear_waiters(&mut self, lua: &Lua) {
        for key in self.waiters.drain(..) {
            let _ = lua.remove_registry_value(key);
        }
    }
}

impl Drop for SignalInner {
    fn drop(&mut self) {
        if self.touch_interest && !self.connections.is_empty() {
            TOUCH_SIGNAL_CONNECTIONS.fetch_sub(self.connections.len(), Ordering::SeqCst);
        }
    }
}

#[derive(Clone)]
pub struct RBXScriptSignal {
    inner: Arc<Mutex<SignalInner>>,
    name: String,
}

pub fn track_yielded_threads(lua: &Lua, threads: Vec<Thread>) -> Result<()> {
    if threads.is_empty() {
        return Ok(());
    }

    let tracker: Function = match lua.globals().get("__clawblox_track_thread") {
        Ok(f) => f,
        Err(_) => {
            eprintln!("[LuaRuntime] Missing coroutine tracker (__clawblox_track_thread)");
            return Ok(());
        }
    };

    for thread in threads {
        if thread.status() == mlua::ThreadStatus::Resumable {
            tracker.call::<()>(thread)?;
        }
    }

    Ok(())
}

pub fn has_touch_signal_connections() -> bool {
    TOUCH_SIGNAL_CONNECTIONS.load(Ordering::SeqCst) > 0
}

impl fmt::Debug for RBXScriptSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RBXScriptSignal")
            .field("name", &self.name)
            .finish()
    }
}

impl RBXScriptSignal {
    pub fn new(name: &str) -> Self {
        let touch_interest = matches!(name, "Touched" | "TouchEnded");
        Self {
            inner: Arc::new(Mutex::new(SignalInner::new(touch_interest))),
            name: name.to_string(),
        }
    }

    pub fn connection_count(&self) -> usize {
        self.inner.lock().unwrap().connections.len()
    }

    pub fn snapshot_connections(&self, lua: &Lua) -> Result<Vec<SignalConnectionSnapshot>> {
        let inner = self.inner.lock().unwrap();
        let mut connections = Vec::with_capacity(inner.connections.len());
        for connection in &inner.connections {
            let callback = lua.registry_value::<Function>(&connection.callback)?;
            connections.push(SignalConnectionSnapshot {
                id: connection.id,
                callback,
                once: connection.once,
            });
        }
        Ok(connections)
    }

    pub fn snapshot_waiters(&self, lua: &Lua) -> Result<Vec<SignalWaiterSnapshot>> {
        let inner = self.inner.lock().unwrap();
        let mut waiters = Vec::with_capacity(inner.waiters.len());
        for waiter in &inner.waiters {
            waiters.push(SignalWaiterSnapshot {
                thread: lua.registry_value::<Thread>(waiter)?,
            });
        }
        Ok(waiters)
    }

    pub fn restore_connections(
        &self,
        lua: &Lua,
        connections: Vec<SignalConnectionSnapshot>,
    ) -> Result<()> {
        let mut restored = Vec::with_capacity(connections.len());
        let mut max_id = 0;
        for connection in connections {
            max_id = max_id.max(connection.id);
            restored.push(ConnectionEntry {
                id: connection.id,
                callback: lua.create_registry_value(connection.callback)?,
                once: connection.once,
            });
        }
        ensure_connection_id_above(max_id);

        let mut inner = self.inner.lock().unwrap();
        inner.clear_connections();
        if inner.touch_interest && !restored.is_empty() {
            TOUCH_SIGNAL_CONNECTIONS.fetch_add(restored.len(), Ordering::SeqCst);
        }
        inner.connections = restored;
        Ok(())
    }

    pub fn restore_waiters(&self, lua: &Lua, waiters: Vec<SignalWaiterSnapshot>) -> Result<()> {
        let mut restored = Vec::with_capacity(waiters.len());
        for waiter in waiters {
            restored.push(lua.create_registry_value(waiter.thread)?);
        }

        let mut inner = self.inner.lock().unwrap();
        inner.clear_waiters(lua);
        inner.waiters = restored;
        Ok(())
    }

    pub fn wait(&self, lua: &Lua, thread: Thread) -> Result<()> {
        let key = lua.create_registry_value(thread)?;
        self.inner.lock().unwrap().waiters.push(key);
        Ok(())
    }

    pub fn connect(&self, lua: &Lua, callback: Function) -> Result<RBXScriptConnection> {
        let key = lua.create_registry_value(callback)?;
        let connection = RBXScriptConnection::new(Arc::clone(&self.inner));
        let id = connection.id();

        let mut inner = self.inner.lock().unwrap();
        if inner.touch_interest {
            TOUCH_SIGNAL_CONNECTIONS.fetch_add(1, Ordering::SeqCst);
        }
        inner.connections.push(ConnectionEntry {
            id,
            callback: key,
            once: false,
        });

        Ok(connection)
    }

    pub fn once(&self, lua: &Lua, callback: Function) -> Result<RBXScriptConnection> {
        let key = lua.create_registry_value(callback)?;
        let connection = RBXScriptConnection::new(Arc::clone(&self.inner));
        let id = connection.id();

        let mut inner = self.inner.lock().unwrap();
        if inner.touch_interest {
            TOUCH_SIGNAL_CONNECTIONS.fetch_add(1, Ordering::SeqCst);
        }
        inner.connections.push(ConnectionEntry {
            id,
            callback: key,
            once: true,
        });

        Ok(connection)
    }

    pub fn fire(&self, lua: &Lua, args: MultiValue) -> Result<()> {
        self.drain_waiters(lua, args.clone())?;
        let mut to_remove = Vec::new();

        let connections: Vec<_> = {
            let inner = self.inner.lock().unwrap();
            inner.connections.iter().map(|c| (c.id, c.once)).collect()
        };

        for (id, once) in connections {
            let callback_key = {
                let inner = self.inner.lock().unwrap();
                inner
                    .connections
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| &c.callback)
                    .and_then(|k| lua.registry_value::<Function>(k).ok())
            };

            if let Some(callback) = callback_key {
                if let Err(e) = callback.call::<()>(args.clone()) {
                    let error_mode = lua
                        .app_data_ref::<ErrorMode>()
                        .map(|m| *m)
                        .unwrap_or(ErrorMode::Continue);
                    if error_mode == ErrorMode::Halt {
                        return Err(e);
                    }
                    eprintln!(
                        "[Lua Error] Callback error in signal '{}': {}",
                        self.name, e
                    );
                }
                if once {
                    to_remove.push(id);
                }
            }
        }

        for id in to_remove {
            self.inner.lock().unwrap().remove_connection(id);
        }

        Ok(())
    }

    /// Fires the signal with each callback running in its own coroutine.
    ///
    /// This allows callbacks to yield (e.g., for async operations like DataStore:GetAsync)
    /// without blocking other callbacks. Returns the coroutine threads for tracking.
    ///
    /// Yielded threads should be tracked by the CoroutineManager if they call async functions.
    pub fn fire_as_coroutines(&self, lua: &Lua, args: MultiValue) -> Result<Vec<Thread>> {
        self.drain_waiters(lua, args.clone())?;
        let mut to_remove = Vec::new();
        let mut threads = Vec::new();
        let profiling = profiling_enabled();
        let mut registry_lookup_ms = 0.0f64;
        let mut create_thread_ms = 0.0f64;
        let mut resume_ms = 0.0f64;

        let connections: Vec<_> = {
            let inner = self.inner.lock().unwrap();
            inner.connections.iter().map(|c| (c.id, c.once)).collect()
        };

        for (id, once) in connections {
            let started = profiling.then(Instant::now);
            let callback_key = {
                let inner = self.inner.lock().unwrap();
                inner
                    .connections
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| &c.callback)
                    .and_then(|k| lua.registry_value::<Function>(k).ok())
            };
            if let Some(started) = started {
                registry_lookup_ms += started.elapsed().as_secs_f64() * 1000.0;
            }

            if let Some(callback) = callback_key {
                // Create a new coroutine for this callback
                let started = profiling.then(Instant::now);
                let thread = lua.create_thread(callback)?;
                if let Some(started) = started {
                    create_thread_ms += started.elapsed().as_secs_f64() * 1000.0;
                }

                // Resume the coroutine with the arguments
                let started = profiling.then(Instant::now);
                match thread.resume::<()>(args.clone()) {
                    Ok(()) => {
                        // Callback completed without yielding
                        // Check if thread yielded after completion (async functions do this)
                        if thread.status() == mlua::ThreadStatus::Resumable {
                            threads.push(thread);
                        }
                    }
                    Err(e) => {
                        // Check if the thread yielded (this is normal for async operations)
                        if thread.status() == mlua::ThreadStatus::Resumable {
                            // Thread yielded - track it for later resumption
                            threads.push(thread);
                        } else {
                            // Actual error — in Halt mode, propagate immediately
                            let error_mode = lua
                                .app_data_ref::<ErrorMode>()
                                .map(|m| *m)
                                .unwrap_or(ErrorMode::Continue);
                            if error_mode == ErrorMode::Halt {
                                return Err(e);
                            }
                            eprintln!(
                                "[Lua Error] Callback error in signal '{}': {}",
                                self.name, e
                            );
                        }
                    }
                }
                if let Some(started) = started {
                    resume_ms += started.elapsed().as_secs_f64() * 1000.0;
                }

                if once {
                    to_remove.push(id);
                }
            }
        }

        for id in to_remove {
            self.inner.lock().unwrap().remove_connection(id);
        }

        if profiling {
            record_values(
                "signal.fire_as_coroutines",
                &[
                    ("connections", self.connection_count() as f64),
                    ("yielded_threads", threads.len() as f64),
                    ("registry_lookup_ms", registry_lookup_ms),
                    ("create_thread_ms", create_thread_ms),
                    ("resume_ms", resume_ms),
                ],
            );
        }

        Ok(threads)
    }

    fn drain_waiters(&self, lua: &Lua, args: MultiValue) -> Result<()> {
        let waiters = {
            let mut inner = self.inner.lock().unwrap();
            inner.waiters.drain(..).collect::<Vec<_>>()
        };
        if waiters.is_empty() {
            return Ok(());
        }

        let args_table: mlua::Table = lua.globals().get("__clawblox_thread_args")?;
        let signal_waiting: mlua::Table = lua.globals().get("__clawblox_signal_waiting_threads")?;
        let args_vec = args.into_vec();
        for key in waiters {
            if let Ok(thread) = lua.registry_value::<Thread>(&key) {
                let packed = lua.create_table()?;
                packed.set("n", args_vec.len())?;
                for (idx, value) in args_vec.iter().cloned().enumerate() {
                    packed.set(idx + 1, value)?;
                }
                signal_waiting.set(thread.clone(), Value::Nil)?;
                args_table.set(thread.clone(), packed)?;
            }
            let _ = lua.remove_registry_value(key);
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_same_signal(&self, other: &RBXScriptSignal) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

fn ensure_connection_id_above(id: u64) {
    let next = id.saturating_add(1);
    let mut current = CONNECTION_ID.load(Ordering::SeqCst);
    while current < next {
        match CONNECTION_ID.compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

impl UserData for RBXScriptSignal {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Wait", |lua, _this| {
            lua.globals().get::<Function>("__clawblox_signal_wait_impl")
        });
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Connect", |lua, this, callback: Function| {
            this.connect(lua, callback)
        });

        methods.add_method("Once", |lua, this, callback: Function| {
            this.once(lua, callback)
        });
    }
}

pub fn create_signal(name: &str) -> RBXScriptSignal {
    RBXScriptSignal::new(name)
}
