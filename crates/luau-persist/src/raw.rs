use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_int;

use mlua_sys::{
    lua_State, lua_absindex, lua_checkstack, lua_gc, lua_getmetatable, lua_getreadonly,
    lua_gettable, lua_gettop, lua_getupvalue, lua_newbuffer, lua_newtable, lua_newthread, lua_next,
    lua_pop, lua_pushboolean, lua_pushlstring_, lua_pushnil, lua_pushnumber, lua_pushvalue,
    lua_pushvector, lua_rawgeti_, lua_rawset, lua_remove, lua_setmetatable, lua_setreadonly,
    lua_toboolean, lua_tobuffer, lua_tolstring, lua_tonumber, lua_topointer, lua_tovector,
    lua_type, lua_unref, LUA_GCRESTART, LUA_GCSTOP, LUA_OK, LUA_REGISTRYINDEX, LUA_TBOOLEAN,
    LUA_TBUFFER, LUA_TFUNCTION, LUA_TLIGHTUSERDATA, LUA_TNIL, LUA_TNONE, LUA_TNUMBER, LUA_TSTRING,
    LUA_TTABLE, LUA_TTHREAD, LUA_TUSERDATA, LUA_TVECTOR, LUA_YIELD,
};
use thiserror::Error;

use crate::bridge::{
    allocate_proto_skeleton, call_frames, closure_uprefs, configure_thread, function_info,
    init_proto_locvars_from_pointer, init_proto_upvalue_names_from_pointer, open_upvalues,
    persist_ref, proto_code, proto_debugname, proto_info, proto_lineinfo, proto_locvar_name,
    proto_locvars, proto_source, proto_typeinfo, proto_upvalue_name, push_child_proto_closure,
    push_lua_closure_env, push_lua_closure_for_proto, push_lua_closure_for_proto_pointer,
    push_proto_constant, push_thread_stack_value, set_call_frame, set_lua_closure_preload,
    set_lua_closure_upref_closed, set_lua_closure_upref_from_upref, set_lua_closure_upref_open,
    set_lua_closure_upref_value, set_proto_child_from_pointer,
    set_proto_constant_boolean_from_pointer, set_proto_constant_from_pointer,
    set_proto_constant_nil_from_pointer, set_proto_constant_number_from_pointer,
    set_proto_constant_string_from_pointer, set_proto_debugname_from_pointer,
    set_proto_lineinfo_from_pointer, set_proto_locvar_from_pointer, set_proto_source_from_pointer,
    set_proto_typeinfo_from_pointer, set_proto_upvalue_name_from_pointer, set_thread_stack_value,
    thread_info, CallFrameBuildInfo, LocVarInfo, ProtoBuildInfo, ProtoInfo,
};
use crate::{read_header, write_header, Tag};

#[derive(Debug, Error, PartialEq)]
pub enum PersistError {
    #[error("cannot persist absent stack value")]
    AbsentValue,
    #[error("unsupported Luau type {type_code} ({type_name})")]
    UnsupportedType { type_code: c_int, type_name: String },
    #[error("invalid persistence header")]
    InvalidMagic,
    #[error("unsupported persistence format version {0}")]
    UnsupportedVersion(u16),
    #[error("unexpected end of persistence data")]
    UnexpectedEof,
    #[error("invalid value tag 0x{0:02x}")]
    InvalidTag(u8),
    #[error("value tag {0} is recognized but not implemented")]
    UnimplementedTag(&'static str),
    #[error("unsupported VM state: {0}")]
    UnsupportedVmState(&'static str),
    #[error("malformed persisted VM state: {0}")]
    MalformedVmState(&'static str),
    #[error("reference id {id} has kind {found}, expected {expected}")]
    ReferenceKindMismatch {
        id: u32,
        expected: &'static str,
        found: &'static str,
    },
    #[error("trailing bytes after persisted value")]
    TrailingBytes,
    #[error("string length does not fit in the target platform")]
    StringTooLarge,
    #[error("reference id {0} was not defined earlier in the stream")]
    UnknownReference(u32),
    #[error("object id {id} is out of order; expected {expected}")]
    InvalidObjectId { id: u32, expected: u32 },
    #[error("too many persisted objects")]
    TooManyObjects,
    #[error("vector component count {found} does not match this Luau build's count {expected}")]
    InvalidVectorSize { found: u8, expected: u8 },
    #[error("unsupported table flags 0x{0:02x}")]
    InvalidTableFlags(u8),
    #[error("permanent value requires a permanent table")]
    MissingPermanentTable,
    #[error("permanent key resolved to nil: {key}")]
    MissingPermanentValue { key: String },
    #[error("permanent key resolved to Luau type {found_type} ({found_name}), expected {expected_type} ({expected_name})")]
    PermanentTypeMismatch {
        expected_type: c_int,
        expected_name: String,
        found_type: c_int,
        found_name: String,
    },
    #[error("unsupported permanent key Luau type {type_code} ({type_name})")]
    UnsupportedPermanentKey { type_code: c_int, type_name: String },
    #[error("userdata value was not handled by the userdata adapter")]
    UnsupportedUserData,
    #[error("userdata adapter returned an invalid type key")]
    InvalidUserDataTypeKey,
    #[error("userdata adapter did not push exactly one userdata value")]
    InvalidUserDataLoad,
    #[error("userdata adapter error: {0}")]
    UserDataAdapter(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDataPayload {
    pub type_key: String,
    pub bytes: Vec<u8>,
}

pub trait UserDataAdapter {
    /// Returns an exact, host-defined payload for the userdata at `index`.
    ///
    /// Returning `Ok(None)` means this adapter does not own the userdata and
    /// persistence must fail. The type key is part of the stable binary format
    /// contract between the dumping and loading host.
    ///
    /// # Safety
    ///
    /// `state` must be a valid Luau state and `index` must name a userdata
    /// value for the duration of the call.
    unsafe fn dump_userdata(
        &mut self,
        state: *mut lua_State,
        index: c_int,
    ) -> Result<Option<UserDataPayload>, PersistError>;

    /// Recreates a userdata value from a host-defined payload and pushes it.
    ///
    /// The implementation must push exactly one userdata value onto `state`.
    ///
    /// # Safety
    ///
    /// `state` must be a valid Luau state. `type_key` and `bytes` are untrusted
    /// persisted data and must be validated by the adapter.
    unsafe fn load_userdata(
        &mut self,
        state: *mut lua_State,
        type_key: &str,
        bytes: &[u8],
    ) -> Result<(), PersistError>;
}

#[derive(Default)]
pub struct DumpOptions<'a> {
    pub permanents_index: Option<c_int>,
    pub userdata_adapter: Option<&'a mut dyn UserDataAdapter>,
}

#[derive(Default)]
pub struct LoadOptions<'a> {
    pub permanents_index: Option<c_int>,
    pub userdata_adapter: Option<&'a mut dyn UserDataAdapter>,
}

/// Serializes one value from a Luau stack.
///
/// Complex objects are serialized only when identity can be preserved exactly.
/// Unsupported VM state is reported as a hard error.
///
/// # Safety
///
/// `state` must point to a valid Luau state, and `index` must be valid for the
/// duration of the call.
pub unsafe fn dump_stack_value(
    state: *mut lua_State,
    index: c_int,
) -> Result<Vec<u8>, PersistError> {
    dump_stack_value_with_permanents(state, index, None)
}

/// Serializes one value using an optional permanent object table.
///
/// When provided, `permanents_index` must be a Lua table mapping permanent
/// objects to serializable keys.
///
/// # Safety
///
/// `state` must point to a valid Luau state. `index` and `permanents_index`
/// must remain valid for the duration of the call.
pub unsafe fn dump_stack_value_with_permanents(
    state: *mut lua_State,
    index: c_int,
    permanents_index: Option<c_int>,
) -> Result<Vec<u8>, PersistError> {
    dump_stack_value_with_options(
        state,
        index,
        DumpOptions {
            permanents_index,
            userdata_adapter: None,
        },
    )
}

pub unsafe fn dump_stack_value_with_options(
    state: *mut lua_State,
    index: c_int,
    options: DumpOptions<'_>,
) -> Result<Vec<u8>, PersistError> {
    let mut out = Vec::new();
    write_header(&mut out);
    let permanents_index = options.permanents_index.map(|idx| lua_absindex(state, idx));
    DumpContext::new(permanents_index, options.userdata_adapter)
        .dump_value(state, index, &mut out)?;
    Ok(out)
}

struct DumpContext<'a> {
    refs: HashMap<ObjectKey, u32>,
    permanents_index: Option<c_int>,
    userdata_adapter: Option<&'a mut dyn UserDataAdapter>,
    open_upvalue_owners: HashMap<usize, OpenUpvalueOwner>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ObjectKey {
    Stack(usize),
    Proto(usize),
    Upvalue(usize),
}

#[derive(Debug, Clone, Copy)]
struct OpenUpvalueOwner {
    thread_id: u32,
    stack_offset: c_int,
}

impl<'a> DumpContext<'a> {
    fn new(
        permanents_index: Option<c_int>,
        userdata_adapter: Option<&'a mut dyn UserDataAdapter>,
    ) -> Self {
        Self {
            refs: HashMap::new(),
            permanents_index,
            userdata_adapter,
            open_upvalue_owners: HashMap::new(),
        }
    }

    unsafe fn dump_value(
        &mut self,
        state: *mut lua_State,
        index: c_int,
        out: &mut Vec<u8>,
    ) -> Result<(), PersistError> {
        if lua_checkstack(state, 64) == 0 {
            return Err(PersistError::UnsupportedVmState(
                "Lua stack cannot grow for VM snapshot dump",
            ));
        }
        let type_code = lua_type(state, index);
        if is_reference_type(type_code)
            && self.dump_reference_or_permanent(state, index, type_code, out)?
        {
            return Ok(());
        }

        match type_code {
            LUA_TNIL => out.push(Tag::Nil as u8),
            LUA_TBOOLEAN => {
                if lua_toboolean(state, index) == 0 {
                    out.push(Tag::BooleanFalse as u8);
                } else {
                    out.push(Tag::BooleanTrue as u8);
                }
            }
            LUA_TNUMBER => {
                out.push(Tag::Number as u8);
                out.extend_from_slice(&lua_tonumber(state, index).to_le_bytes());
            }
            LUA_TSTRING => {
                let mut len = 0usize;
                let ptr = lua_tolstring(state, index, &mut len);
                out.push(Tag::String as u8);
                out.extend_from_slice(&(len as u64).to_le_bytes());
                out.extend_from_slice(std::slice::from_raw_parts(ptr.cast::<u8>(), len));
            }
            LUA_TVECTOR => {
                let ptr = lua_tovector(state, index);
                out.push(Tag::Vector as u8);
                out.push(VECTOR_COMPONENTS);
                for i in 0..usize::from(VECTOR_COMPONENTS) {
                    out.extend_from_slice(&(*ptr.add(i)).to_le_bytes());
                }
            }
            LUA_TBUFFER => self.dump_buffer(state, index, out)?,
            LUA_TTABLE => self.dump_table(state, index, out)?,
            LUA_TFUNCTION => self.dump_function(state, index, out)?,
            LUA_TTHREAD => self.dump_thread(state, index, out)?,
            LUA_TUSERDATA => self.dump_userdata(state, index, out)?,
            LUA_TNONE => return Err(PersistError::AbsentValue),
            type_code => {
                return Err(PersistError::UnsupportedType {
                    type_code,
                    type_name: type_name(type_code).to_string(),
                });
            }
        }

        Ok(())
    }

    unsafe fn dump_reference_or_permanent(
        &mut self,
        state: *mut lua_State,
        index: c_int,
        type_code: c_int,
        out: &mut Vec<u8>,
    ) -> Result<bool, PersistError> {
        let key = ObjectKey::Stack(lua_topointer(state, index) as usize);
        if let Some(id) = self.refs.get(&key) {
            out.push(Tag::Reference as u8);
            out.extend_from_slice(&id.to_le_bytes());
            return Ok(true);
        }

        let Some(permanents_index) = self.permanents_index else {
            return Ok(false);
        };

        lua_pushvalue(state, index);
        lua_gettable(state, permanents_index);
        if lua_type(state, -1) == LUA_TNIL {
            lua_pop(state, 1);
            return Ok(false);
        }

        let id = u32::try_from(self.refs.len()).map_err(|_| PersistError::TooManyObjects)?;
        self.refs.insert(key, id);

        out.push(Tag::Permanent as u8);
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&type_code.to_le_bytes());
        let result = dump_permanent_key(state, -1, out);
        lua_pop(state, 1);
        result?;
        Ok(true)
    }

    unsafe fn dump_table(
        &mut self,
        state: *mut lua_State,
        index: c_int,
        out: &mut Vec<u8>,
    ) -> Result<(), PersistError> {
        let ptr = ObjectKey::Stack(lua_topointer(state, index) as usize);
        let id = u32::try_from(self.refs.len()).map_err(|_| PersistError::TooManyObjects)?;
        self.refs.insert(ptr, id);

        let abs_index = lua_absindex(state, index);
        let mut flags = 0u8;
        if lua_getreadonly(state, abs_index) != 0 {
            flags |= TABLE_READONLY;
        }

        let mut entries = Vec::new();
        let mut count = 0u64;

        lua_pushnil(state);
        while lua_next(state, abs_index) != 0 {
            let result = (|| {
                self.dump_value(state, -2, &mut entries)?;
                self.dump_value(state, -1, &mut entries)?;
                Ok::<_, PersistError>(())
            })();

            if let Err(err) = result {
                lua_pop(state, 2);
                return Err(err);
            }

            count = count.checked_add(1).ok_or(PersistError::TooManyObjects)?;
            lua_pop(state, 1);
        }

        let mut metatable = Vec::new();
        if lua_getmetatable(state, abs_index) != 0 {
            let result = self.dump_value(state, -1, &mut metatable);
            lua_pop(state, 1);
            result?;
        } else {
            metatable.push(Tag::Nil as u8);
        }

        out.push(Tag::Table as u8);
        out.extend_from_slice(&id.to_le_bytes());
        out.push(flags);
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&entries);
        out.extend_from_slice(&metatable);
        Ok(())
    }

    unsafe fn dump_buffer(
        &mut self,
        state: *mut lua_State,
        index: c_int,
        out: &mut Vec<u8>,
    ) -> Result<(), PersistError> {
        let ptr = ObjectKey::Stack(lua_topointer(state, index) as usize);
        let id = u32::try_from(self.refs.len()).map_err(|_| PersistError::TooManyObjects)?;
        self.refs.insert(ptr, id);

        let mut len = 0usize;
        let buffer = lua_tobuffer(state, index, &mut len);
        out.push(Tag::Buffer as u8);
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&(len as u64).to_le_bytes());
        out.extend_from_slice(std::slice::from_raw_parts(buffer.cast::<u8>(), len));
        Ok(())
    }

