use std::ffi::CString;

use luau_persist::bridge::{
    allocate_proto_skeleton, call_frames, closure_uprefs, configure_thread, function_info,
    open_upvalues, proto_code, proto_constant_tags, proto_info, push_child_proto_closure,
    push_lua_closure_env, push_lua_closure_for_proto, push_lua_closure_for_proto_pointer,
    push_proto_constant, push_thread_stack_value, set_call_frame, set_lua_closure_upref_closed,
    set_lua_closure_upref_from_upref, set_lua_closure_upref_open, set_lua_closure_upref_value,
    set_proto_child_from_pointer, set_proto_constant_from_pointer, set_thread_stack_value,
    thread_info, CallFrameBuildInfo, ProtoBuildInfo,
};
use mlua_sys::luaL_openlibs;
use mlua_sys::{
    luaL_newstate, lua_CFunction, lua_CompileOptions, lua_absindex, lua_close, lua_newthread,
    lua_pcall, lua_pop, lua_pushcfunction, lua_pushnil, lua_pushnumber, lua_pushvalue,
    lua_rawequal, lua_resume, lua_tolstring, lua_tonumber, lua_type, lua_xmove, luau_compile,
    luau_load, LUA_GLOBALSINDEX, LUA_OK, LUA_TFUNCTION, LUA_TNUMBER, LUA_TSTRING, LUA_YIELD,
};

struct State(*mut mlua_sys::lua_State);

impl State {
    fn new() -> Self {
        let state = unsafe { luaL_newstate() };
        assert!(!state.is_null());
        unsafe { luaL_openlibs(state) };
        Self(state)
    }
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe { lua_close(self.0) };
    }
}

unsafe extern "C-unwind" fn native_test_fn(_: *mut mlua_sys::lua_State) -> std::os::raw::c_int {
    0
}

fn push_luau_function(state: *mut mlua_sys::lua_State, source: &str) {
    let bytecode = unsafe { luau_compile(source.as_bytes(), lua_CompileOptions::default()) };
    let chunk = CString::new("bridge_test").unwrap();
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
    assert_eq!(unsafe { lua_type(state, -1) }, LUA_TFUNCTION);
}

#[test]
fn function_info_rejects_non_function_values() {
    let state = State::new();

    unsafe {
        lua_pushnil(state.0);
        assert_eq!(function_info(state.0, -1), None);
    }
}

#[test]
fn function_info_reports_c_function() {
    let state = State::new();
    let native_fn: lua_CFunction = native_test_fn;

    unsafe {
        lua_pushcfunction(state.0, native_fn);
        let info = function_info(state.0, -1).unwrap();
        assert_eq!(info.is_function, 1);
        assert_eq!(info.is_c, 1);
        assert_eq!(info.nupvalues, 0);
    }
}

#[test]
fn function_info_reports_luau_function_proto() {
    let state = State::new();
    push_luau_function(state.0, "local x = 41; return function(y) return x + y end");

    unsafe {
        let info = function_info(state.0, -1).unwrap();
        assert_eq!(info.is_function, 1);
        assert_eq!(info.is_c, 0);
        assert!(info.proto_sizecode > 0);
        assert!(info.proto_sizek > 0);
        assert!(info.proto_sizep > 0);
        assert_eq!(info.proto_numparams, 0);
    }
}

#[test]
fn proto_code_and_constant_tags_reject_c_function() {
    let state = State::new();
    let native_fn: lua_CFunction = native_test_fn;

    unsafe {
        lua_pushcfunction(state.0, native_fn);
        assert_eq!(proto_code(state.0, -1), None);
        assert_eq!(proto_constant_tags(state.0, -1), None);
    }
}

