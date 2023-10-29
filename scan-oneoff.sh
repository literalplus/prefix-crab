#!/usr/bin/env bash

PREFIX="$1"
if [[ "$#" -lt 1 ]]; then 
    read -p "Which prefix do you want to scan? " PREFIX
fi

if ! [[ -f target/release/crab-tools ]]; then
    echo "No binary yet, building."
    make build-release
fi

pushd crab-tools >/dev/null || exit 1
../target/release/crab-tools prefix-scan $PREFIX
echo "Done."
