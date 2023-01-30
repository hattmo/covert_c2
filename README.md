# Cobalt Strike Covert C2 Crates
## Overview

This project is intended to provide common capabilities for creating external c2
systems for Cobalt Strike.  There are two crates in this project, one for clients which
have functions for spawning and communicating with beacon instances, and one for servers
which have functions for connecting to the teamserver and starting a session.

## Examples ##

To install the example client
```bash
$ LHOST=localhost:5555 cargo install covert_client --target x86_64-pc-windows-gnu --example client
$ client.exe
```
To install the example server
```bash
$ cargo install covert_server --example server
$ server
```

## Dependencies ##

To build the client on a non windows box you'll need the cross compiler mingw.  Installing
mingw varies based on distro.