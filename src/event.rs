extern crate curl;
extern crate rustc_serialize;
extern crate notify;
extern crate aho_corasick;

use std::io::BufReader;
use std::io::prelude::*;
use std::path::Path;
use std::sync::mpsc::channel;
use std::io::SeekFrom;
use std::fs::File;

use self::aho_corasick::{Automaton, AcAutomaton};
use self::curl::http;
use self::notify::{RecommendedWatcher, Error, Watcher};

use config;

#[derive(Serialize, Deserialize)]
struct DescribeEvent {
    file: String,
    string: String,
    server: String,
    port: String,
    options: String,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct FileHandler {
    pos: u64,
}

trait ConsulEvent {
    fn new(server: String,
           port: String,
           file: String,
           options: String,
           name: String,
           string: String)
           -> Self;

    fn fire(&self) -> ();

    fn file_name(&self) -> String;

    fn search_string(&self) -> String;

    fn monitor(&self);
}

pub trait WatchFile {
    fn watch(&self, file_name: String, search_string: String) -> bool;
}

impl ConsulEvent for DescribeEvent {
    fn new(server: String,
           port: String,
           file: String,
           options: String,
           name: String,
           string: String)
           -> DescribeEvent {
        DescribeEvent {
            server: server,
            port: port,
            file: file,
            options: options,
            name: name,
            string: string,
        }
    }

    fn monitor(&self) {

        println!("[INFO] Monitoring file {}", self.file_name());

        let handler = FileHandler { pos: 0 };

        loop {
            if WatchFile::watch(&handler, self.file.clone(), self.string.clone()) {
                self.fire();
            } else {
            }
        }
    }

    fn file_name(&self) -> String {
        self.file.clone()
    }
    fn search_string(&self) -> String {
        self.string.clone()
    }
    fn fire(&self) {
        let url = format!("http://{}:{}/v1/event/fire/{}{}",
                          self.server,
                          self.port,
                          &*self.name,
                          &*self.options);

        // we really don't need a body but need to pass something.
        let data = "feretto";

        // make connection
        let resp = match http::handle().put(url, data).exec() {
            Ok(resp) => resp,
            Err(err) => panic!("error putting k/v. {}", err),
        };

        if resp.get_code() == 200 {
            println!("[INFO] Event fired for: {}", &*self.name);
        } else {
            println!("Unable to handle HTTP response code {}", resp.get_code());
        };
    }
}

impl WatchFile for FileHandler {
    fn watch(&self, file_name: String, search_string: String) -> bool {

        // Attempt to open the file or error.
        let file = File::open(&file_name).expect("Unable to open file.");

        // Attempt to get the metadata of the file or error.
        let metadata = file.metadata().expect("Can't open file metadata.");

        // Create a buffered reader for the file.
        let mut reader = BufReader::new(&file);
        // Get the initial position of the file from the metadata.
        let mut pos = metadata.len();
        // read from pos as to not read the entire file.
        reader.seek(SeekFrom::Start(pos)).unwrap();


        let (tx, rx) = channel();
        let w: Result<RecommendedWatcher, Error> = Watcher::new(tx);

        if let Ok(mut watcher) = w {

            // Set the file path and initate a watch for it.
            match watcher.watch(Path::new(&file_name)) {
                Ok(x) => x,
                Err(err) => panic!("Could not watch file. {:?}", err),
            };

            // while loop whenever we recieve a message on the channel
            while let Ok(_) = rx.recv() {
                // create a mutable line to store the contents
                let mut line = String::new();
                // read from pos to eof to a string and store in line
                let resp = reader.read_to_string(&mut line);
                match resp {
                    Ok(len) => {
                        if len > 0 {
                            pos += len as u64;
                            reader.seek(SeekFrom::Start(pos)).unwrap();

                            let aut = AcAutomaton::new(vec![search_string.to_owned()]);
                            let mut it = aut.find(line.as_bytes());

                            if let Some(_) = it.next() {
                                return true;
                            }
                        } else {
                            return false;
                        }
                    }
                    Err(err) => {
                        println!("Bad response from file: {:?}", err);
                    }
                }
                // clear the contents of line
                line.clear();
            }
        }
        false
    }
}

// the new function gets its values passed from config
pub fn new(server: &str, port: &str, config_file: Vec<config::Config>) {

    // since config_file is a vector with one element containing our struct
    // we can just grab the first element which is the struct.
    let config_file = &config_file[0];

    // build out our ConsulEvent struct with from the config_file
    let consul_event: DescribeEvent =
        ConsulEvent::new(server.to_owned(),
                         port.to_owned(),
                         config_file.file.to_owned(),
                         config_file.options.to_owned().unwrap_or("".to_string()),
                         config_file.name.to_owned().unwrap_or(config_file.file.to_owned()),
                         config_file.string.to_owned());

    // Launch monitoring from our created consul_event
    consul_event.monitor();

}
