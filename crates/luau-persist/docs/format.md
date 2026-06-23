# Persistence Format

The format is binary, versioned, little-endian, and intentionally strict. A
reader must reject malformed data rather than repair or reinterpret it.

## Header

Every stream starts with:

| Field | Size | Value |
| --- | ---: | --- |
| magic | 8 bytes | `LUPST\0\0\x01` |
| version | 2 bytes | `1`, little-endian `u16` |

The header is followed by exactly one value. Any trailing bytes after that value
are invalid.

## Values

Each value starts with a one-byte tag.

| Tag | Meaning | Payload |
| ---: | --- | --- |
| `0x00` | nil | none |
| `0x01` | false | none |
| `0x02` | true | none |
| `0x03` | number | little-endian `f64` |
| `0x04` | string | byte length as little-endian `u64`, then raw bytes |
| `0x05` | buffer | object id as little-endian `u32`, byte length as `u64`, then raw bytes |
| `0x06` | vector | component count as `u8`, then that many little-endian `f32` values |
| `0x10` | table | object id as little-endian `u32`, flags as `u8`, entry count as `u64`, key/value pairs, then metatable value |
| `0x11` | reference | object id as little-endian `u32` |
| `0x12` | permanent | object id as little-endian `u32`, expected Luau type as little-endian `i32`, then permanent key value |
| `0x20` | function | object id as little-endian `u32`, function kind as `u8`, then kind-specific payload |
| `0x21` | proto | object id as little-endian `u32`, Proto header, code, constants, child Proto references, and metadata |
| `0x22` | upvalue | object id as little-endian `u32`, upvalue kind as `u8`, then upvalue value |
| `0x23` | thread | object id as little-endian `u32`, thread header, stack values, and call frames |
| `0x30` | userdata | object id as little-endian `u32`, adapter type key, and adapter payload bytes |

## Object IDs

Object IDs preserve identity for reference-like VM objects. The first object in
a stream has id `0`, the next has id `1`, and so on. IDs must appear in exact
order. A reader must reject an object whose id is not the next expected id.

Tables, buffers, functions, Protos, upvalues, threads, userdata adapter values,
and permanent objects use object IDs.

Vectors do not use object IDs because they are inline value types in Luau.
Streams include the component count so a reader can reject data written by a
Luau build with a different vector width.

## Cycles

A reader registers a table, buffer, function, Proto representative, or upvalue
representative immediately after allocating or reserving it and before reading
nested contents. This allows later values in the same object payload to refer
back to the object being constructed.

For example, a table containing `self = table` is encoded as a table object with
one entry whose value is a reference to that table's id.

Table metatables are encoded after table entries as an ordinary value, so a
table can use itself as its own metatable. The metatable is encoded as nil when
absent.

## Table Flags

Table flags are a one-byte bitset. Readers must reject unknown bits.

| Bit | Meaning |
| ---: | --- |
| `0x01` | table is readonly |

## Unsupported State

Unsupported Luau value kinds are checkpoint failures. The writer must return an
error for unsupported values and must not encode placeholders, inferred
reconstruction instructions, or fallbacks.

Currently unsupported:

- lightuserdata
- C functions not supplied through permanents
- function states with native execution data
- userdata not handled by a host adapter or permanent table
- running threads
- native Luau call frames

## Functions, Protos, And Upvalues

Function payloads start with a kind byte:

| Kind | Meaning |
| ---: | --- |
| `0x00` | Lua closure |
| `0x01` | C closure/native function through permanents |

Lua closure payload:

1. environment table as an ordinary value;
2. Proto as an ordinary value using the `proto` tag or a reference;
3. upvalue count as `u32`;
4. one upvalue record per closure slot.

Each upvalue record starts with a kind byte:

| Kind | Meaning |
| ---: | --- |
| `0x00` | direct/by-value closure upref |
| `0x01` | shared closed upvalue object |
| `0x02` | open upvalue object owned by a thread stack |

Direct uprefs encode their value inline. Shared and open upvalue records encode
an ordinary value using the `upvalue` tag or a reference. Open upvalues are
patched after the owning thread stack slots are restored.

Proto payload:

