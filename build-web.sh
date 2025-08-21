#!/bin/sh

cd wasm
trunk build -d ../docs --release
echo 'igp-pattern-printer.adno.page' > ../docs/CNAME
