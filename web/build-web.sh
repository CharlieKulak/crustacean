#!/bin/sh
# Builds the browser version into web/dist: crustacean.wasm, the JS loader and index.html.
set -e
cd "$(dirname "$0")/.."
cargo build --release --target wasm32-unknown-unknown
OUT=web/dist
rm -rf "$OUT" && mkdir -p "$OUT"
cp target/wasm32-unknown-unknown/release/crustacean.wasm "$OUT/"
JS=vendor/macroquad/js/mq_js_bundle.js  # (macroquad's own loader, from the vendored copy)
# (macroquad's bundle has a strict-mode slip in its unused quad_net plugin; declare the variable)
# and an opaque canvas, so see-through UI never lets the page show through
sed -e 's/function i(){}register_plugin=function(e){e.env.ws_connect/function i(){}var register_plugin;register_plugin=function(e){e.env.ws_connect/' \
    -e 's/getContext("webgl")/getContext("webgl",{alpha:false})/' -e 's/getContext("webgl2")/getContext("webgl2",{alpha:false})/' \
    "$JS" > "$OUT/mq_js_bundle.js"
cp web/crustacean-net.js "$OUT/"
cp assets/icon.png "$OUT/icon.png" 2>/dev/null || true
HASH=$(cat "$OUT"/*.wasm "$OUT"/*.js | sha1sum | cut -c1-10)
sed "s/__HASH__/$HASH/g" web/index.html > "$OUT/index.html"
echo "Built $OUT ($(du -sh "$OUT" | cut -f1)), version $HASH"
