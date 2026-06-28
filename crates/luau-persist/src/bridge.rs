use std::os::raw::c_int;

use mlua_sys::lua_State;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct FunctionInfo {
    pub is_function: i32,
    pub is_c: i32,
    pub nupvalues: i32,
    pub stack_size: i32,
    pub proto_maxstacksize: i32,
    pub proto_numparams: i32,
    pub proto_is_vararg: i32,
    pub proto_sizecode: i32,
    pub proto_sizek: i32,
    pub proto_sizep: i32,
    pub proto_nups: i32,
    pub preload: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct ProtoInfo {
    pub nups: i32,
    pub numparams: i32,
    pub is_vararg: i32,
    pub maxstacksize: i32,
    pub flags: i32,
    pub sizecode: i32,
    pub sizep: i32,
    pub sizelocvars: i32,
    pub sizeupvalues: i32,
    pub sizek: i32,
    pub sizelineinfo: i32,
    pub linegaplog2: i32,
    pub linedefined: i32,
    pub bytecodeid: i32,
    pub sizetypeinfo: i32,
    pub has_source: i32,
    pub has_debugname: i32,
    pub has_execdata: i32,
    pub exectarget: u64,
    pub identity: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct UprefInfo {
    pub raw_tag: i32,
    pub value_tag: i32,
    pub is_upvalue: i32,
    pub is_open: i32,
    pub stack_offset: i64,
    pub identity: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct ThreadInfo {
    pub status: i32,
    pub is_active: i32,
    pub stack_size: i32,
    pub stack_in_use: i32,
    pub base_offset: i32,
    pub top_offset: i32,
    pub ci_count: i32,
    pub current_ci: i32,
    pub open_upvalue_count: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct CallFrameInfo {
    pub base_offset: i32,
    pub func_offset: i32,
    pub top_offset: i32,
    pub savedpc_offset: i32,
    pub savedpc_present: i32,
    pub nresults: i32,
    pub flags: u32,
    pub is_lua: i32,
    pub is_native: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct CallFrameBuildInfo {
    pub base_offset: i32,
    pub func_offset: i32,
    pub top_offset: i32,
    pub savedpc_offset: i32,
    pub savedpc_present: i32,
    pub nresults: i32,
    pub flags: u32,
}

impl From<CallFrameInfo> for CallFrameBuildInfo {
    fn from(value: CallFrameInfo) -> Self {
        Self {
            base_offset: value.base_offset,
            func_offset: value.func_offset,
            top_offset: value.top_offset,
            savedpc_offset: value.savedpc_offset,
            savedpc_present: value.savedpc_present,
            nresults: value.nresults,
            flags: value.flags,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct OpenUpvalueInfo {
    pub value_tag: i32,
    pub stack_offset: i32,
    pub identity: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct ProtoBuildInfo {
    pub nups: i32,
    pub numparams: i32,
    pub is_vararg: i32,
    pub maxstacksize: i32,
    pub flags: i32,
    pub sizecode: i32,
    pub sizek: i32,
    pub sizep: i32,
    pub linedefined: i32,
    pub bytecodeid: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct LocVarInfo {
    pub startpc: i32,
    pub endpc: i32,
    pub reg: i32,
}

impl From<ProtoInfo> for ProtoBuildInfo {
    fn from(value: ProtoInfo) -> Self {
        Self {
            nups: value.nups,
            numparams: value.numparams,
            is_vararg: value.is_vararg,
            maxstacksize: value.maxstacksize,
            flags: value.flags,
            sizecode: value.sizecode,
            sizek: value.sizek,
            sizep: value.sizep,
            linedefined: value.linedefined,
            bytecodeid: value.bytecodeid,
        }
    }
}

extern "C" {

    fn luau_persist_get_function_info(
        state: *mut lua_State,
        index: c_int,
        out: *mut FunctionInfo,
    ) -> i32;
    fn luau_persist_get_proto_info(state: *mut lua_State, index: c_int, out: *mut ProtoInfo)
        -> i32;
    fn luau_persist_copy_proto_code(
        state: *mut lua_State,
        index: c_int,
        out: *mut u32,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_copy_proto_lineinfo(
        state: *mut lua_State,
        index: c_int,
        out: *mut u8,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_copy_proto_typeinfo(
        state: *mut lua_State,
        index: c_int,
        out: *mut u8,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_copy_proto_source(
        state: *mut lua_State,
        index: c_int,
        out: *mut u8,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_copy_proto_debugname(
        state: *mut lua_State,
        index: c_int,
        out: *mut u8,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_copy_proto_locvar_info(
        state: *mut lua_State,
        index: c_int,
        out: *mut LocVarInfo,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_copy_proto_locvar_name(
        state: *mut lua_State,
        index: c_int,
        locvar_index: c_int,
        out: *mut u8,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_copy_proto_upvalue_name(
        state: *mut lua_State,
        index: c_int,
        upvalue_index: c_int,
        out: *mut u8,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_copy_proto_constant_tags(
        state: *mut lua_State,
        index: c_int,
        out: *mut c_int,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_push_proto_constant(
        state: *mut lua_State,
        index: c_int,
        constant_index: c_int,
    ) -> i32;
    fn luau_persist_ref(state: *mut lua_State, index: c_int) -> c_int;
    fn luau_persist_push_child_proto_closure(
        state: *mut lua_State,
        index: c_int,
        child_index: c_int,
        env_index: c_int,
    ) -> i32;
    fn luau_persist_allocate_proto_skeleton(
        state: *mut lua_State,
        info: *const ProtoBuildInfo,
        code: *const u32,
        code_len: c_int,
    ) -> usize;
    fn luau_persist_lua_closure_pointer(state: *mut lua_State, index: c_int) -> usize;
    fn luau_persist_push_lua_closure_pointer(state: *mut lua_State, pointer: usize) -> i32;
    fn luau_persist_push_lua_closure_for_proto_pointer(
        state: *mut lua_State,
        pointer: usize,
        env_index: c_int,
    ) -> i32;
    fn luau_persist_set_proto_constant_from_stack(
        state: *mut lua_State,
        index: c_int,
        constant_index: c_int,
        value_index: c_int,
    ) -> i32;
    fn luau_persist_set_proto_constant_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        constant_index: c_int,
        value_index: c_int,
    ) -> i32;
    fn luau_persist_set_proto_constant_string_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        constant_index: c_int,
        data: *const u8,
        len: c_int,
    ) -> i32;
    fn luau_persist_set_proto_constant_nil_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        constant_index: c_int,
    ) -> i32;
    fn luau_persist_set_proto_constant_boolean_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        constant_index: c_int,
        value: c_int,
    ) -> i32;
    fn luau_persist_set_proto_constant_number_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        constant_index: c_int,
        value: f64,
    ) -> i32;
    fn luau_persist_set_proto_child_from_closure(
        state: *mut lua_State,
        index: c_int,
        child_index: c_int,
        child_closure_index: c_int,
    ) -> i32;
    fn luau_persist_set_proto_child_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        child_index: c_int,
        child_closure_index: c_int,
    ) -> i32;
    fn luau_persist_set_proto_lineinfo(
        state: *mut lua_State,
        index: c_int,
        linegaplog2: c_int,
        data: *const u8,
        len: c_int,
    ) -> i32;
    fn luau_persist_set_proto_lineinfo_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        linegaplog2: c_int,
        data: *const u8,
        len: c_int,
    ) -> i32;
    fn luau_persist_set_proto_typeinfo(
        state: *mut lua_State,
        index: c_int,
        data: *const u8,
        len: c_int,
    ) -> i32;
    fn luau_persist_set_proto_typeinfo_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        data: *const u8,
        len: c_int,
    ) -> i32;
    fn luau_persist_set_proto_source(
        state: *mut lua_State,
        index: c_int,
        data: *const u8,
        len: c_int,
    ) -> i32;
    fn luau_persist_set_proto_source_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        data: *const u8,
        len: c_int,
    ) -> i32;
    fn luau_persist_set_proto_debugname(
        state: *mut lua_State,
        index: c_int,
        data: *const u8,
        len: c_int,
    ) -> i32;
    fn luau_persist_set_proto_debugname_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        data: *const u8,
        len: c_int,
    ) -> i32;
    fn luau_persist_init_proto_locvars(state: *mut lua_State, index: c_int, count: c_int) -> i32;
    fn luau_persist_init_proto_locvars_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        count: c_int,
    ) -> i32;
    fn luau_persist_set_proto_locvar(
        state: *mut lua_State,
        index: c_int,
        locvar_index: c_int,
        name: *const u8,
        name_len: c_int,
        startpc: c_int,
        endpc: c_int,
        reg: c_int,
    ) -> i32;
    fn luau_persist_set_proto_locvar_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        locvar_index: c_int,
        name: *const u8,
        name_len: c_int,
        startpc: c_int,
        endpc: c_int,
        reg: c_int,
    ) -> i32;
    fn luau_persist_init_proto_upvalue_names(
        state: *mut lua_State,
        index: c_int,
        count: c_int,
    ) -> i32;
    fn luau_persist_init_proto_upvalue_names_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        count: c_int,
    ) -> i32;
    fn luau_persist_set_proto_upvalue_name(
        state: *mut lua_State,
        index: c_int,
        upvalue_index: c_int,
        name: *const u8,
        name_len: c_int,
    ) -> i32;
    fn luau_persist_set_proto_upvalue_name_from_pointer(
        state: *mut lua_State,
        pointer: usize,
        upvalue_index: c_int,
        name: *const u8,
        name_len: c_int,
    ) -> i32;
    fn luau_persist_copy_closure_upref_info(
        state: *mut lua_State,
        index: c_int,
        out: *mut UprefInfo,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_push_lua_closure_env(state: *mut lua_State, index: c_int) -> i32;
    fn luau_persist_get_thread_info(
        state: *mut lua_State,
        index: c_int,
        out: *mut ThreadInfo,
    ) -> i32;
    fn luau_persist_copy_call_frame_info(
        state: *mut lua_State,
        index: c_int,
        out: *mut CallFrameInfo,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_push_thread_stack_value(
        state: *mut lua_State,
        index: c_int,
        stack_offset: c_int,
    ) -> i32;
    fn luau_persist_copy_open_upvalue_info(
        state: *mut lua_State,
        index: c_int,
        out: *mut OpenUpvalueInfo,
        out_len: c_int,
    ) -> i32;
    fn luau_persist_set_thread_stack_value(
        state: *mut lua_State,
        thread_index: c_int,
        stack_offset: c_int,
        value_index: c_int,
    ) -> i32;
    fn luau_persist_configure_thread(
        state: *mut lua_State,
        thread_index: c_int,
        status: c_int,
        stack_in_use: c_int,
        base_offset: c_int,
        top_offset: c_int,
        ci_count: c_int,
        current_ci: c_int,
    ) -> i32;
    fn luau_persist_set_call_frame(
        state: *mut lua_State,
        thread_index: c_int,
        frame_index: c_int,
        info: *const CallFrameBuildInfo,
    ) -> i32;
    fn luau_persist_push_lua_closure_for_proto(
        state: *mut lua_State,
        proto_source_index: c_int,
        env_index: c_int,
    ) -> i32;
    fn luau_persist_set_lua_closure_upref_value(
        state: *mut lua_State,
        closure_index: c_int,
        upref_index: c_int,
        value_index: c_int,
    ) -> i32;
    fn luau_persist_set_lua_closure_preload(
        state: *mut lua_State,
        closure_index: c_int,
        preload: c_int,
    ) -> i32;
    fn luau_persist_set_lua_closure_upref_closed(
        state: *mut lua_State,
        closure_index: c_int,
        upref_index: c_int,
        value_index: c_int,
    ) -> i32;
    fn luau_persist_set_lua_closure_upref_from_upref(
        state: *mut lua_State,
        closure_index: c_int,
        upref_index: c_int,
        source_closure_index: c_int,
        source_upref_index: c_int,
    ) -> i32;
    fn luau_persist_set_lua_closure_upref_open(
        state: *mut lua_State,
        closure_index: c_int,
        upref_index: c_int,
        thread_index: c_int,
        stack_offset: c_int,
    ) -> i32;
}

/// Returns internal Luau function metadata for the value at `index`.
///
/// This is a narrow bridge over Luau internals used by the persistence
/// implementation. It intentionally exposes facts only; object construction and
/// mutation are added separately with tests for each invariant.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn function_info(state: *mut lua_State, index: c_int) -> Option<FunctionInfo> {
    let mut info = FunctionInfo::default();
    if luau_persist_get_function_info(state, index, &mut info) == 0 {
        None
    } else {
        Some(info)
    }
}

/// Returns internal Luau Proto metadata for the Lua closure at `index`.
///
/// Returns `None` when `index` is not a Lua closure.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn proto_info(state: *mut lua_State, index: c_int) -> Option<ProtoInfo> {
    let mut info = ProtoInfo::default();
    if luau_persist_get_proto_info(state, index, &mut info) == 0 {
        None
    } else {
        Some(info)
    }
}

/// Copies a Luau closure's Proto bytecode words.
///
/// Returns `None` when `index` is not a Luau closure.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn proto_code(state: *mut lua_State, index: c_int) -> Option<Vec<u32>> {
    let len = luau_persist_copy_proto_code(state, index, std::ptr::null_mut(), 0);
    if len < 0 {
        return None;
    }

    let mut out = vec![0u32; len as usize];
    let written = luau_persist_copy_proto_code(state, index, out.as_mut_ptr(), len);
    if written != len {
        return None;
    }
    Some(out)
}

unsafe fn copy_proto_bytes(
    state: *mut lua_State,
    index: c_int,
    copy: unsafe extern "C" fn(*mut lua_State, c_int, *mut u8, c_int) -> i32,
) -> Option<Vec<u8>> {
    let len = copy(state, index, std::ptr::null_mut(), 0);
    if len < 0 {
        return None;
    }

    let mut out = vec![0u8; len as usize];
    let written = copy(state, index, out.as_mut_ptr(), len);
    if written != len {
        return None;
    }
    Some(out)
}

pub unsafe fn proto_lineinfo(state: *mut lua_State, index: c_int) -> Option<Vec<u8>> {
    copy_proto_bytes(state, index, luau_persist_copy_proto_lineinfo)
}

pub unsafe fn proto_typeinfo(state: *mut lua_State, index: c_int) -> Option<Vec<u8>> {
    copy_proto_bytes(state, index, luau_persist_copy_proto_typeinfo)
}

pub unsafe fn proto_source(state: *mut lua_State, index: c_int) -> Option<Vec<u8>> {
    copy_proto_bytes(state, index, luau_persist_copy_proto_source)
}

pub unsafe fn proto_debugname(state: *mut lua_State, index: c_int) -> Option<Vec<u8>> {
    copy_proto_bytes(state, index, luau_persist_copy_proto_debugname)
}

pub unsafe fn proto_locvars(state: *mut lua_State, index: c_int) -> Option<Vec<LocVarInfo>> {
    let len = luau_persist_copy_proto_locvar_info(state, index, std::ptr::null_mut(), 0);
    if len < 0 {
        return None;
    }

    let mut out = vec![LocVarInfo::default(); len as usize];
    let written = luau_persist_copy_proto_locvar_info(state, index, out.as_mut_ptr(), len);
    if written != len {
        return None;
    }
    Some(out)
}

pub unsafe fn proto_locvar_name(
    state: *mut lua_State,
    index: c_int,
    locvar_index: c_int,
) -> Option<Vec<u8>> {
    let len =
        luau_persist_copy_proto_locvar_name(state, index, locvar_index, std::ptr::null_mut(), 0);
    if len < 0 {
        return None;
    }

    let mut out = vec![0u8; len as usize];
    let written =
        luau_persist_copy_proto_locvar_name(state, index, locvar_index, out.as_mut_ptr(), len);
    if written != len {
        return None;
    }
    Some(out)
}

pub unsafe fn proto_upvalue_name(
    state: *mut lua_State,
    index: c_int,
    upvalue_index: c_int,
) -> Option<Vec<u8>> {
    let len =
        luau_persist_copy_proto_upvalue_name(state, index, upvalue_index, std::ptr::null_mut(), 0);
    if len < 0 {
        return None;
    }

    let mut out = vec![0u8; len as usize];
    let written =
        luau_persist_copy_proto_upvalue_name(state, index, upvalue_index, out.as_mut_ptr(), len);
    if written != len {
        return None;
    }
    Some(out)
}

pub unsafe fn set_proto_lineinfo(
    state: *mut lua_State,
    index: c_int,
    linegaplog2: c_int,
    data: &[u8],
) -> bool {
    let Ok(len) = c_int::try_from(data.len()) else {
        return false;
    };
    luau_persist_set_proto_lineinfo(state, index, linegaplog2, data.as_ptr(), len) != 0
}

pub unsafe fn set_proto_lineinfo_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    linegaplog2: c_int,
    data: &[u8],
) -> bool {
    let Ok(len) = c_int::try_from(data.len()) else {
        return false;
    };
    luau_persist_set_proto_lineinfo_from_pointer(state, pointer, linegaplog2, data.as_ptr(), len)
        != 0
}

pub unsafe fn set_proto_typeinfo(state: *mut lua_State, index: c_int, data: &[u8]) -> bool {
    let Ok(len) = c_int::try_from(data.len()) else {
        return false;
    };
    luau_persist_set_proto_typeinfo(state, index, data.as_ptr(), len) != 0
}

pub unsafe fn set_proto_typeinfo_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    data: &[u8],
) -> bool {
    let Ok(len) = c_int::try_from(data.len()) else {
        return false;
    };
    luau_persist_set_proto_typeinfo_from_pointer(state, pointer, data.as_ptr(), len) != 0
}

unsafe fn set_optional_proto_bytes(
    state: *mut lua_State,
    index: c_int,
    data: Option<&[u8]>,
    set: unsafe extern "C" fn(*mut lua_State, c_int, *const u8, c_int) -> i32,
) -> bool {
    match data {
        Some(bytes) => {
            let Ok(len) = c_int::try_from(bytes.len()) else {
                return false;
            };
            set(state, index, bytes.as_ptr(), len) != 0
        }
        None => set(state, index, std::ptr::null(), -1) != 0,
    }
}

pub unsafe fn set_proto_source(state: *mut lua_State, index: c_int, data: Option<&[u8]>) -> bool {
    set_optional_proto_bytes(state, index, data, luau_persist_set_proto_source)
}

pub unsafe fn set_proto_debugname(
    state: *mut lua_State,
    index: c_int,
    data: Option<&[u8]>,
) -> bool {
    set_optional_proto_bytes(state, index, data, luau_persist_set_proto_debugname)
}

pub unsafe fn set_proto_source_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    data: Option<&[u8]>,
) -> bool {
    set_optional_proto_bytes_pointer(
        state,
        pointer,
        data,
        luau_persist_set_proto_source_from_pointer,
    )
}

pub unsafe fn set_proto_debugname_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    data: Option<&[u8]>,
) -> bool {
    set_optional_proto_bytes_pointer(
        state,
        pointer,
        data,
        luau_persist_set_proto_debugname_from_pointer,
    )
}

pub unsafe fn init_proto_locvars(state: *mut lua_State, index: c_int, count: c_int) -> bool {
    luau_persist_init_proto_locvars(state, index, count) != 0
}

pub unsafe fn init_proto_locvars_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    count: c_int,
) -> bool {
    luau_persist_init_proto_locvars_from_pointer(state, pointer, count) != 0
}

