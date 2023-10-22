yarrp-buddy
===========

Spawns a yarrp instance and forwards probe requests to it.

yarrp setup
-----------

There is a modified AIM yarrp, which must be used for this project to work properly: https://gitlab.sba-research.org/austrian-internet-measurements/tools/aim_zmap/-/blob/master/INSTALL.md

Upstream is at: https://github.com/cmand/yarrp

Relevant patches:
 * Add new argument `--max_null_reads` that controls `MAXNULLREADS` from
  `yarrp.h` (how many empty reads until probing ends)
 * Add new argument `--shutdown_wait` that controls `SHUTDOWN_WAIT` from
  `yarrp.h` (how long to wait for more responses after sending the last packet) -- relevant since yarrp is designed for *large* probes and not small ones like we do

You can find these patches in the `yarrp-patches` directory.

Installation instructions can be found in upstream's `README.md`, with the exception of the obvious
`sudo make install`.

The yarrp instance lock is at `/root/.yarrp/lock.0` if you need it.
