# Security policy

openIG2 talks to other players over the network (multiplayer, LAN discovery, optional UPnP / NAT-PMP port mapping)
and reads save files and the game's data files. Bugs there can matter.

## Reporting
Please **don't open a public issue** for a vulnerability. Report it privately through GitHub:
[Security → Report a vulnerability](https://github.com/openIG2-org/openIG2/security/advisories/new).

Include what an attacker can do, the version (`ig2-client --version`), and steps or a file that reproduce it. You
will get an answer within a week; fixes go into the next release and the advisory is published after it.

## Supported versions
Only the latest release and `main` get security fixes.
