# Examples for testing

Don't forget to set the correct RabbitMQ credentials in the environment, otherwise it will look like the
connection was just closed.

Routing key: `echo`

```json
{
  "target_net": "fddc:9d0b:e318:8712::bc:1/48"
}
```

To run an all-in-one local setup based on `docker-compose` and `tmux`, run:

```bash
make tmux
```

All further instructions are provided by the program.
