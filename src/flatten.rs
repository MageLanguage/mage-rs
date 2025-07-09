use serde::{Deserialize, Serialize};

use tree_sitter::{Node, Tree};

use crate::{Error, NodeKinds};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatRoot {
    instructions: Vec<FlatInstruction>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatInstruction {
    destination: FlatOperand,
    destination_type: FlatOperation,
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

pub fn flatten_tree(node_kinds: &NodeKinds, tree: &Tree, code: &str) -> Result<FlatRoot, Error> {
    let root = FlatRoot {
        instructions: Vec::new(),
    };
    flatten_node(node_kinds, tree.root_node(), code, &root)?;
    Ok(root)
}

fn flatten_node<'a>(
    node_kinds: &NodeKinds,
    node: Node<'a>,
    code: &'a str,
    root: &FlatRoot,
) -> Result<(), Error> {
    let kind_id = node.kind_id();

    match kind_id {
        source_file if source_file == node_kinds.source_file => {
            //
        }

        _ => (),
    }

    Ok(())
}