    unsafe fn dump_userdata(
        &mut self,
        state: *mut lua_State,
        index: c_int,
        out: &mut Vec<u8>,
    ) -> Result<(), PersistError> {
        let Some(adapter) = self.userdata_adapter.as_deref_mut() else {
            return Err(PersistError::UnsupportedType {
                type_code: LUA_TUSERDATA,
                type_name: type_name(LUA_TUSERDATA).to_string(),
            });
        };

        let payload = adapter
            .dump_userdata(state, index)?
            .ok_or(PersistError::UnsupportedUserData)?;
        if payload.type_key.is_empty() {
            return Err(PersistError::InvalidUserDataTypeKey);
        }

        let ptr = ObjectKey::Stack(lua_topointer(state, index) as usize);
        let id = u32::try_from(self.refs.len()).map_err(|_| PersistError::TooManyObjects)?;
        self.refs.insert(ptr, id);

        out.push(Tag::UserData as u8);
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&(payload.type_key.len() as u64).to_le_bytes());
        out.extend_from_slice(payload.type_key.as_bytes());
        out.extend_from_slice(&(payload.bytes.len() as u64).to_le_bytes());
        out.extend_from_slice(&payload.bytes);
        Ok(())
    }

    unsafe fn dump_function(
        &mut self,
        state: *mut lua_State,
        index: c_int,
        out: &mut Vec<u8>,
    ) -> Result<(), PersistError> {
        let info = function_info(state, index).ok_or(PersistError::MalformedVmState(
            "function metadata unavailable",
        ))?;
        if info.is_c != 0 {
            return Err(PersistError::UnsupportedVmState(
                "C function requires a permanent object mapping",
            ));
        }

        let key = ObjectKey::Stack(lua_topointer(state, index) as usize);
        let id = u32::try_from(self.refs.len()).map_err(|_| PersistError::TooManyObjects)?;
        self.refs.insert(key, id);

        let abs_index = lua_absindex(state, index);
        out.push(Tag::Function as u8);
        out.extend_from_slice(&id.to_le_bytes());
        out.push(FUNCTION_KIND_LUA);
        out.push(if info.preload != 0 { 1 } else { 0 });

        if !push_lua_closure_env(state, abs_index) {
            return Err(PersistError::MalformedVmState(
                "closure environment unavailable",
            ));
        }
        let env_index = lua_absindex(state, -1);
        let result = (|| {
            self.dump_value(state, env_index, out)?;
            self.dump_proto(state, abs_index, env_index, out)?;
            Ok::<_, PersistError>(())
        })();
        lua_pop(state, 1);
        result?;

        let uprefs = closure_uprefs(state, abs_index)
            .ok_or(PersistError::MalformedVmState("closure uprefs unavailable"))?;
        out.extend_from_slice(
            &u32::try_from(uprefs.len())
                .map_err(|_| PersistError::TooManyObjects)?
                .to_le_bytes(),
        );

        for (upref_index, upref) in uprefs.iter().enumerate() {
            if upref.is_upvalue == 0 {
                out.push(UPREF_KIND_DIRECT);
                if lua_getupvalue(state, abs_index, (upref_index + 1) as c_int).is_null() {
                    return Err(PersistError::MalformedVmState(
                        "closure upref value unavailable",
                    ));
                }
                let result = self.dump_value(state, -1, out);
                lua_pop(state, 1);
                result?;
            } else {
                if upref.is_open != 0 {
                    let owner = *self.open_upvalue_owners.get(&upref.identity).ok_or(
                        PersistError::UnsupportedVmState(
                            "open upvalue owner thread is not being persisted",
                        ),
                    )?;
                    out.push(UPREF_KIND_OPEN);
                    self.dump_upvalue(
                        state,
                        abs_index,
                        upref_index,
                        upref.identity,
                        Some(owner),
                        out,
                    )?;
                } else {
                    out.push(UPREF_KIND_SHARED_CLOSED);
                    self.dump_upvalue(state, abs_index, upref_index, upref.identity, None, out)?;
                }
            }
        }

        Ok(())
    }

    unsafe fn dump_proto(
        &mut self,
        state: *mut lua_State,
        closure_index: c_int,
        env_index: c_int,
        out: &mut Vec<u8>,
    ) -> Result<(), PersistError> {
        let proto = proto_info(state, closure_index)
            .ok_or(PersistError::MalformedVmState("Proto metadata unavailable"))?;
        let key = ObjectKey::Proto(proto.identity);
        if let Some(id) = self.refs.get(&key) {
            out.push(Tag::Reference as u8);
            out.extend_from_slice(&id.to_le_bytes());
            return Ok(());
        }

        validate_dumpable_proto(proto)?;
        let code = proto_code(state, closure_index)
            .ok_or(PersistError::MalformedVmState("Proto bytecode unavailable"))?;

        let id = u32::try_from(self.refs.len()).map_err(|_| PersistError::TooManyObjects)?;
        self.refs.insert(key, id);

        out.push(Tag::Proto as u8);
        out.extend_from_slice(&id.to_le_bytes());
        push_u8(out, proto.nups, "Proto nups")?;
        push_u8(out, proto.numparams, "Proto numparams")?;
        push_u8(out, proto.is_vararg, "Proto vararg flag")?;
        push_u8(out, proto.maxstacksize, "Proto maxstacksize")?;
        push_u8(out, proto.flags, "Proto flags")?;
        out.extend_from_slice(&proto.linedefined.to_le_bytes());
        out.extend_from_slice(&proto.bytecodeid.to_le_bytes());
        out.extend_from_slice(
            &u32::try_from(code.len())
                .map_err(|_| PersistError::TooManyObjects)?
                .to_le_bytes(),
        );
        for word in code {
            out.extend_from_slice(&word.to_le_bytes());
        }

        out.extend_from_slice(
            &u32::try_from(proto.sizek)
                .map_err(|_| PersistError::MalformedVmState("negative Proto constant count"))?
                .to_le_bytes(),
        );
        for constant_index in 0..proto.sizek {
            if !push_proto_constant(state, closure_index, constant_index) {
                return Err(PersistError::MalformedVmState("Proto constant unavailable"));
            }
            let result = self.dump_value(state, -1, out);
            lua_pop(state, 1);
            result?;
        }

        out.extend_from_slice(
            &u32::try_from(proto.sizep)
                .map_err(|_| PersistError::MalformedVmState("negative child Proto count"))?
                .to_le_bytes(),
        );
        for child_index in 0..proto.sizep {
            if !push_child_proto_closure(state, closure_index, child_index, env_index) {
                return Err(PersistError::MalformedVmState("child Proto unavailable"));
            }
            let result = self.dump_proto(state, -1, env_index, out);
            lua_pop(state, 1);
            result?;
        }

        self.dump_proto_metadata(state, closure_index, proto, out)?;

        Ok(())
    }

    unsafe fn dump_proto_metadata(
        &mut self,
        state: *mut lua_State,
        closure_index: c_int,
        proto: ProtoInfo,
        out: &mut Vec<u8>,
    ) -> Result<(), PersistError> {
        out.extend_from_slice(&proto.linegaplog2.to_le_bytes());
        write_bytes(
            out,
            &proto_lineinfo(state, closure_index)
                .ok_or(PersistError::MalformedVmState("Proto lineinfo unavailable"))?,
        )?;
        write_bytes(
            out,
            &proto_typeinfo(state, closure_index)
                .ok_or(PersistError::MalformedVmState("Proto typeinfo unavailable"))?,
        )?;
        if proto.has_source != 0 {
            out.push(1);
            write_bytes(
                out,
                &proto_source(state, closure_index)
                    .ok_or(PersistError::MalformedVmState("Proto source unavailable"))?,
            )?;
        } else {
            out.push(0);
        }
        if proto.has_debugname != 0 {
            out.push(1);
            write_bytes(
                out,
                &proto_debugname(state, closure_index).ok_or(PersistError::MalformedVmState(
                    "Proto debug name unavailable",
                ))?,
            )?;
        } else {
            out.push(0);
        }

        let locvars = proto_locvars(state, closure_index).ok_or(PersistError::MalformedVmState(
            "Proto local variables unavailable",
        ))?;
        out.extend_from_slice(
            &u32::try_from(locvars.len())
                .map_err(|_| PersistError::TooManyObjects)?
                .to_le_bytes(),
        );
        for (index, locvar) in locvars.iter().enumerate() {
            out.extend_from_slice(&locvar.startpc.to_le_bytes());
            out.extend_from_slice(&locvar.endpc.to_le_bytes());
            out.extend_from_slice(&locvar.reg.to_le_bytes());
            write_bytes(
                out,
                &proto_locvar_name(state, closure_index, index as c_int).ok_or(
                    PersistError::MalformedVmState("Proto local variable name unavailable"),
                )?,
            )?;
        }

        let upvalue_name_count = u32::try_from(proto.sizeupvalues)
            .map_err(|_| PersistError::MalformedVmState("negative Proto upvalue name count"))?;
        out.extend_from_slice(&upvalue_name_count.to_le_bytes());
        for index in 0..upvalue_name_count {
            write_bytes(
                out,
                &proto_upvalue_name(state, closure_index, index as c_int).ok_or(
                    PersistError::MalformedVmState("Proto upvalue name unavailable"),
                )?,
            )?;
        }

        Ok(())
    }

    unsafe fn dump_upvalue(
        &mut self,
        state: *mut lua_State,
        closure_index: c_int,
        upref_index: usize,
        identity: usize,
        open_owner: Option<OpenUpvalueOwner>,
        out: &mut Vec<u8>,
    ) -> Result<(), PersistError> {
        let key = ObjectKey::Upvalue(identity);
        if let Some(id) = self.refs.get(&key) {
            out.push(Tag::Reference as u8);
            out.extend_from_slice(&id.to_le_bytes());
            return Ok(());
        }

        let id = u32::try_from(self.refs.len()).map_err(|_| PersistError::TooManyObjects)?;
        self.refs.insert(key, id);

        out.push(Tag::Upvalue as u8);
        out.extend_from_slice(&id.to_le_bytes());
        if let Some(owner) = open_owner {
            out.push(UPVALUE_KIND_OPEN);
            out.push(Tag::Reference as u8);
            out.extend_from_slice(&owner.thread_id.to_le_bytes());
            out.extend_from_slice(&owner.stack_offset.to_le_bytes());
            Ok(())
        } else {
            out.push(UPVALUE_KIND_SHARED_CLOSED);

            if lua_getupvalue(state, closure_index, (upref_index + 1) as c_int).is_null() {
                return Err(PersistError::MalformedVmState("upvalue value unavailable"));
            }
            let result = self.dump_value(state, -1, out);
            lua_pop(state, 1);
            result
        }
    }

    unsafe fn dump_thread(
        &mut self,
        state: *mut lua_State,
        index: c_int,
        out: &mut Vec<u8>,
    ) -> Result<(), PersistError> {
        let info = thread_info(state, index).ok_or(PersistError::MalformedVmState(
            "thread metadata unavailable",
        ))?;
        if info.is_active != 0 {
            return Err(PersistError::UnsupportedVmState(
                "running thread cannot be persisted",
            ));
        }
        if info.status != LUA_YIELD && info.status != LUA_OK {
            return Err(PersistError::UnsupportedVmState(
                "only yielded or inactive threads are supported",
            ));
        }
        let open = open_upvalues(state, index).ok_or(PersistError::MalformedVmState(
            "thread open upvalues unavailable",
        ))?;
        if open.len() != info.open_upvalue_count as usize {
            return Err(PersistError::MalformedVmState(
                "thread open upvalue count mismatch",
            ));
        }
        for upvalue in &open {
            if upvalue.stack_offset < 0 || upvalue.stack_offset >= info.top_offset {
                return Err(PersistError::MalformedVmState(
                    "invalid open upvalue stack offset",
                ));
            }
        }

        let frames = call_frames(state, index).ok_or(PersistError::MalformedVmState(
            "thread call frames unavailable",
        ))?;
        if frames.len() != info.ci_count as usize {
            return Err(PersistError::MalformedVmState(
                "thread call frame count mismatch",
            ));
        }
        for frame in &frames {
            if frame.is_native != 0 && frame.is_lua != 0 {
                return Err(PersistError::UnsupportedVmState(
                    "native Luau call frame cannot be persisted",
                ));
            }
            if frame.savedpc_present != 0 && frame.savedpc_offset < 0 {
                return Err(PersistError::MalformedVmState("invalid savedpc offset"));
            }
        }

        let key = ObjectKey::Stack(lua_topointer(state, index) as usize);
        let id = u32::try_from(self.refs.len()).map_err(|_| PersistError::TooManyObjects)?;
        self.refs.insert(key, id);
        let mut replaced_open_owners = Vec::new();
        for upvalue in &open {
            let owner = OpenUpvalueOwner {
                thread_id: id,
                stack_offset: upvalue.stack_offset,
            };
            replaced_open_owners.push((
                upvalue.identity,
                self.open_upvalue_owners.insert(upvalue.identity, owner),
            ));
        }

        let abs_index = lua_absindex(state, index);
        let result = (|| {
            out.push(Tag::Thread as u8);
            out.extend_from_slice(&id.to_le_bytes());
            out.extend_from_slice(&info.status.to_le_bytes());
            out.extend_from_slice(&info.top_offset.to_le_bytes());
            out.extend_from_slice(&info.base_offset.to_le_bytes());
            out.extend_from_slice(&info.current_ci.to_le_bytes());
            out.extend_from_slice(
                &u32::try_from(frames.len())
                    .map_err(|_| PersistError::TooManyObjects)?
                    .to_le_bytes(),
            );

            for stack_offset in 0..info.top_offset {
                if !push_thread_stack_value(state, abs_index, stack_offset) {
                    return Err(PersistError::MalformedVmState(
                        "thread stack slot unavailable",
                    ));
                }
                let result = self.dump_value(state, -1, out);
                lua_pop(state, 1);
                result?;
            }

            for frame in frames {
                out.extend_from_slice(&frame.base_offset.to_le_bytes());
                out.extend_from_slice(&frame.func_offset.to_le_bytes());
                out.extend_from_slice(&frame.top_offset.to_le_bytes());
                out.extend_from_slice(&frame.savedpc_offset.to_le_bytes());
                out.extend_from_slice(&frame.savedpc_present.to_le_bytes());
                out.extend_from_slice(&frame.nresults.to_le_bytes());
                out.extend_from_slice(&frame.flags.to_le_bytes());
            }

            Ok(())
        })();
        for (identity, previous) in replaced_open_owners.into_iter().rev() {
            if let Some(previous) = previous {
                self.open_upvalue_owners.insert(identity, previous);
            } else {
                self.open_upvalue_owners.remove(&identity);
            }
        }
        result
    }
}

