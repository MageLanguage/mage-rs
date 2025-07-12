use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Tree};

use crate::{Error, NodeKinds};

pub fn flatten_tree(node_kinds: &NodeKinds, tree: Tree, code: &str) -> Result<FlatSource, Error> {
    //
}

pub fn flatten_node(
    source: &mut FlatSource,
    node_kinds: &NodeKinds,
    node: Node,
    code: &str,
) -> Result<(), Error> {
    //
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatSource {
    pub expressions: Vec<FlatExpression>,
}

impl FlatSource {
    pub fn new() -> Self {
        Self {
            expressions: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatExpression {
    Literal(FlatLiteral),
    Additive(FlatAdditive),
    Assign(FlatAssign),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatLiteral {
    Number(String),
    String(String),
    Identifier(String),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatAdditive {
    one: Option<FlatLiteral>,
    two: Option<FlatLiteral>,
    operator: Option<FlatOperator>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatAssign {
    one: Option<FlatLiteral>,
    two: Option<FlatLiteral>,
    operator: Option<FlatOperator>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatOperator {
    Add,
    Subtract,
    Constant,
    Variable,
}
