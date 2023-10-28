#!/usr/bin/env bash

if [[ "$#" -ne 1 ]]; then
    echo "$0 [module directory name]"
    exit 5
fi

MODULE=$1
TARGET_HOST=measurement-aim.etchosts.internal

pushd .. || exit 3

if [[ ! -d "$MODULE" ]]; then
    echo "$MODULE is not a directory"
    exit 6
fi

make "docker-builder" || exit 2
pushd "$MODULE" || exit 3
docker build -t "prefix-crab.local/$MODULE" . || exit 18
popd || exit 3

echo " --- Now sending over image prefix-crab.local/$MODULE ---"
docker save "prefix-crab.local/$MODULE" | ssh "$TARGET_HOST" podman load
echo " --- Done ---"
