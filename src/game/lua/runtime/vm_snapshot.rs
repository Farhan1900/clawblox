use super::*;
use crate::game::lua::animation::{
    AnimationScheduler, AnimationTrack, AnimationTrackStateSnapshot,
};
use crate::game::lua::events::{
    RBXScriptConnection, RBXScriptSignal, SignalConnectionSnapshot, SignalWaiterSnapshot,
};
use crate::game::lua::services::data_store::{
    DataStore, DataStoreHandleSnapshot, OrderedDataStore,
};
use crate::game::lua::services::workspace::{
    OverlapParams, OverlapParamsSnapshot, RaycastParams, RaycastParamsSnapshot, RaycastResult,
    RaycastResultSnapshot,
};
use crate::game::lua::types::{
    enums::{FontEnum, TextXAlignmentEnum, TextYAlignmentEnum},
    ActuatorRelativeTo, FluidForces, HumanoidStateType, Material, PartType, RaycastFilterType,
};
use luau_persist::{
    dump_stack_value_with_options, load_stack_value_with_options, DumpOptions, LoadOptions,
    PersistError, UserDataAdapter, UserDataPayload,
};
use mlua_sys::{
    lua_State, lua_getglobal, lua_gettop, lua_newtable, lua_pop, lua_pushnil, lua_pushvalue,
    lua_remove, lua_setglobal,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_int;

type VmSnapshotResult<T> = std::result::Result<T, String>;
type PersistResult<T> = std::result::Result<T, PersistError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuaVmSnapshot {
    pub format_version: u32,
    pub payload: Vec<u8>,
}

pub struct LuaVmSnapshotCapture {
    pub snapshot: LuaVmSnapshot,
    pub referenced_instances: Vec<Instance>,
}

const LUA_VM_SNAPSHOT_VERSION: u32 = 1;
const GLOBAL_BASELINE_REGISTRY_KEY: &str = "__clawblox.global_baseline";
const PERMANENT_GLOBALS_KEY: &str = "__clawblox.globals";
const USERDATA_BRIDGE_GLOBAL: &str = "__clawblox_vm_snapshot_userdata_bridge";
const USERDATA_BRIDGE_GLOBAL_CSTR: &[u8] = b"__clawblox_vm_snapshot_userdata_bridge\0";
const PERMANENT_GLOBAL_NAMES: &[&str] = &[
    "game",
    "workspace",
    "Workspace",
    "Players",
    "RunService",
    "PhysicsService",
    "RemoteEventService",
    "ReplicatedStorage",
    "ServerScriptService",
    "ServerStorage",
    "serverStorage",
    "Instance",
    "Vector2",
    "Vector3",
    "CFrame",
    "Color3",
    "DateTime",
    "BrickColor",
    "Enum",
    "PhysicalProperties",
    "Ray",
    "Region3",
    "UDim",
    "UDim2",
    "task",
    "coroutine",
    "table",
    "math",
    "string",
    "utf8",
    "bit32",
    "buffer",
    "print",
    "warn",
    "typeof",
    "tick",
    "time",
    "wait",
    "require",
    "__clawblox_tick_impl",
    "__clawblox_schedule_wait",
    "__clawblox_now",
    "__clawblox_track_thread",
    "__clawblox_signal_wait_register",
    "__clawblox_signal_wait_impl",
];

impl LuaRuntime {
    pub fn capture_vm_snapshot(&self) -> VmSnapshotResult<LuaVmSnapshot> {
        Ok(self.capture_vm_snapshot_with_references()?.snapshot)
    }

    pub fn capture_vm_snapshot_with_references(&self) -> VmSnapshotResult<LuaVmSnapshotCapture> {
        let root = self.lua.create_table().map_err(|err| err.to_string())?;
        root.set("format_version", LUA_VM_SNAPSHOT_VERSION)
            .map_err(|err| err.to_string())?;
        root.set("clock", self.runtime_clock.now())
            .map_err(|err| err.to_string())?;
        root.set("script_loaded", self.script_loaded)
            .map_err(|err| err.to_string())?;

        let pending = self.lua.create_table().map_err(|err| err.to_string())?;
        for (idx, key) in self.pending_coroutines.lock().unwrap().iter().enumerate() {
            let thread: Thread = self
                .lua
                .registry_value(key)
                .map_err(|err| err.to_string())?;
            pending
                .set(idx + 1, thread)
                .map_err(|err| err.to_string())?;
        }
        root.set("pending", pending)
            .map_err(|err| err.to_string())?;

        let globals = self.lua.globals();
        root.set(
            "schedule",
            globals
                .get::<mlua::Table>("__clawblox_thread_schedule")
                .map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
        root.set(
            "args",
            globals
                .get::<mlua::Table>("__clawblox_thread_args")
                .map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
        root.set(
            "cancelled",
            globals
                .get::<mlua::Table>("__clawblox_cancelled_threads")
                .map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
        root.set(
            "signal_waiting",
            globals
                .get::<mlua::Table>("__clawblox_signal_waiting_threads")
                .map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
        root.set(
            "module_cache",
            globals
                .get::<mlua::Table>("__clawblox_module_cache")
                .map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
        root.set(
            "signals",
            capture_signal_connections(&self.lua, &self.game)?,
        )
        .map_err(|err| err.to_string())?;
        root.set("animation_tracks", capture_animation_tracks(&self.lua)?)
            .map_err(|err| err.to_string())?;
        root.set("bind_to_close", capture_bind_to_close_callbacks(&self.lua)?)
            .map_err(|err| err.to_string())?;
        let (globals_set, globals_unset) = capture_global_delta(&self.lua)?;
        root.set("globals_set", globals_set)
            .map_err(|err| err.to_string())?;
        root.set("globals_unset", globals_unset)
            .map_err(|err| err.to_string())?;

        let executed = self.lua.create_table().map_err(|err| err.to_string())?;
        let mut executed_scripts = self
            .executed_scripts
            .lock()
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        executed_scripts.sort_unstable();
        for (idx, script_id) in executed_scripts.into_iter().enumerate() {
            executed
                .set(idx + 1, script_id as i64)
                .map_err(|err| err.to_string())?;
        }
        root.set("executed_scripts", executed)
            .map_err(|err| err.to_string())?;

        let permanents = self.lua.create_table().map_err(|err| err.to_string())?;
        permanents
            .set(globals, PERMANENT_GLOBALS_KEY)
            .map_err(|err| err.to_string())?;
        register_dump_permanent_globals(&self.lua, &permanents)?;

        let mut result = None;
        let mut userdata_adapter =
            ClawbloxUserDataAdapter::new(self.lua.clone(), self.game.clone());
        unsafe {
            self.lua
                .exec_raw::<()>((root, permanents), |state| {
                    let top = lua_gettop(state);
                    let dumped = dump_stack_value_with_options(
                        state,
                        top - 1,
                        DumpOptions {
                            permanents_index: Some(top),
                            userdata_adapter: Some(&mut userdata_adapter),
                        },
                    );
                    lua_pop(state, 2);
                    result = Some(dumped);
                })
                .map_err(|err| err.to_string())?;
        }

        let mut referenced_instances = userdata_adapter
            .referenced_instances
            .into_values()
            .collect::<Vec<_>>();
        referenced_instances.sort_by_key(|instance| instance.id().0);

        Ok(LuaVmSnapshotCapture {
            snapshot: LuaVmSnapshot {
                format_version: LUA_VM_SNAPSHOT_VERSION,
                payload: result
                    .ok_or_else(|| "Lua VM root dump did not run".to_string())?
                    .map_err(|err| err.to_string())?,
            },
            referenced_instances,
        })
    }

    pub fn restore_vm_snapshot(&mut self, snapshot: &LuaVmSnapshot) -> VmSnapshotResult<()> {
        self.restore_vm_snapshot_with_instances(snapshot, std::iter::empty::<Instance>())
    }

    pub fn restore_vm_snapshot_with_instances<I>(
        &mut self,
        snapshot: &LuaVmSnapshot,
        extra_instances: I,
    ) -> VmSnapshotResult<()>
    where
        I: IntoIterator<Item = Instance>,
    {
        if snapshot.format_version != LUA_VM_SNAPSHOT_VERSION {
            return Err(format!(
                "unsupported Lua VM snapshot version {}",
                snapshot.format_version
            ));
        }

        let globals = self.lua.globals();
        let permanents = self.lua.create_table().map_err(|err| err.to_string())?;
        permanents
            .set(PERMANENT_GLOBALS_KEY, globals.clone())
            .map_err(|err| err.to_string())?;
        register_load_permanent_globals(&self.lua, &permanents)?;

        let mut load_result = None;
        let mut userdata_adapter = ClawbloxUserDataAdapter::new_with_instances(
            self.lua.clone(),
            self.game.clone(),
            extra_instances,
        );
        let root_value: mlua::Value = unsafe {
            self.lua
                .exec_raw(permanents, |state| {
                    let top = lua_gettop(state);
                    let loaded = load_stack_value_with_options(
                        state,
                        &snapshot.payload,
                        LoadOptions {
                            permanents_index: Some(top),
                            userdata_adapter: Some(&mut userdata_adapter),
                        },
                    );
                    if loaded.is_ok() {
                        lua_remove(state, top);
                    } else {
                        while lua_gettop(state) >= top {
                            lua_pop(state, 1);
                        }
                        lua_newtable(state);
                    }
                    load_result = Some(loaded);
                })
                .map_err(|err| err.to_string())?
        };
        load_result
            .ok_or_else(|| "Lua VM root load did not run".to_string())?
            .map_err(|err| err.to_string())?;
        let mlua::Value::Table(root) = root_value else {
            return Err("Lua VM root did not restore as table".to_string());
        };

        let version: u32 = root.get("format_version").map_err(|err| err.to_string())?;
        if version != LUA_VM_SNAPSHOT_VERSION {
            return Err(format!("unsupported Lua VM root version {}", version));
        }

        let clock: f64 = root.get("clock").map_err(|err| err.to_string())?;
        self.runtime_clock.set(clock);
        self.script_loaded = root.get("script_loaded").map_err(|err| err.to_string())?;

        let schedule: mlua::Table = root.get("schedule").map_err(|err| err.to_string())?;
        let args: mlua::Table = root.get("args").map_err(|err| err.to_string())?;
        globals
            .set("__clawblox_thread_schedule", schedule.clone())
            .map_err(|err| err.to_string())?;
        globals
            .set("__clawblox_thread_args", args.clone())
            .map_err(|err| err.to_string())?;
        globals
            .set(
                "__clawblox_cancelled_threads",
                root.get::<mlua::Table>("cancelled")
                    .map_err(|err| err.to_string())?,
            )
            .map_err(|err| err.to_string())?;
        globals
            .set(
                "__clawblox_signal_waiting_threads",
                root.get::<mlua::Table>("signal_waiting")
                    .map_err(|err| err.to_string())?,
            )
            .map_err(|err| err.to_string())?;
        globals
            .set(
                "__clawblox_module_cache",
                root.get::<mlua::Table>("module_cache")
                    .map_err(|err| err.to_string())?,
            )
            .map_err(|err| err.to_string())?;
        globals
            .set(
                "__clawblox_loading_modules",
                self.lua.create_table().map_err(|err| err.to_string())?,
            )
            .map_err(|err| err.to_string())?;

        let signals: mlua::Table = root.get("signals").map_err(|err| err.to_string())?;
        restore_animation_tracks(
            &self.lua,
            root.get("animation_tracks")
                .map_err(|err| err.to_string())?,
        )?;
        restore_signal_connections(&self.lua, &self.game, signals)?;
        restore_bind_to_close_callbacks(
            &self.lua,
            root.get("bind_to_close").map_err(|err| err.to_string())?,
        )?;
        restore_global_delta(
            &self.lua,
            root.get("globals_set").map_err(|err| err.to_string())?,
            root.get("globals_unset").map_err(|err| err.to_string())?,
        )?;

        let mut pending = self.pending_coroutines.lock().unwrap();
        for key in pending.drain(..) {
            let _ = self.lua.remove_registry_value(key);
        }
        let seen = self.lua.create_table().map_err(|err| err.to_string())?;
        for pair in schedule.pairs::<Thread, mlua::Value>() {
            let (thread, _) = pair.map_err(|err| err.to_string())?;
            register_restored_pending_thread(&self.lua, &seen, &mut pending, thread)?;
        }
        for pair in args.pairs::<Thread, mlua::Value>() {
            let (thread, _) = pair.map_err(|err| err.to_string())?;
            register_restored_pending_thread(&self.lua, &seen, &mut pending, thread)?;
        }
        let pending_table: mlua::Table = root.get("pending").map_err(|err| err.to_string())?;
        for thread in pending_table.sequence_values::<Thread>() {
            let thread = thread.map_err(|err| err.to_string())?;
            register_restored_pending_thread(&self.lua, &seen, &mut pending, thread)?;
        }

        let mut executed = self.executed_scripts.lock().unwrap();
        executed.clear();
        let executed_table: mlua::Table = root
            .get("executed_scripts")
            .map_err(|err| err.to_string())?;
        for script_id in executed_table.sequence_values::<i64>() {
            let script_id = script_id.map_err(|err| err.to_string())?;
            if script_id < 0 {
                return Err("negative executed script id in Lua VM snapshot".to_string());
            }
            executed.insert(script_id as u64);
        }

        Ok(())
    }
}

pub(super) fn initialize_global_baseline(lua: &Lua) -> mlua::Result<()> {
    let baseline = lua.create_table()?;
    let globals = lua.globals();
    for pair in globals.pairs::<mlua::Value, mlua::Value>() {
        let (key, value) = pair?;
        if let mlua::Value::String(name) = key {
            baseline.set(name.to_str()?.as_ref(), value)?;
        }
    }
    lua.set_named_registry_value(GLOBAL_BASELINE_REGISTRY_KEY, baseline)
}

fn capture_global_delta(lua: &Lua) -> VmSnapshotResult<(mlua::Table, mlua::Table)> {
    let baseline = lua
        .named_registry_value::<mlua::Table>(GLOBAL_BASELINE_REGISTRY_KEY)
        .map_err(|err| err.to_string())?;
    let globals = lua.globals();
    let globals_set = lua.create_table().map_err(|err| err.to_string())?;
    let globals_unset = lua.create_table().map_err(|err| err.to_string())?;

    let mut set_index = 1;
    for pair in globals.pairs::<mlua::Value, mlua::Value>() {
        let (key, value) = pair.map_err(|err| err.to_string())?;
        let record = lua.create_table().map_err(|err| err.to_string())?;
        if let mlua::Value::String(name) = &key {
            let name = name.to_str().map_err(|err| err.to_string())?.to_string();
            if should_skip_internal_global_delta_name(&name) {
                continue;
            }
            if baseline
                .contains_key(name.as_str())
                .map_err(|err| err.to_string())?
            {
                let baseline_value: mlua::Value =
                    baseline.get(name.as_str()).map_err(|err| err.to_string())?;
                if lua_values_identical(&baseline_value, &value) {
                    continue;
                }
            }
            record.set("name", name).map_err(|err| err.to_string())?;
        } else {
            record.set("key", key).map_err(|err| err.to_string())?;
        }
        record.set("value", value).map_err(|err| err.to_string())?;
        globals_set
            .raw_set(set_index, record)
            .map_err(|err| err.to_string())?;
        set_index += 1;
    }

    let mut unset_index = 1;
    for pair in baseline.pairs::<String, mlua::Value>() {
        let (name, _) = pair.map_err(|err| err.to_string())?;
        if should_skip_internal_global_delta_name(&name) {
            continue;
        }
        let current: mlua::Value = globals.get(name.as_str()).unwrap_or(mlua::Value::Nil);
        if matches!(current, mlua::Value::Nil) {
            globals_unset
                .raw_set(unset_index, name)
                .map_err(|err| err.to_string())?;
            unset_index += 1;
        }
    }

    Ok((globals_set, globals_unset))
}

fn capture_bind_to_close_callbacks(lua: &Lua) -> VmSnapshotResult<mlua::Table> {
    let callbacks = lua.create_table().map_err(|err| err.to_string())?;
    let Some(bind_to_close) = lua.app_data_ref::<BindToCloseCallbacks>() else {
        return Ok(callbacks);
    };

    for (idx, key) in bind_to_close.callbacks.iter().enumerate() {
        let callback = lua
            .registry_value::<Function>(key)
            .map_err(|err| err.to_string())?;
        callbacks
            .set(idx + 1, callback)
            .map_err(|err| err.to_string())?;
    }
    Ok(callbacks)
}

fn capture_animation_tracks(lua: &Lua) -> VmSnapshotResult<mlua::Table> {
    let tracks_table = lua.create_table().map_err(|err| err.to_string())?;
    let Some(scheduler) = lua.app_data_ref::<AnimationScheduler>() else {
        return Ok(tracks_table);
    };
    let tracks = scheduler.tracks.lock().unwrap().clone();
    for (idx, track) in tracks.into_iter().enumerate() {
        tracks_table
            .set(idx + 1, track)
            .map_err(|err| err.to_string())?;
    }
    Ok(tracks_table)
}

fn restore_animation_tracks(lua: &Lua, tracks_table: mlua::Table) -> VmSnapshotResult<()> {
    let scheduler = lua
        .app_data_ref::<AnimationScheduler>()
        .map(|scheduler| scheduler.clone())
        .unwrap_or_default();
    let mut tracks = Vec::new();
    for track in tracks_table.sequence_values::<mlua::AnyUserData>() {
        let track = track.map_err(|err| err.to_string())?;
        tracks.push(
            track
                .borrow::<AnimationTrack>()
                .map_err(|err| err.to_string())?
                .clone(),
        );
    }
    scheduler.restore_tracks(tracks);
    lua.set_app_data(scheduler);
    Ok(())
}

fn restore_bind_to_close_callbacks(lua: &Lua, callbacks: mlua::Table) -> VmSnapshotResult<()> {
    if let Some(existing) = lua.remove_app_data::<BindToCloseCallbacks>() {
        for key in existing.callbacks {
            let _ = lua.remove_registry_value(key);
        }
    }

    let mut restored = Vec::new();
    for callback in callbacks.sequence_values::<Function>() {
        let callback = callback.map_err(|err| err.to_string())?;
        restored.push(
            lua.create_registry_value(callback)
                .map_err(|err| err.to_string())?,
        );
    }
    lua.set_app_data(BindToCloseCallbacks {
        callbacks: restored,
    });
    Ok(())
}

fn restore_global_delta(
    lua: &Lua,
    globals_set: mlua::Table,
    globals_unset: mlua::Table,
) -> VmSnapshotResult<()> {
    let globals = lua.globals();
    for key in globals_unset.sequence_values::<String>() {
        globals
            .set(key.map_err(|err| err.to_string())?, mlua::Value::Nil)
            .map_err(|err| err.to_string())?;
    }
    for record in globals_set.sequence_values::<mlua::Table>() {
        let record = record.map_err(|err| err.to_string())?;
        let name: Option<String> = record.get("name").map_err(|err| err.to_string())?;
        let key = if let Some(name) = name {
            if should_skip_internal_global_delta_name(&name) {
                continue;
            }
            mlua::Value::String(lua.create_string(&name).map_err(|err| err.to_string())?)
        } else {
            record.get("key").map_err(|err| err.to_string())?
        };
        let value: mlua::Value = record.get("value").map_err(|err| err.to_string())?;
        globals.set(key, value).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn should_skip_internal_global_delta_name(name: &str) -> bool {
    name == "_G" || name == USERDATA_BRIDGE_GLOBAL || name.starts_with("__clawblox")
}

fn lua_values_identical(left: &mlua::Value, right: &mlua::Value) -> bool {
    match (left, right) {
        (mlua::Value::Nil, mlua::Value::Nil) => true,
        (mlua::Value::Boolean(left), mlua::Value::Boolean(right)) => left == right,
        (mlua::Value::Integer(left), mlua::Value::Integer(right)) => left == right,
        (mlua::Value::Number(left), mlua::Value::Number(right)) => left == right,
        (mlua::Value::Integer(left), mlua::Value::Number(right)) => (*left as f64) == *right,
        (mlua::Value::Number(left), mlua::Value::Integer(right)) => *left == (*right as f64),
        (mlua::Value::String(left), mlua::Value::String(right)) => {
            left.as_bytes() == right.as_bytes()
        }
        (mlua::Value::Vector(left), mlua::Value::Vector(right)) => left == right,
        _ => !left.to_pointer().is_null() && left.to_pointer() == right.to_pointer(),
    }
}

struct ClawbloxUserDataAdapter {
    lua: Lua,
    game: Game,
    instances_by_id: HashMap<u64, Instance>,
    referenced_instances: HashMap<u64, Instance>,
    restored_animation_tracks: RefCell<HashMap<u64, AnimationTrack>>,
}

impl ClawbloxUserDataAdapter {
    fn new(lua: Lua, game: Game) -> Self {
        Self::new_with_instances(lua, game, std::iter::empty::<Instance>())
    }

    fn new_with_instances<I>(lua: Lua, game: Game, extra_instances: I) -> Self
    where
        I: IntoIterator<Item = Instance>,
    {
        let instances_by_id = collect_instance_map(&game);
        let mut instances_by_id = instances_by_id;
        for instance in extra_instances {
            instances_by_id.insert(instance.id().0, instance);
        }
        Self {
            lua,
            game,
            instances_by_id,
            referenced_instances: HashMap::new(),
            restored_animation_tracks: RefCell::new(HashMap::new()),
        }
    }

    unsafe fn stack_userdata(
        &self,
        state: *mut lua_State,
        index: c_int,
    ) -> PersistResult<mlua::AnyUserData> {
        lua_pushvalue(state, index);
        lua_setglobal(state, USERDATA_BRIDGE_GLOBAL_CSTR.as_ptr().cast());
        let userdata = self
            .lua
            .globals()
            .get::<mlua::AnyUserData>(USERDATA_BRIDGE_GLOBAL)
            .map_err(|err| PersistError::UserDataAdapter(err.to_string()));
        clear_userdata_bridge_global(state);
        userdata
    }

    unsafe fn push_userdata<T>(&self, state: *mut lua_State, value: T) -> PersistResult<()>
    where
        T: mlua::UserData + mlua::MaybeSend + 'static,
    {
        self.lua
            .globals()
            .set(USERDATA_BRIDGE_GLOBAL, value)
            .map_err(|err| PersistError::UserDataAdapter(err.to_string()))?;
        lua_getglobal(state, USERDATA_BRIDGE_GLOBAL_CSTR.as_ptr().cast());
        clear_userdata_bridge_global(state);
        Ok(())
    }

    fn restored_animation_track(&self, snapshot: AnimationTrackStateSnapshot) -> AnimationTrack {
        let mut tracks = self.restored_animation_tracks.borrow_mut();
        if let Some(track) = tracks.get(&snapshot.track_id) {
            return track.clone();
        }
        let track = AnimationTrack::restored(snapshot);
        tracks.insert(track.data.lock().unwrap().track_id, track.clone());
        track
    }

    fn record_referenced_instance(&mut self, instance: &Instance) {
        self.referenced_instances
            .entry(instance.id().0)
            .or_insert_with(|| instance.clone());
    }

    fn record_referenced_instances(&mut self, instances: &[Instance]) {
        for instance in instances {
            self.record_referenced_instance(instance);
        }
    }

    fn resolve_instances(&self, ids: &[u64]) -> PersistResult<Vec<Instance>> {
        ids.iter()
            .map(|id| {
                self.instances_by_id.get(id).cloned().ok_or_else(|| {
                    PersistError::UserDataAdapter(format!("unknown Instance id {id}"))
                })
            })
            .collect()
    }
}

impl UserDataAdapter for ClawbloxUserDataAdapter {
    unsafe fn dump_userdata(
        &mut self,
        state: *mut lua_State,
        index: c_int,
    ) -> PersistResult<Option<UserDataPayload>> {
        let userdata = self.stack_userdata(state, index)?;

        if let Ok(instance) = userdata.borrow::<Instance>() {
            self.record_referenced_instance(&instance);
            let mut bytes = Vec::with_capacity(8);
            write_u64(&mut bytes, instance.id().0);
            return Ok(Some(payload("clawblox.Instance", bytes)));
        }
        if let Ok(signal) = userdata.borrow::<RBXScriptSignal>() {
            let identity =
                find_signal_identity(&self.lua, &self.game, &signal).ok_or_else(|| {
                    PersistError::UserDataAdapter(format!(
                        "RBXScriptSignal '{}' is not attached to a known service or instance",
                        signal.name()
                    ))
                })?;
            let mut bytes = Vec::new();
            write_signal_identity(&mut bytes, &identity);
            return Ok(Some(payload("clawblox.RBXScriptSignal", bytes)));
        }
        if let Ok(connection) = userdata.borrow::<RBXScriptConnection>() {
            let identity = find_connection_signal_identity(&self.lua, &self.game, &connection)
                .ok_or_else(|| {
                    PersistError::UserDataAdapter(format!(
                        "RBXScriptConnection {} is not attached to a known signal",
                        connection.id()
                    ))
                })?;
            let mut bytes = Vec::new();
            write_u64(&mut bytes, connection.id());
            write_u8(&mut bytes, connection.is_connected() as u8);
            write_signal_identity(&mut bytes, &identity);
            return Ok(Some(payload("clawblox.RBXScriptConnection", bytes)));
        }
        if userdata.borrow::<crate::game::lua::runtime::Game>().is_ok() {
            return Ok(Some(payload("clawblox.Service.Game", Vec::new())));
        }
        if userdata
            .borrow::<crate::game::lua::services::WorkspaceService>()
            .is_ok()
        {
            return Ok(Some(payload("clawblox.Service.Workspace", Vec::new())));
        }
        if userdata
            .borrow::<crate::game::lua::services::PlayersService>()
            .is_ok()
        {
            return Ok(Some(payload("clawblox.Service.Players", Vec::new())));
        }
        if userdata
            .borrow::<crate::game::lua::services::RunService>()
            .is_ok()
        {
            return Ok(Some(payload("clawblox.Service.RunService", Vec::new())));
        }
        if userdata
            .borrow::<crate::game::lua::services::AgentInputService>()
            .is_ok()
        {
            return Ok(Some(payload(
                "clawblox.Service.AgentInputService",
                Vec::new(),
            )));
        }
        if userdata
            .borrow::<crate::game::lua::services::DataStoreService>()
            .is_ok()
        {
            return Ok(Some(payload(
                "clawblox.Service.DataStoreService",
                Vec::new(),
            )));
        }
        if let Ok(store) = userdata.borrow::<DataStore>() {
            let mut bytes = Vec::new();
            write_data_store_handle(&mut bytes, &store.snapshot_handle());
            return Ok(Some(payload("clawblox.DataStore", bytes)));
        }
        if let Ok(store) = userdata.borrow::<OrderedDataStore>() {
            let mut bytes = Vec::new();
            write_data_store_handle(&mut bytes, &store.snapshot_handle());
            return Ok(Some(payload("clawblox.OrderedDataStore", bytes)));
        }
        if let Ok(params) = userdata.borrow::<RaycastParams>() {
            self.record_referenced_instances(&params.filter_instances);
            let mut bytes = Vec::new();
            write_raycast_params(&mut bytes, &params.snapshot_state());
            return Ok(Some(payload("clawblox.RaycastParams", bytes)));
        }
        if let Ok(params) = userdata.borrow::<OverlapParams>() {
            self.record_referenced_instances(&params.filter_instances);
            let mut bytes = Vec::new();
            write_overlap_params(&mut bytes, &params.snapshot_state());
            return Ok(Some(payload("clawblox.OverlapParams", bytes)));
        }
        if let Ok(result) = userdata.borrow::<RaycastResult>() {
            self.record_referenced_instance(&result.instance);
            let mut bytes = Vec::new();
            write_raycast_result(&mut bytes, &result.snapshot_state());
            return Ok(Some(payload("clawblox.RaycastResult", bytes)));
        }
        if userdata
            .borrow::<crate::game::lua::services::DebrisService>()
            .is_ok()
        {
            return Ok(Some(payload("clawblox.Service.DebrisService", Vec::new())));
        }
        if userdata
            .borrow::<crate::game::lua::services::PhysicsService>()
            .is_ok()
        {
            return Ok(Some(payload("clawblox.Service.PhysicsService", Vec::new())));
        }
        if userdata
            .borrow::<crate::game::lua::services::RemoteEventService>()
            .is_ok()
        {
            return Ok(Some(payload(
                "clawblox.Service.RemoteEventService",
                Vec::new(),
            )));
        }
        if userdata
            .borrow::<crate::game::lua::services::HttpService>()
            .is_ok()
        {
            return Ok(Some(payload("clawblox.Service.HttpService", Vec::new())));
        }
        if let Ok(value) = userdata.borrow::<crate::game::lua::types::Vector2>() {
            let mut bytes = Vec::with_capacity(8);
            write_vector2(&mut bytes, *value);
            return Ok(Some(payload("clawblox.Vector2", bytes)));
        }
        if let Ok(value) = userdata.borrow::<crate::game::lua::types::Vector3>() {
            let mut bytes = Vec::with_capacity(12);
            write_vector3(&mut bytes, *value);
            return Ok(Some(payload("clawblox.Vector3", bytes)));
        }
        if let Ok(value) = userdata.borrow::<crate::game::lua::types::CFrame>() {
            let mut bytes = Vec::with_capacity(48);
            write_cframe(&mut bytes, *value);
            return Ok(Some(payload("clawblox.CFrame", bytes)));
        }
        if let Ok(value) = userdata.borrow::<crate::game::lua::types::Color3>() {
            let mut bytes = Vec::with_capacity(12);
            write_f32(&mut bytes, value.r);
            write_f32(&mut bytes, value.g);
            write_f32(&mut bytes, value.b);
            return Ok(Some(payload("clawblox.Color3", bytes)));
        }
        if let Ok(value) = userdata.borrow::<crate::game::lua::types::UDim>() {
            let mut bytes = Vec::with_capacity(8);
            write_udim(&mut bytes, *value);
            return Ok(Some(payload("clawblox.UDim", bytes)));
        }
        if let Ok(value) = userdata.borrow::<crate::game::lua::types::UDim2>() {
            let mut bytes = Vec::with_capacity(16);
            write_udim(&mut bytes, value.x);
            write_udim(&mut bytes, value.y);
            return Ok(Some(payload("clawblox.UDim2", bytes)));
        }
        if let Ok(value) = userdata.borrow::<crate::game::lua::types::Ray>() {
            let mut bytes = Vec::with_capacity(24);
            write_vector3(&mut bytes, value.origin);
            write_vector3(&mut bytes, value.direction);
            return Ok(Some(payload("clawblox.Ray", bytes)));
        }
        if let Ok(value) = userdata.borrow::<crate::game::lua::types::Region3>() {
            let mut bytes = Vec::with_capacity(24);
            write_vector3(&mut bytes, value.min);
            write_vector3(&mut bytes, value.max);
            return Ok(Some(payload("clawblox.Region3", bytes)));
        }
        if let Ok(value) = userdata.borrow::<crate::game::lua::types::PhysicalProperties>() {
            let mut bytes = Vec::with_capacity(20);
            write_f32(&mut bytes, value.density);
            write_f32(&mut bytes, value.friction);
            write_f32(&mut bytes, value.elasticity);
            write_f32(&mut bytes, value.friction_weight);
            write_f32(&mut bytes, value.elasticity_weight);
            return Ok(Some(payload("clawblox.PhysicalProperties", bytes)));
        }
        if let Ok(value) = userdata.borrow::<crate::game::lua::types::DateTime>() {
            let mut bytes = Vec::with_capacity(8);
            write_i64(&mut bytes, value.unix_timestamp_millis());
            return Ok(Some(payload("clawblox.DateTime", bytes)));
        }
        if let Ok(value) = userdata.borrow::<PartType>() {
            let mut bytes = Vec::with_capacity(1);
            write_part_type(&mut bytes, *value);
            return Ok(Some(payload("clawblox.Enum.PartType", bytes)));
        }
        if let Ok(value) = userdata.borrow::<Material>() {
            let mut bytes = Vec::with_capacity(1);
            write_material(&mut bytes, *value);
            return Ok(Some(payload("clawblox.Enum.Material", bytes)));
        }
        if let Ok(value) = userdata.borrow::<ActuatorRelativeTo>() {
            let mut bytes = Vec::with_capacity(1);
            write_actuator_relative_to(&mut bytes, *value);
            return Ok(Some(payload("clawblox.Enum.ActuatorRelativeTo", bytes)));
        }
        if let Ok(value) = userdata.borrow::<FluidForces>() {
            let mut bytes = Vec::with_capacity(1);
            write_fluid_forces(&mut bytes, *value);
            return Ok(Some(payload("clawblox.Enum.FluidForces", bytes)));
        }
        if let Ok(value) = userdata.borrow::<HumanoidStateType>() {
            let mut bytes = Vec::with_capacity(1);
            write_humanoid_state_type(&mut bytes, *value);
            return Ok(Some(payload("clawblox.Enum.HumanoidStateType", bytes)));
        }
        if let Ok(value) = userdata.borrow::<RaycastFilterType>() {
            let mut bytes = Vec::with_capacity(1);
            write_raycast_filter_type(&mut bytes, *value);
            return Ok(Some(payload("clawblox.Enum.RaycastFilterType", bytes)));
        }
        if let Ok(value) = userdata.borrow::<TextXAlignmentEnum>() {
            let mut bytes = Vec::with_capacity(1);
            write_text_x_alignment(&mut bytes, *value);
            return Ok(Some(payload("clawblox.Enum.TextXAlignment", bytes)));
        }
        if let Ok(value) = userdata.borrow::<TextYAlignmentEnum>() {
            let mut bytes = Vec::with_capacity(1);
            write_text_y_alignment(&mut bytes, *value);
            return Ok(Some(payload("clawblox.Enum.TextYAlignment", bytes)));
        }
        if let Ok(value) = userdata.borrow::<FontEnum>() {
            let mut bytes = Vec::with_capacity(1);
            write_font(&mut bytes, *value);
            return Ok(Some(payload("clawblox.Enum.Font", bytes)));
        }
        if let Ok(track) = userdata.borrow::<AnimationTrack>() {
            let mut bytes = Vec::new();
            write_animation_track_state(&mut bytes, &track.snapshot_state());
            return Ok(Some(payload("clawblox.AnimationTrack", bytes)));
        }

        Ok(None)
    }

    unsafe fn load_userdata(
        &mut self,
        state: *mut lua_State,
        type_key: &str,
        bytes: &[u8],
    ) -> PersistResult<()> {
        let mut input = ByteReader::new(bytes);
        match type_key {
            "clawblox.Instance" => {
                let id = input.read_u64()?;
                input.finish()?;
                let instance = self.instances_by_id.get(&id).cloned().ok_or_else(|| {
                    PersistError::UserDataAdapter(format!("unknown Instance id {id}"))
                })?;
                let userdata = Instance::cached_userdata(&self.lua, &instance)
                    .map_err(|err| PersistError::UserDataAdapter(err.to_string()))?;
                self.lua
                    .globals()
                    .set(USERDATA_BRIDGE_GLOBAL, userdata)
                    .map_err(|err| PersistError::UserDataAdapter(err.to_string()))?;
                lua_getglobal(state, USERDATA_BRIDGE_GLOBAL_CSTR.as_ptr().cast());
                clear_userdata_bridge_global(state);
            }
            "clawblox.RBXScriptSignal" => {
                let identity = input.read_signal_identity()?;
                input.finish()?;
                let signal = resolve_signal_identity(
                    &self.lua,
                    &self.game,
                    &self.instances_by_id,
                    &identity,
                )?;
                self.push_userdata(state, signal)?;
            }
            "clawblox.RBXScriptConnection" => {
                let id = input.read_u64()?;
                let connected = input.read_u8()? != 0;
                let identity = input.read_signal_identity()?;
                input.finish()?;
                let signal = resolve_signal_identity(
                    &self.lua,
                    &self.game,
                    &self.instances_by_id,
                    &identity,
                )?;
                self.push_userdata(state, RBXScriptConnection::restored(id, &signal, connected))?;
            }
            "clawblox.Service.Game" => {
                input.finish()?;
                self.push_userdata(state, self.game.clone())?;
            }
            "clawblox.Service.Workspace" => {
                input.finish()?;
                self.push_userdata(state, self.game.workspace())?;
            }
            "clawblox.Service.Players" => {
                input.finish()?;
                self.push_userdata(state, self.game.players())?;
            }
            "clawblox.Service.RunService" => {
                input.finish()?;
                self.push_userdata(state, self.game.run_service())?;
            }
            "clawblox.Service.AgentInputService" => {
                input.finish()?;
                self.push_userdata(state, self.game.agent_input_service())?;
            }
            "clawblox.Service.DataStoreService" => {
                input.finish()?;
                self.push_userdata(state, self.game.data_store_service())?;
            }
            "clawblox.DataStore" => {
                let snapshot = input.read_data_store_handle()?;
                input.finish()?;
                let store = self.game.data_store_service().restore_data_store(snapshot);
                self.push_userdata(state, store)?;
            }
            "clawblox.OrderedDataStore" => {
                let snapshot = input.read_data_store_handle()?;
                input.finish()?;
                let store = self
                    .game
                    .data_store_service()
                    .restore_ordered_data_store(snapshot);
                self.push_userdata(state, store)?;
            }
            "clawblox.RaycastParams" => {
                let snapshot = input.read_raycast_params()?;
                input.finish()?;
                let filter_instances = self.resolve_instances(&snapshot.filter_instance_ids)?;
                self.push_userdata(state, RaycastParams::restored(snapshot, filter_instances))?;
            }
            "clawblox.OverlapParams" => {
                let snapshot = input.read_overlap_params()?;
                input.finish()?;
                let filter_instances = self.resolve_instances(&snapshot.filter_instance_ids)?;
                self.push_userdata(state, OverlapParams::restored(snapshot, filter_instances))?;
            }
            "clawblox.RaycastResult" => {
                let snapshot = input.read_raycast_result()?;
                input.finish()?;
                let instance = self
                    .instances_by_id
                    .get(&snapshot.instance_id)
                    .cloned()
                    .ok_or_else(|| {
                        PersistError::UserDataAdapter(format!(
                            "unknown RaycastResult Instance id {}",
                            snapshot.instance_id
                        ))
                    })?;
                self.push_userdata(state, RaycastResult::restored(snapshot, instance))?;
            }
            "clawblox.Service.DebrisService" => {
                input.finish()?;
                self.push_userdata(state, self.game.debris_service())?;
            }
            "clawblox.Service.PhysicsService" => {
                input.finish()?;
                self.push_userdata(state, self.game.physics_service())?;
            }
            "clawblox.Service.RemoteEventService" => {
                input.finish()?;
                self.push_userdata(state, self.game.remote_event_service())?;
            }
            "clawblox.Service.HttpService" => {
                input.finish()?;
                self.push_userdata(state, crate::game::lua::services::HttpService::new())?;
            }
            "clawblox.Vector2" => {
                let value = input.read_vector2()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.Vector3" => {
                let value = input.read_vector3()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.CFrame" => {
                let value = input.read_cframe()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.Color3" => {
                let value = crate::game::lua::types::Color3::new(
                    input.read_f32()?,
                    input.read_f32()?,
                    input.read_f32()?,
                );
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.UDim" => {
                let value = input.read_udim()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.UDim2" => {
                let x = input.read_udim()?;
                let y = input.read_udim()?;
                input.finish()?;
                self.push_userdata(state, crate::game::lua::types::UDim2::from_udims(x, y))?;
            }
            "clawblox.Ray" => {
                let origin = input.read_vector3()?;
                let direction = input.read_vector3()?;
                input.finish()?;
                self.push_userdata(state, crate::game::lua::types::Ray::new(origin, direction))?;
            }
            "clawblox.Region3" => {
                let min = input.read_vector3()?;
                let max = input.read_vector3()?;
                input.finish()?;
                self.push_userdata(state, crate::game::lua::types::Region3 { min, max })?;
            }
            "clawblox.PhysicalProperties" => {
                let value = crate::game::lua::types::PhysicalProperties::new(
                    input.read_f32()?,
                    input.read_f32()?,
                    input.read_f32()?,
                    input.read_f32()?,
                    input.read_f32()?,
                );
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.DateTime" => {
                let value = crate::game::lua::types::DateTime::from_unix_timestamp_millis(
                    input.read_i64()?,
                );
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.Enum.PartType" => {
                let value = input.read_part_type()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.Enum.Material" => {
                let value = input.read_material()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.Enum.ActuatorRelativeTo" => {
                let value = input.read_actuator_relative_to()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.Enum.FluidForces" => {
                let value = input.read_fluid_forces()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.Enum.HumanoidStateType" => {
                let value = input.read_humanoid_state_type()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.Enum.RaycastFilterType" => {
                let value = input.read_raycast_filter_type()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.Enum.TextXAlignment" => {
                let value = input.read_text_x_alignment()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.Enum.TextYAlignment" => {
                let value = input.read_text_y_alignment()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.Enum.Font" => {
                let value = input.read_font()?;
                input.finish()?;
                self.push_userdata(state, value)?;
            }
            "clawblox.AnimationTrack" => {
                let track = self.restored_animation_track(input.read_animation_track_state()?);
                input.finish()?;
                self.push_userdata(state, track)?;
            }
            _ => return Err(PersistError::UnsupportedUserData),
        }
        Ok(())
    }
}

unsafe fn clear_userdata_bridge_global(state: *mut lua_State) {
    lua_pushnil(state);
    lua_setglobal(state, USERDATA_BRIDGE_GLOBAL_CSTR.as_ptr().cast());
}

fn collect_instance_map(game: &Game) -> HashMap<u64, Instance> {
    let mut map = HashMap::new();
    insert_instance_tree(&mut map, game.workspace().instance.clone());
    insert_instance_tree(&mut map, game.players().instance.clone());
    insert_instance_tree(&mut map, game.server_script_service());
    insert_instance_tree(&mut map, game.server_storage());
    insert_instance_tree(&mut map, game.replicated_storage());
    map
}

fn insert_instance_tree(map: &mut HashMap<u64, Instance>, root: Instance) {
    map.insert(root.id().0, root.clone());
    for descendant in root.get_descendants() {
        map.insert(descendant.id().0, descendant);
    }
}

#[derive(Debug, Clone)]
enum SignalIdentity {
    Service {
        service: String,
        signal: String,
        name: Option<String>,
    },
    Instance {
        instance_id: u64,
        signal: String,
        name: Option<String>,
    },
    AnimationTrack {
        track_id: u64,
        signal: String,
        name: Option<String>,
    },
}

fn find_connection_signal_identity(
    lua: &Lua,
    game: &Game,
    connection: &RBXScriptConnection,
) -> Option<SignalIdentity> {
    find_signal_identity_by(lua, game, |signal| connection.is_for_signal(signal))
}

fn find_signal_identity(
    lua: &Lua,
    game: &Game,
    signal: &RBXScriptSignal,
) -> Option<SignalIdentity> {
    find_signal_identity_by(lua, game, |candidate| signal.is_same_signal(candidate))
}

fn find_signal_identity_by<F>(lua: &Lua, game: &Game, mut matches: F) -> Option<SignalIdentity>
where
    F: FnMut(&RBXScriptSignal) -> bool,
{
    let mut found = None;
    let _ = visit_service_signals(game, |service, signal_kind, name, signal| {
        if matches(signal) {
            found = Some(SignalIdentity::Service {
                service: service.to_string(),
                signal: signal_kind.to_string(),
                name: name.map(str::to_string),
            });
        }
        Ok(())
    });
    if found.is_some() {
        return found;
    }

    for instance in collect_instance_map(game).into_values() {
        let instance_id = instance.id().0;
        let _ = visit_instance_signals(&instance, |signal_kind, name, signal| {
            if matches(signal) {
                found = Some(SignalIdentity::Instance {
                    instance_id,
                    signal: signal_kind.to_string(),
                    name: name.map(str::to_string),
                });
            }
            Ok(())
        });
        if found.is_some() {
            return found;
        }
    }
    if let Some(scheduler) = lua.app_data_ref::<AnimationScheduler>() {
        let tracks = scheduler.tracks.lock().unwrap().clone();
        for track in tracks {
            let track_id = track.data.lock().unwrap().track_id;
            let _ = visit_animation_track_signals(&track, |signal_kind, name, signal| {
                if matches(signal) {
                    found = Some(SignalIdentity::AnimationTrack {
                        track_id,
                        signal: signal_kind.to_string(),
                        name: name.map(str::to_string),
                    });
                }
                Ok(())
            });
            if found.is_some() {
                return found;
            }
        }
    }
    found
}

fn visit_service_signals<F>(game: &Game, mut visitor: F) -> VmSnapshotResult<()>
where
    F: FnMut(&str, &str, Option<&str>, &RBXScriptSignal) -> VmSnapshotResult<()>,
{
    let run_service = game.run_service();
    let run_data = run_service.data.lock().unwrap();
    visitor("RunService", "Heartbeat", None, &run_data.heartbeat)?;
    visitor("RunService", "Stepped", None, &run_data.stepped)?;
    drop(run_data);

    let players = game.players();
    let players_data = players.data.lock().unwrap();
    visitor("Players", "PlayerAdded", None, &players_data.player_added)?;
    visitor(
        "Players",
        "PlayerRemoving",
        None,
        &players_data.player_removing,
    )?;
    drop(players_data);

    let agent_input = game.agent_input_service();
    let agent_data = agent_input.data.lock().unwrap();
    visitor(
        "AgentInputService",
        "InputReceived",
        None,
        &agent_data.input_received,
    )?;
    Ok(())
}

fn visit_instance_signals<F>(instance: &Instance, mut visitor: F) -> VmSnapshotResult<()>
where
    F: FnMut(&str, Option<&str>, &RBXScriptSignal) -> VmSnapshotResult<()>,
{
    let data = instance.data.lock().unwrap();

    visitor("ChildAdded", None, &data.child_added)?;
    visitor("ChildRemoved", None, &data.child_removed)?;
    visitor("DescendantAdded", None, &data.descendant_added)?;
    visitor("DescendantRemoving", None, &data.descendant_removing)?;
    visitor("AncestryChanged", None, &data.ancestry_changed)?;
    visitor("Destroying", None, &data.destroying)?;
    visitor("AttributeChanged", None, &data.attribute_changed)?;
    for (name, signal) in &data.property_changed_signals {
        visitor("PropertyChanged", Some(name), signal)?;
    }
    for (name, signal) in &data.attribute_changed_signals {
        visitor("AttributeChangedNamed", Some(name), signal)?;
    }

    if let Some(part) = data.part_data.as_ref() {
        visitor("Touched", None, &part.touched)?;
        visitor("TouchEnded", None, &part.touch_ended)?;
    }
    if let Some(humanoid) = data.humanoid_data.as_ref() {
        visitor("Died", None, &humanoid.died)?;
        visitor("HealthChanged", None, &humanoid.health_changed)?;
        visitor("Running", None, &humanoid.running)?;
        visitor("StateChanged", None, &humanoid.state_changed)?;
        visitor("MoveToFinished", None, &humanoid.move_to_finished)?;
    }
    if let Some(player) = data.player_data.as_ref() {
        visitor("CharacterAdded", None, &player.character_added)?;
        visitor("CharacterRemoving", None, &player.character_removing)?;
    }
    if let Some(value) = data.int_value_data.as_ref() {
        visitor("Changed", None, &value.changed)?;
    }
    if let Some(value) = data.number_value_data.as_ref() {
        visitor("Changed", None, &value.changed)?;
    }
    if let Some(value) = data.bool_value_data.as_ref() {
        visitor("Changed", None, &value.changed)?;
    }
    if let Some(value) = data.string_value_data.as_ref() {
        visitor("Changed", None, &value.changed)?;
    }
    if let Some(value) = data.object_value_data.as_ref() {
        visitor("Changed", None, &value.changed)?;
    }
    if let Some(value) = data.cframe_value_data.as_ref() {
        visitor("Changed", None, &value.changed)?;
    }
    if let Some(sound) = data.sound_data.as_ref() {
        visitor("Ended", None, &sound.ended)?;
        visitor("Stopped", None, &sound.stopped)?;
    }
    if let Some(remote) = data.remote_event_data.as_ref() {
        visitor("OnServerEvent", None, &remote.on_server_event)?;
    }
    if let Some(gui) = data.gui_data.as_ref() {
        if let Some(signal) = gui.mouse_button1_click.as_ref() {
            visitor("MouseButton1Click", None, signal)?;
        }
        if let Some(signal) = gui.mouse_button1_down.as_ref() {
            visitor("MouseButton1Down", None, signal)?;
        }
        if let Some(signal) = gui.mouse_button1_up.as_ref() {
            visitor("MouseButton1Up", None, signal)?;
        }
        if let Some(signal) = gui.mouse_enter.as_ref() {
            visitor("MouseEnter", None, signal)?;
        }
        if let Some(signal) = gui.mouse_leave.as_ref() {
            visitor("MouseLeave", None, signal)?;
        }
    }
    Ok(())
}

fn visit_animation_track_signals<F>(track: &AnimationTrack, mut visitor: F) -> VmSnapshotResult<()>
where
    F: FnMut(&str, Option<&str>, &RBXScriptSignal) -> VmSnapshotResult<()>,
{
    let data = track.data.lock().unwrap();
    visitor("Stopped", None, &data.stopped)?;
    visitor("Ended", None, &data.ended)?;
    Ok(())
}

fn capture_signal_connections(lua: &Lua, game: &Game) -> VmSnapshotResult<mlua::Table> {
    let records = lua.create_table().map_err(|err| err.to_string())?;
    let mut next_index = 1;

    collect_service_signal_records(lua, game, &records, &mut next_index)?;
    for instance in collect_instance_map(game).into_values() {
        collect_instance_signal_records(lua, &records, &mut next_index, &instance)?;
    }
    collect_animation_track_signal_records(lua, &records, &mut next_index)?;

    Ok(records)
}

fn collect_service_signal_records(
    lua: &Lua,
    game: &Game,
    records: &mlua::Table,
    next_index: &mut i64,
) -> VmSnapshotResult<()> {
    let run_service = game.run_service();
    let run_data = run_service.data.lock().unwrap();
    add_service_signal_record(
        lua,
        records,
        next_index,
        "RunService",
        "Heartbeat",
        None,
        &run_data.heartbeat,
    )?;
    add_service_signal_record(
        lua,
        records,
        next_index,
        "RunService",
        "Stepped",
        None,
        &run_data.stepped,
    )?;
    drop(run_data);

    let players = game.players();
    let players_data = players.data.lock().unwrap();
    add_service_signal_record(
        lua,
        records,
        next_index,
        "Players",
        "PlayerAdded",
        None,
        &players_data.player_added,
    )?;
    add_service_signal_record(
        lua,
        records,
        next_index,
        "Players",
        "PlayerRemoving",
        None,
        &players_data.player_removing,
    )?;
    drop(players_data);

    let agent_input = game.agent_input_service();
    let agent_data = agent_input.data.lock().unwrap();
    add_service_signal_record(
        lua,
        records,
        next_index,
        "AgentInputService",
        "InputReceived",
        None,
        &agent_data.input_received,
    )?;

    Ok(())
}

fn collect_instance_signal_records(
    lua: &Lua,
    records: &mlua::Table,
    next_index: &mut i64,
    instance: &Instance,
) -> VmSnapshotResult<()> {
    let instance_id = instance.id().0;
    let data = instance.data.lock().unwrap();

    add_instance_signal_record(
        lua,
        records,
        next_index,
        instance_id,
        "ChildAdded",
        None,
        &data.child_added,
    )?;
    add_instance_signal_record(
        lua,
        records,
        next_index,
        instance_id,
        "ChildRemoved",
        None,
        &data.child_removed,
    )?;
    add_instance_signal_record(
        lua,
        records,
        next_index,
        instance_id,
        "DescendantAdded",
        None,
        &data.descendant_added,
    )?;
    add_instance_signal_record(
        lua,
        records,
        next_index,
        instance_id,
        "DescendantRemoving",
        None,
        &data.descendant_removing,
    )?;
    add_instance_signal_record(
        lua,
        records,
        next_index,
        instance_id,
        "AncestryChanged",
        None,
        &data.ancestry_changed,
    )?;
    add_instance_signal_record(
        lua,
        records,
        next_index,
        instance_id,
        "Destroying",
        None,
        &data.destroying,
    )?;
    add_instance_signal_record(
        lua,
        records,
        next_index,
        instance_id,
        "AttributeChanged",
        None,
        &data.attribute_changed,
    )?;
    for (name, signal) in &data.property_changed_signals {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "PropertyChanged",
            Some(name),
            signal,
        )?;
    }
    for (name, signal) in &data.attribute_changed_signals {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "AttributeChangedNamed",
            Some(name),
            signal,
        )?;
    }

    if let Some(part) = data.part_data.as_ref() {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "Touched",
            None,
            &part.touched,
        )?;
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "TouchEnded",
            None,
            &part.touch_ended,
        )?;
    }
    if let Some(humanoid) = data.humanoid_data.as_ref() {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "Died",
            None,
            &humanoid.died,
        )?;
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "HealthChanged",
            None,
            &humanoid.health_changed,
        )?;
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "Running",
            None,
            &humanoid.running,
        )?;
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "StateChanged",
            None,
            &humanoid.state_changed,
        )?;
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "MoveToFinished",
            None,
            &humanoid.move_to_finished,
        )?;
    }
    if let Some(player) = data.player_data.as_ref() {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "CharacterAdded",
            None,
            &player.character_added,
        )?;
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "CharacterRemoving",
            None,
            &player.character_removing,
        )?;
    }
    if let Some(value) = data.int_value_data.as_ref() {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "Changed",
            None,
            &value.changed,
        )?;
    }
    if let Some(value) = data.number_value_data.as_ref() {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "Changed",
            None,
            &value.changed,
        )?;
    }
    if let Some(value) = data.bool_value_data.as_ref() {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "Changed",
            None,
            &value.changed,
        )?;
    }
    if let Some(value) = data.string_value_data.as_ref() {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "Changed",
            None,
            &value.changed,
        )?;
    }
    if let Some(value) = data.object_value_data.as_ref() {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "Changed",
            None,
            &value.changed,
        )?;
    }
    if let Some(value) = data.cframe_value_data.as_ref() {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "Changed",
            None,
            &value.changed,
        )?;
    }
    if let Some(sound) = data.sound_data.as_ref() {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "Ended",
            None,
            &sound.ended,
        )?;
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "Stopped",
            None,
            &sound.stopped,
        )?;
    }
    if let Some(remote) = data.remote_event_data.as_ref() {
        add_instance_signal_record(
            lua,
            records,
            next_index,
            instance_id,
            "OnServerEvent",
            None,
            &remote.on_server_event,
        )?;
    }
    if let Some(gui) = data.gui_data.as_ref() {
        if let Some(signal) = gui.mouse_button1_click.as_ref() {
            add_instance_signal_record(
                lua,
                records,
                next_index,
                instance_id,
                "MouseButton1Click",
                None,
                signal,
            )?;
        }
        if let Some(signal) = gui.mouse_button1_down.as_ref() {
            add_instance_signal_record(
                lua,
                records,
                next_index,
                instance_id,
                "MouseButton1Down",
                None,
                signal,
            )?;
        }
        if let Some(signal) = gui.mouse_button1_up.as_ref() {
            add_instance_signal_record(
                lua,
                records,
                next_index,
                instance_id,
                "MouseButton1Up",
                None,
                signal,
            )?;
        }
        if let Some(signal) = gui.mouse_enter.as_ref() {
            add_instance_signal_record(
                lua,
                records,
                next_index,
                instance_id,
                "MouseEnter",
                None,
                signal,
            )?;
        }
        if let Some(signal) = gui.mouse_leave.as_ref() {
            add_instance_signal_record(
                lua,
                records,
                next_index,
                instance_id,
                "MouseLeave",
                None,
                signal,
            )?;
        }
    }

    Ok(())
}