pub unsafe fn set_proto_locvar(
    state: *mut lua_State,
    index: c_int,
    locvar_index: c_int,
    name: Option<&[u8]>,
    info: LocVarInfo,
) -> bool {
    let (ptr, len) = match name {
        Some(bytes) => {
            let Ok(len) = c_int::try_from(bytes.len()) else {
                return false;
            };
            (bytes.as_ptr(), len)
        }
        None => (std::ptr::null(), -1),
    };
    luau_persist_set_proto_locvar(
        state,
        index,
        locvar_index,
        ptr,
        len,
        info.startpc,
        info.endpc,
        info.reg,
    ) != 0
}

pub unsafe fn set_proto_locvar_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    locvar_index: c_int,
    name: Option<&[u8]>,
    info: LocVarInfo,
) -> bool {
    let (ptr, len) = match name {
        Some(bytes) => {
            let Ok(len) = c_int::try_from(bytes.len()) else {
                return false;
            };
            (bytes.as_ptr(), len)
        }
        None => (std::ptr::null(), -1),
    };
    luau_persist_set_proto_locvar_from_pointer(
        state,
        pointer,
        locvar_index,
        ptr,
        len,
        info.startpc,
        info.endpc,
        info.reg,
    ) != 0
}

pub unsafe fn init_proto_upvalue_names(state: *mut lua_State, index: c_int, count: c_int) -> bool {
    luau_persist_init_proto_upvalue_names(state, index, count) != 0
}

