#include "lua.h"
#include "lapi.h"
#include "ldo.h"
#include "lfunc.h"
#include "lgc.h"
#include "lmem.h"
#include "lstring.h"
#include "ltable.h"
#include "lvm.h"
#include "lobject.h"
#include "lstate.h"

#include <limits.h>
#include <stdint.h>
#include <string.h>

extern "C" {

int32_t luau_persist_ref(lua_State* L, int32_t index)
{
    int ref = LUA_REFNIL;
    global_State* g = L->global;
    const TValue* value = nullptr;
    if (index > 0)
    {
        TValue* slot = L->base + (index - 1);
        if (slot < L->top)
            value = slot;
    }
    else if (index > LUA_REGISTRYINDEX && index != 0)
    {
        TValue* slot = L->top + index;
        if (slot >= L->base && slot < L->top)
            value = slot;
    }
    else
    {
        value = luaA_toobject(L, index);
    }

    if (!value)
        return LUA_NOREF;

    if (!ttisnil(value))
    {
        LuaTable* reg = hvalue(registry(L));

        if (g->registryfree != 0)
            ref = g->registryfree;
        else
        {
            int registry_count = luaH_getn(reg);
            ref = registry_count + 1;
        }

        if (ref > reg->sizearray)
            luaH_resizearray(L, reg, ref + 4096);

        TValue* slot = luaH_setnum(L, reg, ref);
        if (g->registryfree != 0)
            g->registryfree = int(nvalue(slot));

        setobj2t(L, slot, value);

        if (iscollectable(value) && isblack(obj2gco(reg)) && iswhite(gcvalue(value)) && g->gcstate == GCSpause)
        {
            uint8_t maskmarks = uint8_t(~(bitmask(BLACKBIT) | WHITEBITS));
            obj2gco(reg)->gch.marked = uint8_t((obj2gco(reg)->gch.marked & maskmarks) | luaC_white(g));
        }
        else
        {
            luaC_barriert(L, reg, value);
        }
    }

    return int32_t(ref);
}

struct luau_persist_function_info
{
    int32_t is_function;
    int32_t is_c;
    int32_t nupvalues;
    int32_t stack_size;
    int32_t proto_maxstacksize;
    int32_t proto_numparams;
    int32_t proto_is_vararg;
    int32_t proto_sizecode;
    int32_t proto_sizek;
    int32_t proto_sizep;
    int32_t proto_nups;
    int32_t preload;
};

struct luau_persist_proto_info
{
    int32_t nups;
    int32_t numparams;
    int32_t is_vararg;
    int32_t maxstacksize;
    int32_t flags;
    int32_t sizecode;
    int32_t sizep;
    int32_t sizelocvars;
    int32_t sizeupvalues;
    int32_t sizek;
    int32_t sizelineinfo;
    int32_t linegaplog2;
    int32_t linedefined;
    int32_t bytecodeid;
    int32_t sizetypeinfo;
    int32_t has_source;
    int32_t has_debugname;
    int32_t has_execdata;
    uint64_t exectarget;
    uintptr_t identity;
};

struct luau_persist_upref_info
{
    int32_t raw_tag;
    int32_t value_tag;
    int32_t is_upvalue;
    int32_t is_open;
    int64_t stack_offset;
    uintptr_t identity;
};

struct luau_persist_thread_info
{
    int32_t status;
    int32_t is_active;
    int32_t stack_size;
    int32_t stack_in_use;
    int32_t base_offset;
    int32_t top_offset;
    int32_t ci_count;
    int32_t current_ci;
    int32_t open_upvalue_count;
};

struct luau_persist_call_frame_info
{
    int32_t base_offset;
    int32_t func_offset;
    int32_t top_offset;
    int32_t savedpc_offset;
    int32_t savedpc_present;
    int32_t nresults;
    uint32_t flags;
    int32_t is_lua;
    int32_t is_native;
};

struct luau_persist_call_frame_build_info
{
    int32_t base_offset;
    int32_t func_offset;
    int32_t top_offset;
    int32_t savedpc_offset;
    int32_t savedpc_present;
    int32_t nresults;
    uint32_t flags;
};

struct luau_persist_open_upvalue_info
{
    int32_t value_tag;
    int32_t stack_offset;
    uintptr_t identity;
};

struct luau_persist_proto_build_info
{
    int32_t nups;
    int32_t numparams;
    int32_t is_vararg;
    int32_t maxstacksize;
    int32_t flags;
    int32_t sizecode;
    int32_t sizek;
    int32_t sizep;
    int32_t linedefined;
    int32_t bytecodeid;
};

struct luau_persist_locvar_info
{
    int32_t startpc;
    int32_t endpc;
    int32_t reg;
};

static const TValue* stack_value(lua_State* L, int32_t index)
{
    if (index > 0)
    {
        TValue* slot = L->base + (index - 1);
        return slot < L->top ? slot : nullptr;
    }

    if (index > LUA_REGISTRYINDEX && index != 0)
    {
        TValue* slot = L->top + index;
        return slot >= L->base && slot < L->top ? slot : nullptr;
    }

    return luaA_toobject(L, index);
}

static Proto* get_luau_proto(lua_State* L, int32_t index)
{
    const TValue* value = stack_value(L, index);
    if (!value || !ttisfunction(value))
        return nullptr;

    Closure* closure = clvalue(value);
    if (closure->isC)
        return nullptr;

    return closure->l.p;
}

static Proto* get_luau_proto_from_pointer(uintptr_t pointer)
{
    if (!pointer)
        return nullptr;

    return reinterpret_cast<Proto*>(pointer);
}

static Closure* get_luau_closure(lua_State* L, int32_t index)
{
    const TValue* value = stack_value(L, index);
    if (!value || !ttisfunction(value))
        return nullptr;

    Closure* closure = clvalue(value);
    return closure->isC ? nullptr : closure;
}

static lua_State* get_thread(lua_State* L, int32_t index)
{
    const TValue* value = stack_value(L, index);
    if (!value)
        return nullptr;

    if (ttisthread(value))
        return thvalue(value);

    return nullptr;
}

static int32_t ensure_thread_stack(lua_State* thread, int32_t live_slots, int32_t max_top_offset)
{
    if (live_slots < 0 || max_top_offset < live_slots)
        return 0;

    if (max_top_offset >= thread->stacksize - EXTRA_STACK)
        luaD_reallocstack(thread, max_top_offset + 1, 0);

    if (max_top_offset >= thread->stacksize - EXTRA_STACK)
        return 0;

    return 1;
}

static int32_t ensure_thread_ci(lua_State* thread, int32_t ci_count)
{
    if (ci_count <= 0)
        return 0;

    if (ci_count > thread->size_ci)
        luaD_reallocCI(thread, ci_count);

    return ci_count <= thread->size_ci ? 1 : 0;
}

int32_t luau_persist_get_function_info(lua_State* L, int32_t index, luau_persist_function_info* out)
{
    if (!out)
        return 0;

    *out = {};

    const TValue* value = stack_value(L, index);
    if (!value || !ttisfunction(value))
        return 0;

    Closure* closure = clvalue(value);
    out->is_function = 1;
    out->is_c = closure->isC ? 1 : 0;
    out->nupvalues = closure->nupvalues;
    out->stack_size = closure->stacksize;
    out->preload = closure->preload;

    if (!closure->isC)
    {
        Proto* proto = closure->l.p;
        out->proto_maxstacksize = proto->maxstacksize;
        out->proto_numparams = proto->numparams;
        out->proto_is_vararg = proto->is_vararg;
        out->proto_sizecode = proto->sizecode;
        out->proto_sizek = proto->sizek;
        out->proto_sizep = proto->sizep;
        out->proto_nups = proto->nups;
    }

    return 1;
}

int32_t luau_persist_get_proto_info(lua_State* L, int32_t index, luau_persist_proto_info* out)
{
    if (!out)
        return 0;

    *out = {};

    Proto* proto = get_luau_proto(L, index);
    if (!proto)
        return 0;

    out->nups = proto->nups;
    out->numparams = proto->numparams;
    out->is_vararg = proto->is_vararg;
    out->maxstacksize = proto->maxstacksize;
    out->flags = proto->flags;
    out->sizecode = proto->sizecode;
    out->sizep = proto->sizep;
    out->sizelocvars = proto->sizelocvars;
    out->sizeupvalues = proto->sizeupvalues;
    out->sizek = proto->sizek;
    out->sizelineinfo = proto->sizelineinfo;
    out->linegaplog2 = proto->linegaplog2;
    out->linedefined = proto->linedefined;
    out->bytecodeid = proto->bytecodeid;
    out->sizetypeinfo = proto->sizetypeinfo;
    out->has_source = proto->source ? 1 : 0;
    out->has_debugname = proto->debugname ? 1 : 0;
    out->has_execdata = proto->execdata ? 1 : 0;
    out->exectarget = uint64_t(proto->exectarget);
    out->identity = uintptr_t(proto);

    return 1;
}

int32_t luau_persist_copy_proto_code(lua_State* L, int32_t index, uint32_t* out, int32_t out_len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || out_len < 0)
        return -1;

    if (!out)
        return proto->sizecode;

    if (out_len < proto->sizecode)
        return -1;

    for (int32_t i = 0; i < proto->sizecode; ++i)
        out[i] = proto->code[i];

    return proto->sizecode;
}

