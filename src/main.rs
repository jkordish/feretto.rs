#![cfg_attr(feature="clippy", plugin(clippy))]
#![feature(custom_derive, plugin, custom_attribute, type_macros)]
#![plugin(serde_macros, docopt_macros)]

extern crate serde;
extern crate rustc_serialize;
extern crate docopt;

mod event;
mod config;

docopt!(Args derive, "
feretto -- generate consul events from log sources

Usage:
    feretto [options] run
    feretto (-h)

Options:
    -h, --help                show this screen
    -s, --server <host>       consul server to connect [default: localhost]
    -p, --port <port>         consul server port to connect [default: 8500]
    -c, --config <config>     config directory holding event definitions [default: ./config/]

");

fn main() {

    // Decode docopts
    let args: Args = Args::docopt()
        .decode()
        .unwrap_or_else(|e| e.exit());

    if args.cmd_run {
        config::new(&args.flag_server, &args.flag_port, &args.flag_config);
    }
}