pub unsafe fn init_proto_upvalue_names_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    count: c_int,
) -> bool {
    luau_persist_init_proto_upvalue_names_from_pointer(state, pointer, count) != 0
}

pub unsafe fn set_proto_upvalue_name(
    state: *mut lua_State,
    index: c_int,
    upvalue_index: c_int,
    name: Option<&[u8]>,
) -> bool {
    let (ptr, len) = match name {
        Some(bytes) => {
            let Ok(len) = c_int::try_from(bytes.len()) else {
                return false;
            };
            (bytes.as_ptr(), len)
        }
        None => (std::ptr::null(), -1),
    };
    luau_persist_set_proto_upvalue_name(state, index, upvalue_index, ptr, len) != 0
}

pub unsafe fn set_proto_upvalue_name_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    upvalue_index: c_int,
    name: Option<&[u8]>,
) -> bool {
    let (ptr, len) = match name {
        Some(bytes) => {
            let Ok(len) = c_int::try_from(bytes.len()) else {
                return false;
            };
            (bytes.as_ptr(), len)
        }
        None => (std::ptr::null(), -1),
    };
    luau_persist_set_proto_upvalue_name_from_pointer(state, pointer, upvalue_index, ptr, len) != 0
}

unsafe fn set_optional_proto_bytes_pointer(
    state: *mut lua_State,
    pointer: usize,
    data: Option<&[u8]>,
    setter: unsafe extern "C" fn(*mut lua_State, usize, *const u8, c_int) -> i32,
) -> bool {
    match data {
        Some(bytes) => {
            let Ok(len) = c_int::try_from(bytes.len()) else {
                return false;
            };
            setter(state, pointer, bytes.as_ptr(), len) != 0
        }
        None => setter(state, pointer, std::ptr::null(), -1) != 0,
    }
}