static int32_t copy_bytes(const void* src, int32_t size, uint8_t* out, int32_t out_len)
{
    if (size < 0)
        return -1;

    if (!out)
        return size;

    if (out_len < size)
        return -1;

    if (size > 0)
        memcpy(out, src, size_t(size));

    return size;
}

static int32_t copy_tstring(TString* str, uint8_t* out, int32_t out_len)
{
    if (!str)
        return 0;

    if (str->len > INT32_MAX)
        return -1;

    return copy_bytes(getstr(str), int32_t(str->len), out, out_len);
}

int32_t luau_persist_copy_proto_lineinfo(lua_State* L, int32_t index, uint8_t* out, int32_t out_len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto)
        return -1;

    return copy_bytes(proto->lineinfo, proto->sizelineinfo, out, out_len);
}

int32_t luau_persist_copy_proto_typeinfo(lua_State* L, int32_t index, uint8_t* out, int32_t out_len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto)
        return -1;

    return copy_bytes(proto->typeinfo, proto->sizetypeinfo, out, out_len);
}

int32_t luau_persist_copy_proto_source(lua_State* L, int32_t index, uint8_t* out, int32_t out_len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto)
        return -1;

    return copy_tstring(proto->source, out, out_len);
}

int32_t luau_persist_copy_proto_debugname(lua_State* L, int32_t index, uint8_t* out, int32_t out_len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto)
        return -1;

    return copy_tstring(proto->debugname, out, out_len);
}

int32_t luau_persist_copy_proto_locvar_info(lua_State* L, int32_t index, luau_persist_locvar_info* out, int32_t out_len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || out_len < 0)
        return -1;

    if (!out)
        return proto->sizelocvars;

    if (out_len < proto->sizelocvars)
        return -1;

    for (int32_t i = 0; i < proto->sizelocvars; ++i)
    {
        out[i].startpc = proto->locvars[i].startpc;
        out[i].endpc = proto->locvars[i].endpc;
        out[i].reg = proto->locvars[i].reg;
    }

    return proto->sizelocvars;
}

