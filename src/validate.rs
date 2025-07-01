use tree_sitter::{Language, Node, Tree};

use crate::{Error, ValidationError};

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

pub fn validate_tree(tree: Tree, code: &str) -> Result<(), Error> {
    let kinds = get_node_kind_ids();
    validate_node(tree.root_node(), code, &kinds).map_err(|e| Error::ValidationError(e))
}

fn validate_node(
    node: Node,
    code: &str,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), ValidationError> {
    // Check for source blocks which currently cause issues
    if node.kind_id() == node_kind_ids.source {
        return Err(ValidationError::UnsupportedSourceBlock);
    }

    if node.kind_id() == node_kind_ids.source_file {
        for child in node.children(&mut node.walk()) {
            if child.kind_id() == node_kind_ids.statement_chain {
                validate_statement_chain(child, code, node_kind_ids)?;
            } else if child.kind_id() == node_kind_ids.source {
                return Err(ValidationError::UnsupportedSourceBlock);
            }
        }
    }

    Ok(())
}

fn validate_statement_chain(
    node: Node,
    code: &str,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), ValidationError> {
    for child in node.children(&mut node.walk()) {
        if child.kind_id() == node_kind_ids.statement {
            validate_statement(child, code, node_kind_ids)?;
        }
    }
    Ok(())
}

fn validate_statement(
    node: Node,
    code: &str,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), ValidationError> {
    let mut has_expression = false;

    for child in node.children(&mut node.walk()) {
        match child.kind_id() {
            id if id == node_kind_ids.definition => {
                validate_definition(child, code, node_kind_ids)?;
            }
            id if id == node_kind_ids.expression => {
                validate_expression(child, code, node_kind_ids)?;
                has_expression = true;
            }
            _ => (),
        }
    }

    // Check for statements with definitions but no expressions (empty expressions)
    if !has_expression {
        return Err(ValidationError::EmptyExpression);
    }

    Ok(())
}

fn validate_definition(
    node: Node,
    code: &str,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), ValidationError> {
    let mut has_name = false;
    let mut has_operation = false;

    for child in node.children(&mut node.walk()) {
        match child.kind_id() {
            id if id == node_kind_ids.identifier_chain => {
                validate_identifier_chain(child, code, node_kind_ids)?;
                has_name = true;
            }
            id if id == node_kind_ids.definition_operation => {
                has_operation = true;
            }
            _ => (),
        }
    }

    if !has_name || !has_operation {
        return Err(ValidationError::MalformedFunctionCall(
            "Definition missing name or operation".to_string(),
        ));
    }

    Ok(())
}

fn validate_expression(
    node: Node,
    code: &str,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), ValidationError> {
    let mut has_content = false;

    for child in node.children(&mut node.walk()) {
        if child.kind_id() == node_kind_ids.expression_section {
            validate_expression_section(child, code, node_kind_ids)?;
            has_content = true;
        }
    }

    if !has_content {
        return Err(ValidationError::EmptyExpression);
    }

    Ok(())
}

fn validate_expression_section(
    node: Node,
    code: &str,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), ValidationError> {
    let mut operator_count = 0;
    let mut has_variable = false;

    for child in node.children(&mut node.walk()) {
        match child.kind_id() {
            id if id == node_kind_ids.arithmetic => {
                let op_text = &code[child.start_byte()..child.end_byte()];

                // Check for division by zero patterns
                validate_arithmetic_operator(op_text)?;
                operator_count += 1;
            }
            id if id == node_kind_ids.variable => {
                validate_variable(child, code, node_kind_ids)?;
                has_variable = true;
            }
            _ => {}
        }
    }

    // Check for incomplete operator sequences (operators without operands)
    if operator_count > 0 && !has_variable {
        return Err(ValidationError::IncompleteOperatorSequence);
    }

    Ok(())
}

fn validate_arithmetic_operator(op: &str) -> Result<(), ValidationError> {
    match op {
        "+" | "-" | "*" | "/" | "%" => Ok(()),
        _ => Err(ValidationError::IncompleteOperatorSequence),
    }
}

fn validate_variable(
    node: Node,
    code: &str,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), ValidationError> {
    for child in node.children(&mut node.walk()) {
        match child.kind_id() {
            id if id == node_kind_ids.number => {
                validate_number_format(child, code)?;
            }
            id if id == node_kind_ids.identifier_chain => {
                validate_identifier_chain(child, code, node_kind_ids)?;
            }
            id if id == node_kind_ids.string => {
                validate_string(child, code)?;
            }
            id if id == node_kind_ids.prioritize => {
                validate_prioritize(child, code, node_kind_ids)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_number_format(node: Node, code: &str) -> Result<(), ValidationError> {
    let number_text = &code[node.start_byte()..node.end_byte()];

    // Validate different number formats
    if number_text == "0" {
        return Ok(()); // Plain zero is always valid
    }

    if number_text.starts_with("0b") || number_text.starts_with("0B") {
        validate_binary_number(number_text)?;
    } else if number_text.starts_with("0o") || number_text.starts_with("0O") {
        validate_octal_number(number_text)?;
    } else if number_text.starts_with("0d") || number_text.starts_with("0D") {
        validate_decimal_number(number_text)?;
    } else if number_text.starts_with("0x") || number_text.starts_with("0X") {
        validate_hex_number(number_text)?;
    } else if number_text.starts_with("0") && number_text.len() > 1 {
        // Invalid format starting with 0 but not matching any pattern
        return Err(ValidationError::InvalidNumberFormat(
            number_text.to_string(),
        ));
    }

    Ok(())
}

fn validate_binary_number(number: &str) -> Result<(), ValidationError> {
    if number.len() <= 2 {
        return Err(ValidationError::InvalidNumberFormat(number.to_string()));
    }

    let digits = &number[2..];
    if digits.is_empty() || !digits.chars().all(|c| c == '0' || c == '1') {
        return Err(ValidationError::InvalidNumberFormat(number.to_string()));
    }

    Ok(())
}

fn validate_octal_number(number: &str) -> Result<(), ValidationError> {
    if number.len() <= 2 {
        return Err(ValidationError::InvalidNumberFormat(number.to_string()));
    }

    let digits = &number[2..];
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit() && c <= '7') {
        return Err(ValidationError::InvalidNumberFormat(number.to_string()));
    }

    Ok(())
}

fn validate_decimal_number(number: &str) -> Result<(), ValidationError> {
    if number.len() <= 2 {
        return Err(ValidationError::InvalidNumberFormat(number.to_string()));
    }

    let digits = &number[2..];
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(ValidationError::InvalidNumberFormat(number.to_string()));
    }

    Ok(())
}

fn validate_hex_number(number: &str) -> Result<(), ValidationError> {
    if number.len() <= 2 {
        return Err(ValidationError::InvalidNumberFormat(number.to_string()));
    }

    let digits = &number[2..];
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ValidationError::InvalidNumberFormat(number.to_string()));
    }

    Ok(())
}

