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
pub enum FlatExpression {
    Number(String),
    String(String),
}

pub fn flatten_tree(tree: Tree, code: &str) -> Result<(), Error> {
    flatten_node(tree.root_node(), code)
}

pub fn flatten_node(node: Node, code: &str) -> Result<(), Error> {
    let mut root = FlatRoot {
        statement_chains: Vec::new(),
    };

    if node.kind() == "source_file" || node.kind() == "source" {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "statement_chain" {
                flatten_statement_chain(child, code, &mut root)?;
            }
        }
    }

    println!("{:#?}", root);

    Ok(())
}

fn flatten_statement_chain(node: Node, code: &str, root: &mut FlatRoot) -> Result<(), Error> {
    let mut statement_chain = FlatStatementChain {
        statements: Vec::new(),
    };

    for child in node.children(&mut node.walk()) {
        if child.kind() == "statement" {
            flatten_statement(child, code, &mut statement_chain)?
        }
    }

    root.push_statement_chain(statement_chain);
    Ok(())
}

fn flatten_statement(
    node: Node,
    code: &str,
    statement_chain: &mut FlatStatementChain,
) -> Result<(), Error> {
    let mut statement = FlatStatement {
        definition: None,
        expression: None,
    };

    for child in node.children(&mut node.walk()) {
        match child.kind() {
            "definition" => flatten_definition(child, code, statement_chain, &mut statement)?,
            "expression" => flatten_expression(child, code, statement_chain, &mut statement)?,
            _ => (),
        }
    }

    statement_chain.push_statement(statement);
    Ok(())
}

fn flatten_definition(
    node: Node,
    code: &str,
    _statement_chain: &mut FlatStatementChain,
    statement: &mut FlatStatement,
) -> Result<(), Error> {
    let mut definition = FlatDefinition {
        name: "".to_string(),
        operation: FlatDefinitionOperation::Constant,
    };

    for child in node.children(&mut node.walk()) {
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

    statement.definition = Some(definition);
    Ok(())
}

fn flatten_expression(
    node: Node,
    code: &str,
    statement_chain: &mut FlatStatementChain,
    statement: &mut FlatStatement,
) -> Result<(), Error> {
    let name = if let Some(ref definition) = statement.definition {
        definition.name.clone()
    } else {
        "temporary".to_string()
    };

    let mut temp_counter = 1;

    // Collect all tokens from the expression in order
    let mut tokens = Vec::new();
    collect_expression_tokens(node, code, &mut tokens)?;

    // If we have a simple expression (just one operand), return it directly
    if tokens.len() == 1 {
        statement.expression = Some(FlatExpression::String(tokens[0].clone()));
        return Ok(());
    }

    // Process high-precedence operations (* and /) first
    let mut processed_tokens = tokens;

    // Continue processing until no more high-precedence operations can be flattened
    loop {
        let mut found_high_precedence = false;
        let mut new_tokens = Vec::new();
        let mut i = 0;

        while i < processed_tokens.len() {
            if i + 2 < processed_tokens.len() {
                let operator = &processed_tokens[i + 1];

                // Check if this is a high-precedence operation that should be extracted
                if (operator == "*" || operator == "/")
                    && should_extract_operation(&processed_tokens, i)
                {
                    // Create intermediate variable
                    let temp_name = format!("{}_{}", name, temp_counter);
                    temp_counter += 1;

                    // Create the intermediate expression
                    let temp_expr = format!(
                        "{} {} {}",
                        processed_tokens[i],
                        processed_tokens[i + 1],
                        processed_tokens[i + 2]
                    );

                    let temp_statement = FlatStatement {
                        definition: Some(FlatDefinition {
                            name: temp_name.clone(),
                            operation: FlatDefinitionOperation::Constant,
                        }),
                        expression: Some(FlatExpression::String(temp_expr)),
                    };
                    statement_chain.push_statement(temp_statement);

                    // Replace the three tokens with the temp variable name
                    new_tokens.push(temp_name);
                    i += 3;
                    found_high_precedence = true;
                } else {
                    new_tokens.push(processed_tokens[i].clone());
                    i += 1;
                }
            } else {
                new_tokens.push(processed_tokens[i].clone());
                i += 1;
            }
        }

        processed_tokens = new_tokens;

        if !found_high_precedence {
            break;
        }
    }

    statement.expression = Some(FlatExpression::String(processed_tokens.join(" ")));
    Ok(())
}

// Collect all tokens (operands and operators) from expression in order
fn collect_expression_tokens(
    node: Node,
    code: &str,
    tokens: &mut Vec<String>,
) -> Result<(), Error> {
    for child in node.children(&mut node.walk()) {
        match child.kind() {
            "number" => {
                let number_text = &code[child.start_byte()..child.end_byte()];
                tokens.push(number_text.to_string());
            }
            "arithmetic" => {
                let op_text = &code[child.start_byte()..child.end_byte()];
                tokens.push(op_text.to_string());
            }
            "identifier_chain" => {
                let id_text = &code[child.start_byte()..child.end_byte()];
                tokens.push(id_text.to_string());
            }
            _ => {
                // Handle other expression types if needed
            }
        }
    }
    Ok(())
}

// Check if a high-precedence operation should be extracted
// Only extract if there's a low-precedence operation elsewhere in the expression
fn should_extract_operation(tokens: &[String], current_pos: usize) -> bool {
    // Look for low-precedence operations (+ or -) in the token list
    for (i, token) in tokens.iter().enumerate() {
        if i != current_pos + 1 && (token == "+" || token == "-") {
            return true;
        }
    }
    false
}
