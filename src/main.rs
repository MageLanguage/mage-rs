use std::{
    fs,
    io::{self, BufRead},
};

use clap::Parser;

use mage_rs::{Cli, Command, Mage, Output};

fn main() {
    let arguments = Cli::parse();

    let mut mage = match Mage::new() {
        Ok(mage) => mage,
        Err(error) => {
            panic!("Mage error {:?}", error);
        }
    };

    match arguments.command {
        Command::Run(run) => {
            let process = |mage: &mut Mage, text: &str| match mage.process(&run.stage, text) {
                Ok(root) => match arguments.output {
                    Output::Text => println!("{:#?}", &root),
                    Output::Json => {
                        println!("{}", serde_json::to_string(&root).unwrap());
                    }
                },
                Err(err) => {
                    panic!("Processing error {:?}", err);
                }
            };

            match run.path {
                Some(path) => {
                    let file = fs::read_to_string(&path).unwrap();
                    process(&mut mage, file.as_str())
                }
                None => {
                    let stdin = io::stdin();

                    for line in stdin.lock().lines() {
                        if let Ok(text) = line {
                            process(&mut mage, text.as_str());
                        }
                    }
                }
            }
        }
        Command::Environment => {
            panic!("Not implemented")
        }
        Command::LanguageServer => {
            panic!("Not implemented")
        }
    }
}
