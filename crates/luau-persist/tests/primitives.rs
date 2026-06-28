use std::ffi::CString;

use mlua_sys::{
    luaL_newstate, luaL_openlibs, lua_CFunction, lua_CompileOptions, lua_close, lua_getfield,
    lua_getmetatable, lua_getreadonly, lua_gettop, lua_isnil, lua_newbuffer, lua_newtable,
    lua_newthread, lua_newuserdata_t, lua_pcall, lua_pop, lua_pushboolean, lua_pushcfunction,
    lua_pushlightuserdatatagged, lua_pushnil, lua_pushnumber, lua_pushstring_, lua_pushvalue,
    lua_pushvector, lua_rawequal, lua_rawset, lua_remove, lua_resume, lua_setfield,
    lua_setmetatable, lua_setreadonly, lua_toboolean, lua_tobuffer, lua_tolstring, lua_tonumber,
    lua_tothread, lua_touserdata, lua_tovector, lua_type, lua_xmove, luau_compile, luau_load,
    LUA_GLOBALSINDEX, LUA_OK, LUA_TFUNCTION, LUA_TLIGHTUSERDATA, LUA_TNUMBER, LUA_TSTRING,
    LUA_TTHREAD, LUA_TUSERDATA, LUA_YIELD,
};
use pretty_assertions::assert_eq;

use luau_persist::{
    bridge::{closure_uprefs, thread_info},
    dump_stack_value, dump_stack_value_with_options, dump_stack_value_with_permanents,
    load_stack_value, load_stack_value_with_options, load_stack_value_with_permanents, DumpOptions,
    LoadOptions, PersistError, UserDataAdapter, UserDataPayload, FORMAT_MAGIC, FORMAT_VERSION,
};

struct State(*mut mlua_sys::lua_State);

impl State {
    fn new() -> Self {
        let state = unsafe { luaL_newstate() };
        assert!(!state.is_null());
        Self(state)
    }
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe { lua_close(self.0) };
    }
}

unsafe extern "C-unwind" fn permanent_test_fn(_: *mut mlua_sys::lua_State) -> std::os::raw::c_int {
    0
}

unsafe extern "C-unwind" fn permanent_answer_fn(
    state: *mut mlua_sys::lua_State,
) -> std::os::raw::c_int {
    lua_pushnumber(state, 99.0);
    1
}

#[derive(Default)]
struct U64UserDataAdapter;

impl UserDataAdapter for U64UserDataAdapter {
    unsafe fn dump_userdata(
        &mut self,
        state: *mut mlua_sys::lua_State,
        index: std::os::raw::c_int,
    ) -> Result<Option<UserDataPayload>, PersistError> {
        let ptr = lua_touserdata(state, index).cast::<u64>();
        if ptr.is_null() {
            return Ok(None);
        }

        Ok(Some(UserDataPayload {
            type_key: "test.u64".to_string(),
            bytes: (*ptr).to_le_bytes().to_vec(),
        }))
    }

    unsafe fn load_userdata(
        &mut self,
        state: *mut mlua_sys::lua_State,
        type_key: &str,
        bytes: &[u8],
    ) -> Result<(), PersistError> {
        if type_key != "test.u64" || bytes.len() != 8 {
            return Err(PersistError::UnsupportedUserData);
        }

        let value = u64::from_le_bytes(bytes.try_into().expect("length checked"));
        lua_newuserdata_t(state, value);
        Ok(())
    }
}

fn push_stripped_luau_chunk(state: *mut mlua_sys::lua_State, source: &str) {
    let mut options = lua_CompileOptions::default();
    options.debugLevel = 0;
    options.typeInfoLevel = 0;
    let bytecode = unsafe { luau_compile(source.as_bytes(), options) };
    let chunk = CString::new("persist_test").unwrap();
    let status = unsafe {
        luau_load(
            state,
            chunk.as_ptr(),
            bytecode.as_ptr().cast(),
            bytecode.len(),
            0,
        )
    };
    assert_eq!(status, 0);
}

fn push_default_luau_chunk(state: *mut mlua_sys::lua_State, source: &str) {
    let bytecode = unsafe { luau_compile(source.as_bytes(), lua_CompileOptions::default()) };
    let chunk = CString::new("persist_test").unwrap();
    let status = unsafe {
        luau_load(
            state,
            chunk.as_ptr(),
            bytecode.as_ptr().cast(),
            bytecode.len(),
            0,
        )
    };
    assert_eq!(status, 0);
}