/// Loads one serialized value onto a Luau stack.
///
/// # Safety
///
/// `state` must point to a valid Luau state with enough available stack space
/// for one pushed value.
pub unsafe fn load_stack_value(state: *mut lua_State, input: &[u8]) -> Result<(), PersistError> {
    load_stack_value_with_permanents(state, input, None)
}

/// Loads one serialized value using an optional permanent object table.
///
/// When provided, `permanents_index` must be a Lua table mapping serialized
/// permanent keys back to permanent objects.
///
/// # Safety
///
/// `state` must point to a valid Luau state with enough available stack space
/// for one pushed value. `permanents_index` must remain valid for the duration
/// of the call.
pub unsafe fn load_stack_value_with_permanents(
    state: *mut lua_State,
    input: &[u8],
    permanents_index: Option<c_int>,
) -> Result<(), PersistError> {
    load_stack_value_with_options(
        state,
        input,
        LoadOptions {
            permanents_index,
            userdata_adapter: None,
        },
    )
}

pub unsafe fn load_stack_value_with_options<'ctx>(
    state: *mut lua_State,
    input: &[u8],
    options: LoadOptions<'ctx>,
) -> Result<(), PersistError> {
    let _gc_pause = GcPause::new(state);
    let input = read_header(input)?;
    let permanents_index = options.permanents_index.map(|idx| lua_absindex(state, idx));
    let mut context = LoadContext::new(state, permanents_index, options.userdata_adapter);
    let rest = context.load_value(input)?;
    ensure_empty(rest)?;
    Ok(())
}

