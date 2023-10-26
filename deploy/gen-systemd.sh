#!/usr/bin/env bash

if [[ "$#" -ne 1 ]]; then
    echo "$0 [existing container name]"
    exit 5
fi

NAME=$1

podman generate systemd -f -n --new --container-prefix="" "$NAME"

mkdir -p ~/.config/systemd/user/
cp $NAME.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now $NAME.service