unsafe fn push_dump_permanents_with_coroutine(state: *mut mlua_sys::lua_State) {
    lua_newtable(state);
    let permanents = lua_gettop(state);

    lua_pushvalue(state, LUA_GLOBALSINDEX);
    lua_pushstring_(state, c"globals".as_ptr());
    lua_rawset(state, permanents);

    lua_getfield(state, LUA_GLOBALSINDEX, c"coroutine".as_ptr());
    lua_pushstring_(state, c"coroutine".as_ptr());
    lua_rawset(state, permanents);

    lua_getfield(state, LUA_GLOBALSINDEX, c"coroutine".as_ptr());
    lua_getfield(state, -1, c"yield".as_ptr());
    lua_pushstring_(state, c"coroutine.yield".as_ptr());
    lua_rawset(state, permanents);
    lua_pop(state, 1);

    lua_getfield(state, LUA_GLOBALSINDEX, c"pcall".as_ptr());
    lua_pushstring_(state, c"pcall".as_ptr());
    lua_rawset(state, permanents);
}

unsafe fn push_load_permanents_with_coroutine(state: *mut mlua_sys::lua_State) {
    lua_newtable(state);
    let permanents = lua_gettop(state);

    lua_pushstring_(state, c"globals".as_ptr());
    lua_pushvalue(state, LUA_GLOBALSINDEX);
    lua_rawset(state, permanents);

    lua_pushstring_(state, c"coroutine".as_ptr());
    lua_getfield(state, LUA_GLOBALSINDEX, c"coroutine".as_ptr());
    lua_rawset(state, permanents);

    lua_pushstring_(state, c"coroutine.yield".as_ptr());
    lua_getfield(state, LUA_GLOBALSINDEX, c"coroutine".as_ptr());
    lua_getfield(state, -1, c"yield".as_ptr());
    lua_remove(state, -2);
    lua_rawset(state, permanents);

    lua_pushstring_(state, c"pcall".as_ptr());
    lua_getfield(state, LUA_GLOBALSINDEX, c"pcall".as_ptr());
    lua_rawset(state, permanents);
}

#[test]
fn round_trips_nil() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_pushnil(source.0);
        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        assert_eq!(lua_gettop(target.0), 1);
        assert_ne!(lua_isnil(target.0, -1), 0);
    }
}

#[test]
fn round_trips_boolean() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_pushboolean(source.0, 1);
        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        assert_eq!(lua_toboolean(target.0, -1), 1);
    }
}

#[test]
fn round_trips_number() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_pushnumber(source.0, 123.5);
        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        assert_eq!(lua_tonumber(target.0, -1), 123.5);
    }
}

#[test]
fn round_trips_string() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_pushstring_(source.0, c"hello".as_ptr());
        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        let mut len = 0usize;
        let ptr = lua_tolstring(target.0, -1, &mut len);
        let text = std::slice::from_raw_parts(ptr.cast::<u8>(), len);
        assert_eq!(text, b"hello");
    }
}

#[test]
fn round_trips_vector() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_pushvector(source.0, 1.25, -2.5, 3.75);
        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        let ptr = lua_tovector(target.0, -1);
        let values = std::slice::from_raw_parts(ptr, 3);
        assert_eq!(values, [1.25, -2.5, 3.75]);
    }
}

#[test]
fn round_trips_buffer() {
    let source = State::new();
    let target = State::new();

    unsafe {
        let buffer = lua_newbuffer(source.0, 4);
        std::ptr::copy_nonoverlapping([1u8, 2, 3, 4].as_ptr(), buffer.cast::<u8>(), 4);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        let mut len = 0usize;
        let ptr = lua_tobuffer(target.0, -1, &mut len);
        assert_eq!(
            std::slice::from_raw_parts(ptr.cast::<u8>(), len),
            [1, 2, 3, 4]
        );
    }
}