struct GcPause {
    state: *mut lua_State,
}

impl GcPause {
    unsafe fn new(state: *mut lua_State) -> Self {
        lua_gc(state, LUA_GCSTOP, 0);
        Self { state }
    }
}

impl Drop for GcPause {
    fn drop(&mut self) {
        unsafe {
            lua_gc(self.state, LUA_GCRESTART, 0);
        }
    }
}

struct LoadContext<'a> {
    state: *mut lua_State,
    refs: Vec<LoadedRef>,
    permanents_index: Option<c_int>,
    userdata_adapter: Option<&'a mut dyn UserDataAdapter>,
    pending_open_upvalue_patches: Vec<OpenUpvaluePatch>,
}

#[derive(Debug)]
enum LoadedRef {
    Pending {
        kind: &'static str,
    },
    Registry {
        reference: c_int,
        kind: &'static str,
    },
    Upvalue {
        closure_reference: c_int,
        upref_index: c_int,
    },
    OpenUpvalue {
        thread_reference: c_int,
        stack_offset: c_int,
    },
}

#[derive(Debug)]
struct OpenUpvaluePatch {
    closure_reference: c_int,
    upref_index: c_int,
    thread_reference: c_int,
    stack_offset: c_int,
}

impl<'ctx> LoadContext<'ctx> {
    fn new(
        state: *mut lua_State,
        permanents_index: Option<c_int>,
        userdata_adapter: Option<&'ctx mut dyn UserDataAdapter>,
    ) -> Self {
        Self {
            state,
            refs: Vec::new(),
            permanents_index,
            userdata_adapter,
            pending_open_upvalue_patches: Vec::new(),
        }
    }

