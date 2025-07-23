use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Tree};

use crate::{Error, NodeKinds};

pub fn flatten_tree(node_kinds: &NodeKinds, tree: Tree, code: &str) -> Result<FlatRoot, Error> {
    let mut root_builder = FlatRootBuilder::new();

    flatten_node(&mut root_builder, node_kinds, tree.root_node(), code)?;

    Ok(root_builder.root()?)
}

fn flatten_node<Builder: FlatBuilder>(
    builder: &mut Builder,
    node_kinds: &NodeKinds,
    node: Node,
    code: &str,
) -> Result<(), Error> {
    let node_kind = node.kind_id();

    let node_text = node.utf8_text(code.as_bytes()).map_err(|error| {
        Error::FlattenError(format!("Error: Failed to extract UTF-8 text: {}.", error))
    })?;

    match node_kind {
        kind if kind == node_kinds.source_file || kind == node_kinds.source => {
            let mut source_builder = FlatSourceBuilder::new(builder);

            for child in node.named_children(&mut node.walk()) {
                flatten_node(&mut source_builder, node_kinds, child, code)?;
            }

            let source = source_builder.source()?;

            builder.source(source, true)?;
        }
        kind if kind == node_kinds.member => {
            let mut binary_builder = FlatBinaryBuilder::new(builder);

            for child in node.named_children(&mut node.walk()) {
                flatten_node(&mut binary_builder, node_kinds, child, code)?;
            }

            let binary = binary_builder.binary()?;

            builder.take_expression(FlatExpression::Member(binary))?;
        }
        kind if kind == node_kinds.multiplicative => {
            let mut binary_builder = FlatBinaryBuilder::new(builder);

            for child in node.named_children(&mut node.walk()) {
                flatten_node(&mut binary_builder, node_kinds, child, code)?;
            }

            let binary = binary_builder.binary()?;

            builder.take_expression(FlatExpression::Multiplicative(binary))?;
        }
        kind if kind == node_kinds.additive => {
            let mut binary_builder = FlatBinaryBuilder::new(builder);

            for child in node.named_children(&mut node.walk()) {
                flatten_node(&mut binary_builder, node_kinds, child, code)?;
            }

            let binary = binary_builder.binary()?;

            builder.take_expression(FlatExpression::Additive(binary))?;
        }
        kind if kind == node_kinds.comparison => {
            let mut binary_builder = FlatBinaryBuilder::new(builder);

            for child in node.named_children(&mut node.walk()) {
                flatten_node(&mut binary_builder, node_kinds, child, code)?;
            }

            let binary = binary_builder.binary()?;

            builder.take_expression(FlatExpression::Comparison(binary))?;
        }
        kind if kind == node_kinds.logical => {
            let mut binary_builder = FlatBinaryBuilder::new(builder);

            for child in node.named_children(&mut node.walk()) {
                flatten_node(&mut binary_builder, node_kinds, child, code)?;
            }

            let binary = binary_builder.binary()?;

            builder.take_expression(FlatExpression::Logical(binary))?;
        }
        kind if kind == node_kinds.call => {
            let mut binary_builder = FlatBinaryBuilder::new(builder);

            for child in node.named_children(&mut node.walk()) {
                flatten_node(&mut binary_builder, node_kinds, child, code)?;
            }

            let binary = binary_builder.binary()?;

            builder.take_expression(FlatExpression::Call(binary))?;
        }
        kind if kind == node_kinds.assign => {
            let mut binary_builder = FlatBinaryBuilder::new(builder);

            for child in node.named_children(&mut node.walk()) {
                flatten_node(&mut binary_builder, node_kinds, child, code)?;
            }

            let binary = binary_builder.binary()?;

            builder.take_expression(FlatExpression::Assign(binary))?;
        }
        kind if kind == node_kinds.parenthesize => {
            for child in node.named_children(&mut node.walk()) {
                flatten_node(builder, node_kinds, child, code)?;
            }
        }
        kind if kind == node_kinds.binary
            || kind == node_kinds.octal
            || kind == node_kinds.decimal
            || kind == node_kinds.hex =>
        {
            builder.take_expression(FlatExpression::Number(node_text.to_string()))?;
        }
        kind if kind == node_kinds.single_quoted || kind == node_kinds.double_quoted => {
            builder.take_expression(FlatExpression::String(node_text.to_string()))?;
        }
        kind if kind == node_kinds.identifier => {
            builder.take_expression(FlatExpression::Identifier(node_text.to_string()))?;
        }
        kind if kind == node_kinds.extract => {
            builder.operator(FlatOperator::Extract)?;
        }
        kind if kind == node_kinds.pipe => {
            builder.operator(FlatOperator::Pipe)?;
        }
        kind if kind == node_kinds.multiply => {
            builder.operator(FlatOperator::Multiply)?;
        }
        kind if kind == node_kinds.divide => {
            builder.operator(FlatOperator::Divide)?;
        }
        kind if kind == node_kinds.modulo => {
            builder.operator(FlatOperator::Modulo)?;
        }
        kind if kind == node_kinds.add => {
            builder.operator(FlatOperator::Add)?;
        }
        kind if kind == node_kinds.subtract => {
            builder.operator(FlatOperator::Subtract)?;
        }
        kind if kind == node_kinds.equal => {
            builder.operator(FlatOperator::Equal)?;
        }
        kind if kind == node_kinds.not_equal => {
            builder.operator(FlatOperator::NotEqual)?;
        }
        kind if kind == node_kinds.less_than => {
            builder.operator(FlatOperator::LessThan)?;
        }
        kind if kind == node_kinds.greater_than => {
            builder.operator(FlatOperator::GreaterThan)?;
        }
        kind if kind == node_kinds.less_equal => {
            builder.operator(FlatOperator::LessEqual)?;
        }
        kind if kind == node_kinds.greater_equal => {
            builder.operator(FlatOperator::GreaterEqual)?;
        }
        kind if kind == node_kinds.and => {
            builder.operator(FlatOperator::And)?;
        }
        kind if kind == node_kinds.or => {
            builder.operator(FlatOperator::Or)?;
        }
        kind if kind == node_kinds.constant => {
            builder.operator(FlatOperator::Constant)?;
        }
        kind if kind == node_kinds.variable => {
            builder.operator(FlatOperator::Variable)?;
        }
        _ => {
            return Err(Error::FlattenError(format!(
                "Error: Cannot process node of unknown type {}.",
                node.kind()
            )));
        }
    }

    Ok(())
}

