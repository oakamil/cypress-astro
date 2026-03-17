#!/bin/bash

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
cd "$SCRIPT_DIR"
cd ..

cargo build --release -p cypress-display

cd "$SCRIPT_DIR"

rm -rf out
mkdir -p out/cypress/bin
cp -f ../target/release/cypress-display out/cypress/bin
cp -rf web out/cypress/bin
