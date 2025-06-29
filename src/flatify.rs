use tree_sitter::{Node, Tree};

use crate::Error;

#[derive(Debug, Clone)]
pub struct FlatRoot {
    statement_chains: Vec<FlatStatementChain>,
}

impl FlatRoot {
    fn push_statement_chain(&mut self, statement_chain: FlatStatementChain) {
        self.statement_chains.push(statement_chain);
    }
}

#[derive(Debug, Clone)]
pub struct FlatStatementChain {
    statements: Vec<FlatStatement>,
}

impl FlatStatementChain {
    fn push_statement(&mut self, statement: FlatStatement) {
        self.statements.push(statement);
    }
}

#[derive(Debug, Clone)]
pub struct FlatStatement {
    definition: Option<FlatDefinition>,
    expression: Option<FlatExpression>,
}

#[derive(Debug, Clone)]
pub struct FlatDefinition {
    name: String,
    operation: FlatDefinitionOperation,
}

#[derive(Debug, Clone)]
pub enum FlatDefinitionOperation {
    Constant,
    Variable,
}

#[derive(Debug, Clone)]
pub struct FlatExpression {
    //
}

pub fn flatify_tree(tree: Tree, code: &str) -> Result<(), Error> {
    flatify_node(tree.root_node(), code)
}

pub fn flatify_node(node: Node, code: &str) -> Result<(), Error> {
    let kind = node.kind();

    if kind != "source_file" && kind != "source" {
        return Err(Error::ParseError(String::from(
            "node kind should be of type source_file or source".to_string(),
        )));
    }

    let mut root = FlatRoot {
        statement_chains: Vec::new(),
    };

    for child in node.children(&mut node.walk()) {
        if child.kind() == "statement_chain" {
            flatify_statement_chain(child, &mut root, code)?;
        }
    }

    println!("{:#?}", root);
    Ok(())
}

fn flatify_statement_chain(node: Node, root: &mut FlatRoot, code: &str) -> Result<(), Error> {
    let mut statement_chain = FlatStatementChain {
        statements: Vec::new(),
    };

    for child in node.children(&mut node.walk()) {
        if child.kind() != "statement" {
            return Err(Error::ParseError(String::from(
                "statement_chain children nodes shoud be of type statement",
            )));
        }

        flatify_statement(child, &mut statement_chain, code)?;
    }

    root.push_statement_chain(statement_chain);
    Ok(())
}

fn flatify_statement(
    node: Node,
    statement_chain: &mut FlatStatementChain,
    code: &str,
) -> Result<(), Error> {
    let mut statement = FlatStatement {
        definition: None,
        expression: None,
    };

    for child in node.children(&mut node.walk()) {
        match child.kind() {
            "expression" => {}
            "definition" => {}
            "identifier_chain" => {}
            _ => {
                return Err(Error::ParseError(String::from(
                    "statement children nodes shoud be of type identifier_chain, definition or expression",
                )));
            }
        }
    }

    Ok(())
}
