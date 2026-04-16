use std::collections::HashSet;

use mage_ast::{FlatAssign, FlatExpression, FlatIndex, recognize_procedure_declaration};
use serde_json::json;

use super::{
    Definition, DocumentState, compute_diagnostics, content_change_text, extract_statement_text,
    find_definition_at_offset, find_identifier_at_offset, identifier_matches,
    is_identifier_character, lsp_position_to_offset, nested_string, offset_to_lsp_position,
    text_document_position, text_document_uri,
};

fn collect_identifier_occurrences(source: &str, name: &str) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut search_start = 0;

    while let Some(relative_offset) = source[search_start..].find(name) {
        let offset = search_start + relative_offset;
        let start_is_boundary = offset == 0
            || !source[..offset]
                .chars()
                .next_back()
                .is_some_and(is_identifier_character);
        let end = offset + name.len();
        let end_is_boundary = end >= source.len()
            || !source[end..]
                .chars()
                .next()
                .is_some_and(is_identifier_character);

        if start_is_boundary && end_is_boundary {
            offsets.push(offset);
        }

        search_start = end;
    }

    offsets
}

fn binding_key(definition: &Definition) -> (usize, usize, &'static str) {
    (
        definition.source_index,
        definition.statement_index,
        definition.kind,
    )
}

fn root_entries(document: &DocumentState) -> &[FlatIndex] {
    let root_source = document.root.sources[0];
    document
        .root
        .get_extra_indices(root_source.start, root_source.end)
}

fn procedure_parameter_names(document: &DocumentState, procedure_name: &str) -> Vec<String> {
    for &root_entry in root_entries(document) {
        let FlatIndex::Expression(expression_index) = root_entry else {
            continue;
        };

        let FlatExpression::Constant(assign) = document.root.get_expression(expression_index)
        else {
            continue;
        };

        if !identifier_matches(&document.root, &assign.name, procedure_name) {
            continue;
        }

        let Some(shape) = recognize_procedure_declaration(&document.root, &assign.expression)
        else {
            continue;
        };

        let parameter_source = document.root.sources[shape.parameter_source_index];
        let mut parameter_names = Vec::new();

        for &parameter_index in document
            .root
            .get_extra_indices(parameter_source.start, parameter_source.end)
        {
            let FlatIndex::Expression(parameter_expression_index) = parameter_index else {
                continue;
            };

            let FlatExpression::Constant(parameter_assign) =
                document.root.get_expression(parameter_expression_index)
            else {
                continue;
            };

            let FlatIndex::Identifier(parameter_name_index) = parameter_assign.name else {
                continue;
            };

            parameter_names.push(document.root.get_string(parameter_name_index).to_string());
        }

        return parameter_names;
    }

    Vec::new()
}

fn find_variable_definitions(document: &DocumentState, name: &str) -> Vec<Definition> {
    document
        .root
        .sources
        .iter()
        .enumerate()
        .flat_map(|(source_index, source)| {
            document
                .root
                .get_extra_indices(source.start, source.end)
                .iter()
                .enumerate()
                .filter_map(move |(statement_index, statement_index_value)| {
                    let FlatIndex::Expression(expression_index) = statement_index_value else {
                        return None;
                    };

                    let FlatExpression::Variable(assign) =
                        document.root.get_expression(*expression_index)
                    else {
                        return None;
                    };

                    if !identifier_matches(&document.root, &assign.name, name) {
                        return None;
                    }

                    Some(Definition {
                        source_index,
                        statement_index,
                        source_offset: document
                            .source_locations
                            .statement_offset(source_index, statement_index)
                            .unwrap_or(0),
                        kind: "variable",
                    })
                })
        })
        .collect()
}

fn root_statement_assign(document: &DocumentState, statement_index: usize) -> &FlatAssign {
    let FlatIndex::Expression(expression_index) = root_entries(document)[statement_index] else {
        panic!("expected root expression");
    };

    let FlatExpression::Variable(assign) = document.root.get_expression(expression_index) else {
        panic!("expected root variable assignment");
    };

    assign
}