/// Copies the Luau type tags of a closure's Proto constants.
///
/// Returns `None` when `index` is not a Luau closure.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn proto_constant_tags(state: *mut lua_State, index: c_int) -> Option<Vec<c_int>> {
    let len = luau_persist_copy_proto_constant_tags(state, index, std::ptr::null_mut(), 0);
    if len < 0 {
        return None;
    }

    let mut out = vec![0; len as usize];
    let written = luau_persist_copy_proto_constant_tags(state, index, out.as_mut_ptr(), len);
    if written != len {
        return None;
    }
    Some(out)
}

/// Pushes one Proto constant as a regular Luau stack value.
///
/// Returns `false` when `index` is not a Lua closure or when `constant_index`
/// is out of bounds. `constant_index` is zero-based.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn push_proto_constant(
    state: *mut lua_State,
    index: c_int,
    constant_index: c_int,
) -> bool {
    luau_persist_push_proto_constant(state, index, constant_index) != 0
}

/// Stores the value at `index` in the Luau registry and returns its reference.
///
/// This mirrors `lua_ref`, but handles Luau's paused-GC color state for loader
/// roots created while VM persistence is reconstructing object graphs.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must refer to a valid
/// stack value.
pub unsafe fn persist_ref(state: *mut lua_State, index: c_int) -> c_int {
    luau_persist_ref(state, index)
}

