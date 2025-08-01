use std::{
    fs,
    io::{self, BufRead},
};

use clap::Parser;

use mage_rs::{Backend, Cli, Command, Mage, Output};
use tokio::runtime::Runtime;
use tower_lsp_server::{LspService, Server};

fn main() {
    let arguments = Cli::parse();

    let mut mage = Mage::new().unwrap_or_else(|error| {
        panic!("Mage error {:?}", error);
    });

    match arguments.command {
        Command::Run(run) => {
            let process = |mage: &mut Mage, text: &str| match mage.process(&run.stage, text) {
                Ok(result) => match arguments.output {
                    Output::Text => println!("{:#?}", &result),
                    Output::Json => {
                        println!("{}", serde_json::to_string(&result).unwrap());
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
            let rt = Runtime::new().unwrap();

            rt.block_on(async {
                let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());

                let (service, socket) = LspService::new(|client| Backend { client });
                Server::new(stdin, stdout, socket).serve(service).await;
            });
        }
    }
}
