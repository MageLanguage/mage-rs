use serde::{Deserialize, Serialize};

use tree_sitter::{Language, Node, Tree};

use crate::Error;

fn get_node_kind_ids() -> NodeKindIDs {
    let language = Language::from(tree_sitter_mage::LANGUAGE);

    NodeKindIDs {
        source_file: language.id_for_node_kind("source_file", true),
        source: language.id_for_node_kind("source", true),
        statement_chain: language.id_for_node_kind("statement_chain", true),
        statement: language.id_for_node_kind("statement", true),
        definition: language.id_for_node_kind("definition", true),
        expression: language.id_for_node_kind("expression", true),
        identifier_chain: language.id_for_node_kind("identifier_chain", true),
        identifier: language.id_for_node_kind("identifier", true),
        call: language.id_for_node_kind("call", true),

        definition_operation: language.id_for_node_kind("definition_operation", true),
        arithmetic: language.id_for_node_kind("arithmetic", true),
        variable: language.id_for_node_kind("variable", true),
        number: language.id_for_node_kind("number", true),
        string: language.id_for_node_kind("string", true),
        prioritize: language.id_for_node_kind("prioritize", true),
        expression_section: language.id_for_node_kind("expression_section", true),
    }
}

// Struct to hold all node kind IDs
struct NodeKindIDs {
    source_file: u16,
    source: u16,
    statement_chain: u16,
    statement: u16,
    definition: u16,
    expression: u16,
    identifier_chain: u16,
    identifier: u16,
    call: u16,

