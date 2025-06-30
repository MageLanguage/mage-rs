use std::{
    fs,
    io::{self, BufRead},
};

use clap::Parser as CLAParser;

use tree_sitter::{Language as TreeSitterLanguage, Parser as TreeSitterParser};
use tree_sitter_mage::LANGUAGE;

use serde::Serialize;
use serde_json::to_string;

use mage_rs::flatten_tree;

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

    let mut parser = TreeSitterParser::new();
    parser
        .set_language(&TreeSitterLanguage::from(LANGUAGE))
        .unwrap();

    match path {
        Some(path) => {
            let code = fs::read_to_string(&path).unwrap();
            let tree = parser.parse(code.as_str(), None).unwrap();

            if let Ok(root) = flatten_tree(tree, code.as_str()) {
                match output {
                    ArgumentsOutput::Text => {
                        println!("{:#?}", root);
                    }
                    ArgumentsOutput::Json => match to_string(&root) {
                        Ok(json) => println!("{}", json),
                        Err(e) => eprintln!("JSON serialization error: {}", e),
                    },
                }
            }
        }
        None => {
            let stdin = io::stdin();

            for line in stdin.lock().lines() {
                if let Ok(code) = line {
                    let tree = parser.parse(code.as_str(), None).unwrap();

                    if let Ok(root) = flatten_tree(tree, code.as_str()) {
                        match output {
                            ArgumentsOutput::Text => {
                                println!("{:#?}", root);
                            }
                            ArgumentsOutput::Json => match to_string(&root) {
                                Ok(json) => println!("{}", json),
                                Err(e) => eprintln!("JSON serialization error: {}", e),
                            },
                        }
                    }
                }
            }
        }
    }
}
