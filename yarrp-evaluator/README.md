yarrp-evaluator
================

Spawns a yarrp instance to measure a large prefix on /48 granularity.

IN: CIDR of the prefix to scan

The evaluator splits the prefix into /48 subnets and traces each one with 16 probes.

OUT: a CSV

```
    prefix
    num addresses probed
    num trace responses received
    LHRs
```

yarrp setup
-----------

same as yarrp-buddy
