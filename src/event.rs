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
// use std::sync::{Arc, Mutex};
// use std::thread::{self, JoinHandle};
// use std::thread;

use self::aho_corasick::{Automaton, AcAutomaton};
use self::curl::http;
use self::notify::{RecommendedWatcher, Error, Watcher};

#[derive(Debug, Clone)]
struct DescribeEvent {
    file: String,
    string: String,
    server: String,
    port: String,
    options: String,
    name: String,
}

#[derive(Debug)]
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

        println!("Launching monitoring!");

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
            println!("Event fired for: {}", &*self.name);
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
        &reader.seek(SeekFrom::Start(pos)).unwrap();


        let (tx, rx) = channel();
        let w: Result<RecommendedWatcher, Error> = Watcher::new(tx);

        if let Ok(mut watcher) = w {

            // Set the file path and initate a watch for it.
            match watcher.watch(Path::new(&file_name)) {
                Ok(x) => x,
                Err(err) => panic!("Could not watch file. {:?}", err),
            };

            while let Ok(_) = rx.recv() {
                let mut line = String::new();
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
                        println!("bas response from file: {:?}", err);
                    }
                }
                line.clear();
            }
        }
        false
    }
}

pub fn new(server: &str, port: &str, file: &str, options: String, name: String, string: String) {


    println!("Initilizing");

    let consul_event: DescribeEvent = ConsulEvent::new(server.to_owned(),
                                                       port.to_owned(),
                                                       file.to_owned(),
                                                       options,
                                                       name,
                                                       string);

    consul_event.monitor();

}
