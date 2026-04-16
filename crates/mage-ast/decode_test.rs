use serde_text::Format;

use crate::{
    DecodeError, FlatBinaryOperationKind, FlatExpression, FlatIndex, FlatRoot, SourceLocations,
    encode::Encoder,
};

fn decode(code: &str) -> (FlatRoot, SourceLocations) {
    crate::decode(code).unwrap_or_else(|e| panic!("unexpected decode error: {e}"))
}

fn decode_errors(code: &str) -> (FlatRoot, SourceLocations, Vec<DecodeError>) {
    crate::decode_recovering(code)
}

fn encode_pretty(root: &FlatRoot) -> String {
    Encoder::encode(root, Format::Pretty)
}

fn encode_simple(root: &FlatRoot) -> String {
    Encoder::encode(root, Format::Simple)
}

fn round_trip(code: &str) {
    let (root, _) = decode(code);
    let encoded = encode_simple(&root);
    let (root2, _) = decode(&encoded);
    assert_eq!(
        root, root2,
        "round-trip mismatch:\n  input:    {code:?}\n  encoded:  {encoded:?}"
    );
}

fn source_indices(root: &FlatRoot, source_index: usize) -> &[FlatIndex] {
    let source = root.sources[source_index];
    root.get_extra_indices(source.start, source.end)
}

fn first_statement(root: &FlatRoot) -> FlatIndex {
    let statements = source_indices(root, 0);
    assert_eq!(statements.len(), 1);
    statements[0]
}

fn get_expression<'a>(root: &'a FlatRoot, index: &FlatIndex) -> &'a FlatExpression {
    match index {
        FlatIndex::Expression(expression_index) => root.get_expression(*expression_index),
        other => panic!("expected Expression index, got {other:?}"),
    }
}

fn expect_identifier<'a>(root: &'a FlatRoot, index: &FlatIndex) -> &'a str {
    match index {
        FlatIndex::Identifier(string_index) => root.get_string(*string_index),
        other => panic!("expected identifier, got {other:?}"),
    }
}

fn expect_number<'a>(root: &'a FlatRoot, index: &FlatIndex) -> &'a str {
    match index {
        FlatIndex::Number(string_index) => root.get_string(*string_index),
        other => panic!("expected number, got {other:?}"),
    }
}

fn expression_offset(locations: &SourceLocations, index: &FlatIndex) -> u32 {
    match index {
        FlatIndex::Expression(expression_index) => {
            locations.get_expression_offset(*expression_index)
        }
        other => panic!("expected Expression index, got {other:?}"),
    }
}

#[test]
fn empty_input() {
    let (root, _) = decode("");
    assert!(source_indices(&root, 0).is_empty());
}

#[test]
fn whitespace_only() {
    let (root, _) = decode("   \t\n  ");
    assert!(source_indices(&root, 0).is_empty());
}

#[test]
fn semicolons_only() {
    let (root, _) = decode(";;;");
    assert!(source_indices(&root, 0).is_empty());
}

#[test]
fn single_identifier() {
    let (root, _) = decode("foo");
    assert_eq!(expect_identifier(&root, &first_statement(&root)), "foo");
}

#[test]
fn unicode_identifier() {
    let (root, _) = decode("привет");
    assert_eq!(expect_identifier(&root, &first_statement(&root)), "привет");
}

#[test]
fn underscore_identifier() {
    let (root, _) = decode("_foo_bar");
    assert_eq!(
        expect_identifier(&root, &first_statement(&root)),
        "_foo_bar"
    );
}

#[test]
fn zero_literal() {
    let (root, _) = decode("0");
    assert_eq!(expect_number(&root, &first_statement(&root)), "0");
}

#[test]
fn binary_number() {
    let (root, _) = decode("0b1010");
    assert_eq!(expect_number(&root, &first_statement(&root)), "0b1010");
}

#[test]
fn octal_number() {
    let (root, _) = decode("0o777");
    assert_eq!(expect_number(&root, &first_statement(&root)), "0o777");
}

#[test]
fn decimal_number() {
    let (root, _) = decode("0d42");
    if let FlatIndex::Number(index) = source_indices(&root, 0)[0] {
        assert_eq!(root.get_string(index), "0d42");
    } else {
        panic!("expected number");
    }
}

#[test]
fn hex_number() {
    let (root, _) = decode("0xFF");
    if let FlatIndex::Number(index) = source_indices(&root, 0)[0] {
        assert_eq!(root.get_string(index), "0xFF");
    } else {
        panic!("expected number");
    }
}

#[test]
fn number_with_underscores() {
    let (root, _) = decode("0b1010_1100");
    if let FlatIndex::Number(index) = source_indices(&root, 0)[0] {
        assert_eq!(root.get_string(index), "0b1010_1100");
    } else {
        panic!("expected number");
    }
}

#[test]
fn invalid_number_suffix() {
    let (_, _, errors) = decode_errors("0bxyz");
    assert!(!errors.is_empty());
    assert!(matches!(errors[0], DecodeError::InvalidNumber { .. }));
}

#[test]
fn invalid_number_no_digits() {
    let (_, _, errors) = decode_errors("0b");
    assert!(!errors.is_empty());
    assert!(matches!(errors[0], DecodeError::InvalidNumber { .. }));
}

