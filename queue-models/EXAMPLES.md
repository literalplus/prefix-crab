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
sudo ip -6 addr add fddc:9d0b:e318:8710::cc:1/64 dev brgns3
sudo ip -6 route add fddc:9d0b:e318::/48 dev brgns3 via fddc:9d0b:e318:8710::bb:1 metric 3
ip -6 route | grep "e318::/48" || echo "route adding failed"
```

```bash
cd zmap-buddy
cargo run -- rabbit-mq-listen
cargo build --release
../target/release/zmap-buddy prefix-scan fddc:9d0b:e318:8712::bc:1/48
cd ..
cd aggregator
cargo run
```

RabbitMQ: `http://10.45.87.51:15672/`
