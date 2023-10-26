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

# --env-file=/home/lit/prefix-crab/deploy/shared.env \

podman run \
--rm \
--name=prefix-crab-aggregator \
--add-host=localhost.containers.internal:10.0.2.2 \
--network=slirp4netns:allow_host_loopback=true \
--memory=1g \
--pull=never \
--read-only \
--read-only-tmpfs \
--replace \
--requires=prefix-crab-rabbitmq \
--requires=prefix-crab-postgres \
--restart=no \
--secret=prefix-crab-rmq-password,type=env,target=RMQ_PW \
--secret=prefix-crab-postgres-password,type=env,target=POSTGRES_PW \
--mount=type=bind,source=/home/lit/prefix-crab/deploy/shared.env,destination=/home/app/.env \
--tz=UTC \
prefix-crab.local/aggregator

podman run \
--rm \
--name=prefix-crab-seed-guard \
--add-host=localhost.containers.internal:10.0.2.2 \
--network=slirp4netns:allow_host_loopback=true \
--memory=1g \
--pull=never \
--read-only \
--read-only-tmpfs \
--replace \
--requires=prefix-crab-postgres \
--restart=no \
--secret=prefix-crab-postgres-password,type=env,target=POSTGRES_PW \
--mount=type=bind,source=/home/lit/prefix-crab/deploy/shared.env,destination=/home/app/.env \
--volume=prefix-crab-seed-guard:/opt/asn-ip \
--tz=UTC \
prefix-crab.local/seed-guard

Once the systemd unit file is generated, install it to $HOME/.config/systemd/user for installing it as a non-root user. Enable the copied unit file or files using systemctl enable.

Note: Copying a unit file to $HOME/.config/systemd/user and enabling it marks the unit file to be automatically started on user login.

https://docs.podman.io/en/latest/markdown/podman-generate-systemd.1.html

Quadlet supports auto-update and restart if the images change, but we don't have that yet sadly (v4.4):
https://docs.podman.io/en/latest/markdown/podman-auto-update.1.html
