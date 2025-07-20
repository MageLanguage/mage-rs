use std::{
    fs,
    io::{self, BufRead},
};

use clap::Parser;

use mage_rs::{Cli, Command, Mage, Output};

fn main() {
    let arguments = Cli::parse();

    let path = match arguments.command {
        Command::Run(run) => run.path,
        _ => None,
    };

    let mage = Mage::new();

    match path {
        Some(path) => {
            let text = fs::read_to_string(&path).unwrap();

            match mage.process(text.as_str()) {
                Ok(root) => match arguments.output {
                    Output::Text => println!("{:#?}", &root),
                    Output::Json => {
                        println!("{}", serde_json::to_string(&root).unwrap());
                    }
                },
                Err(err) => {
                    eprintln!("Error processing {}: {:?}", path, err);
                }
            }
        }
        None => {
            let stdin = io::stdin();

            for line in stdin.lock().lines() {
                if let Ok(code) = line {
                    match mage.process(code.as_str()) {
                        Ok(root) => match arguments.output {
                            Output::Text => println!("{:#?}", &root),
                            Output::Json => {
                                println!("{}", serde_json::to_string(&root).unwrap())
                            }
                        },
                        Err(err) => {
                            eprintln!("Error processing input: {:?}", err);
                        }
                    }
                }
            }
        }
    }
}
