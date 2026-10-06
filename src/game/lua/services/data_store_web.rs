//! Browser demo implementation of DataStoreService.
//!
//! Web simulations are intentionally ephemeral. Keeping the service present
//! preserves Roblox-compatible discovery while making persistence attempts
//! fail clearly instead of pulling the native SQLx/Tokio bridge into WASM.

use mlua::{Lua, UserData, UserDataMethods};
use uuid::Uuid;

#[derive(Clone)]
pub struct DataStoreService {
    game_id: Uuid,
}

impl DataStoreService {
    pub fn new<T>(game_id: Uuid, _backend: Option<T>) -> Self {
        Self { game_id }
    }
}

impl UserData for DataStoreService {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetDataStore", |lua, this, name: String| {
            lua.create_userdata(UnavailableDataStore {
                game_id: this.game_id,
                name,
            })
        });
        methods.add_method("GetOrderedDataStore", |lua, this, name: String| {
            lua.create_userdata(UnavailableDataStore {
                game_id: this.game_id,
                name,
            })
        });
    }
}

#[derive(Clone)]
struct UnavailableDataStore {
    #[allow(dead_code)]
    game_id: Uuid,
    #[allow(dead_code)]
    name: String,
}

fn unavailable(_: &Lua, _: &UnavailableDataStore, _: mlua::MultiValue) -> mlua::Result<()> {
    Err(mlua::Error::RuntimeError(
        "DataStoreService is unavailable in ephemeral browser worlds".into(),
    ))
}

impl UserData for UnavailableDataStore {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetAsync", unavailable);
        methods.add_method("SetAsync", unavailable);
        methods.add_method("RemoveAsync", unavailable);
        methods.add_method("UpdateAsync", unavailable);
        methods.add_method("GetSortedAsync", unavailable);
    }
}
