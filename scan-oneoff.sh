#!/usr/bin/env bash

PREFIX="$1"
if [[ "$#" -lt 1 ]]; then 
    read -p "Which prefix do you want to scan? " PREFIX
fi

if ! [[ -f target/release/zmap-buddy ]]; then
    echo "No binary yet, building."
    make build-release
fi

pushd zmap-buddy >/dev/null || exit 1
../target/release/zmap-buddy prefix-scan $PREFIX
echo "Done."