fn collect_animation_track_signal_records(
    lua: &Lua,
    records: &mlua::Table,
    next_index: &mut i64,
) -> VmSnapshotResult<()> {
    let Some(scheduler) = lua.app_data_ref::<AnimationScheduler>() else {
        return Ok(());
    };
    let tracks = scheduler.tracks.lock().unwrap().clone();
    for track in tracks {
        let track_id = track.data.lock().unwrap().track_id;
        visit_animation_track_signals(&track, |signal_kind, name, signal| {
            add_animation_track_signal_record(
                lua,
                records,
                next_index,
                track_id,
                signal_kind,
                name,
                signal,
            )
        })?;
    }
    Ok(())
}

fn add_service_signal_record(
    lua: &Lua,
    records: &mlua::Table,
    next_index: &mut i64,
    service: &str,
    signal_kind: &str,
    name: Option<&str>,
    signal: &RBXScriptSignal,
) -> VmSnapshotResult<()> {
    add_signal_record(
        lua,
        records,
        next_index,
        "service",
        None,
        Some(service),
        None,
        signal_kind,
        name,
        signal,
    )
}

fn add_instance_signal_record(
    lua: &Lua,
    records: &mlua::Table,
    next_index: &mut i64,
    instance_id: u64,
    signal_kind: &str,
    name: Option<&str>,
    signal: &RBXScriptSignal,
) -> VmSnapshotResult<()> {
    add_signal_record(
        lua,
        records,
        next_index,
        "instance",
        Some(instance_id),
        None,
        None,
        signal_kind,
        name,
        signal,
    )
}

