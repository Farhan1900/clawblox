# Luau browser-WASM spike

This deliberately isolated proof-of-concept checks the highest-risk part of
the browser runtime: Rust calling the native C++ Luau VM in WebAssembly.

It exercises Luau syntax, a Rust callback, table traversal, closures, and a
yielding coroutine. It does not include `luau-persist`, snapshots, the native
server, or any production Clawblox code.

## Build

Install the Rust target and activate an Emscripten SDK:

```bash
rustup target add wasm32-unknown-emscripten
git clone https://github.com/emscripten-core/emsdk.git ~/.cache/emsdk
~/.cache/emsdk/emsdk install latest
~/.cache/emsdk/emsdk activate latest
```

Then run:

```bash
./wasm-spike/build.sh
```

Serve `wasm-spike/dist` over HTTP and open it in a browser. A successful run
prints `CLAWBLOX_LUAU_WASM_SPIKE_OK` on the page and in the console.

```bash
python3 -m http.server 8765 --directory wasm-spike/dist
```

The generated JavaScript can also run under the Node executable bundled with
Emscripten:

```bash
source ~/.cache/emsdk/emsdk_env.sh
cd wasm-spike/dist
"$EMSDK_NODE" clawblox_luau_wasm_spike.js
```

## Findings

- `mlua 0.11.6` and its vendored C++ Luau VM compile for
  `wasm32-unknown-emscripten` without patching either dependency.
- The Rust target and Luau must use the same exception ABI. Current Rust uses
  native WASM exceptions, while `luau0-src` still requests Emscripten's legacy
  `-fexceptions` behavior. `build.sh` appends `-fwasm-exceptions` for C++ and at
  the final link to align them.
- Rust invokes `emcc` for the final link, so `-sDEFAULT_TO_CXX` is required to
  include libc++.
- The successful optimized output observed during this spike was approximately
  61 KiB of JavaScript and 1.0 MiB of WASM before transport compression.
- The spike passed under both Emscripten's Node runtime and headless Chromium.

This resolves the narrow Luau toolchain question. It does not yet prove that
the full Clawblox `LuaRuntime` and `GameInstance` dependency graph is portable.
