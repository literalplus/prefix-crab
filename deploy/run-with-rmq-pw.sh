#!/usr/bin/env bash

SECRET_NAME=prefix-crab-rmq-password

SECRET_VALUE=$(podman run --secret "$SECRET_NAME" --rm docker.io/alpine cat "/run/secrets/$SECRET_NAME" || (echo "Failed to read credential!" && exit 17))

echo "Providing secret $SECRET_NAME in environment as \$RMQ_PW (for ${@})".

RMQ_PW="$SECRET_VALUE" "${@}"