fn add_animation_track_signal_record(
    lua: &Lua,
    records: &mlua::Table,
    next_index: &mut i64,
    track_id: u64,
    signal_kind: &str,
    name: Option<&str>,
    signal: &RBXScriptSignal,
) -> VmSnapshotResult<()> {
    add_signal_record(
        lua,
        records,
        next_index,
        "animation_track",
        None,
        None,
        Some(track_id),
        signal_kind,
        name,
        signal,
    )
}

#[allow(clippy::too_many_arguments)]
fn add_signal_record(
    lua: &Lua,
    records: &mlua::Table,
    next_index: &mut i64,
    target: &str,
    instance_id: Option<u64>,
    service: Option<&str>,
    track_id: Option<u64>,
    signal_kind: &str,
    name: Option<&str>,
    signal: &RBXScriptSignal,
) -> VmSnapshotResult<()> {
    let connections = signal
        .snapshot_connections(lua)
        .map_err(|err| err.to_string())?;
    let waiters = signal
        .snapshot_waiters(lua)
        .map_err(|err| err.to_string())?;
    if connections.is_empty() && waiters.is_empty() {
        return Ok(());
    }

    let record = lua.create_table().map_err(|err| err.to_string())?;
    record
        .set("target", target)
        .map_err(|err| err.to_string())?;
    if let Some(instance_id) = instance_id {
        record
            .set("instance_id", instance_id as i64)
            .map_err(|err| err.to_string())?;
    }
    if let Some(service) = service {
        record
            .set("service", service)
            .map_err(|err| err.to_string())?;
    }
    if let Some(track_id) = track_id {
        record
            .set("track_id", track_id as i64)
            .map_err(|err| err.to_string())?;
    }
    record
        .set("signal", signal_kind)
        .map_err(|err| err.to_string())?;
    if let Some(name) = name {
        record.set("name", name).map_err(|err| err.to_string())?;
    }

    let connection_table = lua.create_table().map_err(|err| err.to_string())?;
    for (idx, connection) in connections.into_iter().enumerate() {
        let entry = lua.create_table().map_err(|err| err.to_string())?;
        entry
            .set("id", connection.id as i64)
            .map_err(|err| err.to_string())?;
        entry
            .set("once", connection.once)
            .map_err(|err| err.to_string())?;
        entry
            .set("callback", connection.callback)
            .map_err(|err| err.to_string())?;
        connection_table
            .set(idx + 1, entry)
            .map_err(|err| err.to_string())?;
    }
    record
        .set("connections", connection_table)
        .map_err(|err| err.to_string())?;

    let waiter_table = lua.create_table().map_err(|err| err.to_string())?;
    for (idx, waiter) in waiters.into_iter().enumerate() {
        waiter_table
            .set(idx + 1, waiter.thread)
            .map_err(|err| err.to_string())?;
    }
    record
        .set("waiters", waiter_table)
        .map_err(|err| err.to_string())?;
    records
        .set(*next_index, record)
        .map_err(|err| err.to_string())?;
    *next_index += 1;
    Ok(())
}