/// Pushes a Lua closure for one child Proto of the closure at `index`.
///
/// The pushed closure uses `env_index` as its environment and has nil uprefs.
/// Callers that intend to execute it must fill uprefs according to the parent
/// bytecode capture instructions first. This operation is primarily for graph
/// traversal and construction tests.
///
/// Returns `false` when `index` is not a Lua closure, `child_index` is out of
/// bounds, or `env_index` is not a table. `child_index` is zero-based.
///
/// # Safety
///
/// `state` must point to a valid Luau state. `index` and `env_index` must be
/// valid for the duration of the call.
pub unsafe fn push_child_proto_closure(
    state: *mut lua_State,
    index: c_int,
    child_index: c_int,
    env_index: c_int,
) -> bool {
    luau_persist_push_child_proto_closure(state, index, child_index, env_index) != 0
}

/// Allocates a new Proto skeleton.
///
/// The Proto is allocated with Luau's allocator, its bytecode is copied from
/// `code`, constants are initialized to nil, child Proto slots are initialized
/// to null, and `codeentry` is set to the copied bytecode. Callers must fill
/// constants and child Protos before wrapping it in an executable closure.
///
/// Returns `false` when the build info is invalid or `env_index` is not a
/// table.
///
/// # Safety
///
/// `state` must point to a valid Luau state. `env_index` must be valid for the
/// duration of the call.
pub unsafe fn allocate_proto_skeleton(
    state: *mut lua_State,
    info: &ProtoBuildInfo,
    code: &[u32],
) -> Option<usize> {
    let Ok(code_len) = c_int::try_from(code.len()) else {
        return None;
    };
    let pointer = luau_persist_allocate_proto_skeleton(state, info, code.as_ptr(), code_len);
    if pointer == 0 {
        None
    } else {
        Some(pointer)
    }
}