int32_t luau_persist_copy_proto_locvar_name(lua_State* L, int32_t index, int32_t locvar_index, uint8_t* out, int32_t out_len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || locvar_index < 0 || locvar_index >= proto->sizelocvars)
        return -1;

    return copy_tstring(proto->locvars[locvar_index].varname, out, out_len);
}

int32_t luau_persist_copy_proto_upvalue_name(lua_State* L, int32_t index, int32_t upvalue_index, uint8_t* out, int32_t out_len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || upvalue_index < 0 || upvalue_index >= proto->sizeupvalues)
        return -1;

    return copy_tstring(proto->upvalues[upvalue_index], out, out_len);
}

int32_t luau_persist_copy_proto_constant_tags(lua_State* L, int32_t index, int32_t* out, int32_t out_len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || out_len < 0)
        return -1;

    if (!out)
        return proto->sizek;

    if (out_len < proto->sizek)
        return -1;

    for (int32_t i = 0; i < proto->sizek; ++i)
        out[i] = ttype(&proto->k[i]);

    return proto->sizek;
}

int32_t luau_persist_push_proto_constant(lua_State* L, int32_t index, int32_t constant_index)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || constant_index < 0 || constant_index >= proto->sizek)
        return 0;

    const TValue* value = &proto->k[constant_index];
    if (iscollectable(value))
        luaC_threadbarrier(L);
    luaA_pushobject(L, value);

    return 1;
}

int32_t luau_persist_push_child_proto_closure(lua_State* L, int32_t index, int32_t child_index, int32_t env_index)
{
    Proto* parent = get_luau_proto(L, index);
    if (!parent || child_index < 0 || child_index >= parent->sizep)
        return 0;

    const TValue* env_value = stack_value(L, env_index);
    if (!env_value || !ttistable(env_value))
        return 0;

    Proto* proto = parent->p[child_index];
    if (!proto)
        return 0;

    LuaTable* env = hvalue(env_value);
    luaC_checkGC(L);
    luaC_threadbarrier(L);

    Closure* closure = luaF_newLclosure(L, proto->nups, env, proto);
    setclvalue(L, L->top, closure);
    incr_top(L);

    return 1;
}

uintptr_t luau_persist_allocate_proto_skeleton(
    lua_State* L,
    const luau_persist_proto_build_info* info,
    const uint32_t* code,
    int32_t code_len
)
{
    if (!info || !code || code_len < 0 || info->sizecode != code_len)
        return 0;

    if (info->nups < 0 || info->nups > UINT8_MAX || info->numparams < 0 || info->numparams > UINT8_MAX ||
        info->is_vararg < 0 || info->is_vararg > UINT8_MAX || info->maxstacksize < 0 || info->maxstacksize > UINT8_MAX ||
        info->flags < 0 || info->flags > UINT8_MAX || info->sizecode <= 0 || info->sizek < 0 || info->sizep < 0)
        return 0;

    luaC_checkGC(L);

    Proto* proto = luaF_newproto(L);
    proto->nups = uint8_t(info->nups);
    proto->numparams = uint8_t(info->numparams);
    proto->is_vararg = uint8_t(info->is_vararg);
    proto->maxstacksize = uint8_t(info->maxstacksize);
    proto->flags = uint8_t(info->flags);
    proto->linedefined = info->linedefined;
    proto->bytecodeid = info->bytecodeid;

    proto->code = luaM_newarray(L, info->sizecode, Instruction, proto->memcat);
    proto->sizecode = info->sizecode;
    for (int32_t i = 0; i < info->sizecode; ++i)
        proto->code[i] = code[i];
    proto->codeentry = proto->code;

    if (info->sizek > 0)
    {
        proto->k = luaM_newarray(L, info->sizek, TValue, proto->memcat);
        proto->sizek = info->sizek;
        for (int32_t i = 0; i < info->sizek; ++i)
            setnilvalue(&proto->k[i]);
    }

    if (info->sizep > 0)
    {
        proto->p = luaM_newarray(L, info->sizep, Proto*, proto->memcat);
        proto->sizep = info->sizep;
        for (int32_t i = 0; i < info->sizep; ++i)
            proto->p[i] = nullptr;
    }

    return uintptr_t(proto);
}

uintptr_t luau_persist_lua_closure_pointer(lua_State* L, int32_t index)
{
    const TValue* value = stack_value(L, index);
    if (!value || !ttisfunction(value))
        return 0;

    Closure* closure = clvalue(value);
    if (closure->isC)
        return 0;

    return uintptr_t(closure);
}

int32_t luau_persist_push_lua_closure_pointer(lua_State* L, uintptr_t pointer)
{
    if (!pointer)
        return 0;

    Closure* closure = reinterpret_cast<Closure*>(pointer);
    if (closure->isC)
        return 0;

    setclvalue(L, L->top, closure);
    incr_top(L);
    return 1;
}

int32_t luau_persist_push_lua_closure_for_proto_pointer(lua_State* L, uintptr_t pointer, int32_t env_index)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto)
        return 0;

    const TValue* env_value = stack_value(L, env_index);
    if (!env_value || !ttistable(env_value))
        return 0;

    LuaTable* env = hvalue(env_value);
    luaC_checkGC(L);
    luaC_threadbarrier(L);

    Closure* closure = luaF_newLclosure(L, proto->nups, env, proto);
    setclvalue(L, L->top, closure);
    incr_top(L);

    return 1;
}