#[test]
fn rejects_trailing_bytes() {
    let source = State::new();

    unsafe {
        lua_pushnil(source.0);
        let mut bytes = dump_stack_value(source.0, -1).unwrap();
        bytes.push(0xff);

        let target = State::new();
        assert_eq!(
            load_stack_value(target.0, &bytes).unwrap_err(),
            PersistError::TrailingBytes
        );
    }
}

#[test]
fn rejects_unknown_reference() {
    let target = State::new();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(FORMAT_MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.push(0x11);
    bytes.extend_from_slice(&0u32.to_le_bytes());

    unsafe {
        assert_eq!(
            load_stack_value(target.0, &bytes).unwrap_err(),
            PersistError::UnknownReference(0)
        );
    }
}

#[test]
fn rejects_out_of_order_object_id() {
    let target = State::new();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(FORMAT_MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.push(0x10);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());

    unsafe {
        assert_eq!(
            load_stack_value(target.0, &bytes).unwrap_err(),
            PersistError::InvalidObjectId { id: 1, expected: 0 }
        );
    }
}

#[test]
fn rejects_wrong_vector_size() {
    let target = State::new();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(FORMAT_MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.push(0x06);
    bytes.push(4);
    for value in [1.0f32, 2.0, 3.0, 4.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    unsafe {
        assert_eq!(
            load_stack_value(target.0, &bytes).unwrap_err(),
            PersistError::InvalidVectorSize {
                found: 4,
                expected: 3
            }
        );
    }
}

#[test]
fn rejects_unknown_table_flags() {
    let target = State::new();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(FORMAT_MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.push(0x10);
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.push(0x80);

    unsafe {
        assert_eq!(
            load_stack_value(target.0, &bytes).unwrap_err(),
            PersistError::InvalidTableFlags(0x80)
        );
    }
}

#[test]
fn round_trips_permanent_c_function() {
    let source = State::new();
    let target = State::new();
    let permanent_fn: lua_CFunction = permanent_test_fn;

    unsafe {
        lua_newtable(source.0);
        let source_permanents = lua_gettop(source.0);
        lua_pushcfunction(source.0, permanent_fn);
        lua_pushvalue(source.0, -1);
        lua_pushstring_(source.0, c"native.test".as_ptr());
        lua_rawset(source.0, source_permanents);

        let bytes =
            dump_stack_value_with_permanents(source.0, -1, Some(source_permanents)).unwrap();

        lua_newtable(target.0);
        let target_permanents = lua_gettop(target.0);
        lua_pushstring_(target.0, c"native.test".as_ptr());
        lua_pushcfunction(target.0, permanent_fn);
        lua_rawset(target.0, target_permanents);

        load_stack_value_with_permanents(target.0, &bytes, Some(target_permanents)).unwrap();

        assert_eq!(lua_type(target.0, -1), LUA_TFUNCTION);
        lua_getfield(target.0, target_permanents, c"native.test".as_ptr());
        assert_ne!(lua_rawequal(target.0, -1, -2), 0);
    }
}

#[test]
fn round_trips_lua_closure_capturing_permanent_c_function() {
    let source = State::new();
    let target = State::new();
    let permanent_fn: lua_CFunction = permanent_answer_fn;

    unsafe {
        lua_newtable(source.0);
        let source_permanents = lua_gettop(source.0);
        lua_pushcfunction(source.0, permanent_fn);
        let permanent_value = lua_gettop(source.0);
        lua_pushvalue(source.0, -1);
        lua_pushstring_(source.0, c"native.answer".as_ptr());
        lua_rawset(source.0, source_permanents);

        push_stripped_luau_chunk(
            source.0,
            r#"
                local f = ...
                return function()
                    return f()
                end
            "#,
        );
        lua_pushvalue(source.0, permanent_value);
        assert_eq!(lua_pcall(source.0, 1, 1, 0), LUA_OK);

        let bytes =
            dump_stack_value_with_permanents(source.0, -1, Some(source_permanents)).unwrap();

        lua_newtable(target.0);
        let target_permanents = lua_gettop(target.0);
        lua_pushstring_(target.0, c"native.answer".as_ptr());
        lua_pushcfunction(target.0, permanent_fn);
        lua_rawset(target.0, target_permanents);

        load_stack_value_with_permanents(target.0, &bytes, Some(target_permanents)).unwrap();
        assert_eq!(lua_type(target.0, -1), LUA_TFUNCTION);
        assert_eq!(lua_pcall(target.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_tonumber(target.0, -1), 99.0);
    }
}

#[test]
fn permanent_value_requires_load_table() {
    let source = State::new();
    let target = State::new();
    let permanent_fn: lua_CFunction = permanent_test_fn;

    unsafe {
        lua_newtable(source.0);
        let source_permanents = lua_gettop(source.0);
        lua_pushcfunction(source.0, permanent_fn);
        lua_pushvalue(source.0, -1);
        lua_pushstring_(source.0, c"native.test".as_ptr());
        lua_rawset(source.0, source_permanents);

        let bytes =
            dump_stack_value_with_permanents(source.0, -1, Some(source_permanents)).unwrap();

        assert_eq!(
            load_stack_value(target.0, &bytes).unwrap_err(),
            PersistError::MissingPermanentTable
        );
    }
}

#[test]
fn permanent_value_type_mismatch_is_rejected() {
    let source = State::new();
    let target = State::new();
    let permanent_fn: lua_CFunction = permanent_test_fn;

    unsafe {
        lua_newtable(source.0);
        let source_permanents = lua_gettop(source.0);
        lua_pushcfunction(source.0, permanent_fn);
        lua_pushvalue(source.0, -1);
        lua_pushstring_(source.0, c"native.test".as_ptr());
        lua_rawset(source.0, source_permanents);

        let bytes =
            dump_stack_value_with_permanents(source.0, -1, Some(source_permanents)).unwrap();

        lua_newtable(target.0);
        let target_permanents = lua_gettop(target.0);
        lua_pushstring_(target.0, c"native.test".as_ptr());
        lua_pushnumber(target.0, 1.0);
        lua_rawset(target.0, target_permanents);

        assert_eq!(
            load_stack_value_with_permanents(target.0, &bytes, Some(target_permanents))
                .unwrap_err(),
            PersistError::PermanentTypeMismatch {
                expected_type: LUA_TFUNCTION,
                expected_name: "function".to_string(),
                found_type: LUA_TNUMBER,
                found_name: "number".to_string()
            }
        );
    }
}

#[test]
fn round_trips_table_entries() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_newtable(source.0);
        lua_pushstring_(source.0, c"name".as_ptr());
        lua_pushstring_(source.0, c"root".as_ptr());
        lua_rawset(source.0, -3);
        lua_pushstring_(source.0, c"height".as_ptr());
        lua_pushnumber(source.0, 42.0);
        lua_rawset(source.0, -3);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        lua_getfield(target.0, -1, c"name".as_ptr());
        assert_eq!(lua_type(target.0, -1), LUA_TSTRING);
        let mut len = 0usize;
        let ptr = lua_tolstring(target.0, -1, &mut len);
        assert_eq!(std::slice::from_raw_parts(ptr.cast::<u8>(), len), b"root");

        lua_getfield(target.0, -2, c"height".as_ptr());
        assert_eq!(lua_type(target.0, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(target.0, -1), 42.0);
    }
}

#[test]
fn round_trips_self_referential_table() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_newtable(source.0);
        lua_pushstring_(source.0, c"self".as_ptr());
        lua_pushvalue(source.0, -2);
        lua_rawset(source.0, -3);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        lua_getfield(target.0, -1, c"self".as_ptr());
        assert_ne!(lua_rawequal(target.0, -1, -2), 0);
    }
}

#[test]
fn round_trips_table_metatable() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_newtable(source.0); // root
        lua_newtable(source.0); // metatable
        lua_pushstring_(source.0, c"kind".as_ptr());
        lua_pushstring_(source.0, c"meta".as_ptr());
        lua_rawset(source.0, -3);
        lua_setmetatable(source.0, -2);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        assert_ne!(lua_getmetatable(target.0, -1), 0);
        lua_getfield(target.0, -1, c"kind".as_ptr());
        let mut len = 0usize;
        let ptr = lua_tolstring(target.0, -1, &mut len);
        assert_eq!(std::slice::from_raw_parts(ptr.cast::<u8>(), len), b"meta");
    }
}

