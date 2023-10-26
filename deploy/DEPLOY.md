# Deployment

## Base requirements

We assume Podman Rootless to be installed and working. This is used
for everything except the measurement buddies themselves, which
run on bare metal for simplicity of network setup and performance.

## Docker builds

https://www.lpalmieri.com/posts/fast-rust-docker-builds/

```bash
make docker-aggregator
```

## Various notes

Host loopback binds to host.containers.internal, but this is the public IP address of the host, and we cannot
access loopback bindings with it (but need to bind loopback since there is no firewall and we don't want to
expose our internal services to the internet).

Instead, we use the slirp4netns-specific ".2" pattern, which is always at 10.0.2.2 since we don't
change the default CIDR (every container gets the same IP address).
We bind this to the hosts file via `localhost.containers.internal:10.0.2.2`.

Host Ports:
 - 17861 RabbitMQ
 - 17862 Postgres

Do not use spaces in the RabbitMQ password !

echo -n "changeme" | podman secret create prefix-crab-postgres-password -
echo -n "changeme" | podman secret create prefix-crab-rmq-password -

podman run --rm -it --add-host=localhost.containers.internal:10.0.2.2 --network=slirp4netns:allow_host_loopback=true docker.io/nicolaka/netshoot

See units/ folder

scp ./gen-systemd.sh podman-test.lit.plus:.
./gen-systemd.sh prefix-crab-rabbitmq
./gen-systemd.sh prefix-crab-postgres

journalctl --user -u prefix-crab-postgres

ssh podman-test.lit.plus mkdir -p prefix-crab/deploy
scp shared.env podman-test.lit.plus:prefix-crab/deploy

Dependencies between the containers themselves don't play well with systemd (e.g. restart of postgres fails because
removing the container is prohibited due to a running dependent container), so we use native systemd dependencies that
are configured manually.

Note: Using --env-file doesn't work as it doesn't seem to resolve environment variables populated by
secrets. Instead, we bind mount the env file.

Once the systemd unit file is generated, install it to $HOME/.config/systemd/user for installing it as a non-root user. Enable the copied unit file or files using systemctl enable.

Note: Copying a unit file to $HOME/.config/systemd/user and enabling it marks the unit file to be automatically started on user login.

https://docs.podman.io/en/latest/markdown/podman-generate-systemd.1.html

Quadlet supports auto-update and restart if the images change, but we don't have that yet sadly (v4.4):
https://docs.podman.io/en/latest/markdown/podman-auto-update.1.html

We use systemd to manage auto restarts:

```
# https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html#RestartSec=
# Additional manual restart settings
Restart=always
RestartSec=5s
RestartSteps=20
RestartMaxDelaySec=5m
```

The latter two options are supported starting with systemd v254, which isn't far away from the version used in
Debian 12.
