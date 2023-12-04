#!/usr/bin/env bash

echo "Use the \`crab-tools\` command in the container to access the tools"
echo "The ~/prefix-crab directory is mounted at /usr/src/prefix-crab (shadowing the original source)"

/usr/bin/podman run \
	--rm \
	--add-host=localhost.containers.internal:10.0.2.2 \
	--network=slirp4netns:allow_host_loopback=true \
	--pull=never \
	--secret=prefix-crab-rmq-password,type=env,target=RMQ_PW \
	--secret=prefix-crab-postgres-password,type=env,target=POSTGRES_PW \
	--mount=type=bind,source=$(pwd)/containers.env,ro,destination=/.env \
	--mount=type=bind,source=/etc/scanning/blocklist,ro,destination=/etc/scanning/blocklist \
	--mount=type=bind,source=$(pwd)/..,rw,destination=/usr/src/prefix-crab \
    --entrypoint=/bin/bash \
    -it \
	--tz=UTC prefix-crab.local/crab-tools