#[test]
fn round_trips_table_metatable_cycle() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_newtable(source.0);
        lua_pushvalue(source.0, -1);
        lua_setmetatable(source.0, -2);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        assert_ne!(lua_getmetatable(target.0, -1), 0);
        assert_ne!(lua_rawequal(target.0, -1, -2), 0);
    }
}

#[test]
fn round_trips_deeply_nested_table_graph() {
    let source = State::new();
    let target = State::new();

    unsafe {
        push_stripped_luau_chunk(
            source.0,
            r#"
                local root = {}
                local current = root
                for i = 1, 220 do
                    local child = { index = i }
                    current[i] = child
                    current = child
                end
                return root
            "#,
        );
        assert_eq!(lua_pcall(source.0, 0, 1, 0), LUA_OK);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        push_stripped_luau_chunk(
            target.0,
            r#"
                local current = ...
                for i = 1, 220 do
                    current = current[i]
                end
                return current.index
            "#,
        );
        lua_pushvalue(target.0, -2);
        assert_eq!(lua_pcall(target.0, 1, 1, 0), LUA_OK);
        assert_eq!(lua_tonumber(target.0, -1), 220.0);
    }
}

#[test]
fn round_trips_table_readonly_flag() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_newtable(source.0);
        lua_setreadonly(source.0, -1, 1);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        assert_eq!(lua_getreadonly(target.0, -1), 1);
    }
}

