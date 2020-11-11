#!/usr/bin/env bash
set -eux

export PATH=$PATH:$HOME/.cargo/bin

[ -d "wasm" ] && rm -r wasm
[ -d "dist" ] && rm -r dist

cargo build --release --target wasm32-unknown-unknown

wasm-bindgen --target nodejs --out-dir dist/wasm target/wasm32-unknown-unknown/release/nail_salon.wasm

./node_modules/.bin/tsc -p ./tsconfig.ci.json

npm pack .
