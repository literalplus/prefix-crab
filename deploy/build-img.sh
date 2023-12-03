#!/usr/bin/env bash

if [[ "$#" -ne 1 ]]; then
    echo "$0 [module directory name]"
    exit 5
fi

MODULE=$1

pushd .. || exit 3

if [[ ! -d "$MODULE" ]]; then
    echo "$MODULE is not a directory"
    exit 6
fi

make "docker-builder" || exit 2
pushd "$MODULE" || exit 3
docker build -t "prefix-crab.local/$MODULE" . || exit 18
popd || exit 3
