#![cfg_attr(feature = "clippy", plugin(clippy))]
#![feature(plugin)]

extern crate aho_corasick;
extern crate crossbeam;
extern crate ifaces;
extern crate notify;
extern crate rusoto_core;
extern crate rusoto_sns;
extern crate rusoto_sts;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[macro_use]
extern crate slog;
extern crate slog_async;
extern crate slog_term;

use rusoto_core::{default_tls_client, AutoRefreshingProvider, DefaultCredentialsProvider, Region};
use rusoto_sts::{StsAssumeRoleSessionCredentialsProvider, StsClient};
use rusoto_sns::*;
use slog::Drain;
use aho_corasick::{AcAutomaton, Automaton};
use notify::{watcher, RecursiveMode, Watcher};
use std::time::Duration;
use std::io::{BufReader, SeekFrom};
use std::io::prelude::*;
use std::sync::mpsc::channel;
use std::fs::{read_dir, File};
use std::path::Path;
use std::str::FromStr;
use std::env;
use std::process::exit;
use std::error::Error;

#[derive(Serialize, Deserialize)]
struct LogFile {
    file: String,
    string: String,
    name: String
}

// struct for our config file
#[derive(Deserialize, Clone)]
struct ConfigFile {
    target_arn: String,
    role_arn: String,
    region: String
}

// struct for our system information
struct SystemInfo {
    hostname: String,
    ipaddr: String
}

trait Watched {
    fn fire(&self, config: &ConfigFile) -> Result<PublishResponse, PublishError>;
    fn monitor(&self, config: &ConfigFile) -> Result<(), Box<Error>>;
}

trait WatchFile {
    fn watch(&self) -> bool;
}

impl Watched for LogFile {
    fn monitor(&self, config: &ConfigFile) -> Result<(), Box<Error>> {

        logging("info", &format!("Monitoring file {}", Path::new(&self.file).canonicalize().unwrap().display()));

        // create a loop that fires an sns event if pattern is matched in a logfile
        loop {
            if WatchFile::watch(self) {
                self.fire(config)?;
            }
        }
    }

    fn fire(&self, config: &ConfigFile) -> Result<PublishResponse, PublishError> {
        // determine our hostname.
        #[allow(unused_assignments)]
        let mut hostname = String::new();
        if let Ok(mut file) = File::open("/etc/hostname") {
            let mut buffer = String::new();
            let _ = file.read_to_string(&mut buffer).unwrap().to_string();
            hostname = buffer.trim().to_owned();
        } else {
            hostname = "localhost".to_string();
        }

        // need the ipaddr of our interface. will be part of the metadata
        let mut address = String::new();
        for iface in ifaces::Interface::get_all().unwrap() {
            if iface.kind == ifaces::Kind::Ipv4 {
                address = format!("{}", iface.addr.unwrap());
                address = address.replace(":0", "");
            }
            continue;
        }

        // store hostname and ip address in our struct
        let system: SystemInfo = SystemInfo {
            hostname: hostname,
            ipaddr: address
        };

        logging("info", &format!("Firing for event '{}'", &self.string));
        // set up our credentials provider for aws
        let provider = DefaultCredentialsProvider::new()?;

        // initiate our sts client
        let sts_client = StsClient::new(
            default_tls_client().unwrap(),
            provider,
            Region::from_str(&config.region.clone()).unwrap()
        );
        // generate a sts provider
        let sts_provider = StsAssumeRoleSessionCredentialsProvider::new(
            sts_client,
            config.role_arn.clone(),
            "feretto".to_owned(),
            None,
            None,
            None,
            None
        );
        // allow our STS to auto-refresh
        let auto_sts_provider = match AutoRefreshingProvider::with_refcell(sts_provider) {
            Ok(auto_sts_provider) => auto_sts_provider,
            Err(_) => {
                logging("crit", "Unable to load STS credentials");
                exit(1)
            }
        };

        // create our s3 client initialization
        let sns = SnsClient::new(
            default_tls_client().unwrap(),
            auto_sts_provider,
            Region::from_str(&config.region.clone()).unwrap()
        );

        let req = PublishInput {
            message: format!(
                "{}:{}::Triggered {} on {}",
                &system.hostname,
                &system.ipaddr,
                &self.string,
                &self.name
            ),
            subject: Some("feretto: notification".to_string()),
            target_arn: Some(config.target_arn.clone()),
            ..Default::default()
        };

        sns.publish(&req)
    }
}