1. `nups`, `numparams`, `is_vararg`, `maxstacksize`, and `flags` as `u8`;
2. `linedefined` and `bytecodeid` as little-endian `i32`;
3. code word count as little-endian `u32`, then little-endian `u32` code words;
4. constant count as little-endian `u32`, then constants as ordinary values;
5. child Proto count as little-endian `u32`, then child Protos as ordinary
   values using the `proto` tag or references;
6. metadata block:
   `linegaplog2` as little-endian `i32`, lineinfo bytes, typeinfo bytes,
   optional source bytes, optional debug-name bytes, local-variable records,
   and upvalue-name bytes. Byte arrays are length-prefixed with little-endian
   `u64`; optional byte arrays start with presence byte `0` or `1`.

Luau closure capture behavior is encoded in bytecode capture instructions, not
in `Proto::upvalues`; in this Luau version `Proto::upvalues` stores debug
upvalue names. The writer rejects Protos with native execution data
(`execdata`, `exectarget`, or native call frames).

Upvalue payload:

1. kind byte;
2. kind-specific payload.

Closed shared upvalues use kind `0x01` followed by the closed value as an
ordinary value.

Open upvalues use kind `0x02` followed by:

1. owner thread as an ordinary reference value;
2. owner thread stack offset as little-endian `i32`.

Standalone open upvalues without a persisted owning thread are invalid.

Thread payload:

1. status as little-endian `i32`;
2. live stack slot count as little-endian `i32`;
3. base stack offset as little-endian `i32`;
4. current call-frame index as little-endian `i32`;
5. call-frame count as little-endian `u32`;
6. one ordinary value for each live stack slot from offset `0` up to, but not
   including, the live stack slot count;
7. one call-frame record per active `CallInfo`.

Each call-frame record contains:

1. base stack offset as little-endian `i32`;
2. function stack offset as little-endian `i32`;
3. top stack offset as little-endian `i32`;
4. saved program-counter offset as little-endian `i32`;
5. saved program-counter present flag as little-endian `i32`;
6. expected result count as little-endian `i32`;
7. frame flags as little-endian `u32`.

Thread loading currently supports inactive/yielded threads whose stack values,
open upvalues, and call frames can be reconstructed exactly, including host C
function frames when those C functions are supplied through permanents. Call
frame `base`, `func`, and `top` fields are stored as VM stack offsets; protected
C frames may have `top` equal to `base`, so readers must treat these as pointer
offsets rather than require every frame to reserve a Lua argument/result slot.
The loader creates thread objects before loading stack values, records pending
open upvalue patches while closures are loaded, then binds those upvalues to
restored thread stack slots before restoring call-frame metadata.

## Userdata Adapters

Userdata payload:

1. object id as little-endian `u32`;
2. adapter type-key byte length as little-endian `u64`;
3. adapter type-key UTF-8 bytes;
4. payload byte length as little-endian `u64`;
5. adapter-defined payload bytes.

The format does not interpret adapter payloads. The dumping host must provide an
adapter that recognizes the userdata and emits a stable type key plus exact
payload bytes. The loading host must provide an adapter that validates the type
key and payload and pushes exactly one userdata value. Missing adapters,
unrecognized userdata, empty or invalid type keys, and adapters that push
anything other than one userdata value are hard failures.

## Permanent Objects

Permanent objects are host-provided values that cannot or should not be
serialized literally, such as native C functions. They are encoded through
explicit permanent tables.

On dump, the permanent table maps object to key:

```text
permanents[object] = key
```

On load, the permanent table maps key to object:

```text
permanents[key] = object
```

The stream stores the expected Luau type for the permanent object. A reader must
reject missing permanent tables, missing permanent values, or permanent values
whose Luau type differs from the stored expected type.

Permanent keys are currently restricted to scalar key values supported by the
format: nil, booleans, numbers, strings, and vectors. Host integrations should
prefer stable string keys.

## Host Integration Boundary

This format does not define Clawblox concepts. Host integration must map native
functions, host-owned userdata, external resources, and engine scheduler records
through explicit adapter layers built on top of this generic format.

Clawblox currently stores its scheduler roots as one Luau root bundle inside the
host snapshot so references are shared across pending coroutine lists, wait
schedule tables, resume-argument tables, cancellation tables, and module cache
entries. Engine-provided global functions/tables are supplied as permanents by
stable string keys. Clawblox userdata is still expected to go through a
host-side userdata adapter rather than this generic format knowing about
Instances, services, or value classes.