int32_t luau_persist_set_lua_closure_preload(lua_State* L, int32_t index, int32_t preload)
{
    Closure* closure = get_luau_closure(L, index);
    if (!closure)
        return 0;

    closure->preload = preload ? 1 : 0;
    return 1;
}

int32_t luau_persist_set_proto_constant_from_stack(lua_State* L, int32_t index, int32_t constant_index, int32_t value_index)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || constant_index < 0 || constant_index >= proto->sizek)
        return 0;

    const TValue* value = stack_value(L, value_index);
    if (!value)
        return 0;

    setobj(L, &proto->k[constant_index], value);
    luaC_barrier(L, proto, value);

    return 1;
}

int32_t luau_persist_set_proto_constant_from_pointer(lua_State* L, uintptr_t pointer, int32_t constant_index, int32_t value_index)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto || constant_index < 0 || constant_index >= proto->sizek)
        return 0;

    const TValue* value = stack_value(L, value_index);
    if (!value)
        return 0;

    setobj(L, &proto->k[constant_index], value);
    luaC_barrier(L, proto, value);

    return 1;
}

int32_t luau_persist_set_proto_constant_string_from_pointer(lua_State* L, uintptr_t pointer, int32_t constant_index, const uint8_t* data, int32_t len)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto || constant_index < 0 || constant_index >= proto->sizek || len < 0)
        return 0;

    if (!data && len != 0)
        return 0;

    TString* string = luaS_newlstr(L, reinterpret_cast<const char*>(data), size_t(len));
    setsvalue(L, &proto->k[constant_index], string);
    luaC_objbarrier(L, proto, string);

    return 1;
}

int32_t luau_persist_set_proto_constant_nil_from_pointer(lua_State* L, uintptr_t pointer, int32_t constant_index)
{
    (void)L;
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto || constant_index < 0 || constant_index >= proto->sizek)
        return 0;

    setnilvalue(&proto->k[constant_index]);
    return 1;
}

int32_t luau_persist_set_proto_constant_boolean_from_pointer(lua_State* L, uintptr_t pointer, int32_t constant_index, int32_t value)
{
    (void)L;
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto || constant_index < 0 || constant_index >= proto->sizek)
        return 0;

    setbvalue(&proto->k[constant_index], value ? 1 : 0);
    return 1;
}

int32_t luau_persist_set_proto_constant_number_from_pointer(lua_State* L, uintptr_t pointer, int32_t constant_index, double value)
{
    (void)L;
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto || constant_index < 0 || constant_index >= proto->sizek)
        return 0;

    setnvalue(&proto->k[constant_index], value);
    return 1;
}

int32_t luau_persist_set_proto_child_from_closure(lua_State* L, int32_t index, int32_t child_index, int32_t child_closure_index)
{
    Proto* proto = get_luau_proto(L, index);
    Proto* child = get_luau_proto(L, child_closure_index);
    if (!proto || !child || child_index < 0 || child_index >= proto->sizep)
        return 0;

    proto->p[child_index] = child;
    luaC_objbarrier(L, proto, child);

    return 1;
}

int32_t luau_persist_set_proto_child_from_pointer(lua_State* L, uintptr_t pointer, int32_t child_index, int32_t child_closure_index)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    Proto* child = get_luau_proto(L, child_closure_index);
    if (!proto || !child || child_index < 0 || child_index >= proto->sizep)
        return 0;

    proto->p[child_index] = child;
    luaC_objbarrier(L, proto, child);

    return 1;
}

int32_t luau_persist_set_proto_lineinfo(lua_State* L, int32_t index, int32_t linegaplog2, const uint8_t* data, int32_t len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || len < 0 || linegaplog2 < 0 || linegaplog2 > 30)
        return 0;

    if (len == 0)
    {
        proto->lineinfo = nullptr;
        proto->abslineinfo = nullptr;
        proto->sizelineinfo = 0;
        proto->linegaplog2 = 0;
        return 1;
    }

    if (!data || proto->sizecode <= 0)
        return 0;

    int32_t intervals = ((proto->sizecode - 1) >> linegaplog2) + 1;
    int32_t absoffset = (proto->sizecode + 3) & ~3;
    int64_t expected = int64_t(absoffset) + int64_t(intervals) * int64_t(sizeof(int));
    if (expected != len)
        return 0;

    proto->lineinfo = luaM_newarray(L, len, uint8_t, proto->memcat);
    memcpy(proto->lineinfo, data, size_t(len));
    proto->sizelineinfo = len;
    proto->linegaplog2 = linegaplog2;
    proto->abslineinfo = (int*)(proto->lineinfo + absoffset);
    return 1;
}

int32_t luau_persist_set_proto_lineinfo_from_pointer(lua_State* L, uintptr_t pointer, int32_t linegaplog2, const uint8_t* data, int32_t len)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto || len < 0 || linegaplog2 < 0 || linegaplog2 > 30)
        return 0;

    if (len == 0)
    {
        proto->lineinfo = nullptr;
        proto->abslineinfo = nullptr;
        proto->sizelineinfo = 0;
        proto->linegaplog2 = 0;
        return 1;
    }

    if (!data || proto->sizecode <= 0)
        return 0;

    int32_t intervals = ((proto->sizecode - 1) >> linegaplog2) + 1;
    int32_t absoffset = (proto->sizecode + 3) & ~3;
    int64_t expected = int64_t(absoffset) + int64_t(intervals) * int64_t(sizeof(int));
    if (expected != len)
        return 0;

    proto->lineinfo = luaM_newarray(L, len, uint8_t, proto->memcat);
    memcpy(proto->lineinfo, data, size_t(len));
    proto->sizelineinfo = len;
    proto->linegaplog2 = linegaplog2;
    proto->abslineinfo = (int*)(proto->lineinfo + absoffset);
    return 1;
}

