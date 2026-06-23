# Luau VM State Persistence Design

`luau-persist` owns generic Luau VM state persistence only. It must not know
about worlds, instances, services, agents, snapshots, or Clawblox scheduling
policy. Host engines build resumability by supplying stable permanent objects
and by deciding when a VM is in a checkpointable state.

The long-term target is exact restoration of a suspended Luau runtime:

- object identity and cycles are preserved across all supported object kinds;
- Lua closures preserve their Proto, environment, and shared upvalues;
- suspended threads preserve stack slots, call frames, saved program counters,
  status, expected result counts, and open upvalues;
- native functions, host userdata, and external resources are restored only
  through explicit permanent/adaptor tables;
- unsupported state is a checkpoint error, never a fallback reconstruction.

## Engine Contract

The user-facing resume path should not require scripts to opt in or wrapper
scripts to remember extra flags. A host should expose a runtime checkpoint API
roughly shaped as:

```text
host checkpoint:
  pause engine work at a VM-safe boundary
  build dump permanent table from registered host adapters
  ask luau-persist to dump all VM roots selected by the host
  store the generic Luau payload inside the host snapshot

host resume:
  create a fresh Luau state with the same registered native surface
  build load permanent table from registered host adapters
  ask luau-persist to load the VM roots
  reconnect loaded roots to host scheduler/runtime fields
  continue ticks without re-running scripts
```

This keeps script authors out of the persistence protocol. It also keeps the
generic crate reusable: Clawblox decides which runtime roots matter, but
`luau-persist` decides how Luau values, Protos, closures, upvalues, and threads
are encoded.

## Permanent Objects

Permanent objects are the boundary between generic Luau state and host-owned
state. A permanent table maps runtime objects to stable keys during dump, and
stable keys back to runtime objects during load.

Native functions should be permanent unless their implementation is known and
reconstructed by a dedicated bridge operation. Host userdata should be permanent
or serialized through a host adapter that produces generic Luau values plus a
stable type key. A missing permanent value is fatal because closures and threads
can depend on it while being rebuilt.

Clawblox keeps this boundary in its runtime snapshot layer. The generic crate
sees only adapter type keys and payload bytes. Clawblox's adapter maps
`Instance` userdata to stable instance ids, service userdata to stable service
handles on the restored `Game`, datastore handles to their game id and store
name, workspace query userdata such as `RaycastParams`, `OverlapParams`, and
`RaycastResult` to primitive fields plus stable instance ids, and copyable value
userdata such as `Vector3`, `CFrame`, `Color3`, `UDim2`, `Ray`, `Region3`,
`PhysicalProperties`, `DateTime`, and typed enum userdata, including GUI
text-alignment and font enums, to explicit little-endian payloads. The host object graph must be
restored, including original instance ids, before the Luau VM payload is
loaded. Instances reachable only from Lua userdata, including unparented or
destroyed instances, are captured as orphan host graph roots and restored before
VM load so the generic userdata payload can remain a stable id.
`RBXScriptConnection` userdata is serialized as its connection id,
connected state, and the stable identity of the service or instance signal it
belongs to; restoring it requires that signal to exist in the restored host
graph. `RBXScriptSignal` userdata is serialized by the same stable signal
identity so suspended `Signal:Wait()` frames can preserve their `self` signal
argument. An instance id or signal identity that cannot be resolved is a
restore error.

## Protos

Proto persistence must follow Luau's loader/allocation invariants:

- allocate with `luaF_newproto`;
- allocate arrays with Luau's allocator using the Proto's memory category;
- initialize constants to nil before filling them;
- set `codeentry` consistently with bytecode execution mode;
- preserve child Proto references through the object-reference table;
- preserve debug/type metadata when present;
- restore GC edges with the same barrier discipline used by Luau internals.

The loader reconstructs Protos as raw VM objects first, then wraps the completed
Proto in a Lua closure only after constants, child Proto slots, and metadata are
filled. This avoids exposing partially initialized Protos through executable
closures. Primitive constants are written directly into `Proto::k`; object
constants use normal stack values plus Luau barriers.

