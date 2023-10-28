#!/usr/bin/env bash

SECRET_NAME=prefix-crab-rmq-password

SECRET_VALUE=$(podman run --secret "$SECRET_NAME" --rm docker.io/alpine cat "/run/secrets/$SECRET_NAME" || (echo "Failed to read credential!" && exit 17))

echo "oops we forgot to remove this: $SECRET_VALUE"
export RMQ_PW="$SECRET_VALUE"

echo "Provided secret $SECRET_NAME to environment as \$RMQ_PW".

"${@}"
