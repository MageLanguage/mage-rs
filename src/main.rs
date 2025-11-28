use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead},
    path::Path,
};

use clap::Parser;

use mage_rs::{
    Backend, Cli, Command, Mage, NodeKinds, Output, Stage, Type, compile_root, execute_bytecode,
    flatten_tree,
};
use tokio::runtime::Runtime;
use tower_lsp_server::{LspService, Server};

fn main() {
    let arguments = Cli::parse();

    let mut mage = Mage::new().unwrap_or_else(|error| {
        panic!("Mage error {:?}", error);
    });

    match arguments.command {
        Command::Run(run) => {
            let mut modules = HashMap::new();

            let mut process = |mage: &mut Mage, text: &str, path: Option<&str>| {
                let node_kinds = NodeKinds::new(&mage.language);
                let tree = mage.parse_text(text).unwrap();

                let root = flatten_tree(&node_kinds, tree, text).unwrap();

                if let Stage::Flatten = run.stage {
                    let result = Type::Flat(root);
                    match arguments.output {
                        Output::Text => println!("{:#?}", &result),
                        Output::Json => {
                            println!("{}", serde_json::to_string(&result).unwrap());
                        }
                    }
                    return;
                }

                let bytecode = compile_root(root).unwrap();

                if let Stage::Compile = run.stage {
                    let result = Type::Bytecode(bytecode);
                    match arguments.output {
                        Output::Text => println!("{:#?}", &result),
                        Output::Json => {
                            println!("{}", serde_json::to_string(&result).unwrap());
                        }
                    }
                    return;
                }

                let export = execute_bytecode(bytecode, mage, &mut modules).unwrap();
                if let Some(p) = path {
                    let name = Path::new(p)
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    // We need to keep the export table alive. execute_bytecode returns ExportTable.
                    // We box it and leak it to store as a pointer in modules.
                    let ptr = Box::into_raw(Box::new(export.clone())) as usize;
                    modules.insert(name, ptr);
                }

                let result = Type::Export(export);
                match arguments.output {
                    Output::Text => println!("{:#?}", &result),
                    Output::Json => {
                        println!("{}", serde_json::to_string(&result).unwrap());
                    }
                }
            };

            if !run.path.is_empty() {
                for path in &run.path {
                    let file = fs::read_to_string(path).unwrap();
                    process(&mut mage, file.as_str(), Some(path));
                }
            } else {
                let stdin = io::stdin();

                for line in stdin.lock().lines() {
                    if let Ok(text) = line {
                        process(&mut mage, text.as_str(), None);
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