#[test]
fn proto_code_and_constant_tags_report_luau_proto_data() {
    let state = State::new();
    push_luau_function(
        state.0,
        "local label = 'height'; local base = 41.5; return function(y) return label, base + y end",
    );

    unsafe {
        let info = function_info(state.0, -1).unwrap();
        let proto = proto_info(state.0, -1).unwrap();
        let code = proto_code(state.0, -1).unwrap();
        let tags = proto_constant_tags(state.0, -1).unwrap();

        assert_eq!(code.len(), info.proto_sizecode as usize);
        assert_eq!(proto.sizecode, info.proto_sizecode);
        assert_eq!(proto.sizek, info.proto_sizek);
        assert_eq!(proto.sizep, info.proto_sizep);
        assert_eq!(proto.nups, info.proto_nups);
        assert_eq!(proto.has_execdata, 0);
        assert_eq!(tags.len(), info.proto_sizek as usize);
        assert!(code.iter().any(|word| *word != 0));
        assert!(tags.iter().all(|tag| *tag >= 0));
    }
}

#[test]
fn proto_constants_push_as_luau_values() {
    let state = State::new();
    push_luau_function(state.0, "return function() return 'height', 41.5, true end");

    unsafe {
        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        let tags = proto_constant_tags(state.0, -1).unwrap();

        let mut saw_string = false;
        let mut saw_number = false;

        for constant_index in 0..tags.len() {
            assert!(push_proto_constant(
                state.0,
                -1,
                constant_index.try_into().unwrap()
            ));
            match lua_type(state.0, -1) {
                LUA_TSTRING => {
                    let mut len = 0usize;
                    let ptr = lua_tolstring(state.0, -1, &mut len);
                    let value = std::slice::from_raw_parts(ptr.cast::<u8>(), len);
                    if value == b"height" {
                        saw_string = true;
                    }
                }
                LUA_TNUMBER => {
                    if lua_tonumber(state.0, -1) == 41.5 {
                        saw_number = true;
                    }
                }
                _ => {}
            }
            lua_pop(state.0, 1);
        }

        assert!(saw_string);
        assert!(saw_number);
    }
}

#[test]
fn can_push_and_execute_child_proto_without_captures() {
    let state = State::new();
    push_luau_function(
        state.0,
        "local function child() return 42 end; return child",
    );

    unsafe {
        let info = proto_info(state.0, -1).unwrap();
        assert!(info.sizep > 0);

        lua_pushvalue(state.0, LUA_GLOBALSINDEX);
        assert!(push_child_proto_closure(state.0, -2, 0, -1));

        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_type(state.0, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(state.0, -1), 42.0);
    }
}

#[test]
fn can_reconstruct_proto_skeleton_and_fill_constants() {
    let state = State::new();
    push_luau_function(state.0, "return function() return 41.5 end");

    unsafe {
        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        let source_closure = lua_absindex(state.0, -1);

        let proto = proto_info(state.0, source_closure).unwrap();
        assert_eq!(proto.sizep, 0);
        assert_eq!(proto.nups, 0);
        let code = proto_code(state.0, source_closure).unwrap();
        let tags = proto_constant_tags(state.0, source_closure).unwrap();
        assert_eq!(tags.len(), proto.sizek as usize);

        let reconstructed_proto =
            allocate_proto_skeleton(state.0, &ProtoBuildInfo::from(proto), &code).unwrap();

        for constant_index in 0..tags.len() {
            assert!(push_proto_constant(
                state.0,
                source_closure,
                constant_index.try_into().unwrap()
            ));
            assert!(set_proto_constant_from_pointer(
                state.0,
                reconstructed_proto,
                constant_index.try_into().unwrap(),
                -1
            ));
            lua_pop(state.0, 1);
        }

        lua_pushvalue(state.0, LUA_GLOBALSINDEX);
        assert!(push_lua_closure_for_proto_pointer(
            state.0,
            reconstructed_proto,
            -1
        ));

        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_type(state.0, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(state.0, -1), 41.5);
    }
}

