# luau-persist

`luau-persist` is a generic Luau VM persistence layer. It must not depend on
host engine concepts such as worlds, instances, services, agents, snapshots, or
replication.

The design follows the Eris/Pluto approach at the architectural level:

- versioned binary format
- explicit object reference table for shared references and cycles
- hard errors for unsupported VM state
- permanent-object table for native functions and host-owned values
- userdata adapters supplied by the host integration layer

Current support:

- nil
- booleans
- numbers
- strings
- vectors
- tables with shared references, cycles, metatables, and readonly flags
- buffers with shared references
- permanent C/native functions via explicit permanent tables
- Lua closures and Protos, including debug/type metadata
- direct uprefs, shared closed upvalues, and thread-owned open upvalues
- inactive/yielded threads with restorable Lua call frames
- host-adapted userdata via explicit type keys and payload bytes

Unsupported values are checkpoint failures:

- lightuserdata
- userdata not handled by a host adapter or permanent table
- C functions not supplied through permanents
- Protos with native execution data
- running threads and native Luau call frames

The next milestones are, in order:

1. broader Clawblox VM-root coverage for remaining host queues and live roots
   not already represented in the host snapshot
2. more full-world snapshot/resume validation without re-running scripts
3. adapter coverage for any remaining host userdata that scripts can keep live
   across a checkpoint

Clawblox integration belongs outside this crate. It should provide permanent
objects and userdata adapters for Clawblox-specific values without leaking those
types into this crate.

See [docs/format.md](docs/format.md) for the current binary format contract.
See [docs/vm-state-design.md](docs/vm-state-design.md) for the long-term VM
state persistence design and host integration boundary.

## Luau Internals Bridge

Exact closure, upvalue, and thread persistence requires Luau internal headers.
The crate builds a small C++ bridge in `native/bridge.cpp`. The build script
uses `LUAU_SRC_DIR` when set; otherwise it locates `luau0-src` in Cargo's
registry. `LUAU_SRC_DIR` should point at the root of a `luau0-src` crate
checkout, containing `luau/VM/include/lua.h` and `luau/VM/src/lobject.h`.

The bridge exposes narrow metadata and construction operations for function,
Proto, closure upref, thread, call-frame, and upvalue state. Construction APIs
are added one invariant at a time, with tests, rather than exposing broad
unchecked access to Luau internals.
