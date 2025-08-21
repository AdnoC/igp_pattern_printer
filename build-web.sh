#!/bin/sh

git branch -D web
git co -b web

cd wasm
trunk build -d ../docs --release
echo 'igp-pattern-printer.adno.page' > ../docs/CNAME
git add ../docs
git ci -am 'build'
git push -f
