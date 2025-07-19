use std::{
    fs,
    io::{self, BufRead},
};

use clap::Parser as CLAParser;

use tree_sitter::{Language as TreeSitterLanguage, Parser as TreeSitterParser};
use tree_sitter_mage::LANGUAGE;

use serde::Serialize;

use mage_rs::process_tree;

#[derive(Debug, Clone, Serialize, clap::ValueEnum)]
enum ArgumentsOutput {
    Text,
    Json,
}

#[derive(CLAParser, Debug)]
struct Arguments {
    /// path
    path: Option<String>,
    /// output
    #[arg(long, default_value = "text")]
    output: ArgumentsOutput,
}

fn main() {
    let Arguments { path, output } = Arguments::parse();
    let language = TreeSitterLanguage::from(LANGUAGE);

    let mut parser = TreeSitterParser::new();
    parser.set_language(&language).unwrap();

    match path {
        Some(path) => {
            let code = fs::read_to_string(&path).unwrap();
            let tree = parser.parse(code.as_str(), None).unwrap();

            match process_tree(&language, tree, code.as_str()) {
                Ok(root) => match output {
                    ArgumentsOutput::Text => println!("{:#?}", &root),
                    ArgumentsOutput::Json => {
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
                    let tree = parser.parse(code.as_str(), None).unwrap();

                    match process_tree(&language, tree, code.as_str()) {
                        Ok(root) => match output {
                            ArgumentsOutput::Text => println!("{:#?}", &root),
                            ArgumentsOutput::Json => {
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