pub unsafe fn lua_closure_pointer(state: *mut lua_State, index: c_int) -> Option<usize> {
    let pointer = luau_persist_lua_closure_pointer(state, index);
    if pointer == 0 {
        None
    } else {
        Some(pointer)
    }
}

pub unsafe fn push_lua_closure_pointer(state: *mut lua_State, pointer: usize) -> bool {
    luau_persist_push_lua_closure_pointer(state, pointer) != 0
}

pub unsafe fn push_lua_closure_for_proto_pointer(
    state: *mut lua_State,
    pointer: usize,
    env_index: c_int,
) -> bool {
    luau_persist_push_lua_closure_for_proto_pointer(state, pointer, env_index) != 0
}

pub unsafe fn set_lua_closure_preload(
    state: *mut lua_State,
    closure_index: c_int,
    preload: bool,
) -> bool {
    luau_persist_set_lua_closure_preload(state, closure_index, i32::from(preload)) != 0
}

/// Copies a stack value into a Proto constant slot.
///
/// `constant_index` is zero-based. The target `index` must refer to a Lua
/// closure whose Proto was allocated by Luau.
///
/// # Safety
///
/// `state` must point to a valid Luau state. `index` and `value_index` must be
/// valid for the duration of the call.
pub unsafe fn set_proto_constant_from_stack(
    state: *mut lua_State,
    index: c_int,
    constant_index: c_int,
    value_index: c_int,
) -> bool {
    luau_persist_set_proto_constant_from_stack(state, index, constant_index, value_index) != 0
}

pub unsafe fn set_proto_constant_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    constant_index: c_int,
    value_index: c_int,
) -> bool {
    luau_persist_set_proto_constant_from_pointer(state, pointer, constant_index, value_index) != 0
}

pub unsafe fn set_proto_constant_string_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    constant_index: c_int,
    data: &[u8],
) -> bool {
    let Ok(len) = c_int::try_from(data.len()) else {
        return false;
    };
    luau_persist_set_proto_constant_string_from_pointer(
        state,
        pointer,
        constant_index,
        data.as_ptr(),
        len,
    ) != 0
}

pub unsafe fn set_proto_constant_nil_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    constant_index: c_int,
) -> bool {
    luau_persist_set_proto_constant_nil_from_pointer(state, pointer, constant_index) != 0
}

pub unsafe fn set_proto_constant_boolean_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    constant_index: c_int,
    value: bool,
) -> bool {
    luau_persist_set_proto_constant_boolean_from_pointer(
        state,
        pointer,
        constant_index,
        i32::from(value),
    ) != 0
}

pub unsafe fn set_proto_constant_number_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    constant_index: c_int,
    value: f64,
) -> bool {
    luau_persist_set_proto_constant_number_from_pointer(state, pointer, constant_index, value) != 0
}