fn restore_signal_connections(
    lua: &Lua,
    game: &Game,
    records: mlua::Table,
) -> VmSnapshotResult<()> {
    let instances = collect_instance_map(game);
    for record in records.sequence_values::<mlua::Table>() {
        let record = record.map_err(|err| err.to_string())?;
        let signal = resolve_signal_record(lua, game, &instances, &record)?;
        let connection_table: mlua::Table =
            record.get("connections").map_err(|err| err.to_string())?;
        let mut connections = Vec::new();
        for entry in connection_table.sequence_values::<mlua::Table>() {
            let entry = entry.map_err(|err| err.to_string())?;
            let id: i64 = entry.get("id").map_err(|err| err.to_string())?;
            if id < 0 {
                return Err("negative signal connection id in Lua VM snapshot".to_string());
            }
            connections.push(SignalConnectionSnapshot {
                id: id as u64,
                callback: entry.get("callback").map_err(|err| err.to_string())?,
                once: entry.get("once").map_err(|err| err.to_string())?,
            });
        }
        signal
            .restore_connections(lua, connections)
            .map_err(|err| err.to_string())?;

        let waiter_table: mlua::Table = record.get("waiters").map_err(|err| err.to_string())?;
        let mut waiters = Vec::new();
        for thread in waiter_table.sequence_values::<Thread>() {
            waiters.push(SignalWaiterSnapshot {
                thread: thread.map_err(|err| err.to_string())?,
            });
        }
        signal
            .restore_waiters(lua, waiters)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn resolve_signal_record(
    lua: &Lua,
    game: &Game,
    instances: &HashMap<u64, Instance>,
    record: &mlua::Table,
) -> VmSnapshotResult<RBXScriptSignal> {
    let target: String = record.get("target").map_err(|err| err.to_string())?;
    let signal: String = record.get("signal").map_err(|err| err.to_string())?;
    let name: Option<String> = record.get("name").map_err(|err| err.to_string())?;
    match target.as_str() {
        "service" => {
            let service: String = record.get("service").map_err(|err| err.to_string())?;
            resolve_service_signal(game, &service, &signal)
        }
        "instance" => {
            let instance_id: i64 = record.get("instance_id").map_err(|err| err.to_string())?;
            if instance_id < 0 {
                return Err("negative signal instance id in Lua VM snapshot".to_string());
            }
            let instance = instances
                .get(&(instance_id as u64))
                .ok_or_else(|| format!("unknown signal instance id {instance_id}"))?;
            resolve_instance_signal(instance, &signal, name.as_deref())
        }
        "animation_track" => {
            let track_id: i64 = record.get("track_id").map_err(|err| err.to_string())?;
            if track_id < 0 {
                return Err("negative animation track signal id in Lua VM snapshot".to_string());
            }
            resolve_animation_track_signal(lua, track_id as u64, &signal, name.as_deref())
        }
        other => Err(format!("unsupported signal target '{other}'")),
    }
}

fn resolve_signal_identity(
    lua: &Lua,
    game: &Game,
    instances: &HashMap<u64, Instance>,
    identity: &SignalIdentity,
) -> PersistResult<RBXScriptSignal> {
    match identity {
        SignalIdentity::Service {
            service,
            signal,
            name: _,
        } => resolve_service_signal(game, service, signal).map_err(PersistError::UserDataAdapter),
        SignalIdentity::Instance {
            instance_id,
            signal,
            name,
        } => {
            let instance = instances.get(instance_id).ok_or_else(|| {
                PersistError::UserDataAdapter(format!(
                    "unknown RBXScriptConnection instance id {instance_id}"
                ))
            })?;
            resolve_instance_signal(instance, signal, name.as_deref())
                .map_err(PersistError::UserDataAdapter)
        }
        SignalIdentity::AnimationTrack {
            track_id,
            signal,
            name,
        } => resolve_animation_track_signal(lua, *track_id, signal, name.as_deref())
            .map_err(PersistError::UserDataAdapter),
    }
}

fn resolve_service_signal(
    game: &Game,
    service: &str,
    signal: &str,
) -> VmSnapshotResult<RBXScriptSignal> {
    match (service, signal) {
        ("RunService", "Heartbeat") => {
            Ok(game.run_service().data.lock().unwrap().heartbeat.clone())
        }
        ("RunService", "Stepped") => Ok(game.run_service().data.lock().unwrap().stepped.clone()),
        ("Players", "PlayerAdded") => Ok(game.players().data.lock().unwrap().player_added.clone()),
        ("Players", "PlayerRemoving") => {
            Ok(game.players().data.lock().unwrap().player_removing.clone())
        }
        ("AgentInputService", "InputReceived") => Ok(game
            .agent_input_service()
            .data
            .lock()
            .unwrap()
            .input_received
            .clone()),
        _ => Err(format!("unsupported service signal {service}.{signal}")),
    }
}

fn resolve_instance_signal(
    instance: &Instance,
    signal: &str,
    name: Option<&str>,
) -> VmSnapshotResult<RBXScriptSignal> {
    match signal {
        "PropertyChanged" => {
            let name =
                name.ok_or_else(|| "PropertyChanged signal missing property name".to_string())?;
            return Ok(instance.ensure_property_signal(name));
        }
        "AttributeChangedNamed" => {
            let name = name.ok_or_else(|| {
                "named AttributeChanged signal missing attribute name".to_string()
            })?;
            return Ok(instance.ensure_attribute_signal(name));
        }
        _ => {}
    }

    let data = instance.data.lock().unwrap();
    match signal {
        "ChildAdded" => Ok(data.child_added.clone()),
        "ChildRemoved" => Ok(data.child_removed.clone()),
        "DescendantAdded" => Ok(data.descendant_added.clone()),
        "DescendantRemoving" => Ok(data.descendant_removing.clone()),
        "AncestryChanged" => Ok(data.ancestry_changed.clone()),
        "Destroying" => Ok(data.destroying.clone()),
        "AttributeChanged" => Ok(data.attribute_changed.clone()),
        "Touched" => data
            .part_data
            .as_ref()
            .map(|part| part.touched.clone())
            .ok_or_else(|| format!("Instance {} has no Touched signal", instance.id().0)),
        "TouchEnded" => data
            .part_data
            .as_ref()
            .map(|part| part.touch_ended.clone())
            .ok_or_else(|| format!("Instance {} has no TouchEnded signal", instance.id().0)),
        "Died" => data
            .humanoid_data
            .as_ref()
            .map(|humanoid| humanoid.died.clone())
            .ok_or_else(|| format!("Instance {} has no Died signal", instance.id().0)),
        "HealthChanged" => data
            .humanoid_data
            .as_ref()
            .map(|humanoid| humanoid.health_changed.clone())
            .ok_or_else(|| format!("Instance {} has no HealthChanged signal", instance.id().0)),
        "Running" => data
            .humanoid_data
            .as_ref()
            .map(|humanoid| humanoid.running.clone())
            .ok_or_else(|| format!("Instance {} has no Running signal", instance.id().0)),
        "StateChanged" => data
            .humanoid_data
            .as_ref()
            .map(|humanoid| humanoid.state_changed.clone())
            .ok_or_else(|| format!("Instance {} has no StateChanged signal", instance.id().0)),
        "MoveToFinished" => data
            .humanoid_data
            .as_ref()
            .map(|humanoid| humanoid.move_to_finished.clone())
            .ok_or_else(|| format!("Instance {} has no MoveToFinished signal", instance.id().0)),
        "CharacterAdded" => data
            .player_data
            .as_ref()
            .map(|player| player.character_added.clone())
            .ok_or_else(|| format!("Instance {} has no CharacterAdded signal", instance.id().0)),
        "CharacterRemoving" => data
            .player_data
            .as_ref()
            .map(|player| player.character_removing.clone())
            .ok_or_else(|| {
                format!(
                    "Instance {} has no CharacterRemoving signal",
                    instance.id().0
                )
            }),
        "Changed" => resolve_changed_signal(instance, &data),
        "Ended" => data
            .sound_data
            .as_ref()
            .map(|sound| sound.ended.clone())
            .ok_or_else(|| format!("Instance {} has no Ended signal", instance.id().0)),
        "Stopped" => data
            .sound_data
            .as_ref()
            .map(|sound| sound.stopped.clone())
            .ok_or_else(|| format!("Instance {} has no Stopped signal", instance.id().0)),
        "OnServerEvent" => data
            .remote_event_data
            .as_ref()
            .map(|remote| remote.on_server_event.clone())
            .ok_or_else(|| format!("Instance {} has no OnServerEvent signal", instance.id().0)),
        "MouseButton1Click" => resolve_gui_signal(instance, &data, signal, |gui| {
            gui.mouse_button1_click.clone()
        }),
        "MouseButton1Down" => resolve_gui_signal(instance, &data, signal, |gui| {
            gui.mouse_button1_down.clone()
        }),
        "MouseButton1Up" => {
            resolve_gui_signal(instance, &data, signal, |gui| gui.mouse_button1_up.clone())
        }
        "MouseEnter" => resolve_gui_signal(instance, &data, signal, |gui| gui.mouse_enter.clone()),
        "MouseLeave" => resolve_gui_signal(instance, &data, signal, |gui| gui.mouse_leave.clone()),
        other => Err(format!("unsupported instance signal '{other}'")),
    }
}

fn resolve_animation_track_signal(
    lua: &Lua,
    track_id: u64,
    signal: &str,
    _name: Option<&str>,
) -> VmSnapshotResult<RBXScriptSignal> {
    let scheduler = lua
        .app_data_ref::<AnimationScheduler>()
        .ok_or_else(|| "AnimationScheduler is missing during VM snapshot restore".to_string())?;
    let track = scheduler
        .find_track_by_id(track_id)
        .ok_or_else(|| format!("unknown animation track signal id {track_id}"))?;
    let data = track.data.lock().unwrap();
    match signal {
        "Stopped" => Ok(data.stopped.clone()),
        "Ended" => Ok(data.ended.clone()),
        other => Err(format!("unsupported animation track signal '{other}'")),
    }
}

fn resolve_changed_signal(
    instance: &Instance,
    data: &crate::game::lua::instance::InstanceData,
) -> VmSnapshotResult<RBXScriptSignal> {
    if let Some(value) = data.int_value_data.as_ref() {
        return Ok(value.changed.clone());
    }
    if let Some(value) = data.number_value_data.as_ref() {
        return Ok(value.changed.clone());
    }
    if let Some(value) = data.bool_value_data.as_ref() {
        return Ok(value.changed.clone());
    }
    if let Some(value) = data.string_value_data.as_ref() {
        return Ok(value.changed.clone());
    }
    if let Some(value) = data.object_value_data.as_ref() {
        return Ok(value.changed.clone());
    }
    if let Some(value) = data.cframe_value_data.as_ref() {
        return Ok(value.changed.clone());
    }
    Err(format!(
        "Instance {} has no Changed signal",
        instance.id().0
    ))
}

fn resolve_gui_signal<F>(
    instance: &Instance,
    data: &crate::game::lua::instance::InstanceData,
    signal: &str,
    get: F,
) -> VmSnapshotResult<RBXScriptSignal>
where
    F: FnOnce(&crate::game::lua::instance::GuiObjectData) -> Option<RBXScriptSignal>,
{
    data.gui_data
        .as_ref()
        .and_then(get)
        .ok_or_else(|| format!("Instance {} has no {signal} signal", instance.id().0))
}

fn payload(type_key: &str, bytes: Vec<u8>) -> UserDataPayload {
    UserDataPayload {
        type_key: type_key.to_string(),
        bytes,
    }
}

fn write_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn write_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_i64(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    write_u64(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

fn write_optional_string(out: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            write_u8(out, 1);
            write_string(out, value);
        }
        None => write_u8(out, 0),
    }
}

fn write_signal_identity(out: &mut Vec<u8>, identity: &SignalIdentity) {
    match identity {
        SignalIdentity::Service {
            service,
            signal,
            name,
        } => {
            write_u8(out, 0);
            write_string(out, service);
            write_string(out, signal);
            write_optional_string(out, name.as_deref());
        }
        SignalIdentity::Instance {
            instance_id,
            signal,
            name,
        } => {
            write_u8(out, 1);
            write_u64(out, *instance_id);
            write_string(out, signal);
            write_optional_string(out, name.as_deref());
        }
        SignalIdentity::AnimationTrack {
            track_id,
            signal,
            name,
        } => {
            write_u8(out, 2);
            write_u64(out, *track_id);
            write_string(out, signal);
            write_optional_string(out, name.as_deref());
        }
    }
}

fn write_vector2(out: &mut Vec<u8>, value: crate::game::lua::types::Vector2) {
    write_f32(out, value.x);
    write_f32(out, value.y);
}

fn write_vector3(out: &mut Vec<u8>, value: crate::game::lua::types::Vector3) {
    write_f32(out, value.x);
    write_f32(out, value.y);
    write_f32(out, value.z);
}

fn write_cframe(out: &mut Vec<u8>, value: crate::game::lua::types::CFrame) {
    write_vector3(out, value.position);
    for row in value.rotation {
        for component in row {
            write_f32(out, component);
        }
    }
}

fn write_udim(out: &mut Vec<u8>, value: crate::game::lua::types::UDim) {
    write_f32(out, value.scale);
    write_i32(out, value.offset);
}

fn write_animation_track_state(out: &mut Vec<u8>, state: &AnimationTrackStateSnapshot) {
    write_u64(out, state.track_id);
    write_string(out, &state.animation_id);
    write_u64(out, state.owner_animator_id);
    write_f32(out, state.length);
    write_i32(out, state.priority);
    write_u8(out, state.looped as u8);
    write_f32(out, state.speed);
    write_f32(out, state.time_position);
    write_u8(out, state.is_playing as u8);
    write_u8(out, state.is_stopping as u8);
    write_f32(out, state.weight_current);
    write_f32(out, state.weight_target);
    write_f32(out, state.effective_weight);
    write_f32(out, state.fade_from);
    write_f32(out, state.fade_to);
    write_f32(out, state.fade_duration);
    write_f32(out, state.fade_elapsed);
}

fn write_data_store_handle(out: &mut Vec<u8>, state: &DataStoreHandleSnapshot) {
    out.extend_from_slice(state.game_id.as_bytes());
    write_string(out, &state.store_name);
}

fn write_part_type(out: &mut Vec<u8>, part_type: PartType) {
    write_u8(
        out,
        match part_type {
            PartType::Ball => 0,
            PartType::Block => 1,
            PartType::Cylinder => 2,
            PartType::Ring => 3,
            PartType::Wedge => 4,
        },
    );
}

fn write_actuator_relative_to(out: &mut Vec<u8>, relative_to: ActuatorRelativeTo) {
    write_u8(
        out,
        match relative_to {
            ActuatorRelativeTo::Attachment0 => 0,
            ActuatorRelativeTo::Attachment1 => 1,
            ActuatorRelativeTo::World => 2,
        },
    );
}

fn write_fluid_forces(out: &mut Vec<u8>, fluid_forces: FluidForces) {
    write_u8(
        out,
        match fluid_forces {
            FluidForces::Default => 0,
            FluidForces::Experimental => 1,
        },
    );
}

fn write_humanoid_state_type(out: &mut Vec<u8>, state_type: HumanoidStateType) {
    write_u8(
        out,
        match state_type {
            HumanoidStateType::Running => 0,
            HumanoidStateType::Landed => 1,
            HumanoidStateType::Jumping => 2,
            HumanoidStateType::Freefall => 3,
            HumanoidStateType::Dead => 4,
            HumanoidStateType::Physics => 5,
            HumanoidStateType::None => 6,
        },
    );
}

fn write_raycast_filter_type(out: &mut Vec<u8>, filter_type: RaycastFilterType) {
    write_u8(
        out,
        match filter_type {
            RaycastFilterType::Include => 0,
            RaycastFilterType::Exclude => 1,
        },
    );
}

fn write_text_x_alignment(out: &mut Vec<u8>, alignment: TextXAlignmentEnum) {
    write_u8(
        out,
        match alignment {
            TextXAlignmentEnum::Left => 0,
            TextXAlignmentEnum::Center => 1,
            TextXAlignmentEnum::Right => 2,
        },
    );
}

fn write_text_y_alignment(out: &mut Vec<u8>, alignment: TextYAlignmentEnum) {
    write_u8(
        out,
        match alignment {
            TextYAlignmentEnum::Top => 0,
            TextYAlignmentEnum::Center => 1,
            TextYAlignmentEnum::Bottom => 2,
        },
    );
}

fn write_font(out: &mut Vec<u8>, font: FontEnum) {
    write_u8(
        out,
        match font {
            FontEnum::GothamBold => 0,
            FontEnum::Gotham => 1,
            FontEnum::SourceSans => 2,
            FontEnum::SourceSansBold => 3,
        },
    );
}

fn write_material(out: &mut Vec<u8>, material: Material) {
    write_u8(
        out,
        match material {
            Material::Air => 0,
            Material::Plastic => 1,
            Material::Wood => 2,
            Material::Metal => 3,
            Material::Glass => 4,
            Material::Neon => 5,
            Material::Concrete => 6,
            Material::Brick => 7,
            Material::Granite => 8,
            Material::Grass => 9,
            Material::Ice => 10,
            Material::Sand => 11,
            Material::Fabric => 12,
            Material::Marble => 13,
            Material::Slate => 14,
            Material::SmoothPlastic => 15,
            Material::ForceField => 16,
            Material::Water => 17,
        },
    );
}

fn write_instance_id_list(out: &mut Vec<u8>, ids: &[u64]) {
    write_u64(out, ids.len() as u64);
    for id in ids {
        write_u64(out, *id);
    }
}

fn write_raycast_params(out: &mut Vec<u8>, state: &RaycastParamsSnapshot) {
    write_raycast_filter_type(out, state.filter_type);
    write_instance_id_list(out, &state.filter_instance_ids);
    write_u8(out, state.ignore_water as u8);
    write_string(out, &state.collision_group);
    write_u8(out, state.respect_can_collide as u8);
}

fn write_overlap_params(out: &mut Vec<u8>, state: &OverlapParamsSnapshot) {
    write_raycast_filter_type(out, state.filter_type);
    write_instance_id_list(out, &state.filter_instance_ids);
    write_u64(out, state.max_parts as u64);
    write_string(out, &state.collision_group);
    write_u8(out, state.respect_can_collide as u8);
}

fn write_raycast_result(out: &mut Vec<u8>, state: &RaycastResultSnapshot) {
    write_u64(out, state.instance_id);
    write_vector3(out, state.position);
    write_vector3(out, state.normal);
    write_f32(out, state.distance);
    write_material(out, state.material);
}

struct ByteReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn finish(&self) -> PersistResult<()> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(PersistError::UserDataAdapter(
                "trailing bytes in Clawblox userdata payload".to_string(),
            ))
        }
    }

    fn read_exact<const N: usize>(&mut self) -> PersistResult<[u8; N]> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(PersistError::UnexpectedEof)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(PersistError::UnexpectedEof)?;
        self.offset = end;
        Ok(slice.try_into().expect("slice length is checked"))
    }

    fn read_u8(&mut self) -> PersistResult<u8> {
        Ok(self.read_exact::<1>()?[0])
    }

    fn read_f32(&mut self) -> PersistResult<f32> {
        Ok(f32::from_le_bytes(self.read_exact()?))
    }

    fn read_i32(&mut self) -> PersistResult<i32> {
        Ok(i32::from_le_bytes(self.read_exact()?))
    }

    fn read_i64(&mut self) -> PersistResult<i64> {
        Ok(i64::from_le_bytes(self.read_exact()?))
    }

    fn read_u64(&mut self) -> PersistResult<u64> {
        Ok(u64::from_le_bytes(self.read_exact()?))
    }

    fn read_string(&mut self) -> PersistResult<String> {
        let len = self.read_u64()?;
        let len = usize::try_from(len)
            .map_err(|_| PersistError::UserDataAdapter("string payload too large".to_string()))?;
        let end = self
            .offset
            .checked_add(len)
            .ok_or(PersistError::UnexpectedEof)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(PersistError::UnexpectedEof)?;
        self.offset = end;
        std::str::from_utf8(slice)
            .map(|value| value.to_string())
            .map_err(|err| PersistError::UserDataAdapter(err.to_string()))
    }

    fn read_optional_string(&mut self) -> PersistResult<Option<String>> {
        match self.read_u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.read_string()?)),
            other => Err(PersistError::UserDataAdapter(format!(
                "invalid optional string flag {other}"
            ))),
        }
    }

    fn read_signal_identity(&mut self) -> PersistResult<SignalIdentity> {
        match self.read_u8()? {
            0 => Ok(SignalIdentity::Service {
                service: self.read_string()?,
                signal: self.read_string()?,
                name: self.read_optional_string()?,
            }),
            1 => Ok(SignalIdentity::Instance {
                instance_id: self.read_u64()?,
                signal: self.read_string()?,
                name: self.read_optional_string()?,
            }),
            2 => Ok(SignalIdentity::AnimationTrack {
                track_id: self.read_u64()?,
                signal: self.read_string()?,
                name: self.read_optional_string()?,
            }),
            other => Err(PersistError::UserDataAdapter(format!(
                "invalid signal identity tag {other}"
            ))),
        }
    }

    fn read_vector2(&mut self) -> PersistResult<crate::game::lua::types::Vector2> {
        Ok(crate::game::lua::types::Vector2::new(
            self.read_f32()?,
            self.read_f32()?,
        ))
    }

    fn read_vector3(&mut self) -> PersistResult<crate::game::lua::types::Vector3> {
        Ok(crate::game::lua::types::Vector3::new(
            self.read_f32()?,
            self.read_f32()?,
            self.read_f32()?,
        ))
    }

    fn read_cframe(&mut self) -> PersistResult<crate::game::lua::types::CFrame> {
        let position = self.read_vector3()?;
        let mut rotation = [[0.0; 3]; 3];
        for row in &mut rotation {
            for component in row {
                *component = self.read_f32()?;
            }
        }
        Ok(crate::game::lua::types::CFrame { position, rotation })
    }

    fn read_udim(&mut self) -> PersistResult<crate::game::lua::types::UDim> {
        Ok(crate::game::lua::types::UDim::new(
            self.read_f32()?,
            self.read_i32()?,
        ))
    }

    fn read_animation_track_state(&mut self) -> PersistResult<AnimationTrackStateSnapshot> {
        Ok(AnimationTrackStateSnapshot {
            track_id: self.read_u64()?,
            animation_id: self.read_string()?,
            owner_animator_id: self.read_u64()?,
            length: self.read_f32()?,
            priority: self.read_i32()?,
            looped: self.read_u8()? != 0,
            speed: self.read_f32()?,
            time_position: self.read_f32()?,
            is_playing: self.read_u8()? != 0,
            is_stopping: self.read_u8()? != 0,
            weight_current: self.read_f32()?,
            weight_target: self.read_f32()?,
            effective_weight: self.read_f32()?,
            fade_from: self.read_f32()?,
            fade_to: self.read_f32()?,
            fade_duration: self.read_f32()?,
            fade_elapsed: self.read_f32()?,
        })
    }

    fn read_data_store_handle(&mut self) -> PersistResult<DataStoreHandleSnapshot> {
        Ok(DataStoreHandleSnapshot {
            game_id: Uuid::from_bytes(self.read_exact()?),
            store_name: self.read_string()?,
        })
    }

    fn read_part_type(&mut self) -> PersistResult<PartType> {
        match self.read_u8()? {
            0 => Ok(PartType::Ball),
            1 => Ok(PartType::Block),
            2 => Ok(PartType::Cylinder),
            3 => Ok(PartType::Ring),
            4 => Ok(PartType::Wedge),
            other => Err(PersistError::UserDataAdapter(format!(
                "invalid PartType tag {other}"
            ))),
        }
    }

    fn read_actuator_relative_to(&mut self) -> PersistResult<ActuatorRelativeTo> {
        match self.read_u8()? {
            0 => Ok(ActuatorRelativeTo::Attachment0),
            1 => Ok(ActuatorRelativeTo::Attachment1),
            2 => Ok(ActuatorRelativeTo::World),
            other => Err(PersistError::UserDataAdapter(format!(
                "invalid ActuatorRelativeTo tag {other}"
            ))),
        }
    }

    fn read_fluid_forces(&mut self) -> PersistResult<FluidForces> {
        match self.read_u8()? {
            0 => Ok(FluidForces::Default),
            1 => Ok(FluidForces::Experimental),
            other => Err(PersistError::UserDataAdapter(format!(
                "invalid FluidForces tag {other}"
            ))),
        }
    }

    fn read_humanoid_state_type(&mut self) -> PersistResult<HumanoidStateType> {
        match self.read_u8()? {
            0 => Ok(HumanoidStateType::Running),
            1 => Ok(HumanoidStateType::Landed),
            2 => Ok(HumanoidStateType::Jumping),
            3 => Ok(HumanoidStateType::Freefall),
            4 => Ok(HumanoidStateType::Dead),
            5 => Ok(HumanoidStateType::Physics),
            6 => Ok(HumanoidStateType::None),
            other => Err(PersistError::UserDataAdapter(format!(
                "invalid HumanoidStateType tag {other}"
            ))),
        }
    }

    fn read_raycast_filter_type(&mut self) -> PersistResult<RaycastFilterType> {
        match self.read_u8()? {
            0 => Ok(RaycastFilterType::Include),
            1 => Ok(RaycastFilterType::Exclude),
            other => Err(PersistError::UserDataAdapter(format!(
                "invalid RaycastFilterType tag {other}"
            ))),
        }
    }

    fn read_text_x_alignment(&mut self) -> PersistResult<TextXAlignmentEnum> {
        match self.read_u8()? {
            0 => Ok(TextXAlignmentEnum::Left),
            1 => Ok(TextXAlignmentEnum::Center),
            2 => Ok(TextXAlignmentEnum::Right),
            other => Err(PersistError::UserDataAdapter(format!(
                "invalid TextXAlignment tag {other}"
            ))),
        }
    }

    fn read_text_y_alignment(&mut self) -> PersistResult<TextYAlignmentEnum> {
        match self.read_u8()? {
            0 => Ok(TextYAlignmentEnum::Top),
            1 => Ok(TextYAlignmentEnum::Center),
            2 => Ok(TextYAlignmentEnum::Bottom),
            other => Err(PersistError::UserDataAdapter(format!(
                "invalid TextYAlignment tag {other}"
            ))),
        }
    }

    fn read_font(&mut self) -> PersistResult<FontEnum> {
        match self.read_u8()? {
            0 => Ok(FontEnum::GothamBold),
            1 => Ok(FontEnum::Gotham),
            2 => Ok(FontEnum::SourceSans),
            3 => Ok(FontEnum::SourceSansBold),
            other => Err(PersistError::UserDataAdapter(format!(
                "invalid Font tag {other}"
            ))),
        }
    }

    fn read_material(&mut self) -> PersistResult<Material> {
        match self.read_u8()? {
            0 => Ok(Material::Air),
            1 => Ok(Material::Plastic),
            2 => Ok(Material::Wood),
            3 => Ok(Material::Metal),
            4 => Ok(Material::Glass),
            5 => Ok(Material::Neon),
            6 => Ok(Material::Concrete),
            7 => Ok(Material::Brick),
            8 => Ok(Material::Granite),
            9 => Ok(Material::Grass),
            10 => Ok(Material::Ice),
            11 => Ok(Material::Sand),
            12 => Ok(Material::Fabric),
            13 => Ok(Material::Marble),
            14 => Ok(Material::Slate),
            15 => Ok(Material::SmoothPlastic),
            16 => Ok(Material::ForceField),
            17 => Ok(Material::Water),
            other => Err(PersistError::UserDataAdapter(format!(
                "invalid Material tag {other}"
            ))),
        }
    }

    fn read_instance_id_list(&mut self) -> PersistResult<Vec<u64>> {
        let len = self.read_u64()?;
        let len = usize::try_from(len)
            .map_err(|_| PersistError::UserDataAdapter("instance id list too large".to_string()))?;
        let mut ids = Vec::with_capacity(len);
        for _ in 0..len {
            ids.push(self.read_u64()?);
        }
        Ok(ids)
    }

    fn read_raycast_params(&mut self) -> PersistResult<RaycastParamsSnapshot> {
        Ok(RaycastParamsSnapshot {
            filter_type: self.read_raycast_filter_type()?,
            filter_instance_ids: self.read_instance_id_list()?,
            ignore_water: self.read_u8()? != 0,
            collision_group: self.read_string()?,
            respect_can_collide: self.read_u8()? != 0,
        })
    }

    fn read_overlap_params(&mut self) -> PersistResult<OverlapParamsSnapshot> {
        let filter_type = self.read_raycast_filter_type()?;
        let filter_instance_ids = self.read_instance_id_list()?;
        let max_parts = usize::try_from(self.read_u64()?)
            .map_err(|_| PersistError::UserDataAdapter("MaxParts too large".to_string()))?;
        Ok(OverlapParamsSnapshot {
            filter_type,
            filter_instance_ids,
            max_parts,
            collision_group: self.read_string()?,
            respect_can_collide: self.read_u8()? != 0,
        })
    }

    fn read_raycast_result(&mut self) -> PersistResult<RaycastResultSnapshot> {
        Ok(RaycastResultSnapshot {
            instance_id: self.read_u64()?,
            position: self.read_vector3()?,
            normal: self.read_vector3()?,
            distance: self.read_f32()?,
            material: self.read_material()?,
        })
    }
}

