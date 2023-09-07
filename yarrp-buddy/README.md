yarrp-buddy
===========

Spawns a yarrp instance and forwards probe requests to it.

yarrp setup
-----------

There is a modified AIM yarrp, but the changes made to it do not relate this this project: https://gitlab.sba-research.org/austrian-internet-measurements/tools/aim_zmap/-/blob/master/INSTALL.md

Instead, the upstream version can and should be used: https://github.com/cmand/yarrp

A useful patch from AIM yarrp is included at `yarrp-gitignore.patch`.

Installation instructions can be found in upstream's `README.md`, with the exception of the obvious
`sudo make install`.

The yarrp instance lock is at `/root/.yarrp/lock.0` if you need it.