#[test]
fn can_reconstruct_parent_and_child_proto_graph() {
    let state = State::new();
    push_luau_function(
        state.0,
        "local function child() return 42 end; return child()",
    );

    unsafe {
        let source_parent = lua_absindex(state.0, -1);
        let parent_info = proto_info(state.0, source_parent).unwrap();
        assert!(parent_info.sizep > 0);
        assert_eq!(parent_info.nups, 0);

        lua_pushvalue(state.0, LUA_GLOBALSINDEX);
        let env = lua_absindex(state.0, -1);

        assert!(push_child_proto_closure(state.0, source_parent, 0, env));
        let source_child = lua_absindex(state.0, -1);
        let child_info = proto_info(state.0, source_child).unwrap();
        assert_eq!(child_info.nups, 0);

        let child_code = proto_code(state.0, source_child).unwrap();
        let reconstructed_child_proto =
            allocate_proto_skeleton(state.0, &ProtoBuildInfo::from(child_info), &child_code)
                .unwrap();
        for constant_index in 0..child_info.sizek {
            assert!(push_proto_constant(state.0, source_child, constant_index));
            assert!(set_proto_constant_from_pointer(
                state.0,
                reconstructed_child_proto,
                constant_index,
                -1
            ));
            lua_pop(state.0, 1);
        }
        assert!(push_lua_closure_for_proto_pointer(
            state.0,
            reconstructed_child_proto,
            env
        ));
        let reconstructed_child = lua_absindex(state.0, -1);

        let parent_code = proto_code(state.0, source_parent).unwrap();
        let reconstructed_parent_proto =
            allocate_proto_skeleton(state.0, &ProtoBuildInfo::from(parent_info), &parent_code)
                .unwrap();
        for constant_index in 0..parent_info.sizek {
            assert!(push_proto_constant(state.0, source_parent, constant_index));
            assert!(set_proto_constant_from_pointer(
                state.0,
                reconstructed_parent_proto,
                constant_index,
                -1
            ));
            lua_pop(state.0, 1);
        }
        assert!(set_proto_child_from_pointer(
            state.0,
            reconstructed_parent_proto,
            0,
            reconstructed_child
        ));
        assert!(push_lua_closure_for_proto_pointer(
            state.0,
            reconstructed_parent_proto,
            env
        ));

        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_type(state.0, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(state.0, -1), 42.0);
    }
}

#[test]
fn closure_uprefs_distinguish_value_and_shared_upvalue_captures() {
    let state = State::new();
    push_luau_function(
        state.0,
        r#"
        local function make_value(x)
            return function()
                return x
            end
        end

        local function make_shared()
            local shared = 40
            local function inc_shared()
                shared += 1
                return shared
            end
            local function get_shared()
                return shared
            end
            return inc_shared, get_shared
        end

        local inc_shared, get_shared = make_shared()
        return make_value(17), inc_shared, get_shared
        "#,
    );

    unsafe {
        assert_eq!(lua_pcall(state.0, 0, 3, 0), LUA_OK);

        let value_uprefs = closure_uprefs(state.0, -3).unwrap();
        assert_eq!(value_uprefs.len(), 1);
        assert_eq!(value_uprefs[0].raw_tag, LUA_TNUMBER);
        assert_eq!(value_uprefs[0].value_tag, LUA_TNUMBER);
        assert_eq!(value_uprefs[0].is_upvalue, 0);

        let inc_uprefs = closure_uprefs(state.0, -2).unwrap();
        let get_uprefs = closure_uprefs(state.0, -1).unwrap();
        assert_eq!(inc_uprefs.len(), 1);
        assert_eq!(get_uprefs.len(), 1);
        assert_eq!(inc_uprefs[0].is_upvalue, 1);
        assert_eq!(get_uprefs[0].is_upvalue, 1);
        assert_eq!(inc_uprefs[0].is_open, 0);
        assert_eq!(get_uprefs[0].is_open, 0);
        assert_eq!(inc_uprefs[0].value_tag, LUA_TNUMBER);
        assert_eq!(get_uprefs[0].value_tag, LUA_TNUMBER);
        assert_ne!(inc_uprefs[0].identity, 0);
        assert_eq!(inc_uprefs[0].identity, get_uprefs[0].identity);
    }
}

