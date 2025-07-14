use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Tree};

use crate::{Error, NodeKinds};

pub fn flatten_tree(
    root: &mut FlatRoot,
    node_kinds: &NodeKinds,
    tree: Tree,
    code: &str,
) -> Result<(), Error> {
    flatten_node(root, None, node_kinds, tree.root_node(), code)
}

pub fn flatten_node(
    root: &mut FlatRoot,
    mut source: Option<&mut FlatSource>,
    node_kinds: &NodeKinds,
    node: Node,
    code: &str,
) -> Result<(), Error> {
    let node_kind = node.kind_id();

    if is_source_file_node(node_kinds, node_kind) {
        match source {
            Some(source) => {
                for child in node.named_children(&mut node.walk()) {
                    flatten_node(root, Some(source), node_kinds, child, code)?;
                }
            }
            None => {
                let mut source = FlatSource::new();

                for child in node.named_children(&mut node.walk()) {
                    flatten_node(root, Some(&mut source), node_kinds, child, code)?;
                }

                root.sources.push(source);
            }
        }
    } else if is_source_node(node_kinds, node_kind) {
        let mut nested_source = FlatSource::new();

        for child in node.named_children(&mut node.walk()) {
            flatten_node(root, Some(&mut nested_source), node_kinds, child, code)?;
        }

        root.sources.push(nested_source);
    } else if is_parenthesize_node(node_kinds, node_kind) {
        if let Some(source) = source {
            for child in node.named_children(&mut node.walk()) {
                flatten_node(root, Some(source), node_kinds, child, code)?;
            }
        } else {
            return Err(Error::FlattenError(format!(
                "Cannot process parenthesize node without a source context"
            )));
        }
    } else if is_literal_node(node_kinds, node_kind) {
        if let Some(source) = source {
            let text = node
                .utf8_text(code.as_bytes())
                .map_err(|e| Error::FlattenError(format!("UTF8 error: {}", e)))?;

            let literal = if node_kind == node_kinds.binary
                || node_kind == node_kinds.octal
                || node_kind == node_kinds.decimal
                || node_kind == node_kinds.hex
            {
                FlatLiteral::Number(text.to_string())
            } else if node_kind == node_kinds.single_quoted || node_kind == node_kinds.double_quoted
            {
                FlatLiteral::String(text.to_string())
            } else if node_kind == node_kinds.identifier {
                FlatLiteral::Identifier(text.to_string())
            } else {
                FlatLiteral::Identifier(text.to_string())
            };

            source.expressions.push(FlatExpression::Literal(literal));
        } else {
            return Err(Error::FlattenError(format!(
                "Cannot process literal node without a source context"
            )));
        }
    } else if is_binary_operation(node_kinds, node_kind) {
        if let Some(ref mut source) = source {
            let children: Vec<Node> = node.named_children(&mut node.walk()).collect();

            if children.len() < 2 || children.len() > 3 {
                return Err(Error::FlattenError(format!(
                    "Binary operation should have 2-3 children, got {}",
                    children.len()
                )));
            }

            let (one_index, two_index, operator_index) = if children.len() == 2 {
                (None, 1, 0)
            } else {
                if is_operator_node(node_kinds, children[0].kind_id()) {
                    (None, 1, 0)
                } else {
                    (Some(0), 2, 1)
                }
            };

            let one_expression_index = if let Some(one_child_index) = one_index {
                if children[one_child_index].kind_id() == node_kinds.source {
                    let mut nested_source = FlatSource::new();

                    for child in children[one_child_index]
                        .named_children(&mut children[one_child_index].walk())
                    {
                        flatten_node(root, Some(&mut nested_source), node_kinds, child, code)?;
                    }

                    root.sources.push(nested_source);

                    Some(FlatIndex::Source(root.sources.len() - 1))
                } else {
                    flatten_node(
                        root,
                        Some(source),
                        node_kinds,
                        children[one_child_index],
                        code,
                    )?;

                    Some(FlatIndex::Expression(source.expressions.len() - 1))
                }
            } else {
                None
            };

            let two_expression_index = if children[two_index].kind_id() == node_kinds.source {
                let mut nested_source = FlatSource::new();

                for child in children[two_index].named_children(&mut children[two_index].walk()) {
                    flatten_node(root, Some(&mut nested_source), node_kinds, child, code)?;
                }

                root.sources.push(nested_source);

                FlatIndex::Source(root.sources.len() - 1)
            } else {
                flatten_node(root, Some(source), node_kinds, children[two_index], code)?;

                FlatIndex::Expression(source.expressions.len() - 1)
            };

            let operator = node_kind_to_operator(node_kinds, children[operator_index].kind_id())?;

            let binary = FlatBinary {
                one: one_expression_index,
                two: Some(two_expression_index),
                operator,
            };

            let flat_expression = match node_kind {
                k if k == node_kinds.member => FlatExpression::Member(binary),
                k if k == node_kinds.call => FlatExpression::Call(binary),
                k if k == node_kinds.multiplicative => FlatExpression::Multiplicative(binary),
                k if k == node_kinds.additive => FlatExpression::Additive(binary),
                k if k == node_kinds.comparison => FlatExpression::Comparison(binary),
                k if k == node_kinds.logical => FlatExpression::Logical(binary),
                k if k == node_kinds.assign => FlatExpression::Assign(binary),
                _ => {
                    return Err(Error::FlattenError(format!(
                        "Unknown binary operation: {}",
                        node_kind
                    )));
                }
            };

            source.expressions.push(flat_expression);
        } else {
            return Err(Error::FlattenError(format!(
                "Cannot process binary operation without a source context"
            )));
        }
    } else {
        return Err(Error::FlattenError(format!(
            "Unsupported node kind: {}. Expected source_file, source, parenthesize, literal, or binary operation",
            node_kind
        )));
    }

    Ok(())
}

