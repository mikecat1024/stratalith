#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --locked --release --target wasm32-unknown-unknown -p stratalith-wasm
mkdir -p target/example
case "${1:-}" in
    --test)
        cp example/test/contracts.html target/example/contracts.html
        set -- example/test/contracts.ts
        ;;
    --bench) set -- example/bench/replay.ts ;;
    *) set -- ;;
esac
node --input-type=module -e '
    import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
    import { stripTypeScriptTypes } from "node:module";
    import { dirname } from "node:path";
    for (const file of process.argv.slice(1)) {
        const source = readFileSync(file, "utf8");
        const output = "target/example/" + file.replace(/\.ts$/, ".js");
        mkdirSync(dirname(output), { recursive: true });
        writeFileSync(output, stripTypeScriptTypes(source, { mode: "transform" }));
    }
' packages/browser/src/*.ts example/main.ts "$@"
cp example/index.html target/example/
cp target/wasm32-unknown-unknown/release/stratalith_wasm.wasm target/example/stratalith.wasm