#[test]
fn yielded_thread_reports_call_frame_and_savedpc() {
    let state = State::new();
    let thread = unsafe { lua_newthread(state.0) };
    assert!(!thread.is_null());

    push_luau_function(
        state.0,
        r#"
        local function worker()
            coroutine.yield(7)
            return 9
        end
        return worker()
        "#,
    );

    unsafe {
        lua_xmove(state.0, thread, 1);

        let mut nresults = 0;
        assert_eq!(lua_resume(thread, state.0, 0, &mut nresults), LUA_YIELD);
        assert_eq!(nresults, 1);
        assert_eq!(lua_type(thread, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(thread, -1), 7.0);

        let info = thread_info(state.0, -1).unwrap();
        assert_eq!(info.status, LUA_YIELD);
        assert!(info.stack_in_use > 0);
        assert!(info.ci_count >= 1);

        let frames = call_frames(state.0, -1).unwrap();
        assert_eq!(frames.len(), info.ci_count as usize);
        assert!(frames.iter().any(|frame| frame.is_lua == 1));
        assert!(frames
            .iter()
            .any(|frame| frame.savedpc_present == 1 && frame.savedpc_offset >= 0));
    }
}

#[test]
fn yielded_thread_exposes_stack_slots_and_open_upvalues() {
    let state = State::new();
    let thread = unsafe { lua_newthread(state.0) };
    assert!(!thread.is_null());

    push_luau_function(
        state.0,
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

    unsafe {
        lua_xmove(state.0, thread, 1);

        let mut nresults = 0;
        assert_eq!(lua_resume(thread, state.0, 0, &mut nresults), LUA_YIELD);
        assert_eq!(nresults, 1);

        let info = thread_info(state.0, -1).unwrap();
        assert!(info.top_offset > 0);
        assert!(info.open_upvalue_count > 0);

        assert!(push_thread_stack_value(state.0, -1, info.top_offset - 1));
        assert_eq!(lua_type(state.0, -1), LUA_TFUNCTION);
        lua_pop(state.0, 1);

        let open = open_upvalues(state.0, -1).unwrap();
        assert_eq!(open.len(), info.open_upvalue_count as usize);
        assert!(open.iter().all(|up| up.identity != 0));
        let shared = open
            .iter()
            .find(|up| up.value_tag == LUA_TNUMBER)
            .expect("number open upvalue");
        assert!(shared.stack_offset >= 0);

        assert!(push_thread_stack_value(state.0, -1, shared.stack_offset));
        assert_eq!(lua_type(state.0, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(state.0, -1), 41.0);
    }
}

#[test]
fn can_reconstruct_yielded_thread_stack_and_call_frames() {
    let state = State::new();
    let source_thread = unsafe { lua_newthread(state.0) };
    assert!(!source_thread.is_null());

    push_luau_function(
        state.0,
        r#"
        local function worker()
            local value = 7
            coroutine.yield(value)
            return value + 2
        end
        return worker()
        "#,
    );

    unsafe {
        lua_xmove(state.0, source_thread, 1);

        let mut nresults = 0;
        assert_eq!(
            lua_resume(source_thread, state.0, 0, &mut nresults),
            LUA_YIELD
        );
        assert_eq!(nresults, 1);

        let source_thread_index = lua_absindex(state.0, -1);
        let info = thread_info(state.0, source_thread_index).unwrap();
        assert_eq!(info.status, LUA_YIELD);
        assert_eq!(info.open_upvalue_count, 0);

        let frames = call_frames(state.0, source_thread_index).unwrap();
        assert_eq!(frames.len(), info.ci_count as usize);
        assert!(frames.iter().any(|frame| frame.is_lua == 1));
        assert!(frames.iter().all(|frame| frame.is_native == 0));

        let reconstructed_thread = lua_newthread(state.0);
        assert!(!reconstructed_thread.is_null());
        let reconstructed_thread_index = lua_absindex(state.0, -1);

        for stack_offset in 0..info.top_offset {
            assert!(push_thread_stack_value(
                state.0,
                source_thread_index,
                stack_offset
            ));
            assert!(set_thread_stack_value(
                state.0,
                reconstructed_thread_index,
                stack_offset,
                -1
            ));
            lua_pop(state.0, 1);
        }

        assert!(configure_thread(
            state.0,
            reconstructed_thread_index,
            info.status,
            info.top_offset,
            info.base_offset,
            info.top_offset,
            info.ci_count,
            info.current_ci
        ));

        for (frame_index, frame) in frames.iter().enumerate() {
            assert!(set_call_frame(
                state.0,
                reconstructed_thread_index,
                frame_index.try_into().unwrap(),
                &CallFrameBuildInfo::from(*frame)
            ));
        }

        let restored_info = thread_info(state.0, reconstructed_thread_index).unwrap();
        assert_eq!(restored_info.status, LUA_YIELD);
        assert_eq!(restored_info.ci_count, info.ci_count);
        assert_eq!(restored_info.current_ci, info.current_ci);

        let mut restored_results = 0;
        assert_eq!(
            lua_resume(reconstructed_thread, state.0, 0, &mut restored_results),
            LUA_OK
        );
        assert_eq!(restored_results, 1);
        assert_eq!(lua_type(reconstructed_thread, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(reconstructed_thread, -1), 9.0);
    }
}

#[test]
fn can_construct_closure_for_existing_proto_and_environment() {
    let state = State::new();
    push_luau_function(state.0, "return function(value) return value + 2 end");

    unsafe {
        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        lua_pushvalue(state.0, LUA_GLOBALSINDEX);
        assert!(push_lua_closure_for_proto(state.0, -2, -1));

        lua_pushnumber(state.0, 40.0);
        assert_eq!(lua_pcall(state.0, 1, 1, 0), LUA_OK);
        assert_eq!(lua_type(state.0, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(state.0, -1), 42.0);
    }
}

#[test]
fn lua_closure_env_pushes_actual_environment_table() {
    let state = State::new();
    push_luau_function(state.0, "return function(value) return value end");

    unsafe {
        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        lua_pushvalue(state.0, LUA_GLOBALSINDEX);
        let env = lua_absindex(state.0, -1);
        assert!(push_lua_closure_for_proto(state.0, -2, env));
        assert!(push_lua_closure_env(state.0, -1));
        assert_eq!(lua_rawequal(state.0, env, -1), 1);
    }
}

#[test]
fn can_set_constructed_closure_direct_upref_value() {
    let state = State::new();
    push_luau_function(
        state.0,
        r#"
        local function make_value(x)
            return function()
                return x
            end
        end
        return make_value(17)
        "#,
    );

    unsafe {
        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        lua_pushvalue(state.0, LUA_GLOBALSINDEX);
        assert!(push_lua_closure_for_proto(state.0, -2, -1));

        lua_pushnumber(state.0, 23.0);
        assert!(set_lua_closure_upref_value(state.0, -2, 0, -1));
        lua_pop(state.0, 1);

        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_type(state.0, -1), LUA_TNUMBER);
        assert_eq!(lua_tonumber(state.0, -1), 23.0);
    }
}

#[test]
fn can_construct_shared_closed_upvalue_across_closures() {
    let state = State::new();
    push_luau_function(
        state.0,
        r#"
        local function make_shared()
            local shared = 0
            local function inc()
                shared += 1
                return shared
            end
            local function get()
                return shared
            end
            return inc, get
        end
        return make_shared()
        "#,
    );

    unsafe {
        assert_eq!(lua_pcall(state.0, 0, 2, 0), LUA_OK);
        let source_inc = lua_absindex(state.0, -2);
        let source_get = lua_absindex(state.0, -1);

        lua_pushvalue(state.0, LUA_GLOBALSINDEX);
        let env = lua_absindex(state.0, -1);

        assert!(push_lua_closure_for_proto(state.0, source_inc, env));
        let reconstructed_inc = lua_absindex(state.0, -1);
        assert!(push_lua_closure_for_proto(state.0, source_get, env));
        let reconstructed_get = lua_absindex(state.0, -1);

        lua_pushnumber(state.0, 40.0);
        assert!(set_lua_closure_upref_closed(
            state.0,
            reconstructed_inc,
            0,
            -1
        ));
        lua_pop(state.0, 1);

        assert!(set_lua_closure_upref_from_upref(
            state.0,
            reconstructed_get,
            0,
            reconstructed_inc,
            0
        ));

        let inc_uprefs = closure_uprefs(state.0, reconstructed_inc).unwrap();
        let get_uprefs = closure_uprefs(state.0, reconstructed_get).unwrap();
        assert_eq!(inc_uprefs.len(), 1);
        assert_eq!(get_uprefs.len(), 1);
        assert_eq!(inc_uprefs[0].is_upvalue, 1);
        assert_eq!(get_uprefs[0].is_upvalue, 1);
        assert_eq!(inc_uprefs[0].identity, get_uprefs[0].identity);

        lua_pushvalue(state.0, reconstructed_inc);
        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_tonumber(state.0, -1), 41.0);
        lua_pop(state.0, 1);

        lua_pushvalue(state.0, reconstructed_get);
        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_tonumber(state.0, -1), 41.0);
    }
}

#[test]
fn can_construct_shared_open_upvalue_from_thread_stack_slot() {
    let state = State::new();
    let thread = unsafe { lua_newthread(state.0) };
    assert!(!thread.is_null());

    push_luau_function(
        state.0,
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

    unsafe {
        let thread_index = lua_absindex(state.0, -2);
        lua_xmove(state.0, thread, 1);

        let mut nresults = 0;
        assert_eq!(lua_resume(thread, state.0, 0, &mut nresults), LUA_YIELD);
        assert_eq!(nresults, 1);

        let open = open_upvalues(state.0, thread_index).unwrap();
        let shared = open
            .iter()
            .find(|up| up.value_tag == LUA_TNUMBER)
            .expect("number open upvalue");

        let info = thread_info(state.0, thread_index).unwrap();
        assert!(push_thread_stack_value(
            state.0,
            thread_index,
            info.top_offset - 1
        ));
        let source_get = lua_absindex(state.0, -1);

        lua_pushvalue(state.0, LUA_GLOBALSINDEX);
        let env = lua_absindex(state.0, -1);

        assert!(push_lua_closure_for_proto(state.0, source_get, env));
        let reconstructed_a = lua_absindex(state.0, -1);
        assert!(push_lua_closure_for_proto(state.0, source_get, env));
        let reconstructed_b = lua_absindex(state.0, -1);

        assert!(set_lua_closure_upref_open(
            state.0,
            reconstructed_a,
            0,
            thread_index,
            shared.stack_offset
        ));
        assert!(set_lua_closure_upref_open(
            state.0,
            reconstructed_b,
            0,
            thread_index,
            shared.stack_offset
        ));

        let a_uprefs = closure_uprefs(state.0, reconstructed_a).unwrap();
        let b_uprefs = closure_uprefs(state.0, reconstructed_b).unwrap();
        assert_eq!(a_uprefs.len(), 1);
        assert_eq!(b_uprefs.len(), 1);
        assert_eq!(a_uprefs[0].is_upvalue, 1);
        assert_eq!(b_uprefs[0].is_upvalue, 1);
        assert_eq!(a_uprefs[0].is_open, 1);
        assert_eq!(b_uprefs[0].is_open, 1);
        assert_eq!(a_uprefs[0].identity, b_uprefs[0].identity);
        assert_eq!(a_uprefs[0].identity, shared.identity);

        let open = open_upvalues(state.0, thread_index).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].identity, a_uprefs[0].identity);
        assert_eq!(open[0].stack_offset, shared.stack_offset);

        lua_pushvalue(state.0, reconstructed_a);
        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_tonumber(state.0, -1), 41.0);
        lua_pop(state.0, 1);

        lua_pushvalue(state.0, reconstructed_b);
        assert_eq!(lua_pcall(state.0, 0, 1, 0), LUA_OK);
        assert_eq!(lua_tonumber(state.0, -1), 41.0);

        assert!(push_thread_stack_value(
            state.0,
            thread_index,
            shared.stack_offset
        ));
        assert_eq!(lua_tonumber(state.0, -1), 41.0);
    }
}
