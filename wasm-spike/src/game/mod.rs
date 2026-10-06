#[path = "../../../src/game/constants.rs"]
pub mod constants;
#[path = "../../../src/game/lua/mod.rs"]
pub mod lua;
#[path = "../../../src/game/profiling.rs"]
pub mod profiling;
#[path = "../../../src/game/replication.rs"]
pub mod replication;
#[path = "../../../src/game/world_root.rs"]
pub mod world_root;

// Browser worlds have no database bridge. LuaRuntime still accepts an optional
// bridge so native callers retain their existing API; this zero-sized type is
// only used to satisfy that browser build boundary.
pub mod async_bridge {
    #[derive(Clone)]
    pub struct AsyncBridge;
}

// Lua's error routing only needs this small policy enum. GameInstance owns the
// native definition; the full simulation extraction will move it into the
// shared crate rather than retaining this spike-only mirror.
pub mod instance {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ErrorMode {
        Continue,
        Halt,
    }
}