#[test]
fn round_trips_shared_table_identity() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_newtable(source.0); // root
        lua_newtable(source.0); // child

        lua_pushstring_(source.0, c"a".as_ptr());
        lua_pushvalue(source.0, -2);
        lua_rawset(source.0, -4);

        lua_pushstring_(source.0, c"b".as_ptr());
        lua_pushvalue(source.0, -2);
        lua_rawset(source.0, -4);

        let bytes = dump_stack_value(source.0, -2).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        lua_getfield(target.0, -1, c"a".as_ptr());
        lua_getfield(target.0, -2, c"b".as_ptr());
        assert_ne!(lua_rawequal(target.0, -1, -2), 0);
    }
}

#[test]
fn round_trips_shared_buffer_identity() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_newtable(source.0); // root
        let buffer = lua_newbuffer(source.0, 2);
        std::ptr::copy_nonoverlapping([7u8, 8].as_ptr(), buffer.cast::<u8>(), 2);

        lua_pushstring_(source.0, c"a".as_ptr());
        lua_pushvalue(source.0, -2);
        lua_rawset(source.0, -4);

        lua_pushstring_(source.0, c"b".as_ptr());
        lua_pushvalue(source.0, -2);
        lua_rawset(source.0, -4);

        let bytes = dump_stack_value(source.0, -2).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        lua_getfield(target.0, -1, c"a".as_ptr());
        lua_getfield(target.0, -2, c"b".as_ptr());
        assert_ne!(lua_rawequal(target.0, -1, -2), 0);
    }
}

#[test]
fn round_trips_userdata_with_adapter_and_preserves_identity() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_newtable(source.0);
        lua_newuserdata_t(source.0, 0xabcddcba_u64);
        lua_pushvalue(source.0, -1);
        lua_setfield(source.0, -3, c"a".as_ptr());
        lua_setfield(source.0, -2, c"b".as_ptr());

        let mut dump_adapter = U64UserDataAdapter;
        let bytes = dump_stack_value_with_options(
            source.0,
            -1,
            DumpOptions {
                permanents_index: None,
                userdata_adapter: Some(&mut dump_adapter),
            },
        )
        .unwrap();

        let mut load_adapter = U64UserDataAdapter;
        load_stack_value_with_options(
            target.0,
            &bytes,
            LoadOptions {
                permanents_index: None,
                userdata_adapter: Some(&mut load_adapter),
            },
        )
        .unwrap();

        lua_getfield(target.0, -1, c"a".as_ptr());
        assert_eq!(lua_type(target.0, -1), LUA_TUSERDATA);
        let ptr = lua_touserdata(target.0, -1).cast::<u64>();
        assert!(!ptr.is_null());
        assert_eq!(*ptr, 0xabcddcba_u64);

        lua_getfield(target.0, -2, c"b".as_ptr());
        assert_eq!(lua_type(target.0, -1), LUA_TUSERDATA);
        assert_eq!(lua_rawequal(target.0, -1, -2), 1);
    }
}