impl WatchFile for LogFile {
    fn watch(&self) -> bool {
        // Attempt to open the file or error.
        let file = File::open(&self.file).unwrap();

        // Attempt to get the metadata of the file or error.
        let metadata = file.metadata().unwrap();

        // Create a buffered reader for the file.
        let mut reader = BufReader::new(&file);
        // Get the initial position of the file from the metadata.
        let mut pos = metadata.len();
        // read from pos as to not read the entire file.
        reader.seek(SeekFrom::Start(pos)).unwrap();

        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_secs(2)).unwrap();

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        watcher.watch(&self.file, RecursiveMode::NonRecursive)
            .unwrap();

        // while loop whenever we receive a message on the channel
        if let Ok(event) = rx.recv() {
            // create a mutable line to store the contents
            let mut line = String::new();

            // read from pos to eof to a string and store in line
            let resp = reader.read_to_string(&mut line);

            if let Ok(len) = resp {
                if len > 0 {
                    pos += len as u64;
                    reader.seek(SeekFrom::Start(pos)).unwrap();

                    let aut = AcAutomaton::new(vec![self.string.to_owned()]);
                    let mut it = aut.find(line.as_bytes());

                    logging("info", &format!("Event occurred {:?}", &event));
                    if it.next().is_some() {
                        return true;
                    };
                }
            }
            line.clear();
        }
        false
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        println!("feretto <config.json> <configs/>");
        exit(1)
    }

    // open our config file
    let config_file = match File::open(&args[1]) {
        Ok(file) => file,
        Err(_) => {
            logging("crit", &format!("Unable to open {}", &args[1]));
            exit(1)
        }
    };

    // attempt to deserialize the config to our struct
    let config: ConfigFile = match serde_json::from_reader(config_file) {
        Ok(json) => json,
        Err(_) => {
            logging("crit", &format!("{} not valid json", &args[1]));
            exit(1)
        }
    };

    logging("info", &format!("[INFO] Processing configuration file {}", Path::new(&args[1]).canonicalize().unwrap().display()));

    // open the directory to all the json files specifying log sources
    let path = match read_dir(&args[2]) {
        Ok(directory) => directory,
        Err(_) => {
            logging("crit", &format!("Unable to open directory {}", &args[2]));
            exit(1)
        }
    };

    // if we got here we should let someone know
    logging("info", "Ferretto starting up...");

    crossbeam::scope(|scope|
        for file in path {
            scope.spawn( || {
                let clone_log_file = file.unwrap().path().canonicalize().unwrap().clone();
                let watched_log_file = match File::open(&clone_log_file) {
                    Ok(file) => file,
                    Err(_) => {
                        logging("crit", &format!("Unable to open {}", &clone_log_file.to_str().unwrap()));
                        exit(1)
                    }
                };
                let logfile: LogFile = match serde_json::from_reader(watched_log_file) {
                    Ok(file) => file,
                    Err(_) => {
                        logging("crit", &format!("{} not valid json?", &clone_log_file.to_str().unwrap()));
                        exit(1)
                    }
                };

                let _ = Watched::monitor(&logfile, &config);
            });
        });
}

fn logging(log_type: &str, msg: &str) {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();

    let logger = slog::Logger::root(drain, o!());

    match log_type {
        "info" => info!(logger, "copy-ami-tags"; "[*]" => &msg),
        "error" => error!(logger, "copy-ami-tags"; "[*]" => &msg),
        "crit" => crit!(logger, "copy-ami-tags"; "[*]" => &msg),
        _ => {}
    }
}