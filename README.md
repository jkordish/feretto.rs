# feretto

---

Generate [consul](http://consul.io/) events from log sources.

Philosophy behind feretto's creation is to attempt to create self-healing distributed systems.

## help

```shell
feretto -- generate consul events from log sources

Usage:
    feretto [options] run
    feretto (-h)

Options:
    -h, --help                show this screen
    -s, --server <host>       consul server to connect [default: localhost]
    -p, --port <port>         consul server port to connect [default: 8500]
    -c, --config <config>     config directory holding event definitions [default: ./config/]
```

### Config

We utilize JSON config files for describing the log sources, search strings and event name.
Specify the location of these config directives via the **-c** flag otherwise ./config is utilized.

The **name** and **options** values are optional. Without **name** the value used is the filename and **options** are empty.

```json
[
  {
    "file": "/Users/jkordish/log.txt",
    "string": "error",
    "options": "?dc=dc1",
    "name": "test"
  }
]
```

### example

```none
$ feretto run
[INFO] Initilizing: Server: "localhost", Port: "8500", Config Directory: "./config/"
[INFO] Processing configuration file ./config/test2.json
[INFO] Processing configuration file ./config/test.json
[INFO] Monitoring file /Users/jkordish/log2.txt
[INFO] Monitoring file /Users/jkordish/log.txt
[INFO] Event fired for: test2
[INFO] Event fired for: test
```

```none
$ feretto -s 192.168.1.232 -c /etc/feretto/ run
```

### Install

**cargo:**

```shell
    $ cargo install --git https://github.com/jkordish/feretto
```

**osx:**

```shell
    $ wget -O /usr/local/bin/feretto  https://github.com/jkordish/feretto/releases/download/v0.0.2/feretto-osx
```

**linux:**

```shell
    $ wget -O /usr/local/bin/feretto  https://github.com/jkordish/feretto/releases/download/v0.0.2/feretto-linux
```

### Notes

**Requires Rust nightly**

See the [Multirust](https://github.com/brson/multirust) tool for installing and managing Rust installations.