#[test]
fn simple_string() {
    let (root, _) = decode(r#""hello""#);
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);
    if let FlatIndex::String(index) = statements[0] {
        assert_eq!(root.get_string(index), r#""hello""#);
    } else {
        panic!("expected string");
    }
}

#[test]
fn empty_string() {
    let (root, _) = decode(r#""""#);
    if let FlatIndex::String(index) = source_indices(&root, 0)[0] {
        assert_eq!(root.get_string(index), r#""""#);
    } else {
        panic!("expected string");
    }
}

#[test]
fn string_with_escapes() {
    let (root, _) = decode(r#""a\"b\\c\n""#);
    if let FlatIndex::String(index) = source_indices(&root, 0)[0] {
        assert_eq!(root.get_string(index), r#""a\"b\\c\n""#);
    } else {
        panic!("expected string");
    }
}

#[test]
fn string_unicode_content() {
    let (root, _) = decode(r#""日本語""#);
    if let FlatIndex::String(index) = source_indices(&root, 0)[0] {
        assert_eq!(root.get_string(index), r#""日本語""#);
    } else {
        panic!("expected string");
    }
}

#[test]
fn unterminated_string() {
    let (_, _, errors) = decode_errors(r#""hello"#);
    assert!(!errors.is_empty());
    assert!(matches!(
        errors[0],
        DecodeError::UnmatchedDelimiter { delimiter: '"', .. }
    ));
}

#[test]
fn newline_in_string() {
    let (_, _, errors) = decode_errors("\"hello\nworld\"");
    assert!(!errors.is_empty());
    assert!(matches!(errors[0], DecodeError::NewlineInString { .. }));
}

#[test]
fn invalid_escape_in_string() {
    let (_, _, errors) = decode_errors(r#""\q""#);
    assert!(!errors.is_empty());
    assert!(matches!(errors[0], DecodeError::InvalidStringEscape { .. }));
}

#[test]
fn constant_identifier() {
    let (root, _) = decode("x : 0");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);
    if let FlatExpression::Constant(assign) = get_expression(&root, &statements[0]) {
        assert!(matches!(assign.name, FlatIndex::Identifier(_)));
        assert!(matches!(assign.expression, FlatIndex::Number(_)));
    } else {
        panic!("expected Constant");
    }
}

#[test]
fn constant_string_value() {
    let (root, _) = decode(r#"name : "hello""#);
    let statements = source_indices(&root, 0);
    if let FlatExpression::Constant(assign) = get_expression(&root, &statements[0]) {
        assert!(matches!(assign.expression, FlatIndex::String(_)));
    } else {
        panic!("expected Constant");
    }
}

#[test]
fn variable_assignment() {
    let (root, _) = decode("x = 0");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);
    if let FlatExpression::Variable(assign) = get_expression(&root, &statements[0]) {
        assert!(matches!(assign.name, FlatIndex::Identifier(_)));
        assert!(matches!(assign.expression, FlatIndex::Number(_)));
    } else {
        panic!("expected Variable");
    }
}

#[test]
fn variable_does_not_match_equality() {
    let (root, _) = decode("x == y");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);
    if let FlatExpression::BinaryOperation(operation) = get_expression(&root, &statements[0]) {
        assert_eq!(operation.kind, FlatBinaryOperationKind::Equal);
    } else {
        panic!(
            "expected BinaryOperation, got {:?}",
            get_expression(&root, &statements[0])
        );
    }
}

#[test]
fn multiple_variable() {
    let (root, _) = decode("a, b = foo");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);
    if let FlatExpression::MultipleVariable(multi) = get_expression(&root, &statements[0]) {
        let names = root.get_extra_indices(multi.names_start, multi.names_end);
        assert_eq!(names.len(), 2);
        assert!(matches!(names[0], FlatIndex::Identifier(_)));
        assert!(matches!(names[1], FlatIndex::Identifier(_)));
        assert!(matches!(multi.expression, FlatIndex::Identifier(_)));
    } else {
        panic!("expected MultipleVariable");
    }
}

#[test]
fn procedure_constructor_syntax_decodes_as_constant() {
    let (root, _) = decode("add : procedure {x : U64; y : U64}, U64 { return x + y; }");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);

    let FlatExpression::Constant(assign) = get_expression(&root, &statements[0]) else {
        panic!(
            "expected Constant, got {:?}",
            get_expression(&root, &statements[0])
        );
    };

    assert!(matches!(assign.name, FlatIndex::Identifier(_)));

    let FlatExpression::Call(outer_call) = get_expression(&root, &assign.expression) else {
        panic!(
            "expected outer Call, got {:?}",
            get_expression(&root, &assign.expression)
        );
    };

    let FlatExpression::Call(inner_call) = get_expression(&root, &outer_call.name) else {
        panic!(
            "expected inner Call as outer callee, got {:?}",
            get_expression(&root, &outer_call.name)
        );
    };

    assert!(matches!(inner_call.name, FlatIndex::Identifier(_)));

    let inner_arguments =
        root.get_extra_indices(inner_call.arguments_start, inner_call.arguments_end);
    assert_eq!(inner_arguments.len(), 2);
    assert!(matches!(inner_arguments[0], FlatIndex::Source(_)));
    assert!(matches!(inner_arguments[1], FlatIndex::Identifier(_)));

    let outer_arguments =
        root.get_extra_indices(outer_call.arguments_start, outer_call.arguments_end);
    assert_eq!(outer_arguments.len(), 1);
    assert!(matches!(outer_arguments[0], FlatIndex::Source(_)));
}

#[test]
fn grouped_callee_followed_by_source_block_decodes_as_call() {
    let (root, _) = decode("(foo bar) {}");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);

    let FlatExpression::Call(outer_call) = get_expression(&root, &statements[0]) else {
        panic!(
            "expected outer Call, got {:?}",
            get_expression(&root, &statements[0])
        );
    };

    let FlatExpression::Call(inner_call) = get_expression(&root, &outer_call.name) else {
        panic!(
            "expected inner Call as grouped/composite callee, got {:?}",
            get_expression(&root, &outer_call.name)
        );
    };

    let inner_arguments =
        root.get_extra_indices(inner_call.arguments_start, inner_call.arguments_end);
    assert_eq!(inner_arguments.len(), 1);

    let outer_arguments =
        root.get_extra_indices(outer_call.arguments_start, outer_call.arguments_end);
    assert_eq!(outer_arguments.len(), 1);
    assert!(matches!(outer_arguments[0], FlatIndex::Source(_)));
}

#[test]
fn multiple_variable_three_names() {
    let (root, _) = decode("a, b, c = foo");
    if let FlatExpression::MultipleVariable(multi) =
        get_expression(&root, &source_indices(&root, 0)[0])
    {
        let names = root.get_extra_indices(multi.names_start, multi.names_end);
        assert_eq!(names.len(), 3);
    } else {
        panic!("expected MultipleVariable");
    }
}

#[test]
fn call_single_argument() {
    let (root, _) = decode("foo bar");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);
    if let FlatExpression::Call(call) = get_expression(&root, &statements[0]) {
        assert!(matches!(call.name, FlatIndex::Identifier(_)));
        let arguments = root.get_extra_indices(call.arguments_start, call.arguments_end);
        assert_eq!(arguments.len(), 1);
        assert!(matches!(arguments[0], FlatIndex::Identifier(_)));
    } else {
        panic!("expected Call");
    }
}

#[test]
fn call_multiple_arguments() {
    let (root, _) = decode("foo bar, baz");
    if let FlatExpression::Call(call) = get_expression(&root, &source_indices(&root, 0)[0]) {
        let arguments = root.get_extra_indices(call.arguments_start, call.arguments_end);
        assert_eq!(arguments.len(), 2);
    } else {
        panic!("expected Call");
    }
}

#[test]
fn call_chain_is_left_associative() {
    let (root, _) = decode("foo bar baz");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);

    let FlatExpression::Call(outer_call) = get_expression(&root, &statements[0]) else {
        panic!(
            "expected outer Call, got {:?}",
            get_expression(&root, &statements[0])
        );
    };

    let outer_arguments =
        root.get_extra_indices(outer_call.arguments_start, outer_call.arguments_end);
    assert_eq!(outer_arguments.len(), 1);
    assert!(matches!(outer_arguments[0], FlatIndex::Identifier(_)));

    let FlatExpression::Call(inner_call) = get_expression(&root, &outer_call.name) else {
        panic!(
            "expected inner Call as left-associated callee, got {:?}",
            get_expression(&root, &outer_call.name)
        );
    };

    assert!(matches!(inner_call.name, FlatIndex::Identifier(_)));
    let inner_arguments =
        root.get_extra_indices(inner_call.arguments_start, inner_call.arguments_end);
    assert_eq!(inner_arguments.len(), 1);
    assert!(matches!(inner_arguments[0], FlatIndex::Identifier(_)));
}

#[test]
fn return_participates_in_universal_left_associative_call_syntax() {
    let (root, _) = decode("return add 0d3, 0d4");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);

    let FlatExpression::Call(outer_call) = get_expression(&root, &statements[0]) else {
        panic!(
            "expected outer Call, got {:?}",
            get_expression(&root, &statements[0])
        );
    };

    let outer_arguments =
        root.get_extra_indices(outer_call.arguments_start, outer_call.arguments_end);
    assert_eq!(outer_arguments.len(), 2);
    assert!(matches!(outer_arguments[0], FlatIndex::Number(_)));
    assert!(matches!(outer_arguments[1], FlatIndex::Number(_)));

    let FlatExpression::Call(inner_call) = get_expression(&root, &outer_call.name) else {
        panic!(
            "expected inner Call as left-associated callee, got {:?}",
            get_expression(&root, &outer_call.name)
        );
    };

    assert!(matches!(inner_call.name, FlatIndex::Identifier(_)));
    let inner_arguments =
        root.get_extra_indices(inner_call.arguments_start, inner_call.arguments_end);
    assert_eq!(inner_arguments.len(), 1);
    assert!(matches!(inner_arguments[0], FlatIndex::Identifier(_)));
}

#[test]
fn call_string_argument() {
    let (root, _) = decode(r#"print "hello""#);
    if let FlatExpression::Call(call) = get_expression(&root, &source_indices(&root, 0)[0]) {
        let arguments = root.get_extra_indices(call.arguments_start, call.arguments_end);
        assert_eq!(arguments.len(), 1);
        assert!(matches!(arguments[0], FlatIndex::String(_)));
    } else {
        panic!("expected Call");
    }
}

#[test]
fn call_number_argument() {
    let (root, _) = decode("exit 0");
    if let FlatExpression::Call(call) = get_expression(&root, &source_indices(&root, 0)[0]) {
        let arguments = root.get_extra_indices(call.arguments_start, call.arguments_end);
        assert_eq!(arguments.len(), 1);
        assert!(matches!(arguments[0], FlatIndex::Number(_)));
    } else {
        panic!("expected Call");
    }
}

#[test]
fn call_block_argument() {
    let (root, _) = decode("foo {}");
    if let FlatExpression::Call(call) = get_expression(&root, &source_indices(&root, 0)[0]) {
        let arguments = root.get_extra_indices(call.arguments_start, call.arguments_end);
        assert_eq!(arguments.len(), 1);
        assert!(matches!(arguments[0], FlatIndex::Source(_)));
    } else {
        panic!("expected Call");
    }
}

#[test]
fn member_access() {
    let (root, _) = decode("a.b");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);
    if let FlatExpression::Member(member) = get_expression(&root, &statements[0]) {
        assert!(matches!(member.name, FlatIndex::Identifier(_)));
        assert!(matches!(member.expression, FlatIndex::Identifier(_)));
    } else {
        panic!("expected Member");
    }
}

#[test]
fn chained_member_access() {
    let (root, _) = decode("a.b.c");
    let statements = source_indices(&root, 0);
    if let FlatExpression::Member(outer) = get_expression(&root, &statements[0]) {
        assert!(matches!(outer.expression, FlatIndex::Identifier(_)));
        if let FlatExpression::Member(inner) = get_expression(&root, &outer.name) {
            assert!(matches!(inner.name, FlatIndex::Identifier(_)));
            assert!(matches!(inner.expression, FlatIndex::Identifier(_)));
        } else {
            panic!("expected inner Member");
        }
    } else {
        panic!("expected outer Member");
    }
}

#[test]
fn dot_with_space_is_not_member_access() {
    // `.` cannot start an atom, so `foo .bar` is not a call with an
    // implicit member argument. Implicit members are encoder-only.
    let (root, _, errors) = decode_errors("foo .bar");
    let statements = source_indices(&root, 0);
    assert!(!statements.is_empty());
    assert!(matches!(statements[0], FlatIndex::Identifier(_)));
    assert!(!errors.is_empty());
}

#[test]
fn binary_add() {
    let (root, _) = decode("a + b");
    if let FlatExpression::BinaryOperation(operation) =
        get_expression(&root, &source_indices(&root, 0)[0])
    {
        assert_eq!(operation.kind, FlatBinaryOperationKind::Add);
    } else {
        panic!("expected BinaryOperation");
    }
}

#[test]
fn binary_subtract() {
    let (root, _) = decode("a - b");
    if let FlatExpression::BinaryOperation(operation) =
        get_expression(&root, &source_indices(&root, 0)[0])
    {
        assert_eq!(operation.kind, FlatBinaryOperationKind::Subtract);
    } else {
        panic!("expected BinaryOperation");
    }
}

#[test]
fn binary_multiply() {
    let (root, _) = decode("a * b");
    if let FlatExpression::BinaryOperation(operation) =
        get_expression(&root, &source_indices(&root, 0)[0])
    {
        assert_eq!(operation.kind, FlatBinaryOperationKind::Multiply);
    } else {
        panic!("expected BinaryOperation");
    }
}

#[test]
fn binary_all_comparison_operators() {
    for (code, expected_kind) in [
        ("a < b", FlatBinaryOperationKind::LessThan),
        ("a > b", FlatBinaryOperationKind::GreaterThan),
        ("a <= b", FlatBinaryOperationKind::LessThanOrEqual),
        ("a >= b", FlatBinaryOperationKind::GreaterThanOrEqual),
        ("a == b", FlatBinaryOperationKind::Equal),
        ("a != b", FlatBinaryOperationKind::NotEqual),
    ] {
        let (root, _) = decode(code);
        if let FlatExpression::BinaryOperation(operation) =
            get_expression(&root, &source_indices(&root, 0)[0])
        {
            assert_eq!(operation.kind, expected_kind, "failed for: {code}");
        } else {
            panic!("expected BinaryOperation for: {code}");
        }
    }
}

#[test]
fn binary_precedence_mul_over_add() {
    let (root, _) = decode("a + b * c");
    if let FlatExpression::BinaryOperation(operation) =
        get_expression(&root, &source_indices(&root, 0)[0])
    {
        assert_eq!(operation.kind, FlatBinaryOperationKind::Add);
        assert!(matches!(operation.left, FlatIndex::Identifier(_)));
        if let FlatExpression::BinaryOperation(right_operation) =
            get_expression(&root, &operation.right)
        {
            assert_eq!(right_operation.kind, FlatBinaryOperationKind::Multiply);
        } else {
            panic!("expected Multiply on right");
        }
    } else {
        panic!("expected Add");
    }
}

#[test]
fn binary_precedence_left_associative() {
    let (root, _) = decode("a - b - c");
    if let FlatExpression::BinaryOperation(operation) =
        get_expression(&root, &source_indices(&root, 0)[0])
    {
        assert_eq!(operation.kind, FlatBinaryOperationKind::Subtract);
        assert!(matches!(operation.right, FlatIndex::Identifier(_)));
        if let FlatExpression::BinaryOperation(left_operation) =
            get_expression(&root, &operation.left)
        {
            assert_eq!(left_operation.kind, FlatBinaryOperationKind::Subtract);
        } else {
            panic!("expected Subtract on left");
        }
    } else {
        panic!("expected Subtract");
    }
}

#[test]
fn binary_comparison_non_associative() {
    // Comparisons break after one per level, so `a < b` is parsed
    // and `< c` remains unparsed, producing an error.
    let (root, _, errors) = decode_errors("a < b < c");
    let statements = source_indices(&root, 0);
    assert!(!statements.is_empty());
    if let FlatExpression::BinaryOperation(operation) = get_expression(&root, &statements[0]) {
        assert_eq!(operation.kind, FlatBinaryOperationKind::LessThan);
    } else {
        panic!("expected BinaryOperation");
    }
    assert!(!errors.is_empty());
}

#[test]
fn parenthesized_expression() {
    let (root, _) = decode("(a + b) * c");
    if let FlatExpression::BinaryOperation(operation) =
        get_expression(&root, &source_indices(&root, 0)[0])
    {
        assert_eq!(operation.kind, FlatBinaryOperationKind::Multiply);
        if let FlatExpression::BinaryOperation(left_operation) =
            get_expression(&root, &operation.left)
        {
            assert_eq!(left_operation.kind, FlatBinaryOperationKind::Add);
        } else {
            panic!("expected Add inside parens");
        }
    } else {
        panic!("expected Multiply");
    }
}

#[test]
fn nested_parentheses() {
    let (root, _) = decode("((((a))))");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);
    assert!(matches!(statements[0], FlatIndex::Identifier(_)));
}

