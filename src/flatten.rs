use tree_sitter::{Node, Tree};

use serde::Serialize;

use crate::Error;

#[derive(Debug, Clone, Serialize)]
pub struct FlatRoot {
    statement_chains: Vec<FlatStatementChain>,
}

impl FlatRoot {
    fn push_statement_chain(&mut self, statement_chain: FlatStatementChain) {
        self.statement_chains.push(statement_chain);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FlatStatementChain {
    statements: Vec<FlatStatement>,
}

impl FlatStatementChain {
    fn push_statement(&mut self, statement: FlatStatement) {
        self.statements.push(statement);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FlatStatement {
    definition: Option<FlatDefinition>,
    expression: Option<FlatExpression>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlatDefinition {
    name: String,
    operation: FlatDefinitionOperation,
}

#[derive(Debug, Clone, Serialize)]
pub enum FlatDefinitionOperation {
    Constant,
    Variable,
}

#[derive(Debug, Clone, Serialize)]
pub enum FlatExpression {
    Number(String),
    String(String),
    Identifier(String),
    BinaryOperation {
        left: String,
        operator: String,
        right: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum FlatExpressionSection {
    Number(String),
    String(String),
}

pub fn flatten_tree(tree: Tree, code: &str) -> Result<FlatRoot, Error> {
    flatten_node(tree.root_node(), code)
}

pub fn flatten_node(node: Node, code: &str) -> Result<FlatRoot, Error> {
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

    Ok(root)
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

    let mut temporary_counter = 1;

    // Collect expression sections from the structured expression
    let mut expression_parts = Vec::new();
    collect_expression_sections(
        node,
        code,
        &mut expression_parts,
        &name,
        &mut temporary_counter,
        statement_chain,
    )?;

    // Process the expression parts to create binary operations
    let flattened_expr = process_expression_parts(
        expression_parts,
        &name,
        &mut temporary_counter,
        statement_chain,
    )?;

    statement.expression = Some(flattened_expr);
    Ok(())
}

// Collect expression sections from the structured expression
fn collect_expression_sections(
    node: Node,
    code: &str,
    parts: &mut Vec<FlatExpression>,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
) -> Result<(), Error> {
    for child in node.children(&mut node.walk()) {
        if child.kind() == "expression_section" {
            let expr_part = process_expression_section(
                child,
                code,
                base_name,
                temporary_counter,
                statement_chain,
            )?;
            parts.push(expr_part);
        }
    }
    Ok(())
}

// Process a single expression section
fn process_expression_section(
    node: Node,
    code: &str,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
) -> Result<FlatExpression, Error> {
    let mut operators = Vec::new();
    let mut operand = None;

    for child in node.children(&mut node.walk()) {
        match child.kind() {
            "arithmetic" => {
                let op_text = &code[child.start_byte()..child.end_byte()];
                operators.push(op_text.to_string());
            }
            "variable" => {
                operand = Some(process_variable(
                    child,
                    code,
                    base_name,
                    temporary_counter,
                    statement_chain,
                )?);
            }
            _ => {}
        }
    }

    // For the first section, there should be no operators, just return the operand
    if operators.is_empty() {
        return operand.ok_or_else(|| {
            Error::FlattenError("No operand found in expression section".to_string())
        });
    }

    // Handle multiple operators by creating intermediate variables for unary parts
    if operators.len() > 1 {
        // Extract the unary part (all operators except the first + operand)
        let unary_operators: Vec<String> = operators.iter().skip(1).cloned().collect();
        let operand_unwrapped = operand.unwrap();

        // Create intermediate variable for the unary expression
        let temporary_name = format!("{}_{}", base_name, temporary_counter);
        *temporary_counter += 1;

        // Process the unary part as a binary operation with implicit zero
        let unary_expr = if unary_operators.len() == 1 {
            // Single unary operator: create "0 operator operand"
            let operand_str = expression_to_string(&operand_unwrapped);
            FlatExpression::BinaryOperation {
                left: "0".to_string(),
                operator: unary_operators[0].clone(),
                right: operand_str,
            }
        } else {
            // Multiple unary operators: process them recursively from right to left
            let operand_str = expression_to_string(&operand_unwrapped);
            let mut current_operand = operand_str;

            // Process all operators except the last one as intermediate statements
            let operators_rev: Vec<&String> = unary_operators.iter().rev().collect();
            for (i, operator) in operators_rev.iter().enumerate() {
                if i == operators_rev.len() - 1 {
                    // Last operator: return as direct binary operation
                    break;
                }

                let temp_name = format!("{}_{}", base_name, temporary_counter);
                *temporary_counter += 1;

                let temp_statement = FlatStatement {
                    definition: Some(FlatDefinition {
                        name: temp_name.clone(),
                        operation: FlatDefinitionOperation::Constant,
                    }),
                    expression: Some(FlatExpression::BinaryOperation {
                        left: "0".to_string(),
                        operator: (*operator).clone(),
                        right: current_operand,
                    }),
                };
                statement_chain.push_statement(temp_statement);
                current_operand = temp_name;
            }

            // Return the final operation as a direct binary operation
            FlatExpression::BinaryOperation {
                left: "0".to_string(),
                operator: (**operators_rev.last().unwrap()).clone(),
                right: current_operand,
            }
        };

        let temporary_statement = FlatStatement {
            definition: Some(FlatDefinition {
                name: temporary_name.clone(),
                operation: FlatDefinitionOperation::Constant,
            }),
            expression: Some(unary_expr),
        };
        statement_chain.push_statement(temporary_statement);

        // Return the first operator with the temporary variable
        Ok(FlatExpression::String(format!(
            "{} {}",
            operators[0], temporary_name
        )))
    } else {
        // Single operator case - return as before
        let operand_str = match operand.unwrap() {
            FlatExpression::Number(n) => n,
            FlatExpression::String(s) => s,
            FlatExpression::Identifier(i) => i,
            FlatExpression::BinaryOperation {
                left,
                operator,
                right,
            } => {
                format!("{} {} {}", left, operator, right)
            }
        };

        Ok(FlatExpression::String(format!(
            "{} {}",
            operators[0], operand_str
        )))
    }
}

// Helper function to convert FlatExpression to string (moved up for use in process_expression_section)
fn expression_to_string(expr: &FlatExpression) -> String {
    match expr {
        FlatExpression::Number(n) => n.clone(),
        FlatExpression::String(s) => s.clone(),
        FlatExpression::Identifier(i) => i.clone(),
        FlatExpression::BinaryOperation {
            left,
            operator,
            right,
        } => {
            format!("{} {} {}", left, operator, right)
        }
    }
}

// Process a variable node (number, identifier_chain, prioritize, etc.)
fn process_variable(
    node: Node,
    code: &str,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
) -> Result<FlatExpression, Error> {
    for child in node.children(&mut node.walk()) {
        match child.kind() {
            "number" => {
                let number_text = &code[child.start_byte()..child.end_byte()];
                return Ok(FlatExpression::Number(number_text.to_string()));
            }
            "identifier_chain" => {
                let id_text = &code[child.start_byte()..child.end_byte()];
                return Ok(FlatExpression::Identifier(id_text.to_string()));
            }
            "string" => {
                let string_text = &code[child.start_byte()..child.end_byte()];
                return Ok(FlatExpression::String(string_text.to_string()));
            }
            "prioritize" => {
                // Always extract prioritized expressions into intermediate variables
                let temporary_name = format!("{}_{}", base_name, temporary_counter);
                *temporary_counter += 1;

                // Process the inner expression
                for inner_child in child.children(&mut child.walk()) {
                    if inner_child.kind() == "expression" {
                        let mut inner_parts = Vec::new();
                        collect_expression_sections(
                            inner_child,
                            code,
                            &mut inner_parts,
                            base_name,
                            temporary_counter,
                            statement_chain,
                        )?;

                        let flattened_inner = if inner_parts.len() == 1 {
                            inner_parts[0].clone()
                        } else {
                            process_expression_parts(
                                inner_parts,
                                base_name,
                                temporary_counter,
                                statement_chain,
                            )?
                        };

                        let temporary_statement = FlatStatement {
                            definition: Some(FlatDefinition {
                                name: temporary_name.clone(),
                                operation: FlatDefinitionOperation::Constant,
                            }),
                            expression: Some(flattened_inner),
                        };
                        statement_chain.push_statement(temporary_statement);

                        return Ok(FlatExpression::Identifier(temporary_name));
                    }
                }
            }
            _ => {}
        }
    }

    Err(Error::FlattenError("Unknown variable type".to_string()))
}

// Process multiple expression parts into binary operations
fn process_expression_parts(
    parts: Vec<FlatExpression>,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
) -> Result<FlatExpression, Error> {
    if parts.len() < 2 {
        let single_part = parts.into_iter().next().unwrap();
        // Check if this single part is a unary operation (starts with operator)
        if let FlatExpression::String(s) = &single_part {
            let part_split: Vec<&str> = s.splitn(2, ' ').collect();
            if part_split.len() == 2 {
                // This is a unary operation like "- 0d1", convert to binary with implicit zero
                return Ok(FlatExpression::BinaryOperation {
                    left: "0".to_string(),
                    operator: part_split[0].to_string(),
                    right: part_split[1].to_string(),
                });
            }
        }
        return Ok(single_part);
    }

    // Convert expression parts into a list of operands and operators
    let mut operands = Vec::new();
    let mut operators = Vec::new();

    // Check if first part starts with an operator (unary operator case)
    let start_index = if let FlatExpression::String(s) = &parts[0] {
        let first_parts: Vec<&str> = s.splitn(2, ' ').collect();
        if first_parts.len() == 2 {
            // First part is "operator operand" - insert implicit zero
            operands.push("0".to_string());
            operators.push(first_parts[0].to_string());
            operands.push(first_parts[1].to_string());
            1 // Start processing from second part
        } else {
            // First part is just an operand
            operands.push(expression_to_string(&parts[0]));
            1 // Start processing from second part
        }
    } else {
        // First part is just an operand
        operands.push(expression_to_string(&parts[0]));
        1 // Start processing from second part
    };

    // Process remaining parts as "operator operand"
    for part in parts.iter().skip(start_index) {
        if let FlatExpression::String(s) = part {
            let part_split: Vec<&str> = s.splitn(2, ' ').collect();
            if part_split.len() == 2 {
                operators.push(part_split[0].to_string());
                operands.push(part_split[1].to_string());
            }
        }
    }

    // Process high-precedence operations first
    while let Some(pos) = find_high_precedence_operation(&operators) {
        if should_extract_high_precedence(&operators) {
            let temporary_name = format!("{}_{}", base_name, temporary_counter);
            *temporary_counter += 1;

            let left = operands[pos].clone();
            let operator = operators[pos].clone();
            let right = operands[pos + 1].clone();

            let temporary_statement = FlatStatement {
                definition: Some(FlatDefinition {
                    name: temporary_name.clone(),
                    operation: FlatDefinitionOperation::Constant,
                }),
                expression: Some(FlatExpression::BinaryOperation {
                    left,
                    operator,
                    right,
                }),
            };
            statement_chain.push_statement(temporary_statement);

            // Replace the three elements with the temporary variable
            operands[pos] = temporary_name;
            operands.remove(pos + 1);
            operators.remove(pos);
        } else {
            break;
        }
    }

    // Process remaining operations left to right
    while operators.len() > 1 {
        let temporary_name = format!("{}_{}", base_name, temporary_counter);
        *temporary_counter += 1;

        let left = operands[0].clone();
        let operator = operators[0].clone();
        let right = operands[1].clone();

        let temporary_statement = FlatStatement {
            definition: Some(FlatDefinition {
                name: temporary_name.clone(),
                operation: FlatDefinitionOperation::Constant,
            }),
            expression: Some(FlatExpression::BinaryOperation {
                left,
                operator,
                right,
            }),
        };
        statement_chain.push_statement(temporary_statement);

        operands[0] = temporary_name;
        operands.remove(1);
        operators.remove(0);
    }

    // For the final operation, return BinaryOperation directly instead of creating temporary
    if operators.len() == 1 {
        let left = operands[0].clone();
        let operator = operators[0].clone();
        let right = operands[1].clone();

        Ok(FlatExpression::BinaryOperation {
            left,
            operator,
            right,
        })
    } else {
        Ok(FlatExpression::Identifier(operands[0].clone()))
    }
}

// Helper functions

fn find_high_precedence_operation(operators: &[String]) -> Option<usize> {
    operators.iter().position(|op| op == "*" || op == "/")
}

fn should_extract_high_precedence(operators: &[String]) -> bool {
    operators.iter().any(|op| op == "+" || op == "-")
}
