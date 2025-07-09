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
    statement_chain: u16,
    statement: u16,
    definition: u16,
    expression: u16,
    identifier_chain: u16,
    identifier: u16,
    call: u16,

    definition_operation: u16,
    arithmetic: u16,
    variable: u16,
    number: u16,
    string: u16,
    prioritize: u16,
    expression_section: u16,
}

impl NodeKinds {
    pub fn new(language: &Language) -> Self {
        Self {
            source_file: language.id_for_node_kind("source_file", true),
            source: language.id_for_node_kind("source", true),
            statement_chain: language.id_for_node_kind("statement_chain", true),
            statement: language.id_for_node_kind("statement", true),
            definition: language.id_for_node_kind("definition", true),
            expression: language.id_for_node_kind("expression", true),
            identifier_chain: language.id_for_node_kind("identifier_chain", true),
            identifier: language.id_for_node_kind("identifier", true),
            call: language.id_for_node_kind("call", true),

            definition_operation: language.id_for_node_kind("definition_operation", true),
            arithmetic: language.id_for_node_kind("arithmetic", true),
            variable: language.id_for_node_kind("variable", true),
            number: language.id_for_node_kind("number", true),
            string: language.id_for_node_kind("string", true),
            prioritize: language.id_for_node_kind("prioritize", true),
            expression_section: language.id_for_node_kind("expression_section", true),
        }
    }
}