Serialized Proto data must include execution-relevant fields and metadata, not
just bytecode: parameter counts, vararg flag, stack size, flags, constants,
child Protos, bytecode id, line tables, type metadata, source/debug names,
local-variable debug records, and upvalue debug names. In the current Luau VM,
closure capture behavior is encoded by bytecode capture instructions;
`Proto::upvalues` stores debug upvalue names, not Eris-style executable capture
descriptors. Native execution state (`execdata`, `exectarget`,
`LUA_CALLINFO_NATIVE`) is not portable across a checkpoint and should be
rejected until a specific host/VM contract exists.

## Closures And Upvalues

Lua closures are persisted as a closure object plus references to:

- their Proto;
- their environment table;
- each upvalue identity.

Upvalues are separate reference objects so two closures sharing one upvalue
continue sharing one upvalue after load. Closed upvalues are rebuilt as closed
values. Open upvalues encode the owning thread reference plus stack-slot offset;
load records pending closure-slot patches while stack values are reconstructed
and binds them to restored thread stack slots before call-frame metadata is
restored.

Load order follows the Eris placeholder pattern: reserve or register the object
id before reading nested state, then fill fields, then leave exactly the loaded
value on the caller's stack. Luau's `lua_ref` does not pop values, so any
temporary duplicate pushed only to create a registry reference must be popped
before an enclosing table or closure continues loading.

The raw loader can temporarily hold more stack values than the public API
frame's `ci->top` permits. Bridge functions that operate on loader stack slots
therefore resolve ordinary positive/negative stack indices against `L->base`,
`L->top`, and the real stack extent rather than routing through
`luaA_toobject`, whose API assertion is correct for public calls but too strict
for this internal loader phase.

The raw dumper and loader also reserve C API stack capacity before recursively
walking values. Large nested root bundles can otherwise exceed Luau's default
API stack allowance while holding temporary table keys, values, Proto
representatives, and registry-root candidates, causing debug builds to trap
inside ordinary push operations.

Persistence-owned registry roots use a bridge helper instead of public
`lua_ref`. The helper grows the registry array in chunks for large object graphs
and handles the paused-GC case where writing a new white object into a black
registry table would otherwise trip Luau's table-barrier assertion.

Closed shared upvalues also reserve a pending reference before their contained
value is loaded. This matters when the contained value is itself a reference
object, such as adapter-owned userdata; otherwise the nested object would see a
later id while the loader's reference table still expects the upvalue id.

C closures are host/native objects. The C function pointer must resolve through
permanents, and C closure upvalues are ordinary persisted values. If the C
function is not in permanents, dumping fails.

## Threads

Thread persistence is required for exact resume. A suspended thread must encode:

- thread status and whether it is dead/yielded/runnable;
- stack slots from stack base through top, preserving references;
- call frames from base frame through current frame;
- each frame's function slot, base, top, savedpc offset, flags, and nresults;
- open upvalues as stack-slot offsets plus upvalue references.

Running threads must not be snapshotted. The host must request checkpoints only
at safe boundaries where no Luau frame is actively executing and no hidden
non-serializable native continuation is live.

The bridge-level construction path restores inactive yielded thread stacks,
open upvalues, and `CallInfo` arrays well enough to resume yielded threads
across states, including host C function frames when those functions are
supplied through permanents. Protected-call C frames are valid call frames even
when their `top` pointer is equal to `base`; restore code must validate these as
stack pointers, not impose Lua-frame-only `top >= base + 1` shape assumptions.

## Host Scheduler Boundary

Schedulers should not be serialized by re-running scripts. The host should
store scheduler roots as ordinary Luau values where possible:

- pending coroutine/thread roots;
- wait schedule tables;
- pending resume arguments;
- module cache roots;
- signal callback registries.

