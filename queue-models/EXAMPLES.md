# Examples for testing

Don't forget to set the correct RabbitMQ credentials in the environment, otherwise it will look like the
connection was just closed.

Routing key: `echo`

```json
{
  "target_net": "fddc:9d0b:e318:8712::bc:1/48"
}
```

`cargo run -- rabbit-mq-listen --source-address fddc:9d0b:e318:8710::cc:1 --interface=brgns3 --chunk-timeout-secs=2 -vvv --gateway-mac=0C:0C:7B:D2:00:01`