int32_t luau_persist_set_proto_typeinfo(lua_State* L, int32_t index, const uint8_t* data, int32_t len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || len < 0)
        return 0;

    if (len == 0)
    {
        proto->typeinfo = nullptr;
        proto->sizetypeinfo = 0;
        return 1;
    }

    if (!data)
        return 0;

    proto->typeinfo = luaM_newarray(L, len, uint8_t, proto->memcat);
    memcpy(proto->typeinfo, data, size_t(len));
    proto->sizetypeinfo = len;
    return 1;
}

int32_t luau_persist_set_proto_typeinfo_from_pointer(lua_State* L, uintptr_t pointer, const uint8_t* data, int32_t len)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto || len < 0)
        return 0;

    if (len == 0)
    {
        proto->typeinfo = nullptr;
        proto->sizetypeinfo = 0;
        return 1;
    }

    if (!data)
        return 0;

    proto->typeinfo = luaM_newarray(L, len, uint8_t, proto->memcat);
    memcpy(proto->typeinfo, data, size_t(len));
    proto->sizetypeinfo = len;
    return 1;
}

int32_t luau_persist_set_proto_source(lua_State* L, int32_t index, const uint8_t* data, int32_t len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto)
        return 0;

    if (len < 0)
        proto->source = nullptr;
    else
        proto->source = luaS_newlstr(L, reinterpret_cast<const char*>(data), size_t(len));
    if (proto->source)
        luaC_objbarrier(L, proto, proto->source);
    return 1;
}

int32_t luau_persist_set_proto_source_from_pointer(lua_State* L, uintptr_t pointer, const uint8_t* data, int32_t len)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto)
        return 0;

    if (len < 0)
        proto->source = nullptr;
    else
        proto->source = luaS_newlstr(L, reinterpret_cast<const char*>(data), size_t(len));
    if (proto->source)
        luaC_objbarrier(L, proto, proto->source);
    return 1;
}

int32_t luau_persist_set_proto_debugname(lua_State* L, int32_t index, const uint8_t* data, int32_t len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto)
        return 0;

    if (len < 0)
        proto->debugname = nullptr;
    else
        proto->debugname = luaS_newlstr(L, reinterpret_cast<const char*>(data), size_t(len));
    if (proto->debugname)
        luaC_objbarrier(L, proto, proto->debugname);
    return 1;
}

int32_t luau_persist_set_proto_debugname_from_pointer(lua_State* L, uintptr_t pointer, const uint8_t* data, int32_t len)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto)
        return 0;

    if (len < 0)
        proto->debugname = nullptr;
    else
        proto->debugname = luaS_newlstr(L, reinterpret_cast<const char*>(data), size_t(len));
    if (proto->debugname)
        luaC_objbarrier(L, proto, proto->debugname);
    return 1;
}

int32_t luau_persist_init_proto_locvars(lua_State* L, int32_t index, int32_t count)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || count < 0)
        return 0;

    if (count == 0)
    {
        proto->locvars = nullptr;
        proto->sizelocvars = 0;
        return 1;
    }

    proto->locvars = luaM_newarray(L, count, LocVar, proto->memcat);
    proto->sizelocvars = count;
    for (int32_t i = 0; i < count; ++i)
    {
        proto->locvars[i].varname = nullptr;
        proto->locvars[i].startpc = 0;
        proto->locvars[i].endpc = 0;
        proto->locvars[i].reg = 0;
    }
    return 1;
}

int32_t luau_persist_init_proto_locvars_from_pointer(lua_State* L, uintptr_t pointer, int32_t count)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto || count < 0)
        return 0;

    if (count == 0)
    {
        proto->locvars = nullptr;
        proto->sizelocvars = 0;
        return 1;
    }

    proto->locvars = luaM_newarray(L, count, LocVar, proto->memcat);
    proto->sizelocvars = count;
    for (int32_t i = 0; i < count; ++i)
    {
        proto->locvars[i].varname = nullptr;
        proto->locvars[i].startpc = 0;
        proto->locvars[i].endpc = 0;
        proto->locvars[i].reg = 0;
    }
    return 1;
}

int32_t luau_persist_set_proto_locvar(
    lua_State* L,
    int32_t index,
    int32_t locvar_index,
    const uint8_t* name,
    int32_t name_len,
    int32_t startpc,
    int32_t endpc,
    int32_t reg
)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || locvar_index < 0 || locvar_index >= proto->sizelocvars || reg < 0 || reg > UINT8_MAX)
        return 0;

    LocVar* loc = &proto->locvars[locvar_index];
    if (name_len < 0)
        loc->varname = nullptr;
    else
        loc->varname = luaS_newlstr(L, reinterpret_cast<const char*>(name), size_t(name_len));
    loc->startpc = startpc;
    loc->endpc = endpc;
    loc->reg = uint8_t(reg);
    if (loc->varname)
        luaC_objbarrier(L, proto, loc->varname);
    return 1;
}