trait FlatBuilder {
    fn source(&mut self, source: FlatSource, take: bool) -> Result<FlatIndex, Error>;
    fn send_expression(&mut self, expression: FlatExpression) -> Result<FlatIndex, Error>;
    fn take_expression(&mut self, expression: FlatExpression) -> Result<FlatIndex, Error>;
    fn operator(&mut self, operator: FlatOperator) -> Result<(), Error>;
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatRoot {
    pub sources: Vec<FlatSource>,
}

pub struct FlatRootBuilder {
    sources: Vec<FlatSource>,
}

impl FlatRootBuilder {
    fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    fn root(self) -> Result<FlatRoot, Error> {
        Ok(FlatRoot {
            sources: self.sources,
        })
    }
}

impl FlatBuilder for FlatRootBuilder {
    fn source(&mut self, source: FlatSource, _: bool) -> Result<FlatIndex, Error> {
        let index = FlatIndex::Source(self.sources.len());
        self.sources.push(source);
        Ok(index)
    }

    fn send_expression(&mut self, _: FlatExpression) -> Result<FlatIndex, Error> {
        Err(Error::FlattenError(
            "Error: Invalid syntax - expressions cannot be placed at the root level; they must be inside a source block.".to_string(),
        ))
    }

    fn take_expression(&mut self, _: FlatExpression) -> Result<FlatIndex, Error> {
        Err(Error::FlattenError(
            "Error: Invalid syntax - expressions cannot be placed at the root level; they must be inside a source block.".to_string(),
        ))
    }

    fn operator(&mut self, _: FlatOperator) -> Result<(), Error> {
        Err(Error::FlattenError(
            "Error: Invalid syntax - operators cannot be placed at the root level; they must be inside expressions.".to_string(),
        ))
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatSource {
    pub expressions: Vec<FlatExpression>,
}

pub struct FlatSourceBuilder<'a> {
    parent: &'a mut dyn FlatBuilder,
    expressions: Vec<FlatExpression>,
}

impl<'a> FlatSourceBuilder<'a> {
    fn new(parent: &'a mut dyn FlatBuilder) -> Self {
        Self {
            parent: parent,
            expressions: Vec::new(),
        }
    }

    fn source(self) -> Result<FlatSource, Error> {
        Ok(FlatSource {
            expressions: self.expressions,
        })
    }
}

impl<'a> FlatBuilder for FlatSourceBuilder<'a> {
    fn source(&mut self, source: FlatSource, _: bool) -> Result<FlatIndex, Error> {
        Ok(self.parent.source(source, false)?)
    }

    fn send_expression(&mut self, expression: FlatExpression) -> Result<FlatIndex, Error> {
        if let Some(position) = self
            .expressions
            .iter()
            .position(|current| *current == expression)
        {
            return Ok(FlatIndex::Expression(position));
        }

        let index = FlatIndex::Expression(self.expressions.len());
        self.expressions.push(expression);
        Ok(index)
    }

    fn take_expression(&mut self, expression: FlatExpression) -> Result<FlatIndex, Error> {
        if let Some(position) = self
            .expressions
            .iter()
            .position(|current| *current == expression)
        {
            return Ok(FlatIndex::Expression(position));
        }

        let index = FlatIndex::Expression(self.expressions.len());
        self.expressions.push(expression);
        Ok(index)
    }

    fn operator(&mut self, _: FlatOperator) -> Result<(), Error> {
        Err(Error::FlattenError(
            "Error: Invalid syntax - operators cannot be placed directly in a source block; they must be inside binary expressions.".to_string(),
        ))
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatBinary {
    pub one: Option<FlatIndex>,
    pub two: FlatIndex,
    pub operator: FlatOperator,
}

pub struct FlatBinaryBuilder<'a> {
    parent: &'a mut dyn FlatBuilder,
    one: Option<FlatIndex>,
    two: Option<FlatIndex>,
    operator: Option<FlatOperator>,
}

impl<'a> FlatBinaryBuilder<'a> {
    fn new(parent: &'a mut dyn FlatBuilder) -> Self {
        FlatBinaryBuilder {
            parent: parent,
            one: None,
            two: None,
            operator: None,
        }
    }

    fn binary(self) -> Result<FlatBinary, Error> {
        if let (Some(two), Some(operator)) = (self.two, self.operator) {
            Ok(FlatBinary {
                one: self.one,
                two: two,
                operator: operator,
            })
        } else {
            Err(Error::FlattenError(
                "Error: Incomplete binary expression.".to_string(),
            ))
        }
    }
}

impl<'a> FlatBuilder for FlatBinaryBuilder<'a> {
    fn source(&mut self, source: FlatSource, take: bool) -> Result<FlatIndex, Error> {
        let index = self.parent.source(source, false)?;

        if take {
            if self.one.is_none() && self.operator.is_none() {
                self.one = Some(index.clone());
            } else if self.two.is_none() {
                self.two = Some(index.clone());
            } else {
                return Err(Error::FlattenError(
                    "Error: Invalid binary expression - attempted to add a third operand, but binary operations can only have exactly two operands.".to_string(),
                ));
            }
        }

        Ok(index)
    }

    fn send_expression(&mut self, expression: FlatExpression) -> Result<FlatIndex, Error> {
        self.parent.send_expression(expression)
    }

    fn take_expression(&mut self, expression: FlatExpression) -> Result<FlatIndex, Error> {
        let index = self.parent.send_expression(expression)?;

        if self.one.is_none() && self.operator.is_none() {
            self.one = Some(index.clone());
        } else if self.two.is_none() {
            self.two = Some(index.clone());
        } else {
            return Err(Error::FlattenError(
                "Error: Invalid binary expression - attempted to add a third operand, but binary operations can only have exactly two operands.".to_string(),
            ));
        }

        Ok(index)
    }

    fn operator(&mut self, operator: FlatOperator) -> Result<(), Error> {
        if self.operator.is_some() {
            return Err(Error::FlattenError(
                "Error: Invalid binary expression - attempted to add a second operator, but binary operations can only have exactly one operator.".to_string(),
            ));
        }

        self.operator = Some(operator);

        Ok(())
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
