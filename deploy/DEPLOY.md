# Deployment

## Base requirements

We assume Podman Rootless to be installed and working. This is used
for everything except the measurement buddies themselves, which
run on bare metal for simplicity of network setup and performance.

## Docker builds

https://www.lpalmieri.com/posts/fast-rust-docker-builds/

```bash
./push-img.sh [aggregator|seed-guard]
```

## How to set up on a new host

Adjust the TARGET_HOST in `push-*.sh` (by default set to a name that requires configuration in `/etc/hosts`).

On the server:

```bash
# Add deploy key to the repo
cd
git clone git@github.com:literalplus/prefix-crab.git
cd prefix-crab/deploy
mkdir -p ~/.config/systemd/user
systemctl --user link $PWD/units/*
systemctl --user daemon-reload

# leading space to prevent .bash_history
 echo -n "changeme" | podman secret create prefix-crab-postgres-password -
 echo -n "changeme" | podman secret create prefix-crab-rmq-password -
pushd .. && ln -s deploy/bare-metal.env .env; popd
```

Build & push the images on the developer machine. Note that this relies on `docker`, like the local setup, and not
`buildah`. If the target machine has `docker` installed (or `buildah` and the scripts are slightly adapted), it could
be used directly with `build-img.sh`.

```bash
cd deploy
./push-img.sh aggregator
./push-img.sh seed-guard
```

In case your server uses a package manager installation of Rust (that is likely outdated), you need to install
`rustup` for your user (as we depend on non-ancient features). Please note that you need to take extra care with your
`$PATH` in this case, and ideally verify for each binary that the `rustup` version is used using `which cargo` etc.

```bash
# https://rust-lang.github.io/rustup/installation/package-managers.html
curl https://sh.rustup.rs -sSf | sh -s -- -y
source ~/.bashrc
rustup toolchain link system /usr
```

If you for some reason wish to use your package manager's version of Rust, you can e.g. `cargo +system`.
Please note that the bare-metal `units` files expect a Rustup installation of Cargo, and you will need to modify them.

Enable and start the services on the server:

```bash
pushd units && systemctl --user enable --now *.service; popd
# in non-ancient versions of systemd you can also do:
#systemctl --user enable --now "prefix-crab*"
```

### Storage

The only persistent storage in the system are the infrastructure services (RabbitMQ, Postgres).

These are configured to specific host-paths in the `/scans/` directory. If you want to start completely fresh,
you need to create a new directory there and adjust the bind mounts in the systemd units.

You also need to create the data directories (`postgresql-data`, `rabbitmq-data`, `backup`) upfront, empty.

For `asn-ip`, the setup must be done in the container itself because I wasn't able to get it to mount with the correct permissions:

```bash
podman exec -it prefix-crab-seed-guard bash
cd /home/app/asn-ip
git clone https://github.com/ipverse/asn-ip .
```

The `--group-add=keep-groups` flag on the container command lines ensures that the container users are able to access
the volume using the user's group on the host system, which must have r/w access to the directories.

If you need access to the volume directory afterwards from the host, this can be accomplished like so:
`podman unshare chmod g+r /path/to/postgresql-data`. `podman unshare` runs a command in the podman user namespace,
allowing access to the directory -- as the user is `root` in the user namespace.

## Various notes

Host loopback binds to host.containers.internal, but this is the public IP address of the host, and we cannot
access loopback bindings with it (but need to bind loopback since there is no firewall and we don't want to
expose our internal services to the internet).

Instead, we use the slirp4netns-specific ".2" pattern, which is always at 10.0.2.2 since we don't
change the default CIDR (every container gets the same IP address).
We bind this to the hosts file via `localhost.containers.internal:10.0.2.2`.

Host Ports:
 - 17861 RabbitMQ
 - 17862 Postgres `ssh -L 17862:localhost:17862 pnowak@measurement-aim.etchosts.internal`
 - 17863 RabbitMQ UI `ssh -L 17863:localhost:17863 pnowak@measurement-aim.etchosts.internal`

Do not use spaces in the passwords !

echo -n "changeme" | podman secret create prefix-crab-postgres-password -
echo -n "changeme" | podman secret create prefix-crab-rmq-password -
echo -n "changeme" | podman secret create prefix-crab-otlp-header -

OTLP instrumentation can be omitted if not needed.
A free Grafana Cloud account + OTLP setup can be provisioned here: https://grafana.com/docs/grafana-cloud/send-data/otlp/send-data-otlp/

podman run --rm -it --add-host=localhost.containers.internal:10.0.2.2 --network=slirp4netns:allow_host_loopback=true docker.io/nicolaka/netshoot

See units/ folder

scp ./gen-systemd.sh podman-test.lit.plus:.
./gen-systemd.sh prefix-crab-rabbitmq
./gen-systemd.sh prefix-crab-postgres

journalctl --user -u prefix-crab-postgres

ssh podman-test.lit.plus mkdir -p prefix-crab/deploy
scp containers.env podman-test.lit.plus:prefix-crab/deploy

Dependencies between the containers themselves don't play well with systemd (e.g. restart of postgres fails because
removing the container is prohibited due to a running dependent container), so we use native systemd dependencies that
are configured manually.

Note: Using --env-file doesn't work as it doesn't seem to resolve environment variables populated by
secrets. Instead, we bind mount the env file.

Once the systemd unit file is generated, install it to $HOME/.config/systemd/user for installing it as a non-root user. Enable the copied unit file or files using systemctl enable. Note: Copying a unit file to $HOME/.config/systemd/user and enabling it marks the unit file to be automatically started on user login. (i.e. either set `loginctl` linger or use an empty tmux instance to keep the session active)

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

## How to update

### Bare-Metal (buddies)

```bash
# ON THE SERVER
git pull
systemctl --user daemon-reload  # if there are changes to the unit files
systemctl --user restart prefix-crab-{yarrp,zmap}-buddy.service  # will recompile
```

### Containers (everything else)

```bash
# ON THE DEVELOPER MACHINE (server has no Docker builder)
cd deploy
./push-img.sh aggregator
./push-img.sh seed-guard

# ON THE SERVER
git pull
systemctl --user daemon-reload
systemctl --user restart prefix-crab-aggregator.service
systemctl --user restart prefix-crab-seed-guard.service
```

## Access to infrastructure services (port forwarding)

```bash
ssh -L 17863:localhost:17863 pnowak@measurement-aim.etchosts.internal  # rmq UI
ssh -L 17862:localhost:17862 pnowak@measurement-aim.etchosts.internal  # postgres
```

http://localhost:17863

# Taking a DB backup

```bash
podman exec -it prefix-crab-postgres /bin/bash
psql -U postgres # for console if needed
pg_dump -U postgres prefix_crab | gzip >/backup/name.sql.gz
```

For more info, see https://www.postgresql.org/docs/current/backup-dump.html

# FAQ

##   = note: /usr/bin/ld: cannot find -lpq

It's telling you that the postgres client lib is not installed on the system.
This isn't really easily possible to solve without root access.

https://askubuntu.com/questions/1148609/usr-bin-ld-cannot-find-lpq

Instead, if you want to try things locally on the remote server you can use the builder image:

```bash
podman run --rm -it lukemathwalker/cargo-chef:latest-rust-latest bash
cd /usr/src
ssh-keygen -t ed25519
cat ~/.ssh/id_ed25519.pub # store to github deploy keys
git clone git@github.com:literalplus/prefix-crab.git
cd prefix-crab
apt update && apt install vim
vim .env

```