fn validate_string(node: Node, code: &str) -> Result<(), ValidationError> {
    let string_text = &code[node.start_byte()..node.end_byte()];

    // Basic string validation - must start and end with quotes
    if !string_text.starts_with('"') || !string_text.ends_with('"') {
        return Err(ValidationError::MalformedFunctionCall(format!(
            "Invalid string format: {}",
            string_text
        )));
    }

    Ok(())
}

fn validate_prioritize(
    node: Node,
    code: &str,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), ValidationError> {
    let mut has_expression = false;

    for child in node.children(&mut node.walk()) {
        if child.kind_id() == node_kind_ids.expression {
            validate_expression(child, code, node_kind_ids)?;
            has_expression = true;
        }
    }

    // Empty prioritization brackets [] are not allowed
    if !has_expression {
        return Err(ValidationError::EmptyExpression);
    }

    Ok(())
}

fn validate_identifier_chain(
    node: Node,
    code: &str,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), ValidationError> {
    let chain_text = &code[node.start_byte()..node.end_byte()];

    // Check for malformed chains ending with dot
    if chain_text.ends_with('.') {
        return Err(ValidationError::InvalidIdentifierChain(
            chain_text.to_string(),
        ));
    }

    // Check for chains starting with dot
    if chain_text.starts_with('.') {
        return Err(ValidationError::InvalidIdentifierChain(
            chain_text.to_string(),
        ));
    }

    // Validate individual identifiers in the chain
    for child in node.children(&mut node.walk()) {
        if child.kind_id() == node_kind_ids.identifier {
            validate_identifier(child, code, node_kind_ids)?;
        }
    }

    Ok(())
}

fn validate_identifier(
    node: Node,
    code: &str,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), ValidationError> {
    let identifier_text = &code[node.start_byte()..node.end_byte()];

    // Check if this identifier looks like a malformed number
    validate_identifier_as_number(identifier_text)?;

    // Check for function calls
    for child in node.children(&mut node.walk()) {
        if child.kind_id() == node_kind_ids.call {
            validate_function_call(child, code, node_kind_ids)?;
        }
    }

    Ok(())
}

fn validate_function_call(
    node: Node,
    code: &str,
    node_kind_ids: &NodeKindIDs,
) -> Result<(), ValidationError> {
    let call_text = &code[node.start_byte()..node.end_byte()];

    // Check for malformed function calls like "func(" without closing paren
    if call_text.contains('(') && !call_text.contains(')') {
        return Err(ValidationError::MalformedFunctionCall(
            call_text.to_string(),
        ));
    }

    // Validate function call arguments
    for child in node.children(&mut node.walk()) {
        if child.kind_id() == node_kind_ids.expression {
            validate_expression(child, code, node_kind_ids)?;
        }
    }

    Ok(())
}

// Check if an identifier looks like a malformed number format
fn validate_identifier_as_number(identifier: &str) -> Result<(), ValidationError> {
    // Check for patterns that look like malformed numbers
    if identifier.starts_with("0x") || identifier.starts_with("0X") {
        // Looks like hex but parsed as identifier - must be malformed
        return Err(ValidationError::InvalidNumberFormat(identifier.to_string()));
    }

    if identifier.starts_with("0b") || identifier.starts_with("0B") {
        // Looks like binary but parsed as identifier - must be malformed
        return Err(ValidationError::InvalidNumberFormat(identifier.to_string()));
    }

    if identifier.starts_with("0o") || identifier.starts_with("0O") {
        // Looks like octal but parsed as identifier - must be malformed
        return Err(ValidationError::InvalidNumberFormat(identifier.to_string()));
    }

    if identifier.starts_with("0d") || identifier.starts_with("0D") {
        // Looks like decimal but parsed as identifier - must be malformed
        return Err(ValidationError::InvalidNumberFormat(identifier.to_string()));
    }

    // Check for other suspicious patterns that start with 0 and a letter
    if identifier.starts_with('0') && identifier.len() > 1 {
        let second_char = identifier.chars().nth(1).unwrap();
        if second_char.is_alphabetic() && !"box".contains(second_char.to_ascii_lowercase()) {
            // Starts with 0 and a letter that's not b, o, d, or x - likely malformed
            return Err(ValidationError::InvalidNumberFormat(identifier.to_string()));
        }
    }

    Ok(())
}