int32_t luau_persist_set_proto_locvar_from_pointer(
    lua_State* L,
    uintptr_t pointer,
    int32_t locvar_index,
    const uint8_t* name,
    int32_t name_len,
    int32_t startpc,
    int32_t endpc,
    int32_t reg
)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto || locvar_index < 0 || locvar_index >= proto->sizelocvars || reg < 0 || reg > UINT8_MAX)
        return 0;

    LocVar* loc = &proto->locvars[locvar_index];
    if (name_len < 0)
        loc->varname = nullptr;
    else
        loc->varname = luaS_newlstr(L, reinterpret_cast<const char*>(name), size_t(name_len));
    loc->startpc = startpc;
    loc->endpc = endpc;
    loc->reg = uint8_t(reg);
    if (loc->varname)
        luaC_objbarrier(L, proto, loc->varname);
    return 1;
}

int32_t luau_persist_init_proto_upvalue_names(lua_State* L, int32_t index, int32_t count)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || count < 0)
        return 0;

    if (count == 0)
    {
        proto->upvalues = nullptr;
        proto->sizeupvalues = 0;
        return 1;
    }

    proto->upvalues = luaM_newarray(L, count, TString*, proto->memcat);
    proto->sizeupvalues = count;
    for (int32_t i = 0; i < count; ++i)
        proto->upvalues[i] = nullptr;
    return 1;
}

int32_t luau_persist_init_proto_upvalue_names_from_pointer(lua_State* L, uintptr_t pointer, int32_t count)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto || count < 0)
        return 0;

    if (count == 0)
    {
        proto->upvalues = nullptr;
        proto->sizeupvalues = 0;
        return 1;
    }

    proto->upvalues = luaM_newarray(L, count, TString*, proto->memcat);
    proto->sizeupvalues = count;
    for (int32_t i = 0; i < count; ++i)
        proto->upvalues[i] = nullptr;
    return 1;
}

int32_t luau_persist_set_proto_upvalue_name(lua_State* L, int32_t index, int32_t upvalue_index, const uint8_t* name, int32_t name_len)
{
    Proto* proto = get_luau_proto(L, index);
    if (!proto || upvalue_index < 0 || upvalue_index >= proto->sizeupvalues)
        return 0;

    if (name_len < 0)
        proto->upvalues[upvalue_index] = nullptr;
    else
        proto->upvalues[upvalue_index] = luaS_newlstr(L, reinterpret_cast<const char*>(name), size_t(name_len));
    if (proto->upvalues[upvalue_index])
        luaC_objbarrier(L, proto, proto->upvalues[upvalue_index]);
    return 1;
}

int32_t luau_persist_set_proto_upvalue_name_from_pointer(lua_State* L, uintptr_t pointer, int32_t upvalue_index, const uint8_t* name, int32_t name_len)
{
    Proto* proto = get_luau_proto_from_pointer(pointer);
    if (!proto || upvalue_index < 0 || upvalue_index >= proto->sizeupvalues)
        return 0;

    if (name_len < 0)
        proto->upvalues[upvalue_index] = nullptr;
    else
        proto->upvalues[upvalue_index] = luaS_newlstr(L, reinterpret_cast<const char*>(name), size_t(name_len));
    if (proto->upvalues[upvalue_index])
        luaC_objbarrier(L, proto, proto->upvalues[upvalue_index]);
    return 1;
}

int32_t luau_persist_copy_closure_upref_info(lua_State* L, int32_t index, luau_persist_upref_info* out, int32_t out_len)
{
    Closure* closure = get_luau_closure(L, index);
    if (!closure || out_len < 0)
        return -1;

    if (!out)
        return closure->nupvalues;

    if (out_len < closure->nupvalues)
        return -1;

    for (int32_t i = 0; i < closure->nupvalues; ++i)
    {
        TValue* raw = &closure->l.uprefs[i];
        luau_persist_upref_info item = {};
        item.raw_tag = ttype(raw);
        item.value_tag = ttype(raw);
        item.stack_offset = -1;

        if (ttisupval(raw))
        {
            UpVal* up = upvalue(raw);
            item.is_upvalue = 1;
            item.is_open = upisopen(up) ? 1 : 0;
            item.value_tag = ttype(up->v);
            item.identity = uintptr_t(up);
            if (item.is_open && up->v >= L->stack && up->v < L->top)
                item.stack_offset = int64_t(up->v - L->stack);
        }
        else if (iscollectable(raw))
        {
            item.identity = uintptr_t(gcvalue(raw));
        }

        out[i] = item;
    }

    return closure->nupvalues;
}

int32_t luau_persist_push_lua_closure_env(lua_State* L, int32_t index)
{
    Closure* closure = get_luau_closure(L, index);
    if (!closure || !closure->env)
        return 0;

    luaC_threadbarrier(L);
    sethvalue(L, L->top, closure->env);
    incr_top(L);

    return 1;
}

int32_t luau_persist_get_thread_info(lua_State* L, int32_t index, luau_persist_thread_info* out)
{
    if (!out)
        return 0;

    *out = {};

    lua_State* thread = get_thread(L, index);
    if (!thread)
        return 0;

    out->status = thread->status;
    out->is_active = thread->isactive ? 1 : 0;
    out->stack_size = thread->stacksize;
    out->stack_in_use = int32_t(thread->top - thread->stack);
    out->base_offset = int32_t(thread->base - thread->stack);
    out->top_offset = int32_t(thread->top - thread->stack);
    out->ci_count = int32_t(thread->ci - thread->base_ci + 1);
    out->current_ci = int32_t(thread->ci - thread->base_ci);

    int32_t open_count = 0;
    for (UpVal* up = thread->openupval; up; up = up->u.open.threadnext)
        open_count++;
    out->open_upvalue_count = open_count;

    return 1;
}