#[test]
fn unmatched_open_paren() {
    let (_, _, errors) = decode_errors("(a + b");
    assert!(!errors.is_empty());
    assert!(matches!(
        errors[0],
        DecodeError::UnmatchedDelimiter { delimiter: '(', .. }
    ));
}

#[test]
fn unexpected_close_paren() {
    let (_, _, errors) = decode_errors(")");
    assert!(!errors.is_empty());
    assert!(matches!(
        errors[0],
        DecodeError::UnexpectedClosingDelimiter { delimiter: ')', .. }
    ));
}

#[test]
fn empty_block() {
    let (root, _) = decode("foo {}");
    if let FlatExpression::Call(call) = get_expression(&root, &source_indices(&root, 0)[0]) {
        let arguments = root.get_extra_indices(call.arguments_start, call.arguments_end);
        if let FlatIndex::Source(source_index) = arguments[0] {
            assert!(source_indices(&root, source_index as usize).is_empty());
        } else {
            panic!("expected Source");
        }
    } else {
        panic!("expected Call");
    }
}

#[test]
fn block_with_statements() {
    let (root, _) = decode("foo { a; b }");
    if let FlatExpression::Call(call) = get_expression(&root, &source_indices(&root, 0)[0]) {
        let arguments = root.get_extra_indices(call.arguments_start, call.arguments_end);
        if let FlatIndex::Source(source_index) = arguments[0] {
            assert_eq!(source_indices(&root, source_index as usize).len(), 2);
        } else {
            panic!("expected Source");
        }
    } else {
        panic!("expected Call");
    }
}