fn lexical_definition_at_offset(
    document: &DocumentState,
    name: &str,
    usage_offset: usize,
) -> Option<Definition> {
    find_definition_at_offset(document, name, usage_offset)
}

fn binding_accurate_reference_offsets(document: &DocumentState, usage_offset: usize) -> Vec<usize> {
    let Some(name) = find_identifier_at_offset(&document.source, usage_offset) else {
        return Vec::new();
    };
    let Some(target_definition) = lexical_definition_at_offset(document, name, usage_offset) else {
        return Vec::new();
    };
    let target_binding = binding_key(&target_definition);

    let mut offsets = Vec::new();
    let mut seen_offsets = HashSet::new();

    for occurrence_offset in collect_identifier_occurrences(&document.source, name) {
        let Some(resolved_definition) =
            lexical_definition_at_offset(document, name, occurrence_offset)
        else {
            continue;
        };

        if binding_key(&resolved_definition) == target_binding
            && seen_offsets.insert(occurrence_offset)
        {
            offsets.push(occurrence_offset);
        }
    }

    offsets.sort_unstable();
    offsets
}

fn inner_source_index(document: &DocumentState) -> usize {
    if let FlatExpression::Call(call) =
        document
            .root
            .get_expression(match root_entries(document)[0] {
                FlatIndex::Expression(expression_index) => expression_index,
                _ => panic!("expected expression"),
            })
    {
        match document
            .root
            .get_extra_indices(call.arguments_start, call.arguments_end)[0]
        {
            FlatIndex::Source(source_index) => source_index as usize,
            _ => panic!("expected source block"),
        }
    } else {
        panic!("expected call");
    }
}

// --- Parameter extraction ---

#[test]
fn nested_string_reads_existing_value() {
    let value = json!({
        "textDocument": {
            "uri": "file:///test.hex"
        }
    });

    assert_eq!(
        nested_string(&value, &["textDocument", "uri"]),
        Some(String::from("file:///test.hex"))
    );
}

#[test]
fn nested_string_returns_none_for_missing_value() {
    let value = json!({
        "textDocument": {}
    });

    assert_eq!(nested_string(&value, &["textDocument", "uri"]), None);
}

#[test]
fn text_document_uri_reads_uri() {
    let value = json!({
        "textDocument": {
            "uri": "file:///module.hex"
        }
    });

    assert_eq!(
        text_document_uri(&value),
        Some(String::from("file:///module.hex"))
    );
}

#[test]
fn text_document_position_reads_zero_based_lsp_position() {
    let value = json!({
        "position": {
            "line": 2,
            "character": 4
        }
    });

    assert_eq!(text_document_position(&value), Some((2, 4)));
}

#[test]
fn text_document_position_returns_none_when_missing() {
    let value = json!({});

    assert_eq!(text_document_position(&value), None);
}

#[test]
fn content_change_text_reads_last_change() {
    let value = json!({
        "contentChanges": [
            { "text": "old" },
            { "text": "new" }
        ]
    });

    assert_eq!(content_change_text(&value), Some(String::from("new")));
}

#[test]
fn content_change_text_returns_none_when_missing() {
    let value = json!({
        "contentChanges": []
    });

    assert_eq!(content_change_text(&value), None);
}

#[test]
fn content_change_text_returns_none_when_no_content_changes_key() {
    let value = json!({});

    assert_eq!(content_change_text(&value), None);
}

// --- Position conversion ---

#[test]
fn lsp_position_round_trips_with_line_index() {
    let source = "alpha\nbeta\n";
    let line_index = mage_ast::LineIndex::new(source);

    let offset = lsp_position_to_offset(&line_index, 1, 2).unwrap();
    assert_eq!(&source[offset..offset + 1], "t");
    assert_eq!(offset_to_lsp_position(&line_index, offset), (1, 2));
}

