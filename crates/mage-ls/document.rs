use mage_ast::{DecodeError, FlatExpression, FlatIndex, FlatRoot, LineIndex, SourceLocations};
use serde_json::Value;
use unicode_ident::{is_xid_continue, is_xid_start};

#[cfg(test)]
#[path = "document_test.rs"]
mod document_test;

pub struct DocumentState {
    pub source: String,
    pub line_index: LineIndex,
    pub root: FlatRoot,
    pub source_locations: SourceLocations,
    pub decode_errors: Vec<DecodeError>,
}

impl DocumentState {
    pub fn new(source: String) -> Self {
        let mut document = Self {
            source: String::new(),
            line_index: LineIndex::default(),
            root: FlatRoot::default(),
            source_locations: SourceLocations::default(),
            decode_errors: Vec::new(),
        };
        document.refresh(source);
        document
    }

    pub fn update(&mut self, source: String) {
        self.refresh(source);
    }

    fn refresh(&mut self, source: String) {
        let line_index = LineIndex::new(&source);
        let (root, source_locations, decode_errors) = mage_ast::decode_recovering(&source);

        self.source = source;
        self.line_index = line_index;
        self.root = root;
        self.source_locations = source_locations;
        self.decode_errors = decode_errors;
    }
}

pub struct Definition {
    pub source_index: usize,
    pub statement_index: usize,
    pub source_offset: u32,
    pub kind: &'static str,
}

pub fn lsp_position_to_offset(line_index: &LineIndex, line: u32, character: u32) -> Option<usize> {
    line_index.byte_offset(line as usize + 1, character as usize + 1)
}

pub fn offset_to_lsp_position(line_index: &LineIndex, offset: usize) -> (u32, u32) {
    let (line, column) = line_index.line_col(offset);
    (
        line.saturating_sub(1) as u32,
        column.saturating_sub(1) as u32,
    )
}

pub fn find_identifier_at_offset(source: &str, offset: usize) -> Option<&str> {
    if offset >= source.len() {
        return None;
    }

    let character = source[offset..].chars().next()?;
    if !is_identifier_character(character) {
        return None;
    }

    let mut start = offset;
    for (index, character) in source[..offset].char_indices().rev() {
        if is_identifier_character(character) {
            start = index;
        } else {
            break;
        }
    }

    let first_character = source[start..].chars().next()?;
    if !is_identifier_start(first_character) {
        return None;
    }

    let mut end = offset;
    for character in source[end..].chars() {
        if is_identifier_character(character) {
            end += character.len_utf8();
        } else {
            break;
        }
    }

    if start == end {
        return None;
    }

    Some(&source[start..end])
}

pub fn find_definition_at_offset(
    document: &DocumentState,
    name: &str,
    usage_offset: usize,
) -> Option<Definition> {
    resolve_definition_in_source(document, 0, name, usage_offset, document.source.len(), &[])
}

fn resolve_definition_in_source(
    document: &DocumentState,
    source_index: usize,
    name: &str,
    usage_offset: usize,
    source_end: usize,
    sibling_sources: &[usize],
) -> Option<Definition> {
    let source = document.root.sources.get(source_index)?;
    let statements = document.root.get_extra_indices(source.start, source.end);

    let usage_statement_index = document.source_locations.statement_containing_offset(
        source_index,
        usage_offset,
        source_end,
    )?;

    resolve_definition_in_nested_source(
        document,
        statements,
        source_index,
        usage_statement_index,
        name,
        usage_offset,
        source_end,
    )
    .or_else(|| {
        resolve_definition_in_current_source(
            document,
            statements,
            source_index,
            usage_statement_index,
            name,
        )
    })
    .or_else(|| resolve_definition_in_sibling_sources(document, sibling_sources, name))
}

