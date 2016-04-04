extern crate serde_json;
// https://aturon.github.io/crossbeam-doc/crossbeam/struct.Scope.html
extern crate crossbeam;

use std::fs;
use std::fs::File;
use std::io::prelude::*;
use self::crossbeam::*;

use event;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub file: String,
    pub string: String,
    pub options: Option<String>,
    pub name: Option<String>,
}

pub fn new(server: &str, port: &str, config: &str) {

    println!("[INFO] Initilizing: Server: {:?}, Port: {:?}, Config Directory: {:?}", server, port, config);

    let path = fs::read_dir(&config).expect("Unable to read directory");

    // need crossbeam as the std Arc is ineffiecent for my use case
    crossbeam::scope(|scope| {
        for file in path {
            scope.spawn(move || {

                let file = file.unwrap().path().clone();

                println!("[INFO] Processing configuration file {}", &file.display());

                let mut file_open = File::open(&file).expect("Unable to read file");
                let mut buf = String::new();
                let _ = file_open.read_to_string(&mut buf).expect("Unable to read contents of file");

                let config_file: Vec<Config> = match serde_json::from_str(&buf) {
                    Ok(config_file) => config_file,
                    Err(err) => {
                        panic!("Unable to process file: {:?}. Exited with error: {:?}",
                               &file,
                               err)
                    }
                };

                // pass our information to the even function
                event::new(server, port, config_file);
            });
        }
    });
}