#[test]
fn lsp_position_to_offset_returns_none_for_out_of_range() {
    let source = "short";
    let line_index = mage_ast::LineIndex::new(source);

    assert_eq!(lsp_position_to_offset(&line_index, 99, 0), None);
}

// --- Identifier lookup ---

#[test]
fn find_identifier_at_offset_returns_full_identifier() {
    let source = "alpha beta_gamma";
    let offset = source.find("gamma").unwrap();

    assert_eq!(
        find_identifier_at_offset(source, offset),
        Some("beta_gamma")
    );
}

#[test]
fn find_identifier_at_offset_rejects_non_identifier_offset() {
    let source = "alpha + beta";
    let offset = source.find('+').unwrap();

    assert_eq!(find_identifier_at_offset(source, offset), None);
}

#[test]
fn find_identifier_at_offset_returns_none_past_end() {
    let source = "alpha";

    assert_eq!(find_identifier_at_offset(source, source.len()), None);
}

#[test]
fn find_identifier_at_offset_returns_none_on_empty_source() {
    assert_eq!(find_identifier_at_offset("", 0), None);
}

// --- Diagnostics ---

#[test]
fn compute_diagnostics_reports_decode_error_range_and_message() {
    let document = DocumentState::new(String::from("\"unterminated"));

    let diagnostics = compute_diagnostics(&document);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["severity"], 1);
    assert_eq!(diagnostic["source"], "mage");
    assert_eq!(diagnostic["message"], "unmatched '\"'");
    assert_eq!(diagnostic["range"]["start"]["line"], 0);
    assert_eq!(diagnostic["range"]["start"]["character"], 0);
    assert_eq!(diagnostic["range"]["end"]["line"], 0);
    assert_eq!(diagnostic["range"]["end"]["character"], 1);
}

#[test]
fn compute_diagnostics_marks_non_eof_errors_with_one_character_width() {
    let document = DocumentState::new(String::from(")"));

    let diagnostics = compute_diagnostics(&document);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["message"], "unexpected closing ')'");
    assert_eq!(diagnostic["range"]["start"]["character"], 0);
    assert_eq!(diagnostic["range"]["end"]["character"], 1);
}

#[test]
fn compute_diagnostics_returns_empty_for_valid_source() {
    let document = DocumentState::new(String::from("x = 0d1;"));

    let diagnostics = compute_diagnostics(&document);
    assert!(diagnostics.is_empty());
}

// --- Definition lookup ---

#[test]
fn find_definition_returns_constant_definition() {
    let source = "value : 0d1; result = value;";
    let document = DocumentState::new(String::from(source));
    let usage_offset = source.rfind("value").unwrap();

    let definition = find_definition_at_offset(&document, "value", usage_offset).unwrap();

    assert_eq!(definition.kind, "constant");
    assert_eq!(definition.source_index, 0);
    assert_eq!(definition.statement_index, 0);
    assert_eq!(definition.source_offset, 0);
}

#[test]
fn find_definition_returns_variable_definition_from_multiple_assignment() {
    let source = "left, right = pair void; return right;";
    let document = DocumentState::new(String::from(source));
    let usage_offset = source.rfind("right").unwrap();

    let definition = find_definition_at_offset(&document, "right", usage_offset).unwrap();

    assert_eq!(definition.kind, "variable");
    assert_eq!(definition.statement_index, 0);
}

#[test]
fn find_definition_classifies_procedure_definition() {
    let source = "main : procedure {}, Void { return 0d0; }; main void;";
    let document = DocumentState::new(String::from(source));
    let usage_offset = source.rfind("main").unwrap();

    let definition = find_definition_at_offset(&document, "main", usage_offset).unwrap();

    assert_eq!(definition.kind, "procedure");
    assert_eq!(definition.statement_index, 0);
}

#[test]
fn find_definition_returns_none_for_undefined_name() {
    let source = "x = 0d1;";
    let document = DocumentState::new(String::from(source));

    assert!(find_definition_at_offset(&document, "undefined", 0).is_none());
}

