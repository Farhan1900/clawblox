#!/usr/bin/env bash
set -euo pipefail

spike_dir="$(cd "$(dirname "$0")" && pwd)"
emsdk_dir="${EMSDK:-$HOME/.cache/emsdk}"

if [[ ! -f "$emsdk_dir/emsdk_env.sh" ]]; then
  echo "Emscripten SDK not found at $emsdk_dir" >&2
  echo "Install emsdk or set EMSDK to its directory." >&2
  exit 1
fi

source "$emsdk_dir/emsdk_env.sh" >/dev/null
# Rust invokes `emcc` as its linker even though vendored Luau contains C++.
# Rust's Emscripten standard library uses native WASM exceptions. Compile the
# vendored Luau C++ with the same exception ABI, then pull in libc++ explicitly
# at the final link. Without both flags, mlua-sys compiles but cannot link.
export CXXFLAGS_wasm32_unknown_emscripten="${CXXFLAGS_wasm32_unknown_emscripten:-} -fwasm-exceptions"
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-sDEFAULT_TO_CXX -C link-arg=-fwasm-exceptions"
cargo build --jobs 1 --manifest-path "$spike_dir/Cargo.toml" --target wasm32-unknown-emscripten --release

output_dir="$spike_dir/dist"
mkdir -p "$output_dir"
cp "$spike_dir/target/wasm32-unknown-emscripten/release/clawblox-luau-wasm-spike.js" \
  "$output_dir/clawblox_luau_wasm_spike.js"
cp "$spike_dir/target/wasm32-unknown-emscripten/release/clawblox_luau_wasm_spike.wasm" \
  "$output_dir/clawblox_luau_wasm_spike.wasm"
cp "$spike_dir/index.html" "$output_dir/index.html"

echo "Built $output_dir"