    unsafe fn load_value<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], PersistError> {
        if lua_checkstack(self.state, 64) == 0 {
            return Err(PersistError::UnsupportedVmState(
                "Lua stack cannot grow for VM snapshot load",
            ));
        }
        let (&tag, input) = input.split_first().ok_or(PersistError::UnexpectedEof)?;

        let tag = Tag::from_byte(tag)?;
        self.load_value_tag(tag, input)
    }

    unsafe fn load_value_tag<'a>(
        &mut self,
        tag: Tag,
        input: &'a [u8],
    ) -> Result<&'a [u8], PersistError> {
        match tag {
            Tag::Nil => {
                lua_pushnil(self.state);
                Ok(input)
            }
            Tag::BooleanFalse => {
                lua_pushboolean(self.state, 0);
                Ok(input)
            }
            Tag::BooleanTrue => {
                lua_pushboolean(self.state, 1);
                Ok(input)
            }
            Tag::Number => {
                let (bytes, rest) = take(input, 8)?;
                lua_pushnumber(
                    self.state,
                    f64::from_le_bytes(bytes.try_into().expect("slice length checked")),
                );
                Ok(rest)
            }
            Tag::String => {
                let (len_bytes, rest) = take(input, 8)?;
                let len = u64::from_le_bytes(len_bytes.try_into().expect("slice length checked"));
                let len = usize::try_from(len).map_err(|_| PersistError::StringTooLarge)?;
                let (bytes, rest) = take(rest, len)?;
                lua_pushlstring_(self.state, bytes.as_ptr().cast(), bytes.len());
                Ok(rest)
            }
            Tag::Vector => {
                let (&components, mut rest) =
                    input.split_first().ok_or(PersistError::UnexpectedEof)?;
                if components != VECTOR_COMPONENTS {
                    return Err(PersistError::InvalidVectorSize {
                        found: components,
                        expected: VECTOR_COMPONENTS,
                    });
                }

                let mut values = [0.0f32; 4];
                for value in values.iter_mut().take(usize::from(components)) {
                    let (bytes, next) = take(rest, 4)?;
                    *value = f32::from_le_bytes(bytes.try_into().expect("slice length checked"));
                    rest = next;
                }

                push_vector(self.state, values);
                Ok(rest)
            }
            Tag::Buffer => {
                let (id_bytes, rest) = take(input, 4)?;
                let id = u32::from_le_bytes(id_bytes.try_into().expect("slice length checked"));
                if id as usize != self.refs.len() {
                    return Err(PersistError::InvalidObjectId {
                        id,
                        expected: self.refs.len() as u32,
                    });
                }

                let (len_bytes, rest) = take(rest, 8)?;
                let len = u64::from_le_bytes(len_bytes.try_into().expect("slice length checked"));
                let len = usize::try_from(len).map_err(|_| PersistError::StringTooLarge)?;
                let (bytes, rest) = take(rest, len)?;

                let buffer = lua_newbuffer(self.state, bytes.len());
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast::<u8>(), bytes.len());
                let reference = persist_ref(self.state, -1);
                self.refs.push(LoadedRef::Registry {
                    reference,
                    kind: "buffer",
                });

                Ok(rest)
            }
            Tag::Reference => {
                let (bytes, rest) = take(input, 4)?;
                let id = u32::from_le_bytes(bytes.try_into().expect("slice length checked"));
                let reference = self.registry_ref(id, "stack value")?;
                lua_rawgeti_(self.state, LUA_REGISTRYINDEX, reference);
                Ok(rest)
            }
            Tag::Permanent => {
                let Some(permanents_index) = self.permanents_index else {
                    return Err(PersistError::MissingPermanentTable);
                };

                let (id_bytes, rest) = take(input, 4)?;
                let id = u32::from_le_bytes(id_bytes.try_into().expect("slice length checked"));
                if id as usize != self.refs.len() {
                    return Err(PersistError::InvalidObjectId {
                        id,
                        expected: self.refs.len() as u32,
                    });
                }

                let (type_bytes, rest) = take(rest, 4)?;
                let expected_type =
                    c_int::from_le_bytes(type_bytes.try_into().expect("slice length checked"));
                let rest = self.load_value(rest)?;
                let key = stack_string_lossy(self.state, -1)
                    .unwrap_or_else(|| format!("<{}>", type_name(lua_type(self.state, -1))));
                lua_gettable(self.state, permanents_index);

                let found_type = lua_type(self.state, -1);
                if found_type == LUA_TNIL {
                    return Err(PersistError::MissingPermanentValue { key });
                }
                if found_type != expected_type {
                    return Err(PersistError::PermanentTypeMismatch {
                        expected_type,
                        expected_name: type_name(expected_type).to_string(),
                        found_type,
                        found_name: type_name(found_type).to_string(),
                    });
                }

                let reference = persist_ref(self.state, -1);
                self.refs.push(LoadedRef::Registry {
                    reference,
                    kind: "permanent",
                });
                Ok(rest)
            }
            Tag::Table => {
                let (id_bytes, rest) = take(input, 4)?;
                let id = u32::from_le_bytes(id_bytes.try_into().expect("slice length checked"));
                if id as usize != self.refs.len() {
                    return Err(PersistError::InvalidObjectId {
                        id,
                        expected: self.refs.len() as u32,
                    });
                }

                let (&flags, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
                if flags & !TABLE_READONLY != 0 {
                    return Err(PersistError::InvalidTableFlags(flags));
                }
                let (count_bytes, mut rest) = take(rest, 8)?;
                let count =
                    u64::from_le_bytes(count_bytes.try_into().expect("slice length checked"));

                lua_newtable(self.state);
                let table_index = lua_absindex(self.state, -1);
                let reference = persist_ref(self.state, table_index);
                self.refs.push(LoadedRef::Registry {
                    reference,
                    kind: "table",
                });

                for _ in 0..count {
                    rest = self.load_value(rest)?;
                    rest = self.load_value(rest)?;
                    lua_rawset(self.state, table_index);
                }

                rest = self.load_value(rest)?;
                if lua_type(self.state, -1) == LUA_TNIL {
                    lua_pop(self.state, 1);
                } else {
                    lua_setmetatable(self.state, table_index);
                }

                if flags & TABLE_READONLY != 0 {
                    lua_setreadonly(self.state, table_index, 1);
                }

                Ok(rest)
            }
            Tag::Function => self.load_function(input),
            Tag::Proto => self.load_proto(input),
            Tag::Upvalue => Err(PersistError::ReferenceKindMismatch {
                id: self.refs.len() as u32,
                expected: "stack value",
                found: "upvalue",
            }),
            Tag::Thread => self.load_thread(input),
            Tag::UserData => self.load_userdata(input),
        }
    }

    unsafe fn load_userdata<'b>(&mut self, input: &'b [u8]) -> Result<&'b [u8], PersistError> {
        let (id, rest) = read_u32(input)?;
        self.expect_next_id(id)?;
        let (key_len_bytes, rest) = take(rest, 8)?;
        let key_len = u64::from_le_bytes(key_len_bytes.try_into().expect("slice length checked"));
        let key_len = usize::try_from(key_len).map_err(|_| PersistError::StringTooLarge)?;
        let (key_bytes, rest) = take(rest, key_len)?;
        let type_key =
            std::str::from_utf8(key_bytes).map_err(|_| PersistError::InvalidUserDataTypeKey)?;
        if type_key.is_empty() {
            return Err(PersistError::InvalidUserDataTypeKey);
        }
        let (payload_len_bytes, rest) = take(rest, 8)?;
        let payload_len =
            u64::from_le_bytes(payload_len_bytes.try_into().expect("slice length checked"));
        let payload_len = usize::try_from(payload_len).map_err(|_| PersistError::StringTooLarge)?;
        let (payload, rest) = take(rest, payload_len)?;

        let Some(adapter) = self.userdata_adapter.as_deref_mut() else {
            return Err(PersistError::UnsupportedUserData);
        };
        let top_before = lua_gettop(self.state);
        if let Err(err) = adapter.load_userdata(self.state, type_key, payload) {
            while lua_gettop(self.state) > top_before {
                lua_pop(self.state, 1);
            }
            return Err(err);
        }
        if lua_gettop(self.state) != top_before + 1 || lua_type(self.state, -1) != LUA_TUSERDATA {
            while lua_gettop(self.state) > top_before {
                lua_pop(self.state, 1);
            }
            return Err(PersistError::InvalidUserDataLoad);
        }

        let reference = persist_ref(self.state, -1);
        self.refs.push(LoadedRef::Registry {
            reference,
            kind: "userdata",
        });
        Ok(rest)
    }

    unsafe fn load_function<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], PersistError> {
        let (id, rest) = read_u32(input)?;
        self.expect_next_id(id)?;
        let (&kind, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
        if kind != FUNCTION_KIND_LUA {
            return Err(PersistError::UnsupportedVmState(
                "C closure load requires permanents",
            ));
        }
        let (&preload, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
        self.refs.push(LoadedRef::Pending { kind: "function" });

        let rest = self.load_value(rest)?;
        let env_index = lua_absindex(self.state, -1);
        let rest = self.load_value(rest)?;
        let proto_index = lua_absindex(self.state, -1);
        if !push_lua_closure_for_proto(self.state, proto_index, env_index) {
            lua_pop(self.state, 2);
            return Err(PersistError::MalformedVmState(
                "failed to construct Lua closure",
            ));
        }
        let closure_index = lua_absindex(self.state, -1);
        if !set_lua_closure_preload(self.state, closure_index, preload != 0) {
            lua_pop(self.state, 3);
            return Err(PersistError::MalformedVmState(
                "failed to restore Lua closure preload flag",
            ));
        }
        let reference = persist_ref(self.state, closure_index);
        self.refs[id as usize] = LoadedRef::Registry {
            reference,
            kind: "function",
        };

        let (upref_count, mut rest) = read_u32(rest)?;
        for upref_index in 0..upref_count {
            let (&upref_kind, next) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
            rest = next;
            match upref_kind {
                UPREF_KIND_DIRECT => {
                    rest = self.load_value(rest)?;
                    if !set_lua_closure_upref_value(
                        self.state,
                        closure_index,
                        upref_index as c_int,
                        -1,
                    ) {
                        lua_pop(self.state, 1);
                        return Err(PersistError::MalformedVmState("failed to set direct upref"));
                    }
                    lua_pop(self.state, 1);
                }
                UPREF_KIND_SHARED_CLOSED => {
                    rest =
                        self.load_closed_upvalue_into(closure_index, upref_index as c_int, rest)?;
                }
                UPREF_KIND_OPEN => {
                    rest =
                        self.load_open_upvalue_into(closure_index, upref_index as c_int, rest)?;
                }
                _ => return Err(PersistError::MalformedVmState("invalid closure upref kind")),
            }
        }

        lua_remove(self.state, proto_index);
        lua_remove(self.state, env_index);
        Ok(rest)
    }

    unsafe fn load_proto<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], PersistError> {
        let (id, rest) = read_u32(input)?;
        self.expect_next_id(id)?;

        let (&nups, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
        let (&numparams, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
        let (&is_vararg, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
        let (&maxstacksize, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
        let (&flags, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
        let (linedefined, rest) = read_i32(rest)?;
        let (bytecodeid, rest) = read_i32(rest)?;
        let (code_len, mut rest) = read_u32(rest)?;

        let mut code = Vec::with_capacity(code_len as usize);
        for _ in 0..code_len {
            let (word, next) = read_u32(rest)?;
            code.push(word);
            rest = next;
        }

        let (constant_count, after_constant_count) = read_u32(rest)?;
        let (child_count, _) = peek_child_count(after_constant_count, constant_count)?;
        let build_info = ProtoBuildInfo {
            nups: i32::from(nups),
            numparams: i32::from(numparams),
            is_vararg: i32::from(is_vararg),
            maxstacksize: i32::from(maxstacksize),
            flags: i32::from(flags),
            sizecode: i32::try_from(code.len())
                .map_err(|_| PersistError::MalformedVmState("Proto code too large"))?,
            sizek: i32::try_from(constant_count)
                .map_err(|_| PersistError::MalformedVmState("Proto constant count too large"))?,
            sizep: i32::try_from(child_count)
                .map_err(|_| PersistError::MalformedVmState("child Proto count too large"))?,
            linedefined,
            bytecodeid,
        };

        lua_newtable(self.state);
        let env_index = lua_absindex(self.state, -1);
        let Some(proto_pointer) = allocate_proto_skeleton(self.state, &build_info, &code) else {
            lua_pop(self.state, 1);
            return Err(PersistError::MalformedVmState(
                "failed to allocate Proto skeleton",
            ));
        };
        self.refs.push(LoadedRef::Pending { kind: "proto" });

        rest = after_constant_count;
        for constant_index in 0..constant_count {
            if let Some((&tag_byte, after_tag)) = rest.split_first() {
                match Tag::from_byte(tag_byte)? {
                    Tag::String => {
                        let (len_bytes, after_len) = take(after_tag, 8)?;
                        let len =
                            u64::from_le_bytes(len_bytes.try_into().expect("slice length checked"));
                        let len = usize::try_from(len).map_err(|_| PersistError::StringTooLarge)?;
                        let (bytes, after_string) = take(after_len, len)?;
                        if !set_proto_constant_string_from_pointer(
                            self.state,
                            proto_pointer,
                            constant_index as c_int,
                            bytes,
                        ) {
                            lua_remove(self.state, env_index);
                            return Err(PersistError::MalformedVmState(
                                "failed to set Proto string constant",
                            ));
                        }
                        rest = after_string;
                        continue;
                    }
                    Tag::Nil => {
                        if !set_proto_constant_nil_from_pointer(
                            self.state,
                            proto_pointer,
                            constant_index as c_int,
                        ) {
                            lua_remove(self.state, env_index);
                            return Err(PersistError::MalformedVmState(
                                "failed to set Proto nil constant",
                            ));
                        }
                        rest = after_tag;
                        continue;
                    }
                    Tag::BooleanFalse | Tag::BooleanTrue => {
                        if !set_proto_constant_boolean_from_pointer(
                            self.state,
                            proto_pointer,
                            constant_index as c_int,
                            tag_byte == Tag::BooleanTrue as u8,
                        ) {
                            lua_remove(self.state, env_index);
                            return Err(PersistError::MalformedVmState(
                                "failed to set Proto boolean constant",
                            ));
                        }
                        rest = after_tag;
                        continue;
                    }
                    Tag::Number => {
                        let (bytes, after_number) = take(after_tag, 8)?;
                        let value =
                            f64::from_le_bytes(bytes.try_into().expect("slice length checked"));
                        if !set_proto_constant_number_from_pointer(
                            self.state,
                            proto_pointer,
                            constant_index as c_int,
                            value,
                        ) {
                            lua_remove(self.state, env_index);
                            return Err(PersistError::MalformedVmState(
                                "failed to set Proto number constant",
                            ));
                        }
                        rest = after_number;
                        continue;
                    }
                    _ => {}
                }
            }
            rest = self.load_value(rest)?;
            if !set_proto_constant_from_pointer(
                self.state,
                proto_pointer,
                constant_index as c_int,
                -1,
            ) {
                lua_pop(self.state, 1);
                lua_remove(self.state, env_index);
                return Err(PersistError::MalformedVmState(
                    "failed to set Proto constant",
                ));
            }
            lua_pop(self.state, 1);
        }

        let (actual_child_count, mut rest) = read_u32(rest)?;
        if actual_child_count != child_count {
            lua_remove(self.state, env_index);
            return Err(PersistError::MalformedVmState("child Proto count mismatch"));
        }
        for child_index in 0..child_count {
            rest = self.load_value(rest)?;
            if !set_proto_child_from_pointer(self.state, proto_pointer, child_index as c_int, -1) {
                lua_pop(self.state, 1);
                lua_remove(self.state, env_index);
                return Err(PersistError::MalformedVmState("failed to set child Proto"));
            }
            lua_pop(self.state, 1);
        }
        rest = self.load_proto_metadata(proto_pointer, rest)?;
        if !push_lua_closure_for_proto_pointer(self.state, proto_pointer, env_index) {
            lua_remove(self.state, env_index);
            return Err(PersistError::MalformedVmState(
                "failed to push retained Proto closure",
            ));
        }
        let proto_closure_index = lua_absindex(self.state, -1);
        let reference = persist_ref(self.state, proto_closure_index);
        self.refs[id as usize] = LoadedRef::Registry {
            reference,
            kind: "proto",
        };

        lua_remove(self.state, env_index);
        Ok(rest)
    }

    unsafe fn load_proto_metadata<'a>(
        &mut self,
        proto_pointer: usize,
        input: &'a [u8],
    ) -> Result<&'a [u8], PersistError> {
        let (linegaplog2, rest) = read_i32(input)?;
        let (lineinfo, rest) = read_bytes(rest)?;
        if !set_proto_lineinfo_from_pointer(self.state, proto_pointer, linegaplog2, lineinfo) {
            return Err(PersistError::MalformedVmState(
                "failed to restore Proto lineinfo",
            ));
        }

        let (typeinfo, rest) = read_bytes(rest)?;
        if !set_proto_typeinfo_from_pointer(self.state, proto_pointer, typeinfo) {
            return Err(PersistError::MalformedVmState(
                "failed to restore Proto typeinfo",
            ));
        }

        let (source, rest) = read_optional_bytes(rest)?;
        if !set_proto_source_from_pointer(self.state, proto_pointer, source) {
            return Err(PersistError::MalformedVmState(
                "failed to restore Proto source",
            ));
        }

        let (debugname, rest) = read_optional_bytes(rest)?;
        if !set_proto_debugname_from_pointer(self.state, proto_pointer, debugname) {
            return Err(PersistError::MalformedVmState(
                "failed to restore Proto debug name",
            ));
        }

        let (locvar_count, mut rest) = read_u32(rest)?;
        let locvar_count_i32 = i32::try_from(locvar_count)
            .map_err(|_| PersistError::MalformedVmState("too many Proto local variables"))?;
        if !init_proto_locvars_from_pointer(self.state, proto_pointer, locvar_count_i32) {
            return Err(PersistError::MalformedVmState(
                "failed to allocate Proto local variables",
            ));
        }
        for locvar_index in 0..locvar_count {
            let (startpc, next) = read_i32(rest)?;
            let (endpc, next) = read_i32(next)?;
            let (reg, next) = read_i32(next)?;
            let (name, next) = read_bytes(next)?;
            rest = next;
            if !set_proto_locvar_from_pointer(
                self.state,
                proto_pointer,
                locvar_index as c_int,
                Some(name),
                LocVarInfo {
                    startpc,
                    endpc,
                    reg,
                },
            ) {
                return Err(PersistError::MalformedVmState(
                    "failed to restore Proto local variable",
                ));
            }
        }

        let (upvalue_name_count, mut rest) = read_u32(rest)?;
        let upvalue_name_count_i32 = i32::try_from(upvalue_name_count)
            .map_err(|_| PersistError::MalformedVmState("too many Proto upvalue names"))?;
        if !init_proto_upvalue_names_from_pointer(self.state, proto_pointer, upvalue_name_count_i32)
        {
            return Err(PersistError::MalformedVmState(
                "failed to allocate Proto upvalue names",
            ));
        }
        for upvalue_index in 0..upvalue_name_count {
            let (name, next) = read_bytes(rest)?;
            rest = next;
            if !set_proto_upvalue_name_from_pointer(
                self.state,
                proto_pointer,
                upvalue_index as c_int,
                Some(name),
            ) {
                return Err(PersistError::MalformedVmState(
                    "failed to restore Proto upvalue name",
                ));
            }
        }

        Ok(rest)
    }

    unsafe fn load_closed_upvalue_into<'a>(
        &mut self,
        closure_index: c_int,
        upref_index: c_int,
        input: &'a [u8],
    ) -> Result<&'a [u8], PersistError> {
        let (&tag, input) = input.split_first().ok_or(PersistError::UnexpectedEof)?;
        match Tag::from_byte(tag)? {
            Tag::Reference => {
                let (id, rest) = read_u32(input)?;
                let (source_ref, source_upref) = self.upvalue_ref(id)?;
                lua_rawgeti_(self.state, LUA_REGISTRYINDEX, source_ref);
                if !set_lua_closure_upref_from_upref(
                    self.state,
                    closure_index,
                    upref_index,
                    -1,
                    source_upref,
                ) {
                    lua_pop(self.state, 1);
                    return Err(PersistError::MalformedVmState("failed to share upvalue"));
                }
                lua_pop(self.state, 1);
                Ok(rest)
            }
            Tag::Upvalue => {
                let (id, rest) = read_u32(input)?;
                self.expect_next_id(id)?;
                let (&kind, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
                if kind != UPVALUE_KIND_SHARED_CLOSED {
                    return Err(PersistError::MalformedVmState("upvalue kind mismatch"));
                }
                self.refs.push(LoadedRef::Pending { kind: "upvalue" });
                let rest = self.load_value(rest)?;
                if !set_lua_closure_upref_closed(self.state, closure_index, upref_index, -1) {
                    lua_pop(self.state, 1);
                    return Err(PersistError::MalformedVmState(
                        "failed to create closed upvalue",
                    ));
                }
                lua_pop(self.state, 1);

                lua_pushvalue(self.state, closure_index);
                let closure_reference = persist_ref(self.state, -1);
                lua_pop(self.state, 1);
                self.refs[id as usize] = LoadedRef::Upvalue {
                    closure_reference,
                    upref_index,
                };
                Ok(rest)
            }
            other => Err(PersistError::ReferenceKindMismatch {
                id: self.refs.len() as u32,
                expected: "upvalue",
                found: tag_name(other),
            }),
        }
    }

    unsafe fn load_open_upvalue_into<'a>(
        &mut self,
        closure_index: c_int,
        upref_index: c_int,
        input: &'a [u8],
    ) -> Result<&'a [u8], PersistError> {
        let (&tag, input) = input.split_first().ok_or(PersistError::UnexpectedEof)?;
        let ((thread_reference, stack_offset), rest) = match Tag::from_byte(tag)? {
            Tag::Reference => {
                let (id, rest) = read_u32(input)?;
                (self.open_upvalue_ref(id)?, rest)
            }
            Tag::Upvalue => {
                let (id, rest) = read_u32(input)?;
                self.expect_next_id(id)?;
                let (&kind, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
                if kind != UPVALUE_KIND_OPEN {
                    return Err(PersistError::MalformedVmState("upvalue kind mismatch"));
                }
                self.refs.push(LoadedRef::Pending { kind: "upvalue" });
                let (thread_reference, rest) = self.load_thread_reference(rest)?;
                let (stack_offset, rest) = read_i32(rest)?;
                if stack_offset < 0 {
                    return Err(PersistError::MalformedVmState(
                        "invalid open upvalue stack offset",
                    ));
                }
                self.refs[id as usize] = LoadedRef::OpenUpvalue {
                    thread_reference,
                    stack_offset,
                };
                ((thread_reference, stack_offset), rest)
            }
            other => {
                return Err(PersistError::ReferenceKindMismatch {
                    id: self.refs.len() as u32,
                    expected: "upvalue",
                    found: tag_name(other),
                });
            }
        };

        lua_pushvalue(self.state, closure_index);
        let closure_reference = persist_ref(self.state, -1);
        lua_pop(self.state, 1);
        self.pending_open_upvalue_patches.push(OpenUpvaluePatch {
            closure_reference,
            upref_index,
            thread_reference,
            stack_offset,
        });
        Ok(rest)
    }

    unsafe fn load_thread<'a>(&mut self, input: &'a [u8]) -> Result<&'a [u8], PersistError> {
        let (id, rest) = read_u32(input)?;
        self.expect_next_id(id)?;
        let (status, rest) = read_i32(rest)?;
        if status != LUA_YIELD && status != LUA_OK {
            return Err(PersistError::UnsupportedVmState(
                "only yielded or inactive threads are supported",
            ));
        }
        let (top_offset, rest) = read_i32(rest)?;
        let (base_offset, rest) = read_i32(rest)?;
        let (current_ci, rest) = read_i32(rest)?;
        let (frame_count, mut rest) = read_u32(rest)?;

        if top_offset <= 0 || base_offset < 0 || current_ci < 0 {
            return Err(PersistError::MalformedVmState("invalid thread header"));
        }
        if current_ci as u32 >= frame_count {
            return Err(PersistError::MalformedVmState("invalid current call frame"));
        }

        lua_newthread(self.state);
        let thread_index = lua_absindex(self.state, -1);
        lua_pushvalue(self.state, thread_index);
        let reference = persist_ref(self.state, -1);
        lua_pop(self.state, 1);
        self.refs.push(LoadedRef::Registry {
            reference,
            kind: "thread",
        });

        for stack_offset in 0..top_offset {
            rest = self.load_value(rest)?;
            if !set_thread_stack_value(self.state, thread_index, stack_offset, -1) {
                lua_pop(self.state, 1);
                return Err(PersistError::MalformedVmState(
                    "failed to restore thread stack",
                ));
            }
            lua_pop(self.state, 1);
        }

        self.apply_pending_open_upvalue_patches_for_thread(reference)?;

        let mut frames = Vec::with_capacity(frame_count as usize);
        for _ in 0..frame_count {
            let (base_offset, next) = read_i32(rest)?;
            let (func_offset, next) = read_i32(next)?;
            let (top_offset, next) = read_i32(next)?;
            let (savedpc_offset, next) = read_i32(next)?;
            let (savedpc_present, next) = read_i32(next)?;
            let (nresults, next) = read_i32(next)?;
            let (flags, next) = read_u32(next)?;
            rest = next;
            frames.push(CallFrameBuildInfo {
                base_offset,
                func_offset,
                top_offset,
                savedpc_offset,
                savedpc_present,
                nresults,
                flags,
            });
        }

        if !configure_thread(
            self.state,
            thread_index,
            status,
            top_offset,
            base_offset,
            top_offset,
            frame_count as c_int,
            current_ci,
        ) {
            return Err(PersistError::MalformedVmState("failed to configure thread"));
        }

        for (frame_index, frame) in frames.iter().enumerate() {
            if !set_call_frame(self.state, thread_index, frame_index as c_int, frame) {
                return Err(PersistError::MalformedVmState(
                    "failed to restore call frame",
                ));
            }
        }

        Ok(rest)
    }

    unsafe fn apply_pending_open_upvalue_patches_for_thread(
        &mut self,
        thread_reference: c_int,
    ) -> Result<(), PersistError> {
        let mut remaining = Vec::new();
        let mut pending = std::mem::take(&mut self.pending_open_upvalue_patches);
        while let Some(patch) = pending.pop() {
            if patch.thread_reference != thread_reference {
                remaining.push(patch);
                continue;
            }

            lua_rawgeti_(self.state, LUA_REGISTRYINDEX, patch.closure_reference);
            lua_rawgeti_(self.state, LUA_REGISTRYINDEX, patch.thread_reference);
            let ok = set_lua_closure_upref_open(
                self.state,
                -2,
                patch.upref_index,
                -1,
                patch.stack_offset,
            );
            lua_pop(self.state, 2);
            lua_unref(self.state, patch.closure_reference);
            if !ok {
                remaining.extend(pending.into_iter());
                self.pending_open_upvalue_patches = remaining;
                return Err(PersistError::MalformedVmState(
                    "failed to restore open upvalue",
                ));
            }
        }
        self.pending_open_upvalue_patches = remaining;
        Ok(())
    }

    fn expect_next_id(&self, id: u32) -> Result<(), PersistError> {
        if id as usize != self.refs.len() {
            return Err(PersistError::InvalidObjectId {
                id,
                expected: self.refs.len() as u32,
            });
        }
        Ok(())
    }

    fn registry_ref(&self, id: u32, expected: &'static str) -> Result<c_int, PersistError> {
        match self.refs.get(id as usize) {
            Some(LoadedRef::Registry { reference, .. }) => Ok(*reference),
            Some(other) => Err(PersistError::ReferenceKindMismatch {
                id,
                expected,
                found: other.kind(),
            }),
            None => Err(PersistError::UnknownReference(id)),
        }
    }

    fn upvalue_ref(&self, id: u32) -> Result<(c_int, c_int), PersistError> {
        match self.refs.get(id as usize) {
            Some(LoadedRef::Upvalue {
                closure_reference,
                upref_index,
            }) => Ok((*closure_reference, *upref_index)),
            Some(other) => Err(PersistError::ReferenceKindMismatch {
                id,
                expected: "upvalue",
                found: other.kind(),
            }),
            None => Err(PersistError::UnknownReference(id)),
        }
    }

    fn open_upvalue_ref(&self, id: u32) -> Result<(c_int, c_int), PersistError> {
        match self.refs.get(id as usize) {
            Some(LoadedRef::OpenUpvalue {
                thread_reference,
                stack_offset,
            }) => Ok((*thread_reference, *stack_offset)),
            Some(other) => Err(PersistError::ReferenceKindMismatch {
                id,
                expected: "upvalue",
                found: other.kind(),
            }),
            None => Err(PersistError::UnknownReference(id)),
        }
    }

    fn load_thread_reference<'a>(
        &self,
        input: &'a [u8],
    ) -> Result<(c_int, &'a [u8]), PersistError> {
        let (&tag, input) = input.split_first().ok_or(PersistError::UnexpectedEof)?;
        match Tag::from_byte(tag)? {
            Tag::Reference => {
                let (id, rest) = read_u32(input)?;
                Ok((self.registry_ref(id, "thread")?, rest))
            }
            other => Err(PersistError::ReferenceKindMismatch {
                id: self.refs.len() as u32,
                expected: "thread",
                found: tag_name(other),
            }),
        }
    }
}

