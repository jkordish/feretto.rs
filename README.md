# feretto
---
Generate [consul](http://consul.io/) events from log sources.

Philosophy behind feretto's creation is to attempt to create self-healing distributed systems.

### help
````
feretto -- generate consul events from log sources

Usage:
    feretto [options] event [--datacenter <dc> --node <node> --service <service> --tag <tag>] <name> <string> <file>
    feretto (-h)

Options:
    -h, --help                show this screen
    -s, --server <host>       consul server to connect [default: localhost]
    -p, --port <port>         consul server port to connect [default: 8500]
    --datacenter <dc>         datacenter name
    --node <node>             node name
    --service <service>       service name
    --tag <tag>               tag name
````

### example

```
$ feretto event restart_nginx "error!" /var/log/upstart/nginx.log
```

### Install

**cargo:**

    $ cargo install --git https://github.com/jkordish/feretto

**osx:**

    $ wget -O /usr/local/bin/feretto  https://github.com/jkordish/feretto/releases/download/v0.0.1/feretto-osx

**linux:**

    $ wget -O /usr/local/bin/feretto  https://github.com/jkordish/feretto/releases/download/v0.0.1/feretto-linux

### Notes

**Requires Rust nightly**

See the [Multirust](https://github.com/brson/multirust) tool for installing and managing Rust installations.
