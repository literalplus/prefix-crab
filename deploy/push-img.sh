#!/usr/bin/env bash

if [[ "$#" -ne 1 ]]; then
    echo "$0 [module directory name]"
    exit 5
fi

MODULE=$1
TARGET_HOST=pnowak@measurement-aim.etchosts.internal

./build-img.sh "$MODULE"

echo " --- Now sending over image prefix-crab.local/$MODULE ---"
docker save "prefix-crab.local/$MODULE" | ssh "$TARGET_HOST" podman load
echo " --- Done ---"
