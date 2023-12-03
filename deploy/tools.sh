#!/usr/bin/env bash

SECRET_NAME=prefix-crab-postgres-password

SECRET_VALUE=$(podman run --secret "$SECRET_NAME" --log-driver=none --rm docker.io/alpine cat "/run/secrets/$SECRET_NAME" || (echo "Failed to read credential!" && exit 17))

pushd ../crab-tools || exit 6
DATABASE_URL="postgres://postgres:${SECRET_VALUE}@localhost:17862/prefix_crab" rustup run stable -- cargo run -- "${@}"
popd || exit 6