// --- Statement text extraction ---

#[test]
fn extract_statement_text_returns_trimmed_statement_without_semicolon() {
    let source = "value : 0d1;\nresult = value;\n";
    let document = DocumentState::new(String::from(source));

    assert_eq!(
        extract_statement_text(&document, 0, 0),
        String::from("value : 0d1")
    );
    assert_eq!(
        extract_statement_text(&document, 0, 1),
        String::from("result = value")
    );
}

// --- Definition location to LSP range ---

#[test]
fn definition_location_can_be_converted_to_lsp_range() {
    let source = "value : 0d1; result = value;";
    let document = DocumentState::new(String::from(source));
    let usage_offset = source.rfind("value").unwrap();
    let definition = find_definition_at_offset(&document, "value", usage_offset).unwrap();

    let (line, character) =
        offset_to_lsp_position(&document.line_index, definition.source_offset as usize);

    assert_eq!((line, character), (0, 0));

    let range = json!({
        "start": { "line": line, "character": character },
        "end": { "line": line, "character": character + "value".len() as u32 }
    });

    assert_eq!(range["start"]["line"], 0);
    assert_eq!(range["start"]["character"], 0);
    assert_eq!(range["end"]["character"], 5);
}

// --- Hover information ---

#[test]
fn hover_markdown_can_be_built_from_definition() {
    let source = "value : 0d1; result = value;";
    let document = DocumentState::new(String::from(source));
    let usage_offset = source.rfind("value").unwrap();
    let definition = find_definition_at_offset(&document, "value", usage_offset).unwrap();
    let statement_text = extract_statement_text(
        &document,
        definition.source_index,
        definition.statement_index,
    );

    let hover_text = format!(
        "**{}** `{}`\n\n```mage\n{}\n```",
        definition.kind, "value", statement_text
    );

    assert!(hover_text.contains("**constant** `value`"));
    assert!(hover_text.contains("value : 0d1"));
}

#[test]
fn hover_markdown_can_be_built_from_inner_scope_definition() {
    let source = "wrapper { value = 0d2; return value; };";
    let document = DocumentState::new(String::from(source));

    let source_index = inner_source_index(&document);

    let statement_text = extract_statement_text(&document, source_index, 0);
    let hover_text = format!(
        "{}{}{}",
        "**variable** `value`\n\n```mage\n", statement_text, "\n```"
    );

    assert!(hover_text.contains("**variable** `value`"));
    assert!(hover_text.contains("value = 0d2"));
}

// --- Variable definitions across scopes ---

#[test]
fn find_definition_prefers_nearest_scope_variable() {
    let source = "value = 0d1; wrapper { value = 0d2; return value; }; return value;";
    let document = DocumentState::new(String::from(source));

    let definitions = find_variable_definitions(&document, "value");

    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions[0].source_index, 0);
    assert_eq!(definitions[0].source_offset, 0);
    assert_eq!(definitions[1].source_index, 1);
    assert!(definitions[1].source_offset > definitions[0].source_offset);
}

#[test]
fn extract_statement_text_preserves_inner_scope_definition_text() {
    let source = "wrapper { value = 0d2; return value; };";
    let document = DocumentState::new(String::from(source));

    let source_index = inner_source_index(&document);

    assert_eq!(
        extract_statement_text(&document, source_index, 0),
        String::from("value = 0d2")
    );

    let second_statement_text = extract_statement_text(&document, source_index, 1);
    assert!(second_statement_text.starts_with("return value"));
}

// --- Shadowing ---

#[test]
fn shadowing_tests_collect_outer_and_inner_variable_definitions() {
    let source = "value = 0d1; wrapper { value = 0d2; return value; }; return value;";
    let document = DocumentState::new(String::from(source));

    let definitions = find_variable_definitions(&document, "value");

    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions[0].source_index, 0);
    assert_eq!(definitions[0].statement_index, 0);
    assert_eq!(definitions[0].source_offset, 0);

    assert_eq!(definitions[1].source_index, 1);
    assert_eq!(definitions[1].statement_index, 0);
    assert!(definitions[1].source_offset > definitions[0].source_offset);
}

