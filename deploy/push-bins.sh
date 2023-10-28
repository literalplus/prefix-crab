#!/usr/bin/env bash

TARGET_HOST=pnowak@measurement-aim.etchosts.internal

pushd .. || exit 17

make docker-builder

TMP_CONTAINER_ID=$(docker create prefix-crab.local/builder)
echo "Uploading yarrp-buddy..."
docker cp $TMP_CONTAINER_ID:/usr/src/prefix-crab/target/release/yarrp-buddy - | ssh $TARGET_HOST bash -c "cat > ./prefix-crab/deploy/bin/yarrp-buddy"
echo "Uploading zmap-buddy..."
docker cp $TMP_CONTAINER_ID:/usr/src/prefix-crab/target/release/zmap-buddy - | ssh $TARGET_HOST bash -c "cat > ./prefix-crab/deploy/bin/zmap-buddy"
docker rm -v $TMP_CONTAINER_ID
echo "Done."
