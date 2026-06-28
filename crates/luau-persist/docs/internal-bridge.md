# Internal Luau Bridge

The public Luau C API is sufficient for primitive values, tables, buffers,
vectors, and permanent native values. It is not sufficient for exact persistence
of closures, protos, open upvalues, suspended threads, call frames, or saved
program counters.

`native/bridge.cpp` is the boundary where `luau-persist` includes Luau VM
internal headers such as `lobject.h`, `lstate.h`, and later `lfunc.h`/`ldo.h`.

Rules for this bridge:

- Keep it generic. Do not include Clawblox concepts.
- Expose narrow operations with clear invariants.
- Add tests for every new field read or structure constructed.
- Reject unsupported states instead of synthesizing placeholders.
- Keep C++ functions deterministic and side-effect free unless a construction
  operation explicitly requires mutation.

The bridge currently exposes read-only function metadata, Proto metadata,
bounded copies of Proto bytecode words and constant type tags, raw closure
upref shape, closure environments, thread metadata, call-frame metadata,
thread stack slot values, and open-upvalue metadata. It can also push Proto
constants as ordinary stack values, materialize child Protos as closures with
caller-provided environments, allocate Proto skeletons from explicit
header/code data, fill Proto constants and child Proto slots by raw Proto
pointer, wrap completed Protos in Lua closures, allocate closed `LUA_TUPVAL`
objects, bind multiple closure slots to the same upvalue, and bind closure
slots to open upvalues rooted in inactive thread stack slots.

Loader-facing bridge functions resolve ordinary stack indices directly from
Luau's real stack extent instead of relying on `luaA_toobject`. The raw loader
can temporarily push more values than the active API call frame's `ci->top`
advertises, so the public API assertion is too strict for this internal
construction path. Pseudo-indices still use Luau's API conversion path.

Registry references created by the loader go through a persistence-specific
helper rather than public `lua_ref`. The helper keeps large object graphs out
of the registry table rehash path by growing the array part in chunks, and it
handles paused-GC table barrier state when a fresh loader object is rooted.
These operations prove that the crate is compiled against the same Luau
internals as `mlua-sys` and give the serializer/construction work a tested
foundation.

The raw serializer currently uses those operations to round-trip Luau closures,
their environments, Proto bytecode/constants/child Protos/debug/type metadata,
direct uprefs, shared closed upvalues, yielded threads, and thread-owned open
upvalues. It still rejects native execution data, native Luau frames, and
host-owned userdata unless the userdata is handled by a permanent table or host
adapter.

The thread bridge can inspect yielded thread stack slots, call frames, saved
program counters, and open upvalue stack offsets. It can also copy stack values
into an inactive thread, configure suspended thread status/base/top/current
frame fields, restore active `CallInfo` entries including saved program
counters, and create/reuse open upvalues for restored thread stack slots. Tests
currently prove that yielded threads with and without open upvalues can
round-trip across states and resume when host C functions on the stack are
supplied through permanents.

Luau's `Proto::upvalues` array is debug-name metadata in the currently targeted
VM source; executable capture behavior is carried by bytecode capture
instructions. This differs from the Eris Lua implementation, which persists
Proto upvalue descriptors as execution data.

This bridge intentionally does not expose Clawblox scheduler concepts. A host
can use these facts to decide whether a checkpoint is possible and which VM
roots to serialize, but the generic crate remains responsible only for Luau
object graph mechanics.

## Source Discovery

The build script needs the `luau0-src` source tree. It uses:

1. `LUAU_SRC_DIR`, when set.
2. The newest `luau0-src-*` directory found under Cargo's registry source
   cache.

`LUAU_SRC_DIR` must point at the root of the `luau0-src` crate directory, not
the nested `luau` directory.
