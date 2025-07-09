use tree_sitter::{Language, Tree};

use crate::{flatten_tree, validate_tree};

#[derive(Debug)]
pub enum Error {
    FlattenError(String),
}

pub fn process_tree(language: &Language, tree: &Tree, code: &str) -> Result<(), Error> {
    let node_kinds = NodeKinds::new(language);
    validate_tree(&node_kinds, tree, code)?;
    flatten_tree(&node_kinds, tree, code)?;
    Ok(())
}

pub struct NodeKinds {
    source_file: u16,
    source: u16,
    parenthesize: u16,
    member: u16,
    call: u16,
    multiplicative: u16,
    additive: u16,
    comparison: u16,
    logical: u16,
    assign: u16,

    binary: u16,
    octal: u16,
    decimal: u16,
    hex: u16,

    single_quoted: u16,
    double_quoted: u16,
    escape: u16,
    raw: u16,

    identifier: u16,
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
        }
    }
}
