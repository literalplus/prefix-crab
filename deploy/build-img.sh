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

if which docker >/dev/null 2>&1; then
    make "docker-builder" || exit 2
    pushd "$MODULE" || exit 3
    docker build -t "prefix-crab.local/$MODULE" . || exit 18
    popd || exit 3
elif which buildah >/dev/null 2>&1; then
    make "buildah-builder" || exit 2
    pushd "$MODULE" || exit 3
    buildah build -t "prefix-crab.local/$MODULE" . || exit 18
    popd || exit 3
else
    echo "Neither buildah nor docker seem to be installed." >&2
    exit 45
fi
    