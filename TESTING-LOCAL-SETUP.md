# Local setup

To run an all-in-one local setup based on `docker-compose` and `tmux`, run:

```bash
make tmux
```

All further instructions are provided by the program.

Test data:

 * `fddc:9d0b:e318::/48` - full base prefix
 * `fddc:9d0b:e318:8680::/60` - a non-homogenous subnet

Please note that the local setup is based on `docker` for historical reasons. In production, we use `podman`.