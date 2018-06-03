#![feature(nll)]

// extern crate aho_corasick;
// #[macro_use]
// extern crate lazy_static;
extern crate crossbeam;
extern crate get_if_addrs;
extern crate notify;
extern crate regex;
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

// use aho_corasick::{AcAutomaton, Automaton};
use notify::{watcher, RecursiveMode, Watcher};
use regex::Regex;
use rusoto_core::{reactor::RequestDispatcher, AutoRefreshingProvider, Region};
use rusoto_sns::*;
use rusoto_sts::{StsAssumeRoleSessionCredentialsProvider, StsClient};
use slog::Drain;
use std::io::prelude::*;
use std::{env, error::Error, path::Path, process::exit, str::FromStr, time::Duration};
use std::{
    fs::{read_dir, read_to_string, File}, io::{BufReader, SeekFrom}, sync::mpsc::channel
};

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
    fn fire(&self, config: &ConfigFile) -> Result<(), Box<Error>>;
    fn monitor(&self, config: &ConfigFile) -> Result<(), Box<Error>>;
}

trait WatchFile {
    fn watch(&self) -> Result<(bool), Box<Error>>;
}

impl Watched for LogFile {
    fn monitor(&self, config: &ConfigFile) -> Result<(), Box<Error>> {
        logging(
            "info",
            &format!(
                "Monitoring file {}",
                Path::new(&self.file).canonicalize()?.display()
            )
        );

        // create a loop that fires an sns event if pattern is matched in a logfile
        loop {
            if WatchFile::watch(self)? {
                self.fire(config)?;
            }
        }
    }

    fn fire(&self, config: &ConfigFile) -> Result<(), Box<Error>> {
        // determine our hostname.
        let hostname = read_to_string("/etc/hostname")
            .unwrap_or_else(|_| "localhost".to_string())
            .trim()
            .to_string();

        // need the ipaddr of our interface. will be part of the metadata
        let mut ipaddr = String::new();
        for iface in get_if_addrs::get_if_addrs()? {
            if !iface.is_loopback() {
                ipaddr = iface.ip().to_string();
            }
        }

        // store hostname and ip address in our struct
        let system: SystemInfo = SystemInfo { hostname, ipaddr };

        logging("info", &format!("Firing for event '{}'", &self.string));

        // initiate our sts client
        let sts_client = StsClient::simple(Region::from_str(&config.region).unwrap());

        // generate a sts provider
        let sts_provider = StsAssumeRoleSessionCredentialsProvider::new(
            sts_client,
            config.role_arn.to_owned(),
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
            RequestDispatcher::default(),
            auto_sts_provider,
            Region::from_str(&config.region).unwrap()
        );

        let req = PublishInput {
            message: format!(
                "{}:{}::Triggered {} on {}",
                &system.hostname, &system.ipaddr, &self.string, &self.name
            ),
            subject: Some("feretto: notification".to_string()),
            target_arn: Some(config.target_arn.to_owned()),
            ..Default::default()
        };

        match sns.publish(&req).sync() {
            Ok(r) => {
                logging(
                    "info",
                    &format!("Successfuly send notification {:?}", r.message_id)
                );
                Ok(())
            }
            Err(e) => {
                logging("crit", &format!("Failed to send notification {:?}", e));
                Ok(())
            }
        }
    }
}

impl WatchFile for LogFile {
    fn watch(&self) -> Result<(bool), Box<Error>> {
        // Attempt to open the file or error.
        let file = File::open(&self.file)?;

        // Attempt to get the metadata of the file or error.
        let metadata = file.metadata()?;

        // Create a buffered reader for the file.
        let mut reader = BufReader::new(&file);
        // Get the initial position of the file from the metadata.
        let mut pos = metadata.len();
        // read from pos as to not read the entire file.
        reader.seek(SeekFrom::Start(pos))?;

        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_secs(2))?;

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        watcher.watch(&self.file, RecursiveMode::NonRecursive)?;

        // TODO: utilize lazy_static for one time complilation of the regex
        // let regex = format!("(?i).*{}.*", &self.string);
        // lazy_static! {
        //     static ref RE: Regex = Regex::new(regex).unwrap();
        // }

        // while loop whenever we receive a message on the channel
        if let Ok(event) = rx.recv() {
            // create a mutable line to store the contents
            let mut line = String::new();

            // read from pos to eof to a string and store in line
            let resp = reader.read_to_string(&mut line);

            if let Ok(len) = resp {
                if len > 0 {
                    pos += len as u64;
                    reader.seek(SeekFrom::Start(pos))?;
                    let regex = format!("(?i).*{}.*", &self.string);
                    let re = Regex::new(&regex)?;
                    if re.is_match(&line) {
                        logging("info", &format!("Event occurred {:?}", &event));
                        return Ok(true);
                    };
                }
            }
            line.clear();
        }
        Ok(false)
    }
}

fn main() -> Result<(), Box<Error>> {
    let args: Vec<_> = env::args().collect();
    if args.is_empty() {
        println!("feretto <config.json> <configs/>");
        exit(1)
    }

    // attempt to deserialize the config to our struct
    let config: ConfigFile = match serde_json::from_str(&read_to_string(&args[1])?) {
        Ok(json) => json,
        Err(_) => {
            logging("crit", &format!("{} not valid json", &args[1]));
            exit(1)
        }
    };

    logging(
        "info",
        &format!(
            "[INFO] Processing configuration file {}",
            Path::new(&args[1]).canonicalize()?.display()
        )
    );

    // open the directory to all the json files specifying log sources
    let path = match read_dir(&args[2]) {
        Ok(directory) => directory,
        Err(_) => {
            logging("crit", &format!("Unable to open directory {}", &args[2]));
            exit(1)
        }
    };

    // if we got here we should let someone know
    logging("info", "Starting up...");

    crossbeam::scope(|scope| {
        for file in path {
            scope.spawn(|| {
                let clone_log_file = file.unwrap().path().canonicalize().unwrap().clone();

                let logfile: LogFile = match serde_json::from_str(
                    &read_to_string(&clone_log_file.to_str().unwrap()).unwrap()
                ) {
                    Ok(file) => file,
                    Err(_) => {
                        logging(
                            "crit",
                            &format!("{} not valid json?", &clone_log_file.to_str().unwrap())
                        );
                        exit(1)
                    }
                };

                let _ = Watched::monitor(&logfile, &config);
            });
        }
    });
    Ok(())
}

fn logging(log_type: &str, msg: &str) {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();

    let logger = slog::Logger::root(drain, o!());

    match log_type {
        "info" => info!(logger, "feretto"; "[*]" => &msg),
        "error" => error!(logger, "feretto"; "[*]" => &msg),
        "crit" => crit!(logger, "feretto"; "[*]" => &msg),
        _ => {}
    }
}
