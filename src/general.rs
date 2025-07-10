use tree_sitter::{Language, Tree};

use crate::{FlatSource, flatten_tree};

#[derive(Debug)]
pub enum Error {
    FlattenError(String),
}

pub fn process_tree(language: &Language, tree: Tree, code: &str) -> Result<(), Error> {
    let mut source = FlatSource::new();
    let node_kinds = NodeKinds::new(language);
    flatten_tree(&mut source, &node_kinds, tree, code)?;
    println!("{:#?}", source);
    Ok(())
}

pub struct NodeKinds {
    pub source_file: u16,
    pub source: u16,
    pub parenthesize: u16,
    pub member: u16,
    pub call: u16,
    pub multiplicative: u16,
    pub additive: u16,
    pub comparison: u16,
    pub logical: u16,
    pub assign: u16,

    pub binary: u16,
    pub octal: u16,
    pub decimal: u16,
    pub hex: u16,

    pub single_quoted: u16,
    pub double_quoted: u16,
    pub escape: u16,
    pub raw: u16,

    pub identifier: u16,

    pub add: u16,
    pub subtract: u16,

    pub constant: u16,
    pub variable: u16,
}

impl NodeKinds {
    pub fn new(language: &Language) -> Self {
        Self {
            source_file: language.id_for_node_kind("source_file", true),
            source: language.id_for_node_kind("source", true),
            parenthesize: language.id_for_node_kind("parenthesize", true),
            member: language.id_for_node_kind("member", true),
            call: language.id_for_node_kind("call", true),
            multiplicative: language.id_for_node_kind("multiplicative", true),
            additive: language.id_for_node_kind("additive", true),
            comparison: language.id_for_node_kind("comparison", true),
            logical: language.id_for_node_kind("logical", true),
            assign: language.id_for_node_kind("assign", true),
            binary: language.id_for_node_kind("binary", true),
            octal: language.id_for_node_kind("octal", true),
            decimal: language.id_for_node_kind("decimal", true),
            hex: language.id_for_node_kind("hex", true),
            single_quoted: language.id_for_node_kind("single_quoted", true),
            double_quoted: language.id_for_node_kind("double_quoted", true),
            escape: language.id_for_node_kind("escape", true),
            raw: language.id_for_node_kind("raw", true),
            identifier: language.id_for_node_kind("identifier", true),
            add: language.id_for_node_kind("add", true),
            subtract: language.id_for_node_kind("subtract", true),
            constant: language.id_for_node_kind("constant", true),
            variable: language.id_for_node_kind("variable", true),
        }
    }
}