#[test]
fn nested_blocks() {
    let (root, _) = decode("foo { bar { baz } }");
    if let FlatExpression::Call(outer_call) = get_expression(&root, &source_indices(&root, 0)[0]) {
        let outer_arguments =
            root.get_extra_indices(outer_call.arguments_start, outer_call.arguments_end);
        if let FlatIndex::Source(source_index) = outer_arguments[0] {
            let inner_statements = source_indices(&root, source_index as usize);
            assert_eq!(inner_statements.len(), 1);
            if let FlatExpression::Call(inner_call) = get_expression(&root, &inner_statements[0]) {
                let inner_arguments =
                    root.get_extra_indices(inner_call.arguments_start, inner_call.arguments_end);
                assert_eq!(inner_arguments.len(), 1);
                assert!(matches!(inner_arguments[0], FlatIndex::Source(_)));
            } else {
                panic!("expected inner Call");
            }
        } else {
            panic!("expected Source");
        }
    } else {
        panic!("expected outer Call");
    }
}

#[test]
fn unmatched_open_brace() {
    let (_, _, errors) = decode_errors("foo {");
    assert!(!errors.is_empty());
    assert!(matches!(
        errors[0],
        DecodeError::UnmatchedDelimiter { delimiter: '{', .. }
    ));
}