#[test]
fn round_trips_userdata_inside_shared_closed_upvalue() {
    let source = State::new();
    let target = State::new();

    unsafe {
        lua_newuserdata_t(source.0, 0xfeedface_u64);
        lua_setfield(source.0, LUA_GLOBALSINDEX, c"seed".as_ptr());
        push_stripped_luau_chunk(
            source.0,
            r#"
            local shared = seed
            local function first()
                return shared
            end
            local function second()
                return shared
            end
            return { first = first, second = second }
            "#,
        );
        assert_eq!(lua_pcall(source.0, 0, 1, 0), LUA_OK);

        let mut dump_adapter = U64UserDataAdapter;
        let bytes = dump_stack_value_with_options(
            source.0,
            -1,
            DumpOptions {
                permanents_index: None,
                userdata_adapter: Some(&mut dump_adapter),
            },
        )
        .unwrap();

        let mut load_adapter = U64UserDataAdapter;
        load_stack_value_with_options(
            target.0,
            &bytes,
            LoadOptions {
                permanents_index: None,
                userdata_adapter: Some(&mut load_adapter),
            },
        )
        .unwrap();

        lua_getfield(target.0, -1, c"first".as_ptr());
        assert_eq!(lua_pcall(target.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_type(target.0, -1), LUA_TUSERDATA);
        let ptr = lua_touserdata(target.0, -1).cast::<u64>();
        assert!(!ptr.is_null());
        assert_eq!(*ptr, 0xfeedface_u64);

        lua_getfield(target.0, -2, c"second".as_ptr());
        assert_eq!(lua_pcall(target.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_type(target.0, -1), LUA_TUSERDATA);
        assert_eq!(lua_rawequal(target.0, -1, -2), 1);
    }
}

#[test]
fn rejects_userdata_without_adapter() {
    let source = State::new();

    unsafe {
        lua_newuserdata_t(source.0, 123_u64);
        assert_eq!(
            dump_stack_value(source.0, -1).unwrap_err(),
            PersistError::UnsupportedType {
                type_code: LUA_TUSERDATA,
                type_name: "userdata".to_string(),
            }
        );
    }
}

#[test]
fn rejects_malformed_function_payload() {
    let state = State::new();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(FORMAT_MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.push(0x20);

    unsafe {
        assert_eq!(
            load_stack_value(state.0, &bytes).unwrap_err(),
            PersistError::UnexpectedEof
        );
    }
}

#[test]
fn round_trips_luau_closure_without_upvalues() {
    let source = State::new();
    let target = State::new();

    unsafe {
        push_stripped_luau_chunk(source.0, "return function(x) return x + 1 end");
        assert_eq!(lua_pcall(source.0, 0, 1, 0), LUA_OK);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        assert_eq!(lua_type(target.0, -1), LUA_TFUNCTION);
        lua_pushnumber(target.0, 41.0);
        assert_eq!(lua_pcall(target.0, 1, 1, 0), LUA_OK);
        assert_eq!(lua_type(target.0, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(target.0, -1), 42.0);
    }
}

#[test]
fn round_trips_luau_closure_with_direct_upvalue() {
    let source = State::new();
    let target = State::new();

    unsafe {
        push_stripped_luau_chunk(
            source.0,
            r#"
            local function make_value(x)
                return function()
                    return x
                end
            end
            return make_value(17)
            "#,
        );
        assert_eq!(lua_pcall(source.0, 0, 1, 0), LUA_OK);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        assert_eq!(lua_type(target.0, -1), LUA_TFUNCTION);
        assert_eq!(lua_pcall(target.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_type(target.0, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(target.0, -1), 17.0);
    }
}

#[test]
fn round_trips_luau_closures_with_shared_closed_upvalue() {
    let source = State::new();
    let target = State::new();

    unsafe {
        push_stripped_luau_chunk(
            source.0,
            r#"
            local shared = 40
            local function inc()
                shared += 1
                return shared
            end
            local function get()
                return shared
            end
            return { inc = inc, get = get }
            "#,
        );
        assert_eq!(lua_pcall(source.0, 0, 1, 0), LUA_OK);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        lua_getfield(target.0, -1, c"inc".as_ptr());
        let inc_uprefs = closure_uprefs(target.0, -1).unwrap();
        assert_eq!(inc_uprefs.len(), 1);
        assert_eq!(inc_uprefs[0].is_upvalue, 1);
        assert_eq!(inc_uprefs[0].is_open, 0);
        assert_eq!(lua_pcall(target.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_tonumber(target.0, -1), 41.0);

        lua_getfield(target.0, -2, c"get".as_ptr());
        let get_uprefs = closure_uprefs(target.0, -1).unwrap();
        assert_eq!(get_uprefs.len(), 1);
        assert_eq!(get_uprefs[0].identity, inc_uprefs[0].identity);
        assert_eq!(lua_pcall(target.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_tonumber(target.0, -1), 41.0);
    }
}

#[test]
fn round_trips_large_luau_closure_graph() {
    let source = State::new();
    let target = State::new();

    let function_count = 5200usize;
    let mut script = String::from("local t = {}\n");
    for index in 1..=function_count {
        script.push_str(&format!("t.f{index} = function() return {index} end\n"));
    }
    script.push_str("return t");

    unsafe {
        push_stripped_luau_chunk(source.0, &script);
        assert_eq!(lua_pcall(source.0, 0, 1, 0), LUA_OK);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();

        for index in [1usize, 4097, function_count] {
            let field = CString::new(format!("f{index}")).unwrap();
            lua_getfield(target.0, -1, field.as_ptr());
            assert_eq!(lua_type(target.0, -1), LUA_TFUNCTION);
            assert_eq!(lua_pcall(target.0, 0, 1, 0), LUA_OK);
            assert_eq!(lua_type(target.0, -1), LUA_TNUMBER);
            assert_eq!(lua_tonumber(target.0, -1), index as f64);
            lua_pop(target.0, 1);
        }
    }
}

#[test]
fn round_trips_yielded_thread_without_open_upvalues() {
    let source = State::new();
    let target = State::new();

    unsafe {
        luaL_openlibs(source.0);
        luaL_openlibs(target.0);

        let source_thread = lua_newthread(source.0);
        assert!(!source_thread.is_null());
        let source_thread_index = lua_gettop(source.0);

        push_stripped_luau_chunk(
            source.0,
            r#"
            local function worker()
                local value = 7
                coroutine.yield(value)
                return value + 2
            end
            return worker()
            "#,
        );
        lua_xmove(source.0, source_thread, 1);

        let mut nresults = 0;
        assert_eq!(
            lua_resume(source_thread, source.0, 0, &mut nresults),
            LUA_YIELD
        );
        assert!(nresults >= 1);

        push_dump_permanents_with_coroutine(source.0);
        let source_permanents = lua_gettop(source.0);
        let bytes = dump_stack_value_with_permanents(
            source.0,
            source_thread_index,
            Some(source_permanents),
        )
        .unwrap();

        push_load_permanents_with_coroutine(target.0);
        let target_permanents = lua_gettop(target.0);
        load_stack_value_with_permanents(target.0, &bytes, Some(target_permanents)).unwrap();

        assert_eq!(lua_type(target.0, -1), LUA_TTHREAD);
        let loaded_thread = lua_tothread(target.0, -1);
        assert!(!loaded_thread.is_null());
        let info = thread_info(target.0, -1).unwrap();
        assert_eq!(info.status, LUA_YIELD);

        let mut restored_results = 0;
        assert_eq!(
            lua_resume(loaded_thread, target.0, 0, &mut restored_results),
            LUA_OK
        );
        assert_eq!(restored_results, 1);
        assert_eq!(lua_type(loaded_thread, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(loaded_thread, -1), 9.0);
    }
}

#[test]
fn round_trips_yielded_thread_with_open_upvalues() {
    let source = State::new();
    let target = State::new();

    unsafe {
        luaL_openlibs(source.0);
        luaL_openlibs(target.0);

        let source_thread = lua_newthread(source.0);
        assert!(!source_thread.is_null());
        let source_thread_index = lua_gettop(source.0);

        push_stripped_luau_chunk(
            source.0,
            r#"
            local function worker()
                local shared = 41
                local function get_shared()
                    return shared
                end
                coroutine.yield(get_shared)
                shared += 1
                return get_shared()
            end
            return worker()
            "#,
        );
        lua_xmove(source.0, source_thread, 1);

        let mut nresults = 0;
        assert_eq!(
            lua_resume(source_thread, source.0, 0, &mut nresults),
            LUA_YIELD
        );
        assert!(nresults >= 1);

        push_dump_permanents_with_coroutine(source.0);
        let source_permanents = lua_gettop(source.0);
        let bytes = dump_stack_value_with_permanents(
            source.0,
            source_thread_index,
            Some(source_permanents),
        )
        .unwrap();

        push_load_permanents_with_coroutine(target.0);
        let target_permanents = lua_gettop(target.0);
        load_stack_value_with_permanents(target.0, &bytes, Some(target_permanents)).unwrap();

        assert_eq!(lua_type(target.0, -1), LUA_TTHREAD);
        let loaded_thread = lua_tothread(target.0, -1);
        assert!(!loaded_thread.is_null());
        let info = thread_info(target.0, -1).unwrap();
        assert_eq!(info.status, LUA_YIELD);
        assert!(info.open_upvalue_count > 0);

        let mut restored_results = 0;
        assert_eq!(
            lua_resume(loaded_thread, target.0, 0, &mut restored_results),
            LUA_OK
        );
        assert_eq!(restored_results, 1);
        assert_eq!(lua_type(loaded_thread, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(loaded_thread, -1), 42.0);
    }
}

#[test]
fn round_trips_yielded_thread_inside_protected_call() {
    let source = State::new();
    let target = State::new();

    unsafe {
        luaL_openlibs(source.0);
        luaL_openlibs(target.0);

        let source_thread = lua_newthread(source.0);
        assert!(!source_thread.is_null());
        let source_thread_index = lua_gettop(source.0);

        push_stripped_luau_chunk(
            source.0,
            r#"
            local function worker()
                local ok, value = pcall(function()
                    coroutine.yield("inside-pcall")
                    return 37
                end)
                if not ok then
                    return -1
                end
                return value + 5
            end
            return worker()
            "#,
        );
        lua_xmove(source.0, source_thread, 1);

        let mut nresults = 0;
        assert_eq!(
            lua_resume(source_thread, source.0, 0, &mut nresults),
            LUA_YIELD
        );
        assert_eq!(nresults, 1);
        assert_eq!(lua_type(source_thread, -1), LUA_TSTRING);

        push_dump_permanents_with_coroutine(source.0);
        let source_permanents = lua_gettop(source.0);
        let bytes = dump_stack_value_with_permanents(
            source.0,
            source_thread_index,
            Some(source_permanents),
        )
        .unwrap();

        push_load_permanents_with_coroutine(target.0);
        let target_permanents = lua_gettop(target.0);
        load_stack_value_with_permanents(target.0, &bytes, Some(target_permanents)).unwrap();

        assert_eq!(lua_type(target.0, -1), LUA_TTHREAD);
        let loaded_thread = lua_tothread(target.0, -1);
        assert!(!loaded_thread.is_null());
        let info = thread_info(target.0, -1).unwrap();
        assert_eq!(info.status, LUA_YIELD);

        let mut restored_results = 0;
        assert_eq!(
            lua_resume(loaded_thread, target.0, 0, &mut restored_results),
            LUA_OK
        );
        assert_eq!(restored_results, 1);
        assert_eq!(lua_type(loaded_thread, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(loaded_thread, -1), 42.0);
    }
}

#[test]
fn round_trips_luau_proto_debug_metadata() {
    let source = State::new();
    let target = State::new();

    unsafe {
        push_default_luau_chunk(source.0, "return function() return 42 end");
        assert_eq!(lua_pcall(source.0, 0, 1, 0), LUA_OK);

        let bytes = dump_stack_value(source.0, -1).unwrap();
        load_stack_value(target.0, &bytes).unwrap();
        assert_eq!(lua_pcall(target.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_tonumber(target.0, -1), 42.0);
    }
}

#[test]
fn rejects_unsupported_types() {
    let source = State::new();

    unsafe {
        lua_pushlightuserdatatagged(source.0, std::ptr::dangling_mut::<std::ffi::c_void>(), 0);
        let err = dump_stack_value(source.0, -1).unwrap_err();
        assert_eq!(
            err,
            PersistError::UnsupportedType {
                type_code: LUA_TLIGHTUSERDATA,
                type_name: "lightuserdata".to_string()
            }
        );
    }
}
