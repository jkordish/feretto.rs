# feretto [![Build Status](https://travis-ci.org/jkordish/feretto.rs.svg?branch=master)](https://travis-ci.org/jkordish/feretto.rs)

---

## help

```shell
feretto config.json sources/
```

### Config

config.json
```json
{
  "target_arn": "<sns topic arn>",
  "role_arn": "<iam role arn>",
  "region": "<bucket region>"
}
```

sources/log1.json
```json
{
  "file": "<path/to/log>",
  "string": "<string to search for>",
  "name": "<name of the event>"
}

```
### example

```none
$ ./target/release/feretto feretto.example.json test/
Jun 05 18:49:12.814 INFO feretto, message:: [INFO] Processing configuration file /Users/jkordish/work/src/jkordish/feretto/feretto.example.json
Jun 05 18:49:12.815 INFO feretto, message:: [INFO] Feretto starting up!
Jun 05 18:49:12.815 INFO feretto, message:: [INFO] Monitoring file /Users/jkordish/work/src/jkordish/feretto/log1
Jun 05 18:49:12.816 INFO feretto, message:: [INFO] Monitoring file /Users/jkordish/work/src/jkordish/feretto/log2
Jun 05 18:49:35.003 INFO feretto, message:: [INFO] Event occured NoticeWrite("/Users/jkordish/work/src/jkordish/feretto/log1")
Jun 05 18:49:35.004 INFO feretto, message:: [INFO] Firing for event hello
Jun 05 18:49:36.540 INFO feretto, message:: [INFO] Event occured NoticeWrite("/Users/jkordish/work/src/jkordish/feretto/log2")
Jun 05 18:49:36.541 INFO feretto, message:: [INFO] Firing for event hello
```

### Install

**cargo:**

```shell
    $ cargo install --git https://github.com/jkordish/feretto
```

### Notes

**Requires Rust nightly**
