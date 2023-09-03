zmap-buddy
==========

Spawns a ZMAPv6 instance and forwards probe requests to it.

ZMAPv6 Setup
------------

Clone the AIM ZMAP fork from https://gitlab.sba-research.org/austrian-internet-measurements/tools/aim_zmap/-/tree/master/src

This is in turn a fork of https://github.com/tumi8/zmap - which should also work if you don't have access.

Follow the instructions in `INSTALL.md`: https://gitlab.sba-research.org/austrian-internet-measurements/tools/aim_zmap/-/blob/master/INSTALL.md


```bash
yay -Sy gmp json-c libpcap byacc cmake gengetopt git # other common distros can be found in INSTALL.md directly
cmake -DENABLE_DEVELOPMENT=off -DENABLE_LOG_TRACE=OFF .
make -j4
sudo make install
```