int32_t luau_persist_copy_call_frame_info(lua_State* L, int32_t index, luau_persist_call_frame_info* out, int32_t out_len)
{
    lua_State* thread = get_thread(L, index);
    if (!thread || out_len < 0)
        return -1;

    int32_t count = int32_t(thread->ci - thread->base_ci + 1);
    if (!out)
        return count;

    if (out_len < count)
        return -1;

    for (int32_t i = 0; i < count; ++i)
    {
        CallInfo* ci = &thread->base_ci[i];
        luau_persist_call_frame_info item = {};
        item.base_offset = int32_t(ci->base - thread->stack);
        item.func_offset = int32_t(ci->func - thread->stack);
        item.top_offset = int32_t(ci->top - thread->stack);
        item.savedpc_offset = -1;
        item.nresults = ci->nresults;
        item.flags = ci->flags;
        item.is_lua = isLua(ci) ? 1 : 0;
        item.is_native = (ci->flags & LUA_CALLINFO_NATIVE) ? 1 : 0;
        item.savedpc_present = (ci->savedpc && item.is_lua) ? 1 : 0;

        if (item.savedpc_present)
        {
            Proto* proto = ci_func(ci)->l.p;
            if (ci->savedpc >= proto->code && ci->savedpc <= proto->code + proto->sizecode)
                item.savedpc_offset = int32_t(ci->savedpc - proto->code);
        }

        out[i] = item;
    }

    return count;
}

int32_t luau_persist_push_thread_stack_value(lua_State* L, int32_t index, int32_t stack_offset)
{
    lua_State* thread = get_thread(L, index);
    if (!thread || stack_offset < 0 || stack_offset >= thread->stacksize)
        return 0;

    TValue* value = thread->stack + stack_offset;
    if (value >= thread->top)
        return 0;

    if (iscollectable(value))
        luaC_threadbarrier(L);
    luaA_pushobject(L, value);

    return 1;
}

int32_t luau_persist_copy_open_upvalue_info(lua_State* L, int32_t index, luau_persist_open_upvalue_info* out, int32_t out_len)
{
    lua_State* thread = get_thread(L, index);
    if (!thread || out_len < 0)
        return -1;

    int32_t count = 0;
    for (UpVal* up = thread->openupval; up; up = up->u.open.threadnext)
        count++;

    if (!out)
        return count;

    if (out_len < count)
        return -1;

    int32_t i = 0;
    for (UpVal* up = thread->openupval; up; up = up->u.open.threadnext)
    {
        luau_persist_open_upvalue_info item = {};
        item.value_tag = ttype(up->v);
        item.stack_offset = (up->v >= thread->stack && up->v < thread->top) ? int32_t(up->v - thread->stack) : -1;
        item.identity = uintptr_t(up);
        out[i++] = item;
    }

    return count;
}

int32_t luau_persist_set_thread_stack_value(lua_State* L, int32_t thread_index, int32_t stack_offset, int32_t value_index)
{
    lua_State* thread = get_thread(L, thread_index);
    if (!thread || thread->isactive || stack_offset < 0)
        return 0;

    const TValue* value = stack_value(L, value_index);
    if (!value)
        return 0;

    if (!ensure_thread_stack(thread, stack_offset + 1, stack_offset + 1))
        return 0;

    setobj(thread, thread->stack + stack_offset, value);
    if (iscollectable(value))
        luaC_threadbarrier(thread);

    if (thread->top < thread->stack + stack_offset + 1)
        thread->top = thread->stack + stack_offset + 1;

    return 1;
}

int32_t luau_persist_configure_thread(
    lua_State* L,
    int32_t thread_index,
    int32_t status,
    int32_t stack_in_use,
    int32_t base_offset,
    int32_t top_offset,
    int32_t ci_count,
    int32_t current_ci
)
{
    lua_State* thread = get_thread(L, thread_index);
    if (!thread || thread->isactive)
        return 0;

    if (status != LUA_OK && status != LUA_YIELD && status != LUA_BREAK)
        return 0;

    if (stack_in_use <= 0 || base_offset < 0 || top_offset < stack_in_use || ci_count <= 0 || current_ci < 0 || current_ci >= ci_count)
        return 0;

    if (!ensure_thread_stack(thread, stack_in_use, top_offset) || !ensure_thread_ci(thread, ci_count))
        return 0;

    for (int32_t i = stack_in_use; i < thread->stacksize; ++i)
        setnilvalue(thread->stack + i);

    thread->status = uint8_t(status);
    thread->top = thread->stack + stack_in_use;
    thread->base = thread->stack + base_offset;
    thread->ci = thread->base_ci + current_ci;

    for (int32_t i = ci_count; i < thread->size_ci; ++i)
    {
        thread->base_ci[i].base = thread->stack;
        thread->base_ci[i].func = thread->stack;
        thread->base_ci[i].top = thread->stack;
        thread->base_ci[i].savedpc = nullptr;
        thread->base_ci[i].nresults = 0;
        thread->base_ci[i].flags = 0;
    }

    return 1;
}

