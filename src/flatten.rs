use serde::{Deserialize, Serialize};
use std::{cell::RefCell, rc::Rc};
use tree_sitter::{Node, Tree};

use crate::{Error, NodeKinds};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatRoot {
    pub instructions: Vec<FlatInstruction>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatInstruction {
    pub operand_1: FlatOperand,
    pub operand_2: FlatOperand,
    pub operation: FlatOperation,
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

pub fn flatten_tree(node_kinds: &NodeKinds, tree: &Tree, code: &str) -> Result<FlatRoot, Error> {
    let mut builder = FlatBuilder::root(Rc::from(RefCell::new(Vec::new())));

    flatten_node(node_kinds, tree.root_node(), code, &mut builder)?;

    Ok(FlatRoot {
        instructions: builder.instructions.borrow().to_owned(),
    })
}

fn flatten_node<'a>(
    node_kinds: &NodeKinds,
    node: Node<'a>,
    code: &'a str,
    builder: &mut FlatBuilder,
) -> Result<(), Error> {
    let kind_id = node.kind_id();
    let text = &code[node.start_byte()..node.end_byte()];

    if kind_id == node_kinds.source_file {
        let mut builder = builder.source();

        for child in node.named_children(&mut node.walk()) {
            flatten_node(node_kinds, child, code, &mut builder)?;
        }
    } else if kind_id == node_kinds.additive {
        let mut builder = builder.additive();

        for child in node.named_children(&mut node.walk()) {
            flatten_node(node_kinds, child, code, &mut builder)?;
        }

        builder.build();
    } else if kind_id == node_kinds.decimal {
        builder.decimal(text.to_string());
    } else if kind_id == node_kinds.add {
        builder.add();
    } else if kind_id == node_kinds.subtract {
        builder.subtract();
    }

    Ok(())
}

pub struct FlatBuilder {
    context: Context,
    instructions: Rc<RefCell<Vec<FlatInstruction>>>,
}

#[derive(Clone)]
enum Context {
    Root,
    Additive(RefCell<Additive>),
}

#[derive(Default, Clone)]
struct Additive {
    operand_1: Option<FlatOperand>,
    operand_2: Option<FlatOperand>,
    operation: Option<FlatOperation>,
}

impl FlatBuilder {
    fn root(instructions: Rc<RefCell<Vec<FlatInstruction>>>) -> Self {
        Self {
            context: Context::Root,
            instructions: instructions,
        }
    }

    pub fn source(&self) -> Self {
        Self {
            context: Context::Root,
            instructions: self.instructions.clone(),
        }
    }

    pub fn additive(&self) -> Self {
        Self {
            context: Context::Additive(RefCell::new(Additive::default())),
            instructions: self.instructions.clone(),
        }
    }

    pub fn decimal(&self, text: String) {
        let operand = FlatOperand::Number(text);

        if let Context::Additive(ref additive) = self.context {
            let mut additive = additive.borrow_mut();

            if additive.operand_1.is_none() {
                additive.operand_1 = Some(operand);
            } else if additive.operand_2.is_none() {
                additive.operand_2 = Some(operand);
            } else {
                unreachable!()
            }
        }
    }

    pub fn add(&self) {
        if let Context::Additive(ref additive) = self.context {
            let mut additive = additive.borrow_mut();

            if additive.operation.is_none() {
                additive.operation = Some(FlatOperation::Add);
            } else {
                unreachable!()
            }
        }
    }

    pub fn subtract(&self) {
        if let Context::Additive(ref additive) = self.context {
            let mut additive = additive.borrow_mut();

            if additive.operation.is_none() {
                additive.operation = Some(FlatOperation::Subtract);
            } else {
                unreachable!()
            }
        }
    }

    pub fn build(&self) {
        if let Context::Additive(ref additive) = self.context {
            let additive = additive.borrow_mut();

            if let (Some(operand_1), Some(operand_2), Some(operation)) = (
                &additive.operand_1,
                &additive.operand_2,
                &additive.operation,
            ) {
                self.instructions.borrow_mut().push(FlatInstruction {
                    operand_1: operand_1.clone(),
                    operand_2: operand_2.clone(),
                    operation: operation.clone(),
                });
            }
        }
    }
}
