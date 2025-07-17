use serde::{Deserialize, Serialize};
use tree_sitter::{Language, Tree};

use crate::{FlatRoot, flatten_tree};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Error {
    FlattenError(String),
}

pub fn process_tree(language: &Language, tree: Tree, code: &str) -> Result<FlatRoot, Error> {
    let node_kinds = NodeKinds::new(language);
    let root = flatten_tree(&node_kinds, tree, code)?;
    Ok(root)
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

    pub extract: u16,
    pub pipe: u16,
    pub multiply: u16,
    pub divide: u16,
    pub modulo: u16,
    pub add: u16,
    pub subtract: u16,
    pub equal: u16,
    pub not_equal: u16,
    pub less_than: u16,
    pub greater_than: u16,
    pub less_equal: u16,
    pub greater_equal: u16,
    pub and: u16,
    pub or: u16,
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
            extract: language.id_for_node_kind("extract", true),
            pipe: language.id_for_node_kind("pipe", true),
            multiply: language.id_for_node_kind("multiply", true),
            divide: language.id_for_node_kind("divide", true),
            modulo: language.id_for_node_kind("modulo", true),
            add: language.id_for_node_kind("add", true),
            subtract: language.id_for_node_kind("subtract", true),
            equal: language.id_for_node_kind("equal", true),
            not_equal: language.id_for_node_kind("not_equal", true),
            less_than: language.id_for_node_kind("less_than", true),
            greater_than: language.id_for_node_kind("greater_than", true),
            less_equal: language.id_for_node_kind("less_equal", true),
            greater_equal: language.id_for_node_kind("greater_equal", true),
            and: language.id_for_node_kind("and", true),
            or: language.id_for_node_kind("or", true),
            constant: language.id_for_node_kind("constant", true),
            variable: language.id_for_node_kind("variable", true),
        }
    }
}
