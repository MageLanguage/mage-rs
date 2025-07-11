use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Tree};

use crate::{Error, NodeKinds};

pub fn flatten_tree<Builder: FlatBuilder>(
    builder: &mut Builder,
    node_kinds: &NodeKinds,
    tree: Tree,
    code: &str,
) -> Result<(), Error> {
    flatten_node(builder, node_kinds, tree.root_node(), code)
}

pub fn flatten_node<Builder: FlatBuilder>(
    builder: &mut Builder,
    node_kinds: &NodeKinds,
    node: Node,
    code: &str,
) -> Result<(), Error> {
    let kind_id = node.kind_id();
    let text = &code[node.start_byte()..node.end_byte()];

    if kind_id == node_kinds.source_file {
        for child in node.named_children(&mut node.walk()) {
            flatten_node(builder, node_kinds, child, code)?;
        }
    } else if kind_id == node_kinds.additive {
        let mut additive = FlatAdditive {
            one: None,
            two: None,
            operator: None,
        };

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut additive, node_kinds, child, code)?;
        }

        builder.expression(FlatExpression::Additive(additive))?
    } else if kind_id == node_kinds.add {
        builder.operator(FlatOperator::Add)?
    } else if kind_id == node_kinds.subtract {
        builder.operator(FlatOperator::Subtract)?
    } else if kind_id == node_kinds.assign {
        let mut assign = FlatAssign {
            one: None,
            two: None,
            operator: None,
        };

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut assign, node_kinds, child, code)?;
        }

        builder.expression(FlatExpression::Assign(assign))?
    } else if kind_id == node_kinds.constant {
        builder.operator(FlatOperator::Constant)?
    } else if kind_id == node_kinds.variable {
        builder.operator(FlatOperator::Variable)?
    } else if kind_id == node_kinds.decimal {
        builder.expression(FlatExpression::Decimal(text.to_string()))?
    } else if kind_id == node_kinds.identifier {
        builder.expression(FlatExpression::Identifier(text.to_string()))?
    }

    Ok(())
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatExpression {
    Additive(FlatAdditive),
    Assign(FlatAssign),
    Decimal(String),
    String(String),
    Identifier(String),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatOperator {
    Add,
    Subtract,
    Constant,
    Variable,
}

pub trait FlatBuilder {
    fn expression(&mut self, expression: FlatExpression) -> Result<(), Error>;
    fn operator(&mut self, operator: FlatOperator) -> Result<(), Error>;
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatSource {
    pub constants: HashMap<String, usize>,
    pub variables: HashMap<String, usize>,
    pub expressions: Vec<FlatExpression>,
}

impl FlatSource {
    pub fn new() -> Self {
        Self {
            constants: HashMap::new(),
            variables: HashMap::new(),
            expressions: Vec::new(),
        }
    }
}

impl FlatBuilder for FlatSource {
    fn expression(&mut self, expression: FlatExpression) -> Result<(), Error> {
        match expression {
            FlatExpression::Assign(assign) => {
                let one = assign.one.unwrap();
                let two = assign.two.unwrap();
                let operator = assign.operator.unwrap();

                if let FlatExpression::Identifier(identifier) = *one {
                    match operator {
                        FlatOperator::Constant => {
                            self.constants.insert(identifier, self.expressions.len());
                        }
                        FlatOperator::Variable => {
                            self.variables.insert(identifier, self.expressions.len());
                        }
                        _ => (),
                    }
                    self.expressions.push(*two);
                }
            }
            _ => {
                self.expressions.push(expression);
            }
        }

        Ok(())
    }

    fn operator(&mut self, _: FlatOperator) -> Result<(), Error> {
        Err(Error::FlattenError(
            "source builder can't receive operators".to_string(),
        ))
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatAdditive {
    one: Option<Box<FlatExpression>>,
    two: Option<Box<FlatExpression>>,
    operator: Option<FlatOperator>,
}

impl FlatBuilder for FlatAdditive {
    fn expression(&mut self, expression: FlatExpression) -> Result<(), Error> {
        if self.one.is_none() {
            self.one = Some(Box::new(expression));
            return Ok(());
        } else if self.two.is_none() {
            self.two = Some(Box::new(expression));
            return Ok(());
        }

        Err(Error::FlattenError("both expressions are some".to_string()))
    }

    fn operator(&mut self, operator: FlatOperator) -> Result<(), Error> {
        if self.operator.is_none() {
            self.operator = Some(operator);
            return Ok(());
        }

        Err(Error::FlattenError("operator is some".to_string()))
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatAssign {
    one: Option<Box<FlatExpression>>,
    two: Option<Box<FlatExpression>>,
    operator: Option<FlatOperator>,
}

impl FlatBuilder for FlatAssign {
    fn expression(&mut self, expression: FlatExpression) -> Result<(), Error> {
        if self.one.is_none() {
            self.one = Some(Box::new(expression));
            return Ok(());
        } else if self.two.is_none() {
            self.two = Some(Box::new(expression));
            return Ok(());
        }

        Err(Error::FlattenError("both expressions are some".to_string()))
    }

    fn operator(&mut self, operator: FlatOperator) -> Result<(), Error> {
        if self.operator.is_none() {
            self.operator = Some(operator);
            return Ok(());
        }

        Err(Error::FlattenError("operator is some".to_string()))
    }
}