int32_t luau_persist_set_call_frame(
    lua_State* L,
    int32_t thread_index,
    int32_t frame_index,
    const luau_persist_call_frame_build_info* info
)
{
    lua_State* thread = get_thread(L, thread_index);
    if (!thread || !info || thread->isactive || frame_index < 0 || frame_index >= thread->size_ci)
        return 0;

    if (info->base_offset < 0 || info->func_offset < 0 || info->top_offset < 0)
        return 0;

    int32_t max_offset = info->base_offset;
    if (info->func_offset > max_offset)
        max_offset = info->func_offset;
    if (info->top_offset > max_offset)
        max_offset = info->top_offset;

    if (!ensure_thread_stack(thread, max_offset, max_offset))
        return 0;

    TValue* func = thread->stack + info->func_offset;
    if (!ttisfunction(func) && !(frame_index == 0 && !info->savedpc_present))
        return 0;

    CallInfo* ci = thread->base_ci + frame_index;
    ci->base = thread->stack + info->base_offset;
    ci->func = func;
    ci->top = thread->stack + info->top_offset;
    ci->nresults = info->nresults;
    ci->flags = info->flags;
    ci->savedpc = nullptr;

    if (info->savedpc_present)
    {
        Closure* closure = clvalue(func);
        if (closure->isC || info->savedpc_offset < 0)
            return 0;

        Proto* proto = closure->l.p;
        if (info->savedpc_offset > proto->sizecode)
            return 0;

        ci->savedpc = proto->code + info->savedpc_offset;
    }

    return 1;
}

int32_t luau_persist_push_lua_closure_for_proto(lua_State* L, int32_t proto_source_index, int32_t env_index)
{
    Proto* proto = get_luau_proto(L, proto_source_index);
    if (!proto)
        return 0;

    const TValue* env_value = stack_value(L, env_index);
    if (!env_value || !ttistable(env_value))
        return 0;

    LuaTable* env = hvalue(env_value);
    luaC_checkGC(L);
    luaC_threadbarrier(L);

    Closure* closure = luaF_newLclosure(L, proto->nups, env, proto);
    setclvalue(L, L->top, closure);
    incr_top(L);

    return 1;
}

int32_t luau_persist_set_lua_closure_upref_value(lua_State* L, int32_t closure_index, int32_t upref_index, int32_t value_index)
{
    Closure* closure = get_luau_closure(L, closure_index);
    if (!closure || upref_index < 0 || upref_index >= closure->nupvalues)
        return 0;

    const TValue* value = stack_value(L, value_index);
    if (!value)
        return 0;

    TValue* target = &closure->l.uprefs[upref_index];
    setobj(L, target, value);
    luaC_barrier(L, closure, value);

    return 1;
}

int32_t luau_persist_set_lua_closure_upref_closed(lua_State* L, int32_t closure_index, int32_t upref_index, int32_t value_index)
{
    Closure* closure = get_luau_closure(L, closure_index);
    if (!closure || upref_index < 0 || upref_index >= closure->nupvalues)
        return 0;

    const TValue* value = stack_value(L, value_index);
    if (!value)
        return 0;

    luaC_checkGC(L);

    UpVal* uv = luaM_newgco(L, UpVal, sizeof(UpVal), L->activememcat);
    luaC_init(L, uv, LUA_TUPVAL);
    uv->markedopen = 0;
    setobj(L, &uv->u.value, value);
    uv->v = &uv->u.value;

    setupvalue(L, &closure->l.uprefs[upref_index], uv);
    luaC_objbarrier(L, closure, uv);
    luaC_barrier(L, uv, value);

    return 1;
}

int32_t luau_persist_set_lua_closure_upref_from_upref(
    lua_State* L,
    int32_t closure_index,
    int32_t upref_index,
    int32_t source_closure_index,
    int32_t source_upref_index
)
{
    Closure* closure = get_luau_closure(L, closure_index);
    Closure* source = get_luau_closure(L, source_closure_index);
    if (!closure || !source || upref_index < 0 || upref_index >= closure->nupvalues || source_upref_index < 0 ||
        source_upref_index >= source->nupvalues)
        return 0;

    TValue* source_upref = &source->l.uprefs[source_upref_index];
    if (!ttisupval(source_upref))
        return 0;

    UpVal* uv = upvalue(source_upref);
    setupvalue(L, &closure->l.uprefs[upref_index], uv);
    luaC_objbarrier(L, closure, uv);

    return 1;
}

int32_t luau_persist_set_lua_closure_upref_open(
    lua_State* L,
    int32_t closure_index,
    int32_t upref_index,
    int32_t thread_index,
    int32_t stack_offset
)
{
    Closure* closure = get_luau_closure(L, closure_index);
    lua_State* thread = get_thread(L, thread_index);
    if (!closure || !thread || thread->isactive || upref_index < 0 || upref_index >= closure->nupvalues || stack_offset < 0)
        return 0;

    if (stack_offset >= thread->stacksize || thread->stack + stack_offset >= thread->top)
        return 0;

    StkId level = thread->stack + stack_offset;
    global_State* g = thread->global;
    UpVal** pp = &thread->openupval;
    UpVal* uv = nullptr;
    while (*pp != nullptr && (*pp)->v >= level)
    {
        UpVal* candidate = *pp;
        if (!upisopen(candidate))
            return 0;
        if (candidate->v == level)
        {
            uv = candidate;
            break;
        }
        pp = &candidate->u.open.threadnext;
    }

    if (!uv)
    {
        luaC_checkGC(L);

        uv = luaM_newgco(L, UpVal, sizeof(UpVal), L->activememcat);
        luaC_init(L, uv, LUA_TUPVAL);
        uv->markedopen = 0;
        uv->v = level;

        uv->u.open.threadnext = *pp;
        *pp = uv;

        uv->u.open.prev = &g->uvhead;
        uv->u.open.next = g->uvhead.u.open.next;
        uv->u.open.next->u.open.prev = uv;
        g->uvhead.u.open.next = uv;
    }

    if (isblack(obj2gco(thread)))
        luaC_barrierback(L, obj2gco(thread), &thread->gclist);

    setupvalue(L, &closure->l.uprefs[upref_index], uv);
    luaC_objbarrier(L, closure, uv);

    return 1;
}

}