/// Sets one child Proto slot from another Lua closure's Proto.
///
/// `child_index` is zero-based. Both `index` and `child_closure_index` must
/// refer to Lua closures.
///
/// # Safety
///
/// `state` must point to a valid Luau state. Both indices must be valid for
/// the duration of the call.
pub unsafe fn set_proto_child_from_closure(
    state: *mut lua_State,
    index: c_int,
    child_index: c_int,
    child_closure_index: c_int,
) -> bool {
    luau_persist_set_proto_child_from_closure(state, index, child_index, child_closure_index) != 0
}

pub unsafe fn set_proto_child_from_pointer(
    state: *mut lua_State,
    pointer: usize,
    child_index: c_int,
    child_closure_index: c_int,
) -> bool {
    luau_persist_set_proto_child_from_pointer(state, pointer, child_index, child_closure_index) != 0
}

/// Copies raw Luau closure upref metadata.
///
/// A Lua closure upref can hold either a closed/by-value `TValue` directly or a
/// `LUA_TUPVAL` object that points at a closed value or an open stack slot.
/// This function exposes only tags, identity, and stack-offset facts; actual
/// values should still flow through the normal value serializer. For open
/// upvalues, `stack_offset` is only set when the owning stack is the `state`
/// passed to this function. Thread serialization should use the thread's own
/// open-upvalue list as the source of truth.
///
/// Returns `None` when `index` is not a Lua closure.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn closure_uprefs(state: *mut lua_State, index: c_int) -> Option<Vec<UprefInfo>> {
    let len = luau_persist_copy_closure_upref_info(state, index, std::ptr::null_mut(), 0);
    if len < 0 {
        return None;
    }

    let mut out = vec![UprefInfo::default(); len as usize];
    let written = luau_persist_copy_closure_upref_info(state, index, out.as_mut_ptr(), len);
    if written != len {
        return None;
    }
    Some(out)
}

/// Pushes a Lua closure's environment table.
///
/// Returns `false` when `index` is not a Lua closure.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn push_lua_closure_env(state: *mut lua_State, index: c_int) -> bool {
    luau_persist_push_lua_closure_env(state, index) != 0
}

/// Returns internal thread metadata for the thread at `index`.
///
/// Returns `None` when `index` is not a thread value.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn thread_info(state: *mut lua_State, index: c_int) -> Option<ThreadInfo> {
    let mut info = ThreadInfo::default();
    if luau_persist_get_thread_info(state, index, &mut info) == 0 {
        None
    } else {
        Some(info)
    }
}

/// Copies call-frame metadata for the thread at `index`.
///
/// Returns `None` when `index` is not a thread value.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn call_frames(state: *mut lua_State, index: c_int) -> Option<Vec<CallFrameInfo>> {
    let len = luau_persist_copy_call_frame_info(state, index, std::ptr::null_mut(), 0);
    if len < 0 {
        return None;
    }

    let mut out = vec![CallFrameInfo::default(); len as usize];
    let written = luau_persist_copy_call_frame_info(state, index, out.as_mut_ptr(), len);
    if written != len {
        return None;
    }
    Some(out)
}

/// Pushes a value from a thread stack slot onto `state`.
///
/// `stack_offset` is relative to the target thread's stack base and must be
/// less than the thread top. Returns `false` when `index` is not a thread or
/// the slot is outside the live stack range.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn push_thread_stack_value(
    state: *mut lua_State,
    index: c_int,
    stack_offset: c_int,
) -> bool {
    luau_persist_push_thread_stack_value(state, index, stack_offset) != 0
}

/// Copies metadata for every open upvalue currently owned by a thread.
///
/// Returns `None` when `index` is not a thread. Stack offsets are relative to
/// the thread stack base. A negative offset means the upvalue did not point
/// into the live stack range and should be treated as malformed for exact
/// persistence.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn open_upvalues(state: *mut lua_State, index: c_int) -> Option<Vec<OpenUpvalueInfo>> {
    let len = luau_persist_copy_open_upvalue_info(state, index, std::ptr::null_mut(), 0);
    if len < 0 {
        return None;
    }

    let mut out = vec![OpenUpvalueInfo::default(); len as usize];
    let written = luau_persist_copy_open_upvalue_info(state, index, out.as_mut_ptr(), len);
    if written != len {
        return None;
    }
    Some(out)
}

/// Copies a stack value into a thread stack slot.
///
/// The target thread must be inactive. This grows the target stack when needed
/// and advances the thread top to include the written slot.
///
/// # Safety
///
/// `state` must point to a valid Luau state. `thread_index` and `value_index`
/// must be valid for the duration of the call.
pub unsafe fn set_thread_stack_value(
    state: *mut lua_State,
    thread_index: c_int,
    stack_offset: c_int,
    value_index: c_int,
) -> bool {
    luau_persist_set_thread_stack_value(state, thread_index, stack_offset, value_index) != 0
}

