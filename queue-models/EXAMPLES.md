# Examples for testing

Don't forget to set the correct RabbitMQ credentials in the environment, otherwise it will look like the
connection was just closed.

Routing key: `echo`

```json
{
  "target_net": "fddc:9d0b:e318:8712::bc:1/48"
}
```

```bash
sudo systemctl start gns3-server@$(id -nu)
sudo ip link add name brgns3 type bridge
sudo ip link set dev brgns3 up
sudo ip -6 route add fddc:9d0b:e318::/48 dev brgns3 via fddc:9d0b:e318:8710::bb:1 metric 3
sudo ip -6 addr add fddc:9d0b:e318:8710::cc:1/64 dev brgns3
```

`cargo run -- rabbit-mq-listen`

`cargo run -- prefix-scan fddc:9d0b:e318:8712::bc:1/48`