fn resolve_definition_in_nested_source(
    document: &DocumentState,
    statements: &[FlatIndex],
    source_index: usize,
    usage_statement_index: usize,
    name: &str,
    usage_offset: usize,
    source_end: usize,
) -> Option<Definition> {
    for statement_index in (0..=usage_statement_index).rev() {
        let (_, statement_end) = document
            .source_locations
            .statement_range(source_index, statement_index, source_end)
            .unwrap_or((0, source_end));

        let nested_source_ranges =
            nested_source_ranges(document, statements[statement_index], statement_end);

        for (position, nested_source_range) in nested_source_ranges.iter().enumerate() {
            if !nested_source_range.contains(usage_offset) {
                continue;
            }

            let prior_sibling_sources: Vec<usize> = nested_source_ranges[..position]
                .iter()
                .map(|nested_source_range| nested_source_range.source_index)
                .collect();

            return resolve_definition_in_source(
                document,
                nested_source_range.source_index,
                name,
                usage_offset,
                nested_source_range.end,
                &prior_sibling_sources,
            );
        }
    }

    None
}

fn resolve_definition_in_current_source(
    document: &DocumentState,
    statements: &[FlatIndex],
    source_index: usize,
    usage_statement_index: usize,
    name: &str,
) -> Option<Definition> {
    for statement_index in (0..=usage_statement_index).rev() {
        if let Some(definition) = statement_definition_at(
            document,
            source_index,
            statement_index,
            statements[statement_index],
            name,
        ) {
            return Some(definition);
        }
    }

    None
}

fn resolve_definition_in_sibling_sources(
    document: &DocumentState,
    sibling_sources: &[usize],
    name: &str,
) -> Option<Definition> {
    for &sibling_source_index in sibling_sources {
        if let Some(definition) = search_entire_source(document, sibling_source_index, name) {
            return Some(definition);
        }
    }

    None
}

struct NestedSourceRange {
    source_index: usize,
    start: usize,
    end: usize,
}

impl NestedSourceRange {
    fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }
}

fn nested_source_ranges(
    document: &DocumentState,
    statement: FlatIndex,
    statement_end: usize,
) -> Vec<NestedSourceRange> {
    let nested_source_indices = collect_nested_source_indices(&document.root, statement);
    let mut nested_source_starts: Vec<(usize, usize)> = nested_source_indices
        .iter()
        .filter_map(|&nested_source_index| {
            document
                .source_locations
                .statement_offset(nested_source_index, 0)
                .map(|start| (nested_source_index, start as usize))
        })
        .collect();
    nested_source_starts.sort_by_key(|&(_, start)| start);

    nested_source_starts
        .iter()
        .enumerate()
        .map(|(position, &(source_index, start))| NestedSourceRange {
            source_index,
            start,
            end: nested_source_starts
                .get(position + 1)
                .map(|&(_, next_start)| next_start)
                .unwrap_or(statement_end),
        })
        .collect()
}

fn collect_nested_source_indices(root: &FlatRoot, statement: FlatIndex) -> Vec<usize> {
    let FlatIndex::Expression(expression_index) = statement else {
        return Vec::new();
    };

    collect_source_indices_from_expression(root, expression_index)
}

fn collect_source_indices_from_expression(root: &FlatRoot, expression_index: u32) -> Vec<usize> {
    match root.get_expression(expression_index) {
        FlatExpression::Call(call) => {
            let mut sources = collect_source_indices_from_index(root, &call.name);
            for argument in root.get_extra_indices(call.arguments_start, call.arguments_end) {
                sources.extend(collect_source_indices_from_index(root, argument));
            }
            sources
        }
        FlatExpression::Constant(assign) | FlatExpression::Variable(assign) => {
            collect_source_indices_from_index(root, &assign.expression)
        }
        _ => Vec::new(),
    }
}

fn collect_source_indices_from_index(root: &FlatRoot, index: &FlatIndex) -> Vec<usize> {
    match index {
        FlatIndex::Source(source_index) => vec![*source_index as usize],
        FlatIndex::Expression(expression_index) => {
            collect_source_indices_from_expression(root, *expression_index)
        }
        _ => Vec::new(),
    }
}