fn register_dump_permanent_globals(lua: &Lua, permanents: &mlua::Table) -> VmSnapshotResult<()> {
    let globals = lua.globals();
    let baseline = lua
        .named_registry_value::<mlua::Table>(GLOBAL_BASELINE_REGISTRY_KEY)
        .map_err(|err| err.to_string())?;
    for name in PERMANENT_GLOBAL_NAMES {
        let value: mlua::Value = baseline
            .get(*name)
            .or_else(|_| globals.get(*name))
            .unwrap_or(mlua::Value::Nil);
        if is_reference_permanent_candidate(&value) {
            register_dump_permanent_value(permanents, value, permanent_global_key(name), 0)?;
        }
    }
    Ok(())
}

fn register_load_permanent_globals(lua: &Lua, permanents: &mlua::Table) -> VmSnapshotResult<()> {
    let globals = lua.globals();
    for name in PERMANENT_GLOBAL_NAMES {
        let value: mlua::Value = globals.get(*name).unwrap_or(mlua::Value::Nil);
        if is_reference_permanent_candidate(&value) {
            register_load_permanent_value(permanents, permanent_global_key(name), value, 0)?;
        }
    }
    Ok(())
}

fn permanent_global_key(name: &str) -> String {
    format!("__clawblox.global.{name}")
}

