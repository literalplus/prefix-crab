#!/usr/bin/env bash

TARGET_HOST=pnowak@measurement-aim.etchosts.internal

make build-release

pushd .. || exit 17
scp target/release/yarrp-buddy $TARGET_HOST:prefix-crab/deploy/bin/
scp target/release/zmap-buddy $TARGET_HOST:prefix-crab/deploy/bin/