Rust-side handles such as registry keys are host implementation details. On
resume, the host should recreate those handles from loaded Luau roots instead
of expecting scripts to rebuild them.

Clawblox's current integration stores scheduler roots as a single Luau root
bundle rather than independent payloads. That is required because schedule
tables use coroutine objects as keys; dumping roots independently would create
distinct restored thread objects and make scheduled waits resume early. The
Clawblox layer supplies engine-provided global functions and tables as
permanents with stable string keys, and supplies host userdata adapters for
objects that cannot be global permanents. The generic crate remains unaware of
those keys and of Clawblox's scheduler policy.

Signal callbacks are host-owned registries that contain Luau closures, so the
host must expose them as explicit roots in the same root bundle. Clawblox
captures service signals such as `RunService.Heartbeat` and `Players` events,
plus instance signals such as property/attribute changes, touch events,
humanoid/player events, value changes, sound events, remote events, and GUI
input events. On restore, the host clears the target signal registries and
rebuilds each connection from the loaded callback closure, original connection
id, and one-shot flag.

`Signal:Wait()` must not suspend inside a host async future. Clawblox exposes it
as a Lua-frame helper: the helper registers the current coroutine as a waiter on
the signal, marks the thread in an explicit scheduler table, then yields with
`coroutine.yield()`. The signal owns registry references to waiter threads.
Firing the signal clears the waiting marker and stores packed event arguments in
the scheduler args table; the normal pending-coroutine pass resumes the waiter
on the next frame. Signal waiter threads and the waiting-thread table are
serialized in the root bundle, while any signal userdata held by a waiting frame
is adapted through the same stable service/instance signal identity used by
`RBXScriptConnection`.

Mutable globals are host policy, not a generic VM feature. Clawblox records a
baseline of engine-provided string globals after initialization, then stores
user-created globals, top-level replacements of baseline globals, and baseline
names that were unset. Engine permanents are still supplied to `luau-persist`
as identity anchors for native API objects, using the initialization baseline on
dump and the fresh engine runtime on load; they are not excluded from top-level
global mutation tracking. Clawblox internal bridge globals are excluded from the
delta because they are transient implementation details. Internal globals that
hold semantic runtime state, such as the `require` ModuleScript cache, are stored
as explicit VM roots instead of relying on global delta tracking.

Hidden Rust futures or async continuations are not generic Luau state. A host
must either checkpoint only when none are live, or represent them as explicit
host wait records with permanent/adapted results.

Host-owned timers and queues that are not Luau object roots belong in the host
snapshot. Clawblox snapshots RemoteEvent queues, agent input queues, and Debris
timer queues there; the Debris queue stores stable instance ids plus absolute
runtime destroy times and is restored only after the host instance graph exists.
Clawblox also snapshots the already-drained recent replicated-event observation
ring, its next event id, and elapsed instance time so observers do not see
duplicate or missing transient events across resume.
Wall-clock host timers are serialized as elapsed durations, not raw instants:
AFK timeout configuration, per-player idle ages, and empty-instance age must
resume without granting extra grace time or prematurely collecting a server.
Host runtime configuration and lifecycle state also stay in the Clawblox host
snapshot: game status, observation radius, player/object physics coupling,
error mode, and any halted-error message are restored before the VM roots are
reconnected so scheduler and observation behavior match the checkpoint.
Player service configuration such as instance capacity and
`Players.CharacterAutoLoads` is host state too; it must be restored into both
the `GameInstance` and the Lua `Players` service before scripts resume.
Service-owned `Player` and `PlayerGui` instances are also host roots rather
than normal DataModel descendants; Clawblox snapshots their stable instance ids
and `PlayerGui` children so Lua-held userdata resumes to the same objects
returned by `Players:GetPlayers()` and `player.PlayerGui`.
Instance graph snapshots include physics constraint limits and reference links,
for example `AlignPosition.MaxVelocity` and `AlignOrientation.MaxTorque`,
because these fields affect the next physics step after resume.
They also include media payloads such as `Animation.AnimationId`, animator
loaded-track counters, and `Sound` playback fields so restored scripts and
signals see the same instance state.
The high-rate replication dirty table is also host state: Clawblox snapshots
the current replication revision plus dirty instance ids/revisions so clients
that already hold an older revision still receive changed and removed entity
deltas after resume.
Touch transition memory is host state as well: the previous-frame touching part
pairs are serialized by stable instance id so `TouchEnded` fires correctly when
an overlap that existed at checkpoint time separates after resume.
Short-lived host queues are also snapshotted rather than assumed empty: pending
`Players:AddBot()` names, pending `Player:Kick()` requests, and one-frame
BasePart physics commands queued by Lua methods are restored and drained by the
normal tick phases after resume.
Lua functions held in host app data are VM roots too. Clawblox stores
`game:BindToClose()` callbacks in registry keys, so the VM root bundle includes
those callbacks and restores fresh registry keys before normal runtime teardown.
Animation tracks are another host app-data boundary: the `AnimationScheduler`
owns active `AnimationTrack` userdata and each track owns `Stopped`/`Ended`
signal callback registries. Clawblox snapshots the scheduler's track userdata
as VM roots, adapts each track's primitive playback state through a stable host
userdata payload, and restores the scheduler before reconnecting track signal
callbacks.

## Current Status

The generic crate has cross-state tests for primitive values, tables/cycles,
Lua closures, shared closed upvalues, yielded threads, open upvalues, protected
calls, permanent native functions, adapted userdata, debug metadata, and large
closure/proto graphs. Clawblox integrates the generic payload with host
snapshots for runtime roots, scheduler state, module cache, signal waiters,
adapted userdata, player/service userdata, and representative host queues.

Real-world exact resume is currently covered by `pyramid-world-3`: the test
captures after a scripted pickup, restores into a fresh `GameInstance` without
rerunning the world script, verifies the Lua-held block state still follows the
player, and then places the block after restore.

Remaining work should continue broadening full-world coverage across more
mechanics and worlds, and any unsupported VM state should stay strict: fail as
an explicit error with a useful diagnostic instead of silently falling back.

## Eris Comparison

Eris is the closest practical blueprint, but it targets Lua versions with
different internal layouts. The concepts to carry forward are:

- a single object-reference table for ordinary objects plus keyed entries for
  VM-internal identities such as Protos and upvalues;
- permanent-object lookup before literal persistence of native/C state;
- pre-registering objects while loading so cycles can point at placeholders;
- initializing Proto constants and child references to safe values before
  filling them;
- rebuilding closure/upvalue graphs with shared upvalue identity preserved;
- preserving stack discipline while preregistering representatives, because
  leaked temporary closures can corrupt enclosing table key/value loads;
- restoring open upvalues after thread stack slots exist by applying deferred
  closure-slot patches to stack-backed upvalues;
- rejecting running threads, hook-yield state, unsupported native continuations,
  and missing permanents.

Luau-specific differences matter:

- Lua closure uprefs are `TValue` slots that can contain either direct captured
  values or `LUA_TUPVAL` objects, depending on capture mode;
- Luau Proto metadata includes `flags`, `codeentry`, type info, debug
  instruction data, native execution fields, source/debug names, and bytecode
  ids that do not map one-to-one to Eris' Lua Proto format;
- Luau's executable closure capture records live in bytecode instructions; the
  `Proto::upvalues` array contains debug names in this VM version;
- Luau call frames are an array of `CallInfo` records with `base`, `func`,
  `top`, `savedpc`, `nresults`, and `flags`, not Eris' older linked CallInfo
  shape;
- native execution (`execdata`, `exectarget`, `LUA_CALLINFO_NATIVE`) needs an
  explicit policy and should be rejected until that policy exists.

The implementation should therefore borrow Eris' graph algorithm and safety
ordering, while using Luau's current allocation APIs and field invariants.
