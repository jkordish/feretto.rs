#![cfg_attr(feature="clippy", plugin(clippy))]
#![feature(plugin)]

#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;
extern crate notify;
extern crate aho_corasick;
extern crate rusoto_core;
extern crate rusoto_sns;
extern crate rusoto_sts;
extern crate ifaces;
extern crate crossbeam;

use rusoto_core::{AutoRefreshingProvider, default_tls_client, DefaultCredentialsProvider, Region};
use rusoto_sts::{StsClient, StsAssumeRoleSessionCredentialsProvider};
use rusoto_sns::*;
use std::time::Duration;
use std::io::{SeekFrom, BufReader};
use std::io::prelude::*;
use std::sync::mpsc::channel;
use std::fs::{read_dir, File};
use aho_corasick::{Automaton, AcAutomaton};
use notify::{Watcher, RecursiveMode, watcher};
use std::path::Path;
use std::str::FromStr;
use std::env;

#[derive(Serialize, Deserialize)]
struct LogFile {
    file: String,
    string: String,
    name: String,
}

// struct for our config file
#[derive(Deserialize, Clone)]
struct ConfigFile {
    target_arn: String,
    role_arn: String,
    region: String,
}

// struct for our system information
struct SystemInfo {
    hostname: String,
    ipaddr: String,
}

trait Watched {
    fn fire(&self, config: &ConfigFile) -> Result<PublishResponse, PublishError>;
    fn monitor(&self, config: &ConfigFile);
}

trait WatchFile {
    fn watch(&self) -> notify::Result<()>;
}

impl Watched for LogFile {
    fn monitor(&self, config: &ConfigFile) {

        println!("[INFO] Monitoring file {}",
                 Path::new(&self.file).canonicalize().unwrap().display());
        loop {
            if WatchFile::watch(self).is_ok() {
                match self.fire(config) {
                    Ok(_) => println!("Sent SNS for {}", &self.name),
                    Err(e) => println!("Unable to send SNS for {}. {}", &self.name, e),
                }
            }
        }
    }

    fn fire(&self, config: &ConfigFile) -> Result<PublishResponse, PublishError> {

        // determine our hostname.
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
            ipaddr: address,
        };

        println!("[INFO] Firing for event {}", &self.string);
        // set up our credentials provider for aws
        let provider = DefaultCredentialsProvider::new()?;

        // initiate our sts client
        let sts_client = StsClient::new(default_tls_client().unwrap(),
                                        provider,
                                        Region::from_str(&config.region.clone()).unwrap());
        // generate a sts provider
        let sts_provider = StsAssumeRoleSessionCredentialsProvider::new(sts_client,
                                                                        config.role_arn.clone(),
                                                                        "feretto".to_owned(),
                                                                        None,
                                                                        None,
                                                                        None,
                                                                        None);
        // allow our STS to auto-refresh
        let auto_sts_provider = AutoRefreshingProvider::with_refcell(sts_provider);

        // create our s3 client initialization
        let sns = SnsClient::new(default_tls_client().unwrap(),
                                 auto_sts_provider.unwrap(),
                                 Region::from_str(&config.region.clone()).unwrap());

        let req = PublishInput {
            message: format!("{}:{}::Triggered {} on {}",
                             &system.hostname,
                             &system.ipaddr,
                             &self.string,
                             &self.name),
            subject: Some("feretto: notification".to_string()),
            target_arn: Some(config.target_arn.clone()),
            ..Default::default()
        };
        sns.publish(&req)
    }
}

impl WatchFile for LogFile {
    fn watch(&self) -> notify::Result<()> {
        // Attempt to open the file or error.
        let file = File::open(&self.file).expect("Unable to open file.");

        // Attempt to get the metadata of the file or error.
        let metadata = file.metadata().expect("Can't open file metadata.");

        // Create a buffered reader for the file.
        let mut reader = BufReader::new(&file);
        // Get the initial position of the file from the metadata.
        let mut pos = metadata.len();
        // read from pos as to not read the entire file.
        reader.seek(SeekFrom::Start(pos)).unwrap();

        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_secs(2))?;

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        watcher.watch(&self.file, RecursiveMode::NonRecursive)?;

        // while loop whenever we recieve a message on the channel
        while let Ok(event) = rx.recv() {

            // create a mutable line to store the contents
            let mut line = String::new();

            // read from pos to eof to a string and store in line
            let resp = reader.read_to_string(&mut line);
            match resp {
                Ok(len) => {
                    if len > 0 {
                        pos += len as u64;
                        reader.seek(SeekFrom::Start(pos)).unwrap();

                        let aut = AcAutomaton::new(vec![self.string.to_owned()]);
                        let mut it = aut.find(line.as_bytes());

                        println!("[INFO] Event occured {:?}", &event);

                        // evaulate if a match was found
                        if it.next().is_some() {
                            return Ok(());
                        }
                    }
                }
                Err(err) => {
                    println!("Bad response from file: {:?}", err);
                }
            }
            // clear the contents of line
            line.clear();
        }
        Ok(())
    }
}

fn main() {

    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        println!("feretto <config.json> <configs/>");
    }

    // open our config file
    let config_file = File::open(&args[1]).expect("could not open file");
    // attempt to deserialize the config to our struct
    let config: ConfigFile = serde_json::from_reader(config_file).expect("config has invalid json");

    println!("[INFO] Processing configuration file {}",
             Path::new(&args[1]).canonicalize().unwrap().display());

    // open the directory to all the json files specifying log sources
    let path = read_dir(&args[2]).expect("Unable to read directory");

    // if we got here we should let someone know
    println!("[INFO] Feretto starting up!");

    let config_ref = &config;

    crossbeam::scope(|scope| for file in path {
                         scope.spawn(move || {

            let clone_log_file = file.unwrap().path().clone();
            let log_direction = File::open(&clone_log_file).expect("Unable to read file");
            let logfile: LogFile = serde_json::from_reader(log_direction).expect("invalid json");

            Watched::monitor(&logfile, config_ref);
        });
                     });
}
