use std::io::{self, BufRead};

use tree_sitter::Parser;
use tree_sitter_mage::LANGUAGE;

use mage_rs::flatten_tree;

fn main() {
    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE.into()).unwrap();

    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        if let Ok(code) = line {
            let tree = parser.parse(code.as_str(), None).unwrap();
            flatten_tree(tree, code.as_str()).unwrap();
        }
    }
}
