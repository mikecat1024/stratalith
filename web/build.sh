#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --locked --release --target wasm32-unknown-unknown -p stratalith-wasm
mkdir -p target/web
if [ "${1:-}" = "--test" ]; then
    mkdir -p target/web/test
    cp web/test/contracts.html target/web/contracts.html
    set -- main editor markdown rich-text test/contracts
else
    set -- main editor markdown rich-text
fi
node --input-type=module -e '
    import { readFileSync, writeFileSync } from "node:fs";
    import { stripTypeScriptTypes } from "node:module";
    for (const name of process.argv.slice(1)) {
        const source = readFileSync("web/" + name + ".ts", "utf8");
        writeFileSync("target/web/" + name + ".js", stripTypeScriptTypes(source, { mode: "transform" }));
    }
' "$@"
cp web/index.html target/web/
cp target/wasm32-unknown-unknown/release/stratalith_wasm.wasm target/web/stratalith.wasm
