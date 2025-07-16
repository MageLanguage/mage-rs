use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Tree};

use crate::{Error, NodeKinds};

pub fn flatten_tree(node_kinds: &NodeKinds, tree: Tree, code: &str) -> Result<FlatRoot, Error> {
    let mut root_builder = FlatRootBuilder::new();

    flatten_node(&mut root_builder, node_kinds, tree.root_node(), code)?;

    Ok(root_builder.root()?)
}

fn flatten_node(
    builder: &mut dyn FlatBuilder,
    node_kinds: &NodeKinds,
    node: Node,
    code: &str,
) -> Result<(), Error> {
    let node_kind = node.kind_id();
    let node_text = node
        .utf8_text(code.as_bytes())
        .map_err(|e| Error::FlattenError(format!("UTF8 error: {}", e)))?;

    if node_kind == node_kinds.source_file || node_kind == node_kinds.source {
        let mut source_builder = FlatSourceBuilder::new(builder);

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut source_builder, node_kinds, child, code)?;
        }

        let source = source_builder.source()?;

        builder.source(source)?;
    } else if node_kind == node_kinds.member {
        let mut binary_builder = FlatBinaryBuilder::new(builder);

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut binary_builder, node_kinds, child, code)?;
        }

        let binary = binary_builder.binary()?;

        builder.expression(FlatExpression::Member(binary), true)?;
    } else if node_kind == node_kinds.multiplicative {
        let mut binary_builder = FlatBinaryBuilder::new(builder);

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut binary_builder, node_kinds, child, code)?;
        }

        let binary = binary_builder.binary()?;

        builder.expression(FlatExpression::Multiplicative(binary), true)?;
    } else if node_kind == node_kinds.additive {
        let mut binary_builder = FlatBinaryBuilder::new(builder);

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut binary_builder, node_kinds, child, code)?;
        }

        let binary = binary_builder.binary()?;

        builder.expression(FlatExpression::Additive(binary), true)?;
    } else if node_kind == node_kinds.comparison {
        let mut binary_builder = FlatBinaryBuilder::new(builder);

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut binary_builder, node_kinds, child, code)?;
        }

        let binary = binary_builder.binary()?;

        builder.expression(FlatExpression::Comparison(binary), true)?;
    } else if node_kind == node_kinds.logical {
        let mut binary_builder = FlatBinaryBuilder::new(builder);

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut binary_builder, node_kinds, child, code)?;
        }

        let binary = binary_builder.binary()?;

        builder.expression(FlatExpression::Logical(binary), true)?;
    } else if node_kind == node_kinds.call {
        let mut binary_builder = FlatBinaryBuilder::new(builder);

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut binary_builder, node_kinds, child, code)?;
        }

        let binary = binary_builder.binary()?;

        builder.expression(FlatExpression::Call(binary), true)?;
    } else if node_kind == node_kinds.assign {
        let mut binary_builder = FlatBinaryBuilder::new(builder);

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut binary_builder, node_kinds, child, code)?;
        }

        let binary = binary_builder.binary()?;

        builder.expression(FlatExpression::Assign(binary), true)?;
    } else if node_kind == node_kinds.parenthesize {
        for child in node.named_children(&mut node.walk()) {
            flatten_node(builder, node_kinds, child, code)?;
        }
    } else if node_kind == node_kinds.binary
        || node_kind == node_kinds.octal
        || node_kind == node_kinds.decimal
        || node_kind == node_kinds.hex
    {
        builder.expression(FlatExpression::Number(node_text.to_string()), true)?;
    } else if node_kind == node_kinds.single_quoted || node_kind == node_kinds.double_quoted {
        builder.expression(FlatExpression::String(node_text.to_string()), true)?;
    } else if node_kind == node_kinds.identifier {
        builder.expression(FlatExpression::Identifier(node_text.to_string()), true)?;
    } else if node_kind == node_kinds.extract {
        builder.operator(FlatOperator::Extract)?;
    } else if node_kind == node_kinds.pipe {
        builder.operator(FlatOperator::Pipe)?;
    } else if node_kind == node_kinds.multiply {
        builder.operator(FlatOperator::Multiply)?;
    } else if node_kind == node_kinds.divide {
        builder.operator(FlatOperator::Divide)?;
    } else if node_kind == node_kinds.modulo {
        builder.operator(FlatOperator::Modulo)?;
    } else if node_kind == node_kinds.add {
        builder.operator(FlatOperator::Add)?;
    } else if node_kind == node_kinds.subtract {
        builder.operator(FlatOperator::Subtract)?;
    } else if node_kind == node_kinds.equal {
        builder.operator(FlatOperator::Equal)?;
    } else if node_kind == node_kinds.not_equal {
        builder.operator(FlatOperator::NotEqual)?;
    } else if node_kind == node_kinds.less_than {
        builder.operator(FlatOperator::LessThan)?;
    } else if node_kind == node_kinds.greater_than {
        builder.operator(FlatOperator::GreaterThan)?;
    } else if node_kind == node_kinds.less_equal {
        builder.operator(FlatOperator::LessEqual)?;
    } else if node_kind == node_kinds.greater_equal {
        builder.operator(FlatOperator::GreaterEqual)?;
    } else if node_kind == node_kinds.and {
        builder.operator(FlatOperator::And)?;
    } else if node_kind == node_kinds.or {
        builder.operator(FlatOperator::Or)?;
    } else if node_kind == node_kinds.constant {
        builder.operator(FlatOperator::Constant)?;
    } else if node_kind == node_kinds.variable {
        builder.operator(FlatOperator::Variable)?;
    }

    Ok(())
}

