#!/bin/bash

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
pushd "$SCRIPT_DIR"

rm  -rf dist/*
./build.sh
mkdir -p dist
cp -f install.sh out
cp -f cypress-display.service out
cd out
zip -r ../dist/cypress-display.zip .
popd