impl Drop for LoadContext<'_> {
    fn drop(&mut self) {
        for patch in self.pending_open_upvalue_patches.drain(..) {
            unsafe { lua_unref(self.state, patch.closure_reference) };
        }
        for reference in self.refs.drain(..) {
            let reference = match reference {
                LoadedRef::Pending { .. } => continue,
                LoadedRef::Registry { reference, .. } => reference,
                LoadedRef::Upvalue {
                    closure_reference, ..
                } => closure_reference,
                LoadedRef::OpenUpvalue { .. } => continue,
            };
            unsafe { lua_unref(self.state, reference) };
        }
    }
}

impl LoadedRef {
    fn kind(&self) -> &'static str {
        match self {
            Self::Pending { kind } => kind,
            Self::Registry { kind, .. } => kind,
            Self::Upvalue { .. } => "upvalue",
            Self::OpenUpvalue { .. } => "upvalue",
        }
    }
}

#[cfg(feature = "luau-vector4")]
const VECTOR_COMPONENTS: u8 = 4;

#[cfg(not(feature = "luau-vector4"))]
const VECTOR_COMPONENTS: u8 = 3;

#[cfg(feature = "luau-vector4")]
unsafe fn push_vector(state: *mut lua_State, values: [f32; 4]) {
    lua_pushvector(state, values[0], values[1], values[2], values[3]);
}

