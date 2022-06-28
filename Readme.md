# Cobalt Strike Covert C2 Crates
## 🚧 Work in progress 🚧

This project is intended to provide common capabilities for creating external c2
systems for Cobalt Strike.  There are two crates in this project, one for clients which
have functions for spawning and communicating with beacon instances, and one for servers
which have functions for connecting to the teamserver and starting a session.

## Building ##

To build the artifacts
```bash
$cargo build
```

## Dependencies ##

To build the client on a non windows box you'll need the cross compiler mingw.  Installing
mingw varies based on distro.