#[test]
fn unexpected_close_brace_at_top() {
    let (_, _, errors) = decode_errors("}");
    assert!(!errors.is_empty());
    assert!(matches!(
        errors[0],
        DecodeError::UnexpectedClosingDelimiter { delimiter: '}', .. }
    ));
}

#[test]
fn multiple_statements_semicolon() {
    let (root, _) = decode("a; b; c");
    assert_eq!(source_indices(&root, 0).len(), 3);
}

#[test]
fn multiple_statements_trailing_semicolon() {
    let (root, _) = decode("a; b;");
    assert_eq!(source_indices(&root, 0).len(), 2);
}

#[test]
fn multiple_statements_extra_semicolons() {
    let (root, _) = decode(";;a;;;b;;");
    assert_eq!(source_indices(&root, 0).len(), 2);
}

#[test]
fn string_interning() {
    let (root, _) = decode("foo; foo; foo");
    let statements = source_indices(&root, 0);
    if let (FlatIndex::Identifier(a), FlatIndex::Identifier(b), FlatIndex::Identifier(c)) =
        (statements[0], statements[1], statements[2])
    {
        assert_eq!(a, b);
        assert_eq!(b, c);
    } else {
        panic!("expected three identifiers");
    }
}

#[test]
fn different_strings_different_indices() {
    let (root, _) = decode("foo; bar");
    let statements = source_indices(&root, 0);
    if let (FlatIndex::Identifier(a), FlatIndex::Identifier(b)) = (statements[0], statements[1]) {
        assert_ne!(a, b);
    } else {
        panic!("expected two identifiers");
    }
}

#[test]
fn constant_with_call_value() {
    let (root, _) = decode("x : foo bar");
    if let FlatExpression::Constant(assign) = get_expression(&root, &source_indices(&root, 0)[0]) {
        assert!(matches!(
            get_expression(&root, &assign.expression),
            FlatExpression::Call(_)
        ));
    } else {
        panic!("expected Constant");
    }
}