#[cfg(not(feature = "luau-vector4"))]
unsafe fn push_vector(state: *mut lua_State, values: [f32; 4]) {
    lua_pushvector(state, values[0], values[1], values[2]);
}

const TABLE_READONLY: u8 = 0x01;
const FUNCTION_KIND_LUA: u8 = 0x00;
const UPREF_KIND_DIRECT: u8 = 0x00;
const UPREF_KIND_SHARED_CLOSED: u8 = 0x01;
const UPREF_KIND_OPEN: u8 = 0x02;
const UPVALUE_KIND_SHARED_CLOSED: u8 = 0x01;
const UPVALUE_KIND_OPEN: u8 = 0x02;
const THREAD_FRAME_BYTES: usize = 28;

fn is_reference_type(type_code: c_int) -> bool {
    matches!(
        type_code,
        LUA_TTABLE | LUA_TBUFFER | LUA_TFUNCTION | LUA_TUSERDATA | LUA_TTHREAD
    )
}

unsafe fn dump_permanent_key(
    state: *mut lua_State,
    index: c_int,
    out: &mut Vec<u8>,
) -> Result<(), PersistError> {
    match lua_type(state, index) {
        LUA_TNIL => out.push(Tag::Nil as u8),
        LUA_TBOOLEAN => {
            if lua_toboolean(state, index) == 0 {
                out.push(Tag::BooleanFalse as u8);
            } else {
                out.push(Tag::BooleanTrue as u8);
            }
        }
        LUA_TNUMBER => {
            out.push(Tag::Number as u8);
            out.extend_from_slice(&lua_tonumber(state, index).to_le_bytes());
        }
        LUA_TSTRING => {
            let mut len = 0usize;
            let ptr = lua_tolstring(state, index, &mut len);
            out.push(Tag::String as u8);
            out.extend_from_slice(&(len as u64).to_le_bytes());
            out.extend_from_slice(std::slice::from_raw_parts(ptr.cast::<u8>(), len));
        }
        LUA_TVECTOR => {
            let ptr = lua_tovector(state, index);
            out.push(Tag::Vector as u8);
            out.push(VECTOR_COMPONENTS);
            for i in 0..usize::from(VECTOR_COMPONENTS) {
                out.extend_from_slice(&(*ptr.add(i)).to_le_bytes());
            }
        }
        type_code => {
            return Err(PersistError::UnsupportedPermanentKey {
                type_code,
                type_name: type_name(type_code).to_string(),
            });
        }
    }

    Ok(())
}

fn take(input: &[u8], count: usize) -> Result<(&[u8], &[u8]), PersistError> {
    if input.len() < count {
        return Err(PersistError::UnexpectedEof);
    }
    Ok(input.split_at(count))
}

fn read_u32(input: &[u8]) -> Result<(u32, &[u8]), PersistError> {
    let (bytes, rest) = take(input, 4)?;
    Ok((
        u32::from_le_bytes(bytes.try_into().expect("slice length checked")),
        rest,
    ))
}

fn read_i32(input: &[u8]) -> Result<(i32, &[u8]), PersistError> {
    let (bytes, rest) = take(input, 4)?;
    Ok((
        i32::from_le_bytes(bytes.try_into().expect("slice length checked")),
        rest,
    ))
}

fn read_u64(input: &[u8]) -> Result<(u64, &[u8]), PersistError> {
    let (bytes, rest) = take(input, 8)?;
    Ok((
        u64::from_le_bytes(bytes.try_into().expect("slice length checked")),
        rest,
    ))
}

fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), PersistError> {
    out.extend_from_slice(
        &u64::try_from(bytes.len())
            .map_err(|_| PersistError::TooManyObjects)?
            .to_le_bytes(),
    );
    out.extend_from_slice(bytes);
    Ok(())
}

fn read_bytes(input: &[u8]) -> Result<(&[u8], &[u8]), PersistError> {
    let (len, rest) = read_u64(input)?;
    let len = usize::try_from(len).map_err(|_| PersistError::StringTooLarge)?;
    take(rest, len)
}

fn read_optional_bytes(input: &[u8]) -> Result<(Option<&[u8]>, &[u8]), PersistError> {
    let (&present, rest) = input.split_first().ok_or(PersistError::UnexpectedEof)?;
    match present {
        0 => Ok((None, rest)),
        1 => {
            let (bytes, rest) = read_bytes(rest)?;
            Ok((Some(bytes), rest))
        }
        _ => Err(PersistError::MalformedVmState(
            "invalid optional bytes flag",
        )),
    }
}

fn push_u8(out: &mut Vec<u8>, value: i32, name: &'static str) -> Result<(), PersistError> {
    let value = u8::try_from(value).map_err(|_| PersistError::MalformedVmState(name))?;
    out.push(value);
    Ok(())
}

fn validate_dumpable_proto(proto: ProtoInfo) -> Result<(), PersistError> {
    if proto.has_execdata != 0 || proto.exectarget != 0 {
        return Err(PersistError::UnsupportedVmState(
            "Proto has native execution data",
        ));
    }
    Ok(())
}

fn peek_child_count(input: &[u8], mut constants: u32) -> Result<(u32, &[u8]), PersistError> {
    let mut rest = input;
    while constants > 0 {
        rest = skip_value(rest)?;
        constants -= 1;
    }
    read_u32(rest)
}

fn skip_value(input: &[u8]) -> Result<&[u8], PersistError> {
    let (&tag, input) = input.split_first().ok_or(PersistError::UnexpectedEof)?;
    match Tag::from_byte(tag)? {
        Tag::Nil | Tag::BooleanFalse | Tag::BooleanTrue => Ok(input),
        Tag::Number => Ok(take(input, 8)?.1),
        Tag::String => {
            let (len, rest) = read_u64(input)?;
            let len = usize::try_from(len).map_err(|_| PersistError::StringTooLarge)?;
            Ok(take(rest, len)?.1)
        }
        Tag::Vector => {
            let (&components, rest) = input.split_first().ok_or(PersistError::UnexpectedEof)?;
            Ok(take(rest, usize::from(components) * 4)?.1)
        }
        Tag::Buffer => {
            let rest = take(input, 4)?.1;
            let (len, rest) = read_u64(rest)?;
            let len = usize::try_from(len).map_err(|_| PersistError::StringTooLarge)?;
            Ok(take(rest, len)?.1)
        }
        Tag::Reference => Ok(take(input, 4)?.1),
        Tag::Permanent => {
            let rest = take(input, 8)?.1;
            skip_value(rest)
        }
        Tag::Table => {
            let rest = take(input, 4)?.1;
            let rest = take(rest, 1)?.1;
            let (count, mut rest) = read_u64(rest)?;
            for _ in 0..count {
                rest = skip_value(rest)?;
                rest = skip_value(rest)?;
            }
            skip_value(rest)
        }
        Tag::Function => skip_function(input),
        Tag::Proto => skip_proto(input),
        Tag::Upvalue => skip_upvalue(input),
        Tag::Thread => skip_thread(input),
        Tag::UserData => skip_userdata(input),
    }
}

fn skip_userdata(input: &[u8]) -> Result<&[u8], PersistError> {
    let rest = take(input, 4)?.1;
    let (key_len, rest) = read_u64(rest)?;
    let key_len = usize::try_from(key_len).map_err(|_| PersistError::StringTooLarge)?;
    let rest = take(rest, key_len)?.1;
    let (payload_len, rest) = read_u64(rest)?;
    let payload_len = usize::try_from(payload_len).map_err(|_| PersistError::StringTooLarge)?;
    Ok(take(rest, payload_len)?.1)
}

fn skip_function(input: &[u8]) -> Result<&[u8], PersistError> {
    let rest = take(input, 4)?.1;
    let (&kind, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
    if kind != FUNCTION_KIND_LUA {
        return Err(PersistError::UnsupportedVmState(
            "C closure load requires permanents",
        ));
    }

    let rest = take(rest, 1)?.1;
    let rest = skip_value(rest)?;
    let rest = skip_value(rest)?;
    let (upref_count, mut rest) = read_u32(rest)?;
    for _ in 0..upref_count {
        let (&upref_kind, next) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
        rest = next;
        match upref_kind {
            UPREF_KIND_DIRECT => rest = skip_value(rest)?,
            UPREF_KIND_SHARED_CLOSED => rest = skip_value(rest)?,
            UPREF_KIND_OPEN => rest = skip_value(rest)?,
            _ => return Err(PersistError::MalformedVmState("invalid closure upref kind")),
        }
    }
    Ok(rest)
}

fn skip_upvalue(input: &[u8]) -> Result<&[u8], PersistError> {
    let rest = take(input, 4)?.1;
    let (&kind, rest) = rest.split_first().ok_or(PersistError::UnexpectedEof)?;
    if kind == UPVALUE_KIND_SHARED_CLOSED {
        skip_value(rest)
    } else if kind == UPVALUE_KIND_OPEN {
        let rest = skip_value(rest)?;
        Ok(take(rest, 4)?.1)
    } else {
        Err(PersistError::MalformedVmState("invalid upvalue kind"))
    }
}

fn skip_proto(input: &[u8]) -> Result<&[u8], PersistError> {
    let rest = take(input, 4)?.1;
    let rest = take(rest, 5)?.1;
    let rest = take(rest, 8)?.1;
    let (code_len, rest) = read_u32(rest)?;
    let mut rest = take(rest, code_len as usize * 4)?.1;
    let (constant_count, next) = read_u32(rest)?;
    rest = next;
    for _ in 0..constant_count {
        rest = skip_value(rest)?;
    }
    let (child_count, next) = read_u32(rest)?;
    rest = next;
    for _ in 0..child_count {
        rest = skip_value(rest)?;
    }

    let (_, next) = read_i32(rest)?;
    let (_, next) = read_bytes(next)?;
    let (_, next) = read_bytes(next)?;
    let (_, next) = read_optional_bytes(next)?;
    let (_, mut rest) = read_optional_bytes(next)?;
    let (locvar_count, next) = read_u32(rest)?;
    rest = next;
    for _ in 0..locvar_count {
        let (_, next) = read_i32(rest)?;
        let (_, next) = read_i32(next)?;
        let (_, next) = read_i32(next)?;
        let (_, next) = read_bytes(next)?;
        rest = next;
    }
    let (upvalue_name_count, next) = read_u32(rest)?;
    rest = next;
    for _ in 0..upvalue_name_count {
        let (_, next) = read_bytes(rest)?;
        rest = next;
    }
    Ok(rest)
}

fn skip_thread(input: &[u8]) -> Result<&[u8], PersistError> {
    let rest = take(input, 4)?.1;
    let rest = take(rest, 16)?.1;
    let (frame_count, mut rest) = read_u32(rest)?;

    let stack_count = {
        let (top_offset, _) = read_i32(take(input, 8)?.1)?;
        top_offset
    };
    if stack_count < 0 {
        return Err(PersistError::MalformedVmState("invalid thread stack count"));
    }
    for _ in 0..stack_count {
        rest = skip_value(rest)?;
    }
    let frame_bytes = usize::try_from(frame_count)
        .map_err(|_| PersistError::MalformedVmState("thread frame count too large"))?
        * THREAD_FRAME_BYTES;
    Ok(take(rest, frame_bytes)?.1)
}

fn tag_name(tag: Tag) -> &'static str {
    match tag {
        Tag::Nil => "nil",
        Tag::BooleanFalse | Tag::BooleanTrue => "boolean",
        Tag::Number => "number",
        Tag::String => "string",
        Tag::Buffer => "buffer",
        Tag::Vector => "vector",
        Tag::Table => "table",
        Tag::Reference => "reference",
        Tag::Permanent => "permanent",
        Tag::Function => "function",
        Tag::Proto => "proto",
        Tag::Upvalue => "upvalue",
        Tag::Thread => "thread",
        Tag::UserData => "userdata",
    }
}

unsafe fn stack_string_lossy(state: *mut lua_State, index: c_int) -> Option<String> {
    if lua_type(state, index) != LUA_TSTRING {
        return None;
    }
    let mut len = 0usize;
    let ptr = lua_tolstring(state, index, &mut len);
    if ptr.is_null() {
        return None;
    }
    let bytes = std::slice::from_raw_parts(ptr.cast::<u8>(), len);
    Some(String::from_utf8_lossy(bytes).into_owned())
}

fn ensure_empty(input: &[u8]) -> Result<(), PersistError> {
    if input.is_empty() {
        Ok(())
    } else {
        Err(PersistError::TrailingBytes)
    }
}

fn type_name(type_code: c_int) -> &'static str {
    match type_code {
        LUA_TNONE => "none",
        LUA_TNIL => "nil",
        LUA_TBOOLEAN => "boolean",
        LUA_TLIGHTUSERDATA => "lightuserdata",
        LUA_TNUMBER => "number",
        LUA_TVECTOR => "vector",
        LUA_TSTRING => "string",
        LUA_TTABLE => "table",
        LUA_TFUNCTION => "function",
        LUA_TUSERDATA => "userdata",
        LUA_TTHREAD => "thread",
        LUA_TBUFFER => "buffer",
        _ => "unknown",
    }
}

#[allow(dead_code)]
fn cstr_to_string(ptr: *const std::os::raw::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}