fn is_source_file_node(node_kinds: &NodeKinds, node_kind: u16) -> bool {
    node_kind == node_kinds.source_file
}

fn is_source_node(node_kinds: &NodeKinds, node_kind: u16) -> bool {
    node_kind == node_kinds.source
}

fn is_parenthesize_node(node_kinds: &NodeKinds, node_kind: u16) -> bool {
    node_kind == node_kinds.parenthesize
}

fn is_operator_node(node_kinds: &NodeKinds, node_kind: u16) -> bool {
    node_kind == node_kinds.extract
        || node_kind == node_kinds.pipe
        || node_kind == node_kinds.multiply
        || node_kind == node_kinds.divide
        || node_kind == node_kinds.modulo
        || node_kind == node_kinds.add
        || node_kind == node_kinds.subtract
        || node_kind == node_kinds.equal
        || node_kind == node_kinds.not_equal
        || node_kind == node_kinds.less_than
        || node_kind == node_kinds.greater_than
        || node_kind == node_kinds.less_equal
        || node_kind == node_kinds.greater_equal
        || node_kind == node_kinds.and
        || node_kind == node_kinds.or
        || node_kind == node_kinds.constant
        || node_kind == node_kinds.variable
}

fn is_literal_node(node_kinds: &NodeKinds, node_kind: u16) -> bool {
    node_kind == node_kinds.binary
        || node_kind == node_kinds.octal
        || node_kind == node_kinds.decimal
        || node_kind == node_kinds.hex
        || node_kind == node_kinds.single_quoted
        || node_kind == node_kinds.double_quoted
        || node_kind == node_kinds.identifier
}

fn is_binary_operation(node_kinds: &NodeKinds, node_kind: u16) -> bool {
    node_kind == node_kinds.member
        || node_kind == node_kinds.call
        || node_kind == node_kinds.multiplicative
        || node_kind == node_kinds.additive
        || node_kind == node_kinds.comparison
        || node_kind == node_kinds.logical
        || node_kind == node_kinds.assign
}

fn node_kind_to_operator(
    node_kinds: &NodeKinds,
    operator_kind: u16,
) -> Result<FlatOperator, Error> {
    let operator = if operator_kind == node_kinds.extract {
        FlatOperator::Extract
    } else if operator_kind == node_kinds.pipe {
        FlatOperator::Pipe
    } else if operator_kind == node_kinds.multiply {
        FlatOperator::Multiply
    } else if operator_kind == node_kinds.divide {
        FlatOperator::Divide
    } else if operator_kind == node_kinds.modulo {
        FlatOperator::Modulo
    } else if operator_kind == node_kinds.add {
        FlatOperator::Add
    } else if operator_kind == node_kinds.subtract {
        FlatOperator::Subtract
    } else if operator_kind == node_kinds.equal {
        FlatOperator::Equal
    } else if operator_kind == node_kinds.not_equal {
        FlatOperator::NotEqual
    } else if operator_kind == node_kinds.less_than {
        FlatOperator::LessThan
    } else if operator_kind == node_kinds.greater_than {
        FlatOperator::GreaterThan
    } else if operator_kind == node_kinds.less_equal {
        FlatOperator::LessEqual
    } else if operator_kind == node_kinds.greater_equal {
        FlatOperator::GreaterEqual
    } else if operator_kind == node_kinds.and {
        FlatOperator::And
    } else if operator_kind == node_kinds.or {
        FlatOperator::Or
    } else if operator_kind == node_kinds.constant {
        FlatOperator::Constant
    } else if operator_kind == node_kinds.variable {
        FlatOperator::Variable
    } else {
        return Err(Error::FlattenError(format!(
            "Unknown operator kind: {}",
            operator_kind
        )));
    };

    Ok(operator)
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatRoot {
    pub sources: Vec<FlatSource>,
}

impl FlatRoot {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }
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
    Member(FlatBinary),
    Call(FlatBinary),
    Multiplicative(FlatBinary),
    Additive(FlatBinary),
    Comparison(FlatBinary),
    Logical(FlatBinary),
    Assign(FlatBinary),
    Literal(FlatLiteral),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatLiteral {
    Number(String),
    String(String),
    Identifier(String),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatIndex {
    Source(usize),
    Expression(usize),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatBinary {
    pub one: Option<FlatIndex>,
    pub two: Option<FlatIndex>,
    pub operator: FlatOperator,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatOperator {
    Extract,
    Pipe,
    Multiply,
    Divide,
    Modulo,
    Add,
    Subtract,
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
