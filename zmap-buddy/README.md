zmap-buddy
==========

Spawns a ZMAPv6 instance and forwards probe requests to it.

## RabbitMQ Setup

Queue: `prefix-crab.probe-request.echo`

Type should be classic, unless deploying RabbitMQ HA, then it should be quorum (Note: Performance impact).

Further parameters: durable, no auto delete.

Bind to direct exchange `prefix-crab.probe-request` with routing key `echo`.
