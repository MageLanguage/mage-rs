use serde::{Deserialize, Serialize};

use tree_sitter::{Node, Tree};

use crate::{Error, NodeKinds};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatRoot {
    instructions: Vec<FlatInstruction>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatInstruction {
    operand_1: FlatOperand,
    operand_2: FlatOperand,
    operation: FlatOperation,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatOperand {
    Identifier(String),
    String(String),
    Number(String),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatOperation {
    Extract,

    Pipe,

    Multiply,
    Divide,
    Modulo,

    Add,
    Substract,

    Equal,
    NotEqual,
    LessThan,
    GreaterThan,
    LessEqual,
    GreaterEqual,

    And,
    Or,

    Constant,
    Variable,
}

pub fn flatten_tree(node_kinds: &NodeKinds, tree: &Tree, code: &str) -> Result<(), Error> {
    let builder = FlatBuilder {};

    flatten_node(node_kinds, tree.root_node(), code, &builder)?;

    Ok(())
}

fn flatten_node<'a>(
    node_kinds: &NodeKinds,
    node: Node<'a>,
    code: &'a str,
    builder: &FlatBuilder,
) -> Result<(), Error> {
    let kind_id = node.kind_id();
    let text = &code[node.start_byte()..node.end_byte()];

    if kind_id == node_kinds.source_file {
        let builder = builder.source();

        for child in node.named_children(&mut node.walk()) {
            flatten_node(node_kinds, child, code, &builder)?;
        }
    } else if kind_id == node_kinds.additive {
        let builder = builder.additive();

        for child in node.named_children(&mut node.walk()) {
            flatten_node(node_kinds, child, code, &builder)?;
        }
    } else if kind_id == node_kinds.decimal {
        builder.decimal(text.to_string());
    } else if kind_id == node_kinds.add {
        builder.add();
    } else if kind_id == node_kinds.substract {
        builder.substract();
    }

    Ok(())
}

pub struct FlatBuilder {}

impl FlatBuilder {
    pub fn source(&self) -> FlatBuilder {
        return FlatBuilder {};
    }

    pub fn additive(&self) -> FlatBuilder {
        return FlatBuilder {};
    }

    pub fn decimal(&self, text: String) {}

    pub fn add(&self) {}

    pub fn substract(&self) {}
}
