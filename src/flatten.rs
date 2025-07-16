use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Tree};

use crate::{Error, NodeKinds};

pub fn flatten_tree(node_kinds: &NodeKinds, tree: Tree, code: &str) -> Result<FlatRoot, Error> {
    let mut root_builder = FlatRootBuilder {
        root: FlatRoot::new(),
    };

    flatten_node(&mut root_builder, node_kinds, tree.root_node(), code)?;

    Ok(root_builder.root)
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
        let mut source_builder = FlatSourceBuilder {
            parent: builder,
            source: FlatSource::new(),
        };

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut source_builder, node_kinds, child, code)?;
        }

        let source = source_builder.source;

        builder.source(source)?;
    } else if node_kind == node_kinds.additive {
        let mut binary_builder = FlatBinaryBuilder {
            parent: builder,
            binary: FlatBinary::new(),
        };

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut binary_builder, node_kinds, child, code)?;
        }

        let binary = binary_builder.binary;

        builder.expression(FlatExpression::Additive(binary), true)?;
    } else if node_kind == node_kinds.assign {
        let mut binary_builder = FlatBinaryBuilder {
            parent: builder,
            binary: FlatBinary::new(),
        };

        for child in node.named_children(&mut node.walk()) {
            flatten_node(&mut binary_builder, node_kinds, child, code)?;
        }

        let binary = binary_builder.binary;

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
    } else if node_kind == node_kinds.add {
        builder.operator(FlatOperator::Add)?;
    } else if node_kind == node_kinds.constant {
        builder.operator(FlatOperator::Constant)?;
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
    sources: Vec<FlatSource>,
}

impl FlatRoot {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }
}

pub struct FlatRootBuilder {
    root: FlatRoot,
}

impl FlatBuilder for FlatRootBuilder {
    fn source(&mut self, source: FlatSource) -> Result<(), Error> {
        self.root.sources.push(source);
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
    expressions: Vec<FlatExpression>,
}

impl FlatSource {
    pub fn new() -> Self {
        Self {
            expressions: Vec::new(),
        }
    }
}

pub struct FlatSourceBuilder<'a> {
    parent: &'a mut dyn FlatBuilder,
    source: FlatSource,
}

impl<'a> FlatBuilder for FlatSourceBuilder<'a> {
    fn source(&mut self, source: FlatSource) -> Result<(), Error> {
        self.parent.source(source)?;
        Ok(())
    }

    fn expression(&mut self, expression: FlatExpression, _: bool) -> Result<FlatIndex, Error> {
        let index = FlatIndex::Expression(self.source.expressions.len());
        self.source.expressions.push(expression);
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
    pub one: Option<FlatIndex>,
    pub two: Option<FlatIndex>,
    pub operator: Option<FlatOperator>,
}

impl FlatBinary {
    pub fn new() -> Self {
        Self {
            one: None,
            two: None,
            operator: None,
        }
    }
}

pub struct FlatBinaryBuilder<'a> {
    parent: &'a mut dyn FlatBuilder,
    binary: FlatBinary,
}

impl<'a> FlatBuilder for FlatBinaryBuilder<'a> {
    fn source(&mut self, source: FlatSource) -> Result<(), Error> {
        self.parent.source(source)
    }

    fn expression(&mut self, expression: FlatExpression, take: bool) -> Result<FlatIndex, Error> {
        let index = self.parent.expression(expression.clone(), false)?;

        if take {
            if self.binary.one.is_none() {
                self.binary.one = Some(index.clone());
            } else if self.binary.two.is_none() {
                self.binary.two = Some(index.clone());
            } else {
                return Err(Error::FlattenError(
                    "Binary operation can only have two operands".to_string(),
                ));
            }
        }

        Ok(index)
    }

    fn operator(&mut self, operator: FlatOperator) -> Result<(), Error> {
        if self.binary.operator.is_some() {
            return Err(Error::FlattenError(
                "Binary operation can only have one operator".to_string(),
            ));
        }

        self.binary.operator = Some(operator.clone());

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
