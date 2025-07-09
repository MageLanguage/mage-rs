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
    let shared_instr: Rc<RefCell<Vec<FlatInstruction>>> = Rc::new(RefCell::new(Vec::new()));

    let builder = FlatBuilder::root(shared_instr.clone());
    flatten_node(node_kinds, tree.root_node(), code, &builder)?;

    Ok(FlatRoot {
        instructions: shared_instr.borrow().clone(),
    })
}

/// Depth-first walk delegating semantics to `FlatBuilder`.
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
    } else if kind_id == node_kinds.subtract {
        builder.subtract();
    }

    Ok(())
}

#[derive(Clone)]
pub struct FlatBuilder {
    sink: Rc<RefCell<Vec<FlatInstruction>>>,
    ctx: Context,
}

#[derive(Clone)]
enum Context {
    Root,
    Additive(RefCell<AdditiveState>),
}

#[derive(Default, Clone)]
struct AdditiveState {
    left: Option<FlatOperand>,
    op: Option<FlatOperation>,
    right: Option<FlatOperand>,
}

impl AdditiveState {
    fn push_if_ready(&mut self, sink: &Rc<RefCell<Vec<FlatInstruction>>>) {
        if let (Some(lhs), Some(op), Some(rhs)) = (&self.left, &self.op, &self.right) {
            sink.borrow_mut().push(FlatInstruction {
                operand_1: lhs.clone(),
                operand_2: rhs.clone(),
                operation: op.clone(),
            });

            self.left = None;
            self.op = None;
            self.right = None;
        }
    }
}

impl FlatBuilder {
    fn root(sink: Rc<RefCell<Vec<FlatInstruction>>>) -> Self {
        Self {
            sink,
            ctx: Context::Root,
        }
    }

    pub fn source(&self) -> Self {
        Self {
            sink: self.sink.clone(),
            ctx: Context::Root,
        }
    }

    pub fn additive(&self) -> Self {
        Self {
            sink: self.sink.clone(),
            ctx: Context::Additive(RefCell::new(AdditiveState::default())),
        }
    }

    pub fn decimal(&self, text: String) {
        let operand = FlatOperand::Number(text);

        if let Context::Additive(ref state_cell) = self.ctx {
            let mut st = state_cell.borrow_mut();

            if st.left.is_none() {
                st.left = Some(operand);
            } else {
                st.right = Some(operand);
            }

            st.push_if_ready(&self.sink);
        }
    }

    pub fn add(&self) {
        if let Context::Additive(ref state_cell) = self.ctx {
            let mut st = state_cell.borrow_mut();

            st.op = Some(FlatOperation::Add);

            st.push_if_ready(&self.sink);
        }
    }

    pub fn subtract(&self) {
        if let Context::Additive(ref state_cell) = self.ctx {
            let mut st = state_cell.borrow_mut();

            st.op = Some(FlatOperation::Subtract);

            st.push_if_ready(&self.sink);
        }
    }
}