/// Configures high-level suspended thread fields after stack slots are written.
///
/// This does not populate call frames; callers must fill every active frame
/// with [`set_call_frame`] before resuming the thread.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `thread_index` must be valid
/// for the duration of the call.
pub unsafe fn configure_thread(
    state: *mut lua_State,
    thread_index: c_int,
    status: c_int,
    stack_in_use: c_int,
    base_offset: c_int,
    top_offset: c_int,
    ci_count: c_int,
    current_ci: c_int,
) -> bool {
    luau_persist_configure_thread(
        state,
        thread_index,
        status,
        stack_in_use,
        base_offset,
        top_offset,
        ci_count,
        current_ci,
    ) != 0
}

/// Writes one active `CallInfo` entry for a suspended thread.
///
/// `savedpc_offset` is interpreted relative to the Lua function stored at
/// `func_offset` when `savedpc_present` is nonzero.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `thread_index` must be valid
/// for the duration of the call.
pub unsafe fn set_call_frame(
    state: *mut lua_State,
    thread_index: c_int,
    frame_index: c_int,
    info: &CallFrameBuildInfo,
) -> bool {
    luau_persist_set_call_frame(state, thread_index, frame_index, info) != 0
}

/// Pushes a new Lua closure that reuses the Proto from `proto_source_index` and
/// uses `env_index` as its environment.
///
/// The new closure's uprefs are initialized to nil by Luau. Callers that use
/// this for loading must fill every upref required by the Proto before running
/// the closure.
///
/// Returns `false` when `proto_source_index` is not a Lua closure or when
/// `env_index` is not a table.
///
/// # Safety
///
/// `state` must point to a valid Luau state. `proto_source_index` and
/// `env_index` must be valid for the duration of the call.
pub unsafe fn push_lua_closure_for_proto(
    state: *mut lua_State,
    proto_source_index: c_int,
    env_index: c_int,
) -> bool {
    luau_persist_push_lua_closure_for_proto(state, proto_source_index, env_index) != 0
}

/// Copies the value at `value_index` into a Lua closure upref slot.
///
/// `upref_index` is zero-based. This sets a direct/by-value upref slot; shared
/// `LUA_TUPVAL` identity and open upvalue reconstruction require separate
/// construction APIs.
///
/// # Safety
///
/// `state` must point to a valid Luau state. `closure_index` and `value_index`
/// must be valid for the duration of the call.
pub unsafe fn set_lua_closure_upref_value(
    state: *mut lua_State,
    closure_index: c_int,
    upref_index: c_int,
    value_index: c_int,
) -> bool {
    luau_persist_set_lua_closure_upref_value(state, closure_index, upref_index, value_index) != 0
}

/// Allocates a closed `LUA_TUPVAL` object from `value_index` and stores it in a
/// Lua closure upref slot.
///
/// `upref_index` is zero-based. This is the construction primitive used when a
/// persisted upvalue identity first appears during load. Later closures that
/// share the same upvalue should use [`set_lua_closure_upref_from_upref`].
///
/// # Safety
///
/// `state` must point to a valid Luau state. `closure_index` and `value_index`
/// must be valid for the duration of the call.
pub unsafe fn set_lua_closure_upref_closed(
    state: *mut lua_State,
    closure_index: c_int,
    upref_index: c_int,
    value_index: c_int,
) -> bool {
    luau_persist_set_lua_closure_upref_closed(state, closure_index, upref_index, value_index) != 0
}

/// Makes one Lua closure upref slot refer to an existing `LUA_TUPVAL` object
/// from another closure slot.
///
/// Both upref indices are zero-based. The source slot must already contain a
/// `LUA_TUPVAL`, not a direct/by-value upref.
///
/// # Safety
///
/// `state` must point to a valid Luau state. Both closure indices must be valid
/// for the duration of the call.
pub unsafe fn set_lua_closure_upref_from_upref(
    state: *mut lua_State,
    closure_index: c_int,
    upref_index: c_int,
    source_closure_index: c_int,
    source_upref_index: c_int,
) -> bool {
    luau_persist_set_lua_closure_upref_from_upref(
        state,
        closure_index,
        upref_index,
        source_closure_index,
        source_upref_index,
    ) != 0
}

/// Makes one Lua closure upref slot refer to an open upvalue rooted at a
/// coroutine stack slot.
///
/// The target thread must be inactive, and `stack_offset` must name a live stack
/// slot. Repeated calls for the same thread slot reuse the same open upvalue,
/// preserving sharing between closures.
///
/// # Safety
///
/// `state` must point to a valid Luau state. The closure and thread indices
/// must be valid for the duration of the call.
pub unsafe fn set_lua_closure_upref_open(
    state: *mut lua_State,
    closure_index: c_int,
    upref_index: c_int,
    thread_index: c_int,
    stack_offset: c_int,
) -> bool {
    luau_persist_set_lua_closure_upref_open(
        state,
        closure_index,
        upref_index,
        thread_index,
        stack_offset,
    ) != 0
}