#[test]
fn variable_with_binary_expression() {
    let (root, _) = decode("x = a + b");
    if let FlatExpression::Variable(assign) = get_expression(&root, &source_indices(&root, 0)[0]) {
        if let FlatExpression::BinaryOperation(operation) =
            get_expression(&root, &assign.expression)
        {
            assert_eq!(operation.kind, FlatBinaryOperationKind::Add);
        } else {
            panic!("expected BinaryOperation");
        }
    } else {
        panic!("expected Variable");
    }
}

#[test]
fn call_with_binary_expression_argument() {
    let (root, _) = decode("foo (a + b)");
    if let FlatExpression::Call(call) = get_expression(&root, &source_indices(&root, 0)[0]) {
        let arguments = root.get_extra_indices(call.arguments_start, call.arguments_end);
        assert_eq!(arguments.len(), 1);
        if let FlatExpression::BinaryOperation(operation) = get_expression(&root, &arguments[0]) {
            assert_eq!(operation.kind, FlatBinaryOperationKind::Add);
        } else {
            panic!("expected BinaryOperation in argument");
        }
    } else {
        panic!("expected Call");
    }
}

#[test]
fn parenthesized_call_in_binary_operation() {
    let (root, _) = decode("(foo bar) + x");
    let statements = source_indices(&root, 0);
    if let FlatExpression::BinaryOperation(operation) = get_expression(&root, &statements[0]) {
        assert_eq!(operation.kind, FlatBinaryOperationKind::Add);
        assert!(matches!(
            get_expression(&root, &operation.left),
            FlatExpression::Call(_)
        ));
    } else {
        panic!("expected BinaryOperation");
    }
}

#[test]
fn grouped_call_followed_by_member_access() {
    let (root, _) = decode("(foo bar).baz");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);

    let FlatExpression::Member(member) = get_expression(&root, &statements[0]) else {
        panic!(
            "expected Member, got {:?}",
            get_expression(&root, &statements[0])
        );
    };

    assert!(matches!(member.expression, FlatIndex::Identifier(_)));
    if let FlatExpression::Call(call) = get_expression(&root, &member.name) {
        assert!(matches!(call.name, FlatIndex::Identifier(_)));
        let arguments = root.get_extra_indices(call.arguments_start, call.arguments_end);
        assert_eq!(arguments.len(), 1);
        assert!(matches!(arguments[0], FlatIndex::Identifier(_)));
    } else {
        panic!("expected Call as member base");
    }
}

#[test]
fn grouped_call_member_followed_by_call() {
    let (root, _) = decode("(foo bar).baz qux");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);

    let FlatExpression::Call(outer_call) = get_expression(&root, &statements[0]) else {
        panic!(
            "expected outer Call, got {:?}",
            get_expression(&root, &statements[0])
        );
    };

    let outer_arguments =
        root.get_extra_indices(outer_call.arguments_start, outer_call.arguments_end);
    assert_eq!(outer_arguments.len(), 1);
    assert!(matches!(outer_arguments[0], FlatIndex::Identifier(_)));

    let FlatExpression::Member(member) = get_expression(&root, &outer_call.name) else {
        panic!(
            "expected Member as outer callee, got {:?}",
            get_expression(&root, &outer_call.name)
        );
    };

    assert!(matches!(member.expression, FlatIndex::Identifier(_)));
    assert!(matches!(
        get_expression(&root, &member.name),
        FlatExpression::Call(_)
    ));
}

#[test]
fn hello_example_grouped_member_call_decodes() {
    let (root, _) = decode(r#"(core.getStdoutWriter void).write "Hello world!""#);
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);

    let FlatExpression::Call(write_call) = get_expression(&root, &statements[0]) else {
        panic!(
            "expected outer Call, got {:?}",
            get_expression(&root, &statements[0])
        );
    };

    let write_arguments =
        root.get_extra_indices(write_call.arguments_start, write_call.arguments_end);
    assert_eq!(write_arguments.len(), 1);
    assert!(matches!(write_arguments[0], FlatIndex::String(_)));

    let FlatExpression::Member(write_member) = get_expression(&root, &write_call.name) else {
        panic!(
            "expected Member as outer callee, got {:?}",
            get_expression(&root, &write_call.name)
        );
    };

    assert!(matches!(write_member.expression, FlatIndex::Identifier(_)));

    let FlatExpression::Call(get_writer_call) = get_expression(&root, &write_member.name) else {
        panic!(
            "expected Call as member base, got {:?}",
            get_expression(&root, &write_member.name)
        );
    };

    let get_writer_arguments = root.get_extra_indices(
        get_writer_call.arguments_start,
        get_writer_call.arguments_end,
    );
    assert_eq!(get_writer_arguments.len(), 1);
    assert!(matches!(get_writer_arguments[0], FlatIndex::Identifier(_)));
}

#[test]
fn continued_call_application_after_comma_separated_arguments() {
    let (root, _) =
        decode("for environment.arguments, {argument : String} { writer.write argument; }");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);

    let FlatExpression::Call(outer_call) = get_expression(&root, &statements[0]) else {
        panic!(
            "expected outer Call, got {:?}",
            get_expression(&root, &statements[0])
        );
    };

    let outer_arguments =
        root.get_extra_indices(outer_call.arguments_start, outer_call.arguments_end);
    assert_eq!(outer_arguments.len(), 1);
    assert!(matches!(outer_arguments[0], FlatIndex::Source(_)));

    let FlatExpression::Call(inner_call) = get_expression(&root, &outer_call.name) else {
        panic!(
            "expected inner Call as left-associated callee, got {:?}",
            get_expression(&root, &outer_call.name)
        );
    };

    assert!(matches!(inner_call.name, FlatIndex::Identifier(_)));
    let inner_arguments =
        root.get_extra_indices(inner_call.arguments_start, inner_call.arguments_end);
    assert_eq!(inner_arguments.len(), 2);
    assert!(matches!(inner_arguments[0], FlatIndex::Expression(_)));
    assert!(matches!(inner_arguments[1], FlatIndex::Source(_)));
}

