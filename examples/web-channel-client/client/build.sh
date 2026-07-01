#!/bin/bash
set -e

# Build the browser WASM MCP client and stage the glue for the demo harness.
#
# Mirrors examples/wasm-client/build.sh (changing only --out-name + this crate's
# layout). Produces pkg/web_channel_client.js + pkg/web_channel_client_bg.wasm,
# which index.html imports from ./pkg/.

# 1. Build the WASM package (LTO off keeps the dev build fast; --no-opt skips
#    wasm-opt, matching [package.metadata.wasm-pack] wasm-opt = false).
export CARGO_PROFILE_RELEASE_LTO=false
wasm-pack build --target web --out-name web_channel_client --no-opt

echo "Build complete."
echo "Then, in a SEPARATE terminal, start the bundled demo server:"
echo "  cargo run --manifest-path ../server/Cargo.toml"
echo "Serve THIS directory on http://127.0.0.1:8080 (the registered redirect origin):"
echo "  python3 serve.py    # callback-aware; plain 'python3 -m http.server' 404s on /callback"
echo "Open http://127.0.0.1:8080/index.html and click Login."