    definition_operation: u16,
    arithmetic: u16,
    variable: u16,
    number: u16,
    string: u16,
    prioritize: u16,
    expression_section: u16,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatRoot {
    pub statement_chains: Vec<FlatStatementChain>,
}

impl FlatRoot {
    fn push_statement_chain(&mut self, statement_chain: FlatStatementChain) {
        self.statement_chains.push(statement_chain);
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatStatementChain {
    pub statements: Vec<FlatStatement>,
}

impl FlatStatementChain {
    fn push_statement(&mut self, statement: FlatStatement) {
        self.statements.push(statement);
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatStatement {
    pub definition: Option<FlatDefinition>,
    pub expression: Option<FlatExpression>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatDefinition {
    pub name: String,
    pub operation: FlatDefinitionOperation,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatDefinitionOperation {
    Constant,
    Variable,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatArithmetic {
    Add,
    Substract,
    Multiply,
    Divide,
    Modulo,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum FlatVariable {
    Number(String),
    String(String),
    Identifier(String),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatExpression {
    pub one: FlatVariable,
    pub two: Option<FlatVariable>,
    pub arithmetic: Option<FlatArithmetic>,
}

// Helper function to convert string operator to FlatArithmetic
fn string_to_arithmetic(op: &str) -> Option<FlatArithmetic> {
    match op {
        "+" => Some(FlatArithmetic::Add),
        "-" => Some(FlatArithmetic::Substract),
        "*" => Some(FlatArithmetic::Multiply),
        "/" => Some(FlatArithmetic::Divide),
        "%" => Some(FlatArithmetic::Modulo),
        _ => None,
    }
}

// Helper function to create a simple variable expression
fn create_variable_expression(variable: FlatVariable) -> FlatExpression {
    FlatExpression {
        one: variable,
        two: None,
        arithmetic: None,
    }
}

// Helper function to create a binary expression
fn create_binary_expression(
    left: FlatVariable,
    op: FlatArithmetic,
    right: FlatVariable,
) -> FlatExpression {
    FlatExpression {
        one: left,
        two: Some(right),
        arithmetic: Some(op),
    }
}

// Helper function to convert string to FlatVariable
fn string_to_variable(s: &str) -> FlatVariable {
    // Check if it's a number (starts with digit or 0x/0b/0o/0d)
    if s.chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
        || s.starts_with("0")
    {
        FlatVariable::Number(s.to_string())
    } else if s.starts_with('"') && s.ends_with('"') {
        FlatVariable::String(s.to_string())
    } else {
        FlatVariable::Identifier(s.to_string())
    }
}

// Helper function to convert FlatVariable to string
fn variable_to_string(var: &FlatVariable) -> String {
    match var {
        FlatVariable::Number(n) => n.clone(),
        FlatVariable::String(s) => s.clone(),
        FlatVariable::Identifier(i) => i.clone(),
    }
}

// Helper function to convert FlatExpression to string
fn expression_to_string_simple(expr: &FlatExpression) -> String {
    match (&expr.two, &expr.arithmetic) {
        (Some(right), Some(op)) => {
            let op_str = match op {
                FlatArithmetic::Add => "+",
                FlatArithmetic::Substract => "-",
                FlatArithmetic::Multiply => "*",
                FlatArithmetic::Divide => "/",
                FlatArithmetic::Modulo => "%",
            };
            format!(
                "{} {} {}",
                variable_to_string(&expr.one),
                op_str,
                variable_to_string(right)
            )
        }
        _ => variable_to_string(&expr.one),
    }
}

pub fn flatten_tree(tree: Tree, code: &str) -> Result<FlatRoot, Error> {
    let kinds = get_node_kind_ids();
    flatten_node(tree.root_node(), code, &kinds)
}

fn flatten_node(node: Node, code: &str, node_kind_ids: &NodeKindIDs) -> Result<FlatRoot, Error> {
    let mut root = FlatRoot {
        statement_chains: Vec::new(),
    };

    if node.kind_id() == node_kind_ids.source_file || node.kind_id() == node_kind_ids.source {
        for child in node.children(&mut node.walk()) {
            if child.kind_id() == node_kind_ids.statement_chain {
                flatten_statement_chain(child, code, &mut root, node_kind_ids)?;
            }
        }
    }

    Ok(root)
}

fn flatten_statement_chain(
    node: Node,
    code: &str,
    root: &mut FlatRoot,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), Error> {
    let mut statement_chain = FlatStatementChain {
        statements: Vec::new(),
    };

    for child in node.children(&mut node.walk()) {
        if child.kind_id() == node_kind_ids.statement {
            flatten_statement(child, code, &mut statement_chain, node_kind_ids)?
        }
    }

    root.push_statement_chain(statement_chain);
    Ok(())
}

fn flatten_statement(
    node: Node,
    code: &str,
    statement_chain: &mut FlatStatementChain,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), Error> {
    let mut statement = FlatStatement {
        definition: None,
        expression: None,
    };

    for child in node.children(&mut node.walk()) {
        match child.kind_id() {
            id if id == node_kind_ids.definition => {
                flatten_definition(child, code, statement_chain, &mut statement, node_kind_ids)?
            }
            id if id == node_kind_ids.expression => {
                flatten_expression(child, code, statement_chain, &mut statement, node_kind_ids)?
            }
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
    node_kind_ids: &NodeKindIDs,
) -> Result<(), Error> {
    let mut definition = FlatDefinition {
        name: "".to_string(),
        operation: FlatDefinitionOperation::Constant,
    };

    for child in node.children(&mut node.walk()) {
        let text = &code[child.start_byte()..child.end_byte()];

        match child.kind_id() {
            id if id == node_kind_ids.identifier_chain => {
                definition.name = text.to_string();
            }
            id if id == node_kind_ids.definition_operation => {
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
    node_kind_ids: &NodeKindIDs,
) -> Result<(), Error> {
    let name = if let Some(ref definition) = statement.definition {
        definition.name.clone()
    } else {
        "temporary".to_string()
    };

    let mut temporary_counter = 1;

    // Count expression sections to determine if this is a simple expression
    let section_count = node
        .children(&mut node.walk())
        .filter(|child| child.kind_id() == node_kind_ids.expression_section)
        .count();

    // Force call extraction if there are multiple sections (indicating arithmetic)
    let force_call_extraction = section_count > 1;

    // Collect expression sections from the structured expression
    let mut expression_parts = Vec::new();
    collect_expression_sections(
        node,
        code,
        &mut expression_parts,
        &name,
        &mut temporary_counter,
        statement_chain,
        node_kind_ids,
        force_call_extraction,
    )?;

    // Process the expression parts to create binary operations
    let flattened_expr = process_expression_parts(
        expression_parts,
        &name,
        &mut temporary_counter,
        statement_chain,
        node_kind_ids,
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
    node_kind_ids: &NodeKindIDs,
    force_call_extraction: bool,
) -> Result<(), Error> {
    for child in node.children(&mut node.walk()) {
        if child.kind_id() == node_kind_ids.expression_section {
            let expr_part = process_expression_section(
                child,
                code,
                base_name,
                temporary_counter,
                statement_chain,
                node_kind_ids,
                force_call_extraction,
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
    node_kind_ids: &NodeKindIDs,
    force_call_extraction: bool,
) -> Result<FlatExpression, Error> {
    let mut operators = Vec::new();
    let mut operand = None;

    for child in node.children(&mut node.walk()) {
        match child.kind_id() {
            id if id == node_kind_ids.arithmetic => {
                let op_text = &code[child.start_byte()..child.end_byte()];
                operators.push(op_text.to_string());
            }
            id if id == node_kind_ids.variable => {
                operand = Some(process_variable(
                    child,
                    code,
                    base_name,
                    temporary_counter,
                    statement_chain,
                    node_kind_ids,
                    force_call_extraction,
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
            if let Some(op) = string_to_arithmetic(&unary_operators[0]) {
                create_binary_expression(
                    FlatVariable::Number("0".to_string()),
                    op,
                    string_to_variable(&operand_str),
                )
            } else {
                return Err(Error::FlattenError(format!(
                    "Unknown operator: {}",
                    unary_operators[0]
                )));
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
                    expression: Some(if let Some(op) = string_to_arithmetic(operator) {
                        create_binary_expression(
                            FlatVariable::Number("0".to_string()),
                            op,
                            string_to_variable(&current_operand),
                        )
                    } else {
                        return Err(Error::FlattenError(format!(
                            "Unknown operator: {}",
                            operator
                        )));
                    }),
                };
                statement_chain.push_statement(temp_statement);
                current_operand = temp_name;
            }

            // Return the final operation as a direct binary operation
            if let Some(op) = string_to_arithmetic(operators_rev.last().unwrap()) {
                create_binary_expression(
                    FlatVariable::Number("0".to_string()),
                    op,
                    string_to_variable(&current_operand),
                )
            } else {
                return Err(Error::FlattenError(format!(
                    "Unknown operator: {}",
                    operators_rev.last().unwrap()
                )));
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
        Ok(create_variable_expression(FlatVariable::String(format!(
            "{} {}",
            operators[0], temporary_name
        ))))
    } else {
        // Single operator case - return as before
        let operand_str = expression_to_string_simple(&operand.unwrap());

        Ok(create_variable_expression(FlatVariable::String(format!(
            "{} {}",
            operators[0], operand_str
        ))))
    }
}

// Helper function to convert FlatExpression to string (moved up for use in process_expression_section)
fn expression_to_string(expr: &FlatExpression) -> String {
    expression_to_string_simple(expr)
}

// Process a variable node (number, identifier_chain, prioritize, etc.)
fn process_variable(
    node: Node,
    code: &str,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
    node_kind_ids: &NodeKindIDs,
    force_call_extraction: bool,
) -> Result<FlatExpression, Error> {
    for child in node.children(&mut node.walk()) {
        match child.kind_id() {
            id if id == node_kind_ids.number => {
                let number_text = &code[child.start_byte()..child.end_byte()];
                return Ok(create_variable_expression(FlatVariable::Number(
                    number_text.to_string(),
                )));
            }
            id if id == node_kind_ids.identifier_chain => {
                return process_identifier_chain(
                    child,
                    code,
                    base_name,
                    temporary_counter,
                    statement_chain,
                    node_kind_ids,
                    force_call_extraction,
                );
            }
            id if id == node_kind_ids.string => {
                let string_text = &code[child.start_byte()..child.end_byte()];
                return Ok(create_variable_expression(FlatVariable::String(
                    string_text.to_string(),
                )));
            }
            id if id == node_kind_ids.prioritize => {
                // Always extract prioritized expressions into intermediate variables
                let temporary_name = format!("{}_{}", base_name, temporary_counter);
                *temporary_counter += 1;

                // Process the inner expression
                for inner_child in child.children(&mut child.walk()) {
                    if inner_child.kind_id() == node_kind_ids.expression {
                        let mut inner_parts = Vec::new();
                        collect_expression_sections(
                            inner_child,
                            code,
                            &mut inner_parts,
                            base_name,
                            temporary_counter,
                            statement_chain,
                            node_kind_ids,
                            true, // Always force extraction for prioritized expressions
                        )?;

                        let flattened_inner = if inner_parts.len() == 1 {
                            inner_parts[0].clone()
                        } else {
                            process_expression_parts(
                                inner_parts,
                                base_name,
                                temporary_counter,
                                statement_chain,
                                node_kind_ids,
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

                        return Ok(create_variable_expression(FlatVariable::Identifier(
                            temporary_name,
                        )));
                    }
                }
            }
            _ => {}
        }
    }

    Err(Error::FlattenError("Unknown variable type".to_string()))
}

// Process identifier chain, extracting function calls into temporary variables
fn process_identifier_chain(
    node: Node,
    code: &str,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
    node_kind_ids: &NodeKindIDs,
    force_call_extraction: bool,
) -> Result<FlatExpression, Error> {
    let mut chain_parts = Vec::new();

    // Collect all identifiers in the chain
    for child in node.children(&mut node.walk()) {
        if child.kind_id() == node_kind_ids.identifier {
            chain_parts.push(child);
        }
    }

    if chain_parts.is_empty() {
        return Err(Error::FlattenError("Empty identifier chain".to_string()));
    }

    // Only extract calls if there are multiple parts in the chain
    if chain_parts.len() > 1 {
        // Find the first call in the chain
        let mut first_call_index = None;
        for (i, identifier_node) in chain_parts.iter().enumerate() {
            if has_call_in_identifier(identifier_node, node_kind_ids)? {
                first_call_index = Some(i);
                break;
            }
        }

        if let Some(call_index) = first_call_index {
            // Extract the call into a temporary variable
            let temporary_name = format!("{}_{}", base_name, temporary_counter);
            *temporary_counter += 1;

            // Process the call with its arguments
            let processed_call = process_call_with_arguments(
                chain_parts[call_index],
                code,
                base_name,
                temporary_counter,
                statement_chain,
                node_kind_ids,
                force_call_extraction,
            )?;

            // Build the prefix (parts before the call)
            let mut prefix_parts = Vec::new();
            for i in 0..call_index {
                let part_text =
                    code[chain_parts[i].start_byte()..chain_parts[i].end_byte()].to_string();
                prefix_parts.push(part_text);
            }

            // Create the call expression
            let mut call_parts = prefix_parts;
            call_parts.push(processed_call);
            let call_expression = call_parts.join(".");

            // Create temporary statement for the call
            let temporary_statement = FlatStatement {
                definition: Some(FlatDefinition {
                    name: temporary_name.clone(),
                    operation: FlatDefinitionOperation::Constant,
                }),
                expression: Some(create_variable_expression(FlatVariable::Identifier(
                    call_expression,
                ))),
            };
            statement_chain.push_statement(temporary_statement);

            // Build the suffix (parts after the call)
            let remaining_parts = &chain_parts[call_index + 1..];
            if remaining_parts.is_empty() {
                Ok(create_variable_expression(FlatVariable::Identifier(
                    temporary_name,
                )))
            } else {
                let mut suffix_parts = Vec::new();
                for identifier_node in remaining_parts {
                    let processed_part = process_identifier_part_with_calls(
                        *identifier_node,
                        code,
                        base_name,
                        temporary_counter,
                        statement_chain,
                        node_kind_ids,
                        force_call_extraction,
                    )?;
                    suffix_parts.push(processed_part);
                }

                let final_identifier = format!("{}.{}", temporary_name, suffix_parts.join("."));
                Ok(create_variable_expression(FlatVariable::Identifier(
                    final_identifier,
                )))
            }
        } else {
            // No calls found in multi-part chain, return as-is
            let id_text = code[node.start_byte()..node.end_byte()].to_string();
            Ok(create_variable_expression(FlatVariable::Identifier(
                id_text,
            )))
        }
    } else {
        // Single identifier - check if it contains nested calls like j()()
        if has_nested_calls_in_single_identifier(&chain_parts[0], code, node_kind_ids)? {
            return process_nested_call(
                chain_parts[0],
                code,
                base_name,
                temporary_counter,
                statement_chain,
                node_kind_ids,
            );
        }

        // Check if it contains any calls and extract them only if forced
        if force_call_extraction && has_call_in_identifier(&chain_parts[0], node_kind_ids)? {
            let processed_call = process_call_with_arguments(
                chain_parts[0],
                code,
                base_name,
                temporary_counter,
                statement_chain,
                node_kind_ids,
                force_call_extraction,
            )?;

            let temporary_name = format!("{}_{}", base_name, temporary_counter);
            *temporary_counter += 1;

            let temporary_statement = FlatStatement {
                definition: Some(FlatDefinition {
                    name: temporary_name.clone(),
                    operation: FlatDefinitionOperation::Constant,
                }),
                expression: Some(create_variable_expression(FlatVariable::Identifier(
                    processed_call,
                ))),
            };
            statement_chain.push_statement(temporary_statement);

            Ok(create_variable_expression(FlatVariable::Identifier(
                temporary_name,
            )))
        } else {
            // Simple single identifier, return as-is
            let id_text = code[node.start_byte()..node.end_byte()].to_string();
            Ok(create_variable_expression(FlatVariable::Identifier(
                id_text,
            )))
        }
    }
}

// Check if a single identifier has a call
fn has_call_in_identifier(
    identifier_node: &Node,
    node_kind_ids: &NodeKindIDs,
) -> Result<bool, Error> {
    for identifier_child in identifier_node.children(&mut identifier_node.walk()) {
        if identifier_child.kind_id() == node_kind_ids.call {
            return Ok(true);
        }
    }
    Ok(false)
}

// Check if a single identifier contains nested calls like j()()
fn has_nested_calls_in_single_identifier(
    identifier_node: &Node,
    code: &str,
    _node_kind_ids: &NodeKindIDs,
) -> Result<bool, Error> {
    let id_text = code[identifier_node.start_byte()..identifier_node.end_byte()].to_string();
    // Simple heuristic: if the identifier contains ")(" it likely has nested calls
    Ok(id_text.contains(")("))
}

// Process nested calls like j()() -> j_1 : j(); result : j_1()
fn process_nested_call(
    identifier_node: Node,
    code: &str,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
    _node_kind_ids: &NodeKindIDs,
) -> Result<FlatExpression, Error> {
    let id_text = code[identifier_node.start_byte()..identifier_node.end_byte()].to_string();

    // Find the first ")(" pattern to split on
    if let Some(split_pos) = id_text.find(")(") {
        let first_part = &id_text[..split_pos + 1]; // Include the first ")"
        let second_part = &id_text[split_pos + 1..]; // Start from "("

        let temporary_name = format!("{}_{}", base_name, temporary_counter);
        *temporary_counter += 1;

        let temporary_statement = FlatStatement {
            definition: Some(FlatDefinition {
                name: temporary_name.clone(),
                operation: FlatDefinitionOperation::Constant,
            }),
            expression: Some(create_variable_expression(FlatVariable::Identifier(
                first_part.to_string(),
            ))),
        };
        statement_chain.push_statement(temporary_statement);

        let final_identifier = format!("{}{}", temporary_name, second_part);
        Ok(create_variable_expression(FlatVariable::Identifier(
            final_identifier,
        )))
    } else {
        // Fallback - return as is
        Ok(create_variable_expression(FlatVariable::Identifier(
            id_text,
        )))
    }
}

// Process a single identifier part that may contain calls with arguments
fn process_identifier_part_with_calls(
    identifier_node: Node,
    code: &str,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
    node_kind_ids: &NodeKindIDs,
    force_call_extraction: bool,
) -> Result<String, Error> {
    if has_call_in_identifier(&identifier_node, node_kind_ids)? {
        process_call_with_arguments(
            identifier_node,
            code,
            base_name,
            temporary_counter,
            statement_chain,
            node_kind_ids,
            force_call_extraction,
        )
    } else {
        Ok(code[identifier_node.start_byte()..identifier_node.end_byte()].to_string())
    }
}

// Process a call with its arguments, extracting nested calls from arguments
fn process_call_with_arguments(
    identifier_node: Node,
    code: &str,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
    node_kind_ids: &NodeKindIDs,
    _force_call_extraction: bool,
) -> Result<String, Error> {
    // Find the call node within the identifier
    for identifier_child in identifier_node.children(&mut identifier_node.walk()) {
        if identifier_child.kind_id() == node_kind_ids.call {
            return process_call_node(
                identifier_child,
                code,
                base_name,
                temporary_counter,
                statement_chain,
                node_kind_ids,
            );
        }
    }

    // If no call found, return the text as-is
    Ok(code[identifier_node.start_byte()..identifier_node.end_byte()].to_string())
}

// Process a call node, handling arguments that may contain nested calls
fn process_call_node(
    call_node: Node,
    code: &str,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
    node_kind_ids: &NodeKindIDs,
) -> Result<String, Error> {
    let mut call_identifier = String::new();
    let mut processed_args = Vec::new();
    let mut found_identifier = false;

    // Process call children to extract identifier and arguments
    for call_child in call_node.children(&mut call_node.walk()) {
        if call_child.kind_id() == node_kind_ids.identifier {
            // Process the identifier part (which might itself be a call)
            call_identifier = process_identifier_part_with_calls(
                call_child,
                code,
                base_name,
                temporary_counter,
                statement_chain,
                node_kind_ids,
                true, // Force extraction for nested calls
            )?;
            found_identifier = true;
        } else if call_child.kind_id() == node_kind_ids.expression {
            // Process expression argument, extracting any nested calls
            let processed_arg = process_call_argument(
                call_child,
                code,
                base_name,
                temporary_counter,
                statement_chain,
                node_kind_ids,
            )?;
            processed_args.push(processed_arg);
        }
    }

    if !found_identifier {
        return Err(Error::FlattenError("Call without identifier".to_string()));
    }

    // Reconstruct the call with processed arguments
    if processed_args.is_empty() {
        Ok(format!("{}()", call_identifier))
    } else {
        Ok(format!(
            "{}({})",
            call_identifier,
            processed_args.join(", ")
        ))
    }
}

// Process a call argument (expression), extracting any nested calls
fn process_call_argument(
    expression_node: Node,
    code: &str,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
    node_kind_ids: &NodeKindIDs,
) -> Result<String, Error> {
    // Process the expression, which will extract any nested calls
    let mut expression_parts = Vec::new();

    collect_expression_sections(
        expression_node,
        code,
        &mut expression_parts,
        base_name,
        temporary_counter,
        statement_chain,
        node_kind_ids,
        true, // Always force call extraction in arguments
    )?;

    if expression_parts.is_empty() {
        return Err(Error::FlattenError(
            "Empty expression in call argument".to_string(),
        ));
    }

    if expression_parts.len() == 1 {
        // Simple argument - check if it needs to be extracted
        let arg_expr = &expression_parts[0];
        if let FlatVariable::Identifier(id) = &arg_expr.one {
            if id.contains('(') && id.contains(')') {
                // This is a call that was extracted, create a temporary for it
                let temp_name = format!("{}_{}", base_name, temporary_counter);
                *temporary_counter += 1;

                let temp_statement = FlatStatement {
                    definition: Some(FlatDefinition {
                        name: temp_name.clone(),
                        operation: FlatDefinitionOperation::Constant,
                    }),
                    expression: Some(arg_expr.clone()),
                };
                statement_chain.push_statement(temp_statement);

                Ok(temp_name)
            } else {
                Ok(id.clone())
            }
        } else {
            // Non-identifier argument (number, string, etc.)
            Ok(expression_to_string_simple(arg_expr))
        }
    } else {
        // Complex argument with multiple parts - process as binary operations
        let flattened_expr = process_expression_parts(
            expression_parts,
            base_name,
            temporary_counter,
            statement_chain,
            node_kind_ids,
        )?;

        // Create a temporary for the complex argument
        let temp_name = format!("{}_{}", base_name, temporary_counter);
        *temporary_counter += 1;

        let temp_statement = FlatStatement {
            definition: Some(FlatDefinition {
                name: temp_name.clone(),
                operation: FlatDefinitionOperation::Constant,
            }),
            expression: Some(flattened_expr),
        };
        statement_chain.push_statement(temp_statement);

        Ok(temp_name)
    }
}

// Process multiple expression parts into binary operations
fn process_expression_parts(
    parts: Vec<FlatExpression>,
    base_name: &str,
    temporary_counter: &mut usize,
    statement_chain: &mut FlatStatementChain,
    _kinds: &NodeKindIDs,
) -> Result<FlatExpression, Error> {
    if parts.len() < 2 {
        let single_part = parts.into_iter().next().unwrap();
        // Check if this single part is a unary operation (starts with operator)
        if let FlatVariable::String(s) = &single_part.one {
            let part_split: Vec<&str> = s.splitn(2, ' ').collect();
            if part_split.len() == 2 {
                // This is a unary operation like "- 0d1", convert to binary with implicit zero
                if let Some(op) = string_to_arithmetic(part_split[0]) {
                    return Ok(create_binary_expression(
                        FlatVariable::Number("0".to_string()),
                        op,
                        string_to_variable(part_split[1]),
                    ));
                }
            }
        }
        return Ok(single_part);
    }

    // Convert expression parts into a list of operands and operators
    let mut operands = Vec::new();
    let mut operators = Vec::new();

    // Check if first part starts with an operator (unary operator case)
    let start_index = if let FlatVariable::String(s) = &parts[0].one {
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
        if let FlatVariable::String(s) = &part.one {
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
                expression: Some(if let Some(op) = string_to_arithmetic(&operator) {
                    create_binary_expression(
                        string_to_variable(&left),
                        op,
                        string_to_variable(&right),
                    )
                } else {
                    return Err(Error::FlattenError(format!(
                        "Unknown operator: {}",
                        operator
                    )));
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
            expression: Some(if let Some(op) = string_to_arithmetic(&operator) {
                create_binary_expression(string_to_variable(&left), op, string_to_variable(&right))
            } else {
                return Err(Error::FlattenError(format!(
                    "Unknown operator: {}",
                    operator
                )));
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

        if let Some(op) = string_to_arithmetic(&operator) {
            Ok(create_binary_expression(
                string_to_variable(&left),
                op,
                string_to_variable(&right),
            ))
        } else {
            Err(Error::FlattenError(format!(
                "Unknown operator: {}",
                operator
            )))
        }
    } else {
        Ok(create_variable_expression(FlatVariable::Identifier(
            operands[0].clone(),
        )))
    }
}

// Helper functions

fn find_high_precedence_operation(operators: &[String]) -> Option<usize> {
    operators.iter().position(|op| op == "*" || op == "/")
}

fn should_extract_high_precedence(operators: &[String]) -> bool {
    operators.iter().any(|op| op == "+" || op == "-")
}