#[test]
fn cat_example_for_shape_decodes() {
    let (root, _) = decode(
        r#"label : for environment.arguments, {argument : String} {
    if argument == "exit", {
        break label;
    };

    writer.write (core.File.read argument);
}"#,
    );
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);

    let FlatExpression::Constant(assign) = get_expression(&root, &statements[0]) else {
        panic!(
            "expected Constant, got {:?}",
            get_expression(&root, &statements[0])
        );
    };

    let FlatExpression::Call(outer_call) = get_expression(&root, &assign.expression) else {
        panic!(
            "expected outer Call as constant value, got {:?}",
            get_expression(&root, &assign.expression)
        );
    };

    let outer_arguments =
        root.get_extra_indices(outer_call.arguments_start, outer_call.arguments_end);
    assert_eq!(outer_arguments.len(), 1);
    assert!(matches!(outer_arguments[0], FlatIndex::Source(_)));

    let FlatExpression::Call(inner_call) = get_expression(&root, &outer_call.name) else {
        panic!(
            "expected inner Call as left-associated callee, got {:?}",
            get_expression(&root, &outer_call.name)
        );
    };

    assert!(matches!(inner_call.name, FlatIndex::Identifier(_)));
    let inner_arguments =
        root.get_extra_indices(inner_call.arguments_start, inner_call.arguments_end);
    assert_eq!(inner_arguments.len(), 2);
    assert!(matches!(inner_arguments[0], FlatIndex::Expression(_)));
    assert!(matches!(inner_arguments[1], FlatIndex::Source(_)));
}

#[test]
fn constant_with_block_value() {
    let (root, _) = decode("x : { a; b }");
    if let FlatExpression::Constant(assign) = get_expression(&root, &source_indices(&root, 0)[0]) {
        assert!(matches!(assign.expression, FlatIndex::Source(_)));
    } else {
        panic!("expected Constant");
    }
}

#[test]
fn recovery_skips_to_semicolon() {
    let (root, _, errors) = decode_errors("@@@; b");
    assert!(!errors.is_empty());
    assert!(!source_indices(&root, 0).is_empty());
}

#[test]
fn recovery_multiple_errors() {
    let (_, _, errors) = decode_errors("@; @; @");
    assert!(errors.len() >= 3);
}

#[test]
fn recovery_preserves_valid_statements() {
    let (root, _, errors) = decode_errors("a; @@@; b");
    assert!(!errors.is_empty());
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 2);
    assert!(matches!(statements[0], FlatIndex::Identifier(_)));
    assert!(matches!(statements[1], FlatIndex::Identifier(_)));
}

#[test]
fn recovery_nested_braces() {
    let (root, _, errors) = decode_errors("a; @@@ { inner; stuff }; b");
    assert!(!errors.is_empty());
    let statements = source_indices(&root, 0);
    assert!(
        statements
            .iter()
            .any(|s| matches!(s, FlatIndex::Identifier(index) if root.get_string(*index) == "a"))
    );
    assert!(
        statements
            .iter()
            .any(|s| matches!(s, FlatIndex::Identifier(index) if root.get_string(*index) == "b"))
    );
}

#[test]
fn recovery_unterminated_string_in_statement() {
    let (root, _, errors) = decode_errors(r#"a; "unterminated; b"#);
    assert!(!errors.is_empty());
    let statements = source_indices(&root, 0);
    assert!(!statements.is_empty());
    assert!(matches!(statements[0], FlatIndex::Identifier(_)));
}

#[test]
fn source_locations_single_statement() {
    let (_, locations) = decode("foo");
    let locations = locations.get(0);
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0], 0);
}

#[test]
fn source_locations_multiple_statements() {
    let (_, locations) = decode("foo; bar");
    let locations = locations.get(0);
    assert_eq!(locations.len(), 2);
    assert_eq!(locations[0], 0);
    assert_eq!(locations[1], 5);
}

#[test]
fn source_locations_with_leading_whitespace() {
    let (_, locations) = decode("  foo");
    let locations = locations.get(0);
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0], 2);
}

#[test]
fn expression_location_constant() {
    let (_, locations) = decode("x : 0");
    assert!(locations.has_expressions());
}

#[test]
fn expression_location_constant_offset() {
    let (root, locations) = decode("x : 0");
    let statements = source_indices(&root, 0);
    assert_eq!(expression_offset(&locations, &statements[0]), 0);
}

#[test]
fn expression_location_variable_offset() {
    let (root, locations) = decode("  x = 0");
    let statements = source_indices(&root, 0);
    assert_eq!(expression_offset(&locations, &statements[0]), 2);
}

#[test]
fn expression_location_call_offset() {
    let (root, locations) = decode("foo bar");
    let statements = source_indices(&root, 0);
    assert_eq!(expression_offset(&locations, &statements[0]), 0);
}

#[test]
fn expression_location_binary_op_offset() {
    let (root, locations) = decode("a + b");
    let statements = source_indices(&root, 0);
    assert_eq!(expression_offset(&locations, &statements[0]), 2);
}

#[test]
fn expression_location_member_offset() {
    let (root, locations) = decode("a.b");
    let statements = source_indices(&root, 0);
    assert_eq!(expression_offset(&locations, &statements[0]), 1);
}

