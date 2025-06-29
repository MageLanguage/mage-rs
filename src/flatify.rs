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
#[allow(dead_code)]
pub struct FlatStatement {
    definition: Option<FlatDefinition>,
    expression: Option<FlatExpression>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
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
#[allow(dead_code)]
pub struct FlatExpression {
    content: String,
}

pub fn flatify_tree(tree: Tree, code: &str) -> Result<(), Error> {
    flatify_node(tree.root_node(), code)
}

pub fn flatify_node(node: Node, code: &str) -> Result<(), Error> {
    let mut root = FlatRoot {
        statement_chains: Vec::new(),
    };

    if node.kind() == "source_file" || node.kind() == "source" {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "statement_chain" {
                flatify_statement_chain(child, &mut root, code)?;
            }
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
        if child.kind() == "statement" {
            flatify_statement(child, &mut statement_chain, code)?
        }
    }

    root.push_statement_chain(statement_chain);
    Ok(())
}

fn flatify_statement(
    node: Node,
    statement_chain: &mut FlatStatementChain,
    code: &str,
) -> Result<(), Error> {
    let mut main_statement = FlatStatement {
        definition: None,
        expression: None,
    };

    for child in node.children(&mut node.walk()) {
        match child.kind() {
            "definition" => {
                let mut definition = FlatDefinition {
                    name: "".to_string(),
                    operation: FlatDefinitionOperation::Constant,
                };

                for child in child.children(&mut child.walk()) {
                    let text = &code[child.start_byte()..child.end_byte()];

                    match child.kind() {
                        "identifier_chain" => {
                            definition.name = text.to_string();
                        }
                        "definition_operation" => {
                            if text == ":" {
                                definition.operation = FlatDefinitionOperation::Constant
                            }
                            if text == "=" {
                                definition.operation = FlatDefinitionOperation::Variable
                            }
                        }
                        _ => (),
                    }
                }

                main_statement.definition = Some(definition);
            }
            "expression" => {
                let mut expression = FlatExpression {
                    content: "".to_string(),
                };

                for child in child.children(&mut child.walk()) {
                    let text = &code[child.start_byte()..child.end_byte()];

                    match child.kind() {
                        "number" => {
                            expression.content = text.to_string();
                        }
                        _ => (),
                    }
                }

                main_statement.expression = Some(expression);
            }
            _ => (),
        }
    }

    statement_chain.push_statement(main_statement);
    Ok(())
}