#[test]
fn shadowing_tests_keep_outer_assignment_accessible_after_nested_block() {
    let source = "value = 0d1; wrapper { value = 0d2; return value; }; value = value;";
    let document = DocumentState::new(String::from(source));

    let outer_assign = root_statement_assign(&document, 0);
    let final_assign = root_statement_assign(&document, 2);

    assert!(identifier_matches(
        &document.root,
        &outer_assign.name,
        "value"
    ));
    assert!(identifier_matches(
        &document.root,
        &final_assign.name,
        "value"
    ));
    assert!(identifier_matches(
        &document.root,
        &final_assign.expression,
        "value"
    ));
}

#[test]
fn shadowing_tests_preserve_inner_scope_statement_text_for_hover() {
    let source = "wrapper { value = 0d2; return value; };";
    let document = DocumentState::new(String::from(source));

    let source_index = inner_source_index(&document);

    assert_eq!(
        extract_statement_text(&document, source_index, 0),
        String::from("value = 0d2")
    );

    let hover_text = format!(
        "{}{}{}",
        "**variable** `value`\n\n```mage\n",
        extract_statement_text(&document, source_index, 0),
        "\n```"
    );

    assert!(hover_text.contains("**variable** `value`"));
    assert!(hover_text.contains("value = 0d2"));
}

// --- Procedure parameters ---

#[test]
fn procedure_parameter_names_collect_declared_parameters() {
    let source = "identity : procedure { value : Any }, Void { return value; };";
    let document = DocumentState::new(String::from(source));

    let parameter_names = procedure_parameter_names(&document, "identity");

    assert_eq!(parameter_names, vec![String::from("value")]);
}

#[test]
fn procedure_parameter_names_preserve_parameter_order() {
    let source =
        "pair : procedure { left : Any; right : Any }, Void { return left; return right; };";
    let document = DocumentState::new(String::from(source));

    let parameter_names = procedure_parameter_names(&document, "pair");

    assert_eq!(
        parameter_names,
        vec![String::from("left"), String::from("right")]
    );
}

#[test]
fn procedure_parameter_names_ignore_non_procedure_bindings() {
    let source = "value : 0d1; wrapper { value = 0d2; };";
    let document = DocumentState::new(String::from(source));

    let parameter_names = procedure_parameter_names(&document, "value");

    assert!(parameter_names.is_empty());
}

// --- Identifier occurrences ---

#[test]
fn collect_identifier_occurrences_returns_all_identifier_usages() {
    let source = "value = 0d1; wrapper { value = value; }; return value;";
    let offsets = collect_identifier_occurrences(source, "value");

    assert_eq!(offsets.len(), 4);
}

#[test]
fn collect_identifier_occurrences_ignores_identifier_substrings() {
    let source = "value = 0d1; value_extra = value;";
    let offsets = collect_identifier_occurrences(source, "value");

    assert_eq!(offsets.len(), 2);
}

#[test]
fn collect_identifier_occurrences_handles_parameter_name_references() {
    let source = "identity : procedure { value : Any }, Void { return value; };";
    let offsets = collect_identifier_occurrences(source, "value");

    assert_eq!(offsets.len(), 2);
}

#[test]
fn collect_identifier_occurrences_handles_shadowed_names_in_nested_scope() {
    let source = "value = 0d1; wrapper { value = 0d2; return value; }; return value;";
    let offsets = collect_identifier_occurrences(source, "value");

    assert_eq!(offsets.len(), 4);
}

#[test]
fn collect_identifier_occurrences_handles_procedure_name_references() {
    let source = "identity : procedure { value : Any }, Void { return value; }; identity value;";
    let offsets = collect_identifier_occurrences(source, "identity");

    assert_eq!(offsets.len(), 2);
}

