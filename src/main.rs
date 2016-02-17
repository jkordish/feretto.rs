#![cfg_attr(feature="clippy", plugin(clippy))]

#![feature(custom_derive, plugin, custom_attribute, type_macros)]
#![plugin(docopt_macros)]

extern crate rustc_serialize;
extern crate docopt;

mod event;

docopt!(Args derive, "
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
");

fn main() {

    // Decode docopts
    let args: Args = Args::docopt()
                         .decode()
                         .unwrap_or_else(|e| e.exit());

    if args.cmd_event {
        // Error conditions
        if &args.arg_name == "" {
            panic!("Please supply a name to the event.");
        }
        if &args.arg_string == "" {
            panic!("Please supply string to search for.");
        }
        if &args.arg_file == "" {
            panic!("Please supply the file to search within.");
        }

        // Build options
        let mut options: String = String::new();
        if &args.flag_datacenter != "" {
            let datacenter = format!("?dc={}", &args.flag_datacenter);
            options.push_str(&datacenter);
        }
        if &args.flag_node != "" {
            let node = format!("?node={}", &args.flag_node);
            options.push_str(&node);
        }
        if &args.flag_service != "" {
            let service = format!("?service={}", &args.flag_service);
            options.push_str(&service);
        }
        if &args.flag_tag != "" {
            let tag = format!("?tag={}", &args.flag_tag);
            options.push_str(&tag);
        }

        event::new(&args.flag_server,
			&args.flag_port,
            &args.arg_file,
			options,
			args.arg_name,
			args.arg_string);
    } else {
        panic!("Not sure what to do.");
    }
}
