#!/bin/sh
# Build the WebAssembly engine into web/pkg, then serve web/ to try it locally:
#   ./build.sh && python3 -m http.server -d . 8080   (open http://localhost:8080)
set -e
cd "$(dirname "$0")/engine"
wasm-pack build --target web --release --out-dir ../pkg