// --- Binding-accurate references ---

#[test]
fn binding_accurate_references_for_outer_variable_exclude_shadowed_inner_variable() {
    let source = "value = 0d1; wrapper { value = 0d2; return value; }; return value;";
    let document = DocumentState::new(String::from(source));
    let usage_offset = source
        .rfind("return value;")
        .and_then(|offset| {
            source[offset..]
                .find("value")
                .map(|relative| offset + relative)
        })
        .expect("expected outer value usage");

    let reference_offsets = binding_accurate_reference_offsets(&document, usage_offset);

    let expected_offsets = vec![
        source
            .find("value = 0d1")
            .expect("expected outer definition"),
        source
            .rfind("value")
            .expect("expected final outer value reference"),
    ];

    assert_eq!(reference_offsets, expected_offsets);
}

#[test]
fn binding_accurate_references_for_inner_variable_exclude_outer_variable() {
    let source = "value = 0d1; wrapper { value = 0d2; return value; }; return value;";
    let document = DocumentState::new(String::from(source));
    let usage_offset = source
        .find("return value; }")
        .and_then(|offset| {
            source[offset..]
                .find("value")
                .map(|relative| offset + relative)
        })
        .expect("expected inner value usage");

    let reference_offsets = binding_accurate_reference_offsets(&document, usage_offset);

    let expected_offsets = vec![
        source
            .find("value = 0d2")
            .expect("expected inner definition"),
        source
            .find("return value;")
            .and_then(|offset| {
                source[offset..]
                    .find("value")
                    .map(|relative| offset + relative)
            })
            .expect("expected inner value reference"),
    ];

    assert_eq!(reference_offsets, expected_offsets);
}

#[test]
fn binding_accurate_references_for_parameter_exclude_outer_binding_with_same_name() {
    let source =
        "value = 0d1; identity : procedure { value : Any }, Void { return value; }; return value;";
    let document = DocumentState::new(String::from(source));
    let procedure_body_offset = source
        .rfind("{ return value; }")
        .and_then(|offset| {
            source[offset..]
                .find("value")
                .map(|relative| offset + relative)
        })
        .expect("expected parameter usage");

    let reference_offsets = binding_accurate_reference_offsets(&document, procedure_body_offset);

    let expected_offsets = vec![
        source
            .find("{ value : Any")
            .and_then(|offset| {
                source[offset..]
                    .find("value")
                    .map(|relative| offset + relative)
            })
            .expect("expected parameter definition"),
        source
            .rfind("{ return value; }")
            .and_then(|offset| {
                source[offset..]
                    .find("value")
                    .map(|relative| offset + relative)
            })
            .expect("expected parameter body usage"),
    ];

    assert_eq!(reference_offsets, expected_offsets);
}

#[test]
fn binding_accurate_references_for_procedure_name_include_declaration_and_call() {
    let source = "identity : procedure { value : Any }, Void { return value; }; identity value;";
    let document = DocumentState::new(String::from(source));
    let usage_offset = source.rfind("identity").expect("expected procedure call");

    let reference_offsets = binding_accurate_reference_offsets(&document, usage_offset);

    let expected_offsets = vec![
        source
            .find("identity")
            .expect("expected procedure definition"),
        source.rfind("identity").expect("expected procedure call"),
    ];

    assert_eq!(reference_offsets, expected_offsets);
}

// --- Document state ---

#[test]
fn document_state_new_parses_source() {
    let document = DocumentState::new(String::from("x = 0d1;"));

    assert_eq!(document.source, "x = 0d1;");
    assert!(document.decode_errors.is_empty());
    assert!(!document.root.sources.is_empty());
}

#[test]
fn document_state_update_reparses_source() {
    let mut document = DocumentState::new(String::from("x = 0d1;"));
    assert!(document.decode_errors.is_empty());

    document.update(String::from("\"unterminated"));
    assert!(!document.decode_errors.is_empty());
    assert_eq!(document.source, "\"unterminated");
}
