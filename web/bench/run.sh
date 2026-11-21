#!/bin/sh
set -eu
cd "$(dirname "$0")/../.."
sh web/build.sh
out=target/web
trace="$out/seph-blog1.json.gz"
if [ ! -f "$trace" ]; then
    curl --fail --location --output "$trace.tmp" https://raw.githubusercontent.com/josephg/editing-traces/762fa6c51605c88a05ebe5c4b9d4540caca30b97/sequential_traces/ascii_only/seph-blog1.json.gz
    mv "$trace.tmp" "$trace"
fi
echo "3a9967515e1dce748103b623512629f7d25a9a8bd362e465b9520aec750c536f  $trace" | shasum -a 256 -c -
gzip -dc "$trace" > "$out/seph-blog1.json"
mkdir -p "$out/bench"
node --input-type=module -e '
import { stripTypeScriptTypes } from "node:module";
import { readFileSync, writeFileSync } from "node:fs";
writeFileSync("target/web/bench/replay.js", stripTypeScriptTypes(readFileSync("web/bench/replay.ts", "utf8")));
'
sed 's|src="./main.js"|src="./bench/replay.js"|' web/index.html > "$out/benchmark.html"
port=${PORT:-8080}
echo "Open http://127.0.0.1:$port/benchmark.html"
exec python3 -m http.server "$port" --bind 127.0.0.1 --directory "$out"