trait FlatBuilder {
    fn source(&mut self, source: FlatSource) -> Result<(), Error>;
    fn expression(&mut self, expression: FlatExpression, take: bool) -> Result<FlatIndex, Error>;
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
    fn source(&mut self, source: FlatSource) -> Result<(), Error> {
        self.sources.push(source);
        Ok(())
    }

    fn expression(&mut self, _: FlatExpression, _: bool) -> Result<FlatIndex, Error> {
        Err(Error::FlattenError(
            "Can not place expressions into root context".to_string(),
        ))
    }

    fn operator(&mut self, _: FlatOperator) -> Result<(), Error> {
        Err(Error::FlattenError(
            "Can not place operators into root context".to_string(),
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
    fn source(&mut self, source: FlatSource) -> Result<(), Error> {
        self.parent.source(source)?;
        Ok(())
    }

    fn expression(&mut self, expression: FlatExpression, _: bool) -> Result<FlatIndex, Error> {
        let index = FlatIndex::Expression(self.expressions.len());
        self.expressions.push(expression);
        Ok(index)
    }

    fn operator(&mut self, _: FlatOperator) -> Result<(), Error> {
        Err(Error::FlattenError(
            "Can not place operators into source context".to_string(),
        ))
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatBinary {
    pub one: FlatIndex,
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
        if let (Some(one), Some(two), Some(operator)) = (self.one, self.two, self.operator) {
            Ok(FlatBinary {
                one: one,
                two: two,
                operator: operator,
            })
        } else {
            Err(Error::FlattenError("Incomplete binary node".into()))
        }
    }
}

impl<'a> FlatBuilder for FlatBinaryBuilder<'a> {
    fn source(&mut self, source: FlatSource) -> Result<(), Error> {
        self.parent.source(source)
    }

    fn expression(&mut self, expression: FlatExpression, take: bool) -> Result<FlatIndex, Error> {
        let index = self.parent.expression(expression, false)?;

        if take {
            if self.one.is_none() {
                self.one = Some(index.clone());
            } else if self.two.is_none() {
                self.two = Some(index.clone());
            } else {
                return Err(Error::FlattenError(
                    "Binary operation can only have two operands".to_string(),
                ));
            }
        }

        Ok(index)
    }

    fn operator(&mut self, operator: FlatOperator) -> Result<(), Error> {
        if self.operator.is_some() {
            return Err(Error::FlattenError(
                "Binary operation can only have one operator".to_string(),
            ));
        }

        self.operator = Some(operator.clone());

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