#[test]
fn expression_location_multiple_variable_offset() {
    let (root, locations) = decode("  a, b = x");
    let statements = source_indices(&root, 0);
    assert_eq!(expression_offset(&locations, &statements[0]), 2);
}

#[test]
fn expression_location_nested() {
    let (root, locations) = decode("x = a + b");
    let statements = source_indices(&root, 0);
    assert_eq!(expression_offset(&locations, &statements[0]), 0);
    if let FlatExpression::Variable(assign) = get_expression(&root, &statements[0]) {
        assert_eq!(expression_offset(&locations, &assign.expression), 6);
    }
}

#[test]
fn expression_locations_parallel_to_expressions() {
    let (root, locations) = decode("a : 0; b = foo bar; c + d");
    assert_eq!(root.expressions.len(), locations.expression_count());
}

#[test]
fn round_trip_identifier() {
    round_trip("foo");
}

#[test]
fn round_trip_number() {
    round_trip("0");
    round_trip("0b1010");
    round_trip("0xFF");
}

#[test]
fn round_trip_string() {
    round_trip(r#""hello""#);
    round_trip(r#""a\"b""#);
}

#[test]
fn round_trip_constant() {
    round_trip("x:0");
}

#[test]
fn round_trip_variable() {
    round_trip("x=0");
}

#[test]
fn round_trip_multiple_variable() {
    round_trip("a,b=foo");
}

#[test]
fn round_trip_call() {
    round_trip("foo bar");
    round_trip("foo bar,baz");
}

#[test]
fn round_trip_member() {
    round_trip("a.b");
    round_trip("a.b.c");
}

#[test]
fn round_trip_binary_operations() {
    round_trip("a+b");
    round_trip("a-b");
    round_trip("a*b");
    round_trip("a/b");
    round_trip("a%b");
    round_trip("a<b");
    round_trip("a>b");
    round_trip("a<=b");
    round_trip("a>=b");
    round_trip("a==b");
    round_trip("a!=b");
}

#[test]
fn round_trip_precedence() {
    round_trip("a+b*c");
    round_trip("a*b+c");
}

#[test]
fn round_trip_parenthesized_precedence() {
    let (root, _) = decode("(a+b)*c");
    let encoded = encode_simple(&root);
    assert_eq!(encoded, "(a+b)*c");
}

#[test]
fn round_trip_block() {
    round_trip("foo {}");
    round_trip("foo {a;b}");
}

#[test]
fn round_trip_nested_blocks() {
    round_trip("foo {bar {baz}}");
}

#[test]
fn round_trip_multiple_statements() {
    round_trip("a;b;c");
}

#[test]
fn round_trip_complex() {
    round_trip("x:foo bar,baz;y=a+b*c;z.w");
}

#[test]
fn pretty_encoding_basic() {
    let (root, _) = decode("x : 0");
    assert_eq!(encode_pretty(&root), "x : 0;");
}

#[test]
fn pretty_encoding_multiple_statements() {
    let (root, _) = decode("a; b");
    assert_eq!(encode_pretty(&root), "a;\nb;");
}

#[test]
fn pretty_encoding_block_indentation() {
    let (root, _) = decode("foo { a; b }");
    let pretty = encode_pretty(&root);
    assert!(pretty.contains('\t'));
    assert!(
        pretty.contains("{\n\ta;\n\tb;\n}"),
        "unexpected: {pretty:?}"
    );
}

#[test]
fn deeply_nested_parens() {
    let (root, _) = decode("((((a))))");
    let statements = source_indices(&root, 0);
    assert_eq!(statements.len(), 1);
    assert!(matches!(statements[0], FlatIndex::Identifier(_)));
}

#[test]
fn deeply_nested_blocks() {
    let (root, _) = decode("a { b { c { d } } }");
    assert_eq!(source_indices(&root, 0).len(), 1);
}

#[test]
fn constant_with_member_name() {
    let (root, _) = decode("a.b : 0");
    if let FlatExpression::Constant(assign) = get_expression(&root, &source_indices(&root, 0)[0]) {
        assert!(matches!(
            get_expression(&root, &assign.name),
            FlatExpression::Member(_)
        ));
    } else {
        panic!("expected Constant");
    }
}

#[test]
fn variable_with_member_name() {
    let (root, _) = decode("a.b = 0");
    if let FlatExpression::Variable(assign) = get_expression(&root, &source_indices(&root, 0)[0]) {
        assert!(matches!(
            get_expression(&root, &assign.name),
            FlatExpression::Member(_)
        ));
    } else {
        panic!("expected Variable");
    }
}

#[test]
fn multiple_variable_with_member_names() {
    let (root, _) = decode("a.b, c.d = foo");
    if let FlatExpression::MultipleVariable(multi) =
        get_expression(&root, &source_indices(&root, 0)[0])
    {
        let names = root.get_extra_indices(multi.names_start, multi.names_end);
        assert_eq!(names.len(), 2);
        assert!(matches!(
            get_expression(&root, &names[0]),
            FlatExpression::Member(_)
        ));
        assert!(matches!(
            get_expression(&root, &names[1]),
            FlatExpression::Member(_)
        ));
    } else {
        panic!("expected MultipleVariable");
    }
}

#[test]
fn decode_strict_rejects_errors() {
    assert!(crate::decode("@@@").is_err());
}

#[test]
fn decode_recovering_returns_errors() {
    let (_, _, errors) = crate::decode_recovering("@@@");
    assert!(!errors.is_empty());
}