fn register_dump_permanent_value(
    permanents: &mlua::Table,
    value: mlua::Value,
    key: String,
    depth: usize,
) -> VmSnapshotResult<()> {
    permanents
        .set(value.clone(), key.clone())
        .map_err(|err| err.to_string())?;
    if depth >= 3 {
        return Ok(());
    }
    let mlua::Value::Table(table) = value else {
        return Ok(());
    };
    for pair in table.pairs::<mlua::Value, mlua::Value>() {
        let (child_key, child_value) = pair.map_err(|err| err.to_string())?;
        if !is_reference_permanent_candidate(&child_value) {
            continue;
        }
        if let Some(segment) = permanent_key_segment(child_key)? {
            register_dump_permanent_value(
                permanents,
                child_value,
                format!("{key}.{segment}"),
                depth + 1,
            )?;
        }
    }
    Ok(())
}

fn register_load_permanent_value(
    permanents: &mlua::Table,
    key: String,
    value: mlua::Value,
    depth: usize,
) -> VmSnapshotResult<()> {
    permanents
        .set(key.clone(), value.clone())
        .map_err(|err| err.to_string())?;
    if depth >= 3 {
        return Ok(());
    }
    let mlua::Value::Table(table) = value else {
        return Ok(());
    };
    for pair in table.pairs::<mlua::Value, mlua::Value>() {
        let (child_key, child_value) = pair.map_err(|err| err.to_string())?;
        if !is_reference_permanent_candidate(&child_value) {
            continue;
        }
        if let Some(segment) = permanent_key_segment(child_key)? {
            register_load_permanent_value(
                permanents,
                format!("{key}.{segment}"),
                child_value,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

fn permanent_key_segment(value: mlua::Value) -> VmSnapshotResult<Option<String>> {
    match value {
        mlua::Value::String(s) => Ok(Some(s.to_str().map_err(|err| err.to_string())?.to_string())),
        mlua::Value::Integer(i) if i >= 0 => Ok(Some(i.to_string())),
        _ => Ok(None),
    }
}

fn register_restored_pending_thread(
    lua: &Lua,
    seen: &mlua::Table,
    pending: &mut Vec<RegistryKey>,
    thread: Thread,
) -> VmSnapshotResult<()> {
    if thread.status() != ThreadStatus::Resumable {
        return Ok(());
    }
    if seen
        .get::<Option<bool>>(thread.clone())
        .map_err(|err| err.to_string())?
        .unwrap_or(false)
    {
        return Ok(());
    }
    seen.set(thread.clone(), true)
        .map_err(|err| err.to_string())?;
    pending.push(
        lua.create_registry_value(thread)
            .map_err(|err| err.to_string())?,
    );
    Ok(())
}

fn is_reference_permanent_candidate(value: &mlua::Value) -> bool {
    matches!(
        value,
        mlua::Value::Table(_)
            | mlua::Value::Function(_)
            | mlua::Value::Thread(_)
            | mlua::Value::UserData(_)
    )
}