fn match_statement_definition(
    root: &FlatRoot,
    source_index: usize,
    statement_index: usize,
    source_offset: u32,
    statement: FlatIndex,
    name: &str,
) -> Option<Definition> {
    let FlatIndex::Expression(expression_index) = statement else {
        return None;
    };

    match root.get_expression(expression_index) {
        FlatExpression::Constant(assign) if identifier_matches(root, &assign.name, name) => {
            let kind = if mage_ast::is_procedure_definition(root, &assign.expression) {
                "procedure"
            } else {
                "constant"
            };
            return Some(Definition {
                source_index,
                statement_index,
                source_offset,
                kind,
            });
        }
        FlatExpression::Variable(assign) if identifier_matches(root, &assign.name, name) => {
            return Some(Definition {
                source_index,
                statement_index,
                source_offset,
                kind: "variable",
            });
        }
        FlatExpression::MultipleVariable(multiple) => {
            for variable_name in root.get_extra_indices(multiple.names_start, multiple.names_end) {
                if identifier_matches(root, variable_name, name) {
                    return Some(Definition {
                        source_index,
                        statement_index,
                        source_offset,
                        kind: "variable",
                    });
                }
            }
        }
        _ => {}
    }

    None
}

fn statement_definition_at(
    document: &DocumentState,
    source_index: usize,
    statement_index: usize,
    statement: FlatIndex,
    name: &str,
) -> Option<Definition> {
    let source_offset = document
        .source_locations
        .statement_offset(source_index, statement_index)
        .unwrap_or(0);

    match_statement_definition(
        &document.root,
        source_index,
        statement_index,
        source_offset,
        statement,
        name,
    )
}

fn search_entire_source(
    document: &DocumentState,
    source_index: usize,
    name: &str,
) -> Option<Definition> {
    let source = document.root.sources.get(source_index)?;
    let statements = document.root.get_extra_indices(source.start, source.end);

    for (statement_index, &statement) in statements.iter().enumerate() {
        if let Some(definition) =
            statement_definition_at(document, source_index, statement_index, statement, name)
        {
            return Some(definition);
        }
    }

    None
}

pub fn identifier_matches(root: &FlatRoot, index: &FlatIndex, name: &str) -> bool {
    match index {
        FlatIndex::Identifier(string_index) => root.get_string(*string_index) == name,
        FlatIndex::Expression(expression_index) => match root.get_expression(*expression_index) {
            FlatExpression::Member(member) => identifier_matches(root, &member.expression, name),
            _ => false,
        },
        _ => false,
    }
}

pub fn extract_statement_text(
    document: &DocumentState,
    source_index: usize,
    statement_index: usize,
) -> String {
    let (start, end) = document
        .source_locations
        .statement_range(source_index, statement_index, document.source.len())
        .unwrap_or((0, 0));

    let text = &document.source[start..end.min(document.source.len())];
    text.trim_end().trim_end_matches(';').trim().to_string()
}

pub fn compute_diagnostics(document: &DocumentState) -> Vec<Value> {
    document
        .decode_errors
        .iter()
        .map(|error| {
            let location = error.location(&document.line_index);
            let line = location.line.saturating_sub(1) as u32;
            let character = location.column.saturating_sub(1) as u32;

            let (end_line, end_character) =
                if matches!(error, DecodeError::UnexpectedEndOfInput { .. }) {
                    (line, character)
                } else {
                    (line, character + 1)
                };

            serde_json::json!({
                "range": {
                    "start": { "line": line, "character": character },
                    "end": { "line": end_line, "character": end_character }
                },
                "severity": 1,
                "source": "mage",
                "message": error.to_string()
            })
        })
        .collect()
}

fn nested_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(key)?;
    }
    Some(current)
}

pub fn nested_string(value: &Value, path: &[&str]) -> Option<String> {
    nested_value(value, path)?.as_str().map(ToOwned::to_owned)
}

pub fn text_document_uri(parameters: &Value) -> Option<String> {
    nested_string(parameters, &["textDocument", "uri"])
}

pub fn text_document_position(parameters: &Value) -> Option<(u32, u32)> {
    let position = parameters.get("position")?;
    let line = position.get("line")?.as_u64()? as u32;
    let character = position.get("character")?.as_u64()? as u32;
    Some((line, character))
}

pub fn content_change_text(parameters: &Value) -> Option<String> {
    parameters
        .get("contentChanges")?
        .as_array()?
        .last()?
        .get("text")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn is_identifier_character(character: char) -> bool {
    is_xid_continue(character)
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || is_xid_start(character)
}
