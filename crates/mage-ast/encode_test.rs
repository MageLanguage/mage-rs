use serde_text::Format;

use crate::{
    FlatAssign, FlatBinaryOperation, FlatBinaryOperationKind, FlatCall, FlatExpression, FlatIndex,
    FlatMember, FlatMultipleVariable, FlatRoot, FlatSource, FlatString, encode::Encoder,
};

fn decode_encode_pretty(code: &str) -> String {
    let (root, _) = crate::decode(code).unwrap();
    Encoder::encode(&root, Format::Pretty)
}

fn decode_encode_simple(code: &str) -> String {
    let (root, _) = crate::decode(code).unwrap();
    Encoder::encode(&root, Format::Simple)
}

struct Builder {
    root: FlatRoot,
}

impl Builder {
    fn new() -> Self {
        Self {
            root: FlatRoot {
                sources: vec![FlatSource { start: 0, end: 0 }],
                ..FlatRoot::default()
            },
        }
    }

    fn add_string(&mut self, s: &str) -> u32 {
        let index = self.root.strings.len() as u32;
        let start = self.root.buffer.len() as u32;
        self.root.buffer.push_str(s);
        let end = self.root.buffer.len() as u32;
        self.root.strings.push(FlatString { start, end });
        index
    }

    fn add_expression(&mut self, expression: FlatExpression) -> FlatIndex {
        let index = self.root.expressions.len() as u32;
        self.root.expressions.push(expression);
        FlatIndex::Expression(index)
    }

    fn push_indices(&mut self, indices: &[FlatIndex]) -> (u32, u32) {
        let start = self.root.indices.len() as u32;
        self.root.indices.extend_from_slice(indices);
        let end = self.root.indices.len() as u32;
        (start, end)
    }

    fn set_root_source(&mut self, statements: &[FlatIndex]) {
        let (start, end) = self.push_indices(statements);
        self.root.sources[0] = FlatSource { start, end };
    }

    fn encode_pretty(&self) -> String {
        Encoder::encode(&self.root, Format::Pretty)
    }

    fn encode_simple(&self) -> String {
        Encoder::encode(&self.root, Format::Simple)
    }
}

#[test]
fn simple_identifier() {
    assert_eq!(decode_encode_simple("foo"), "foo");
}

#[test]
fn simple_number() {
    assert_eq!(decode_encode_simple("0"), "0");
    assert_eq!(decode_encode_simple("0xFF"), "0xFF");
    assert_eq!(decode_encode_simple("0b1010"), "0b1010");
}

#[test]
fn simple_string() {
    assert_eq!(decode_encode_simple(r#""hello""#), r#""hello""#);
    assert_eq!(decode_encode_simple(r#""""#), r#""""#);
}

#[test]
fn simple_constant() {
    assert_eq!(decode_encode_simple("x : 0"), "x:0");
}

#[test]
fn simple_variable() {
    assert_eq!(decode_encode_simple("x = 0"), "x=0");
}

#[test]
fn simple_multiple_variable() {
    assert_eq!(decode_encode_simple("a , b = x"), "a,b=x");
}

#[test]
fn simple_call_single_arg() {
    assert_eq!(decode_encode_simple("foo bar"), "foo bar");
}

#[test]
fn simple_call_multiple_args() {
    assert_eq!(decode_encode_simple("foo bar , baz"), "foo bar,baz");
}

#[test]
fn simple_member() {
    assert_eq!(decode_encode_simple("a.b"), "a.b");
}

#[test]
fn simple_chained_member() {
    assert_eq!(decode_encode_simple("a.b.c"), "a.b.c");
}

#[test]
fn simple_multiple_statements() {
    assert_eq!(decode_encode_simple("a ; b ; c"), "a;b;c");
}

#[test]
fn simple_binary_all_operators() {
    assert_eq!(decode_encode_simple("a + b"), "a+b");
    assert_eq!(decode_encode_simple("a - b"), "a-b");
    assert_eq!(decode_encode_simple("a * b"), "a*b");
    assert_eq!(decode_encode_simple("a / b"), "a/b");
    assert_eq!(decode_encode_simple("a % b"), "a%b");
    assert_eq!(decode_encode_simple("a < b"), "a<b");
    assert_eq!(decode_encode_simple("a > b"), "a>b");
    assert_eq!(decode_encode_simple("a <= b"), "a<=b");
    assert_eq!(decode_encode_simple("a >= b"), "a>=b");
    assert_eq!(decode_encode_simple("a == b"), "a==b");
    assert_eq!(decode_encode_simple("a != b"), "a!=b");
}

#[test]
fn pretty_constant() {
    assert_eq!(decode_encode_pretty("x:0"), "x : 0;");
}

#[test]
fn pretty_variable() {
    assert_eq!(decode_encode_pretty("x=0"), "x = 0;");
}

#[test]
fn pretty_multiple_variable() {
    assert_eq!(decode_encode_pretty("a,b=x"), "a, b = x;");
}

#[test]
fn pretty_call_single_arg() {
    assert_eq!(decode_encode_pretty("foo bar"), "foo bar;");
}

#[test]
fn pretty_call_multiple_args() {
    assert_eq!(decode_encode_pretty("foo bar,baz"), "foo bar, baz;");
}

#[test]
fn pretty_binary_operations() {
    assert_eq!(decode_encode_pretty("a+b"), "a + b;");
    assert_eq!(decode_encode_pretty("a-b"), "a - b;");
    assert_eq!(decode_encode_pretty("a*b"), "a * b;");
    assert_eq!(decode_encode_pretty("a/b"), "a / b;");
    assert_eq!(decode_encode_pretty("a%b"), "a % b;");
    assert_eq!(decode_encode_pretty("a<b"), "a < b;");
    assert_eq!(decode_encode_pretty("a<=b"), "a <= b;");
    assert_eq!(decode_encode_pretty("a==b"), "a == b;");
    assert_eq!(decode_encode_pretty("a!=b"), "a != b;");
}

#[test]
fn pretty_member() {
    assert_eq!(decode_encode_pretty("a.b"), "a.b;");
}

#[test]
fn pretty_multiple_statements() {
    assert_eq!(decode_encode_pretty("a;b;c"), "a;\nb;\nc;");
}

#[test]
fn pretty_empty_block() {
    assert_eq!(decode_encode_pretty("foo {}"), "foo {};");
}

#[test]
fn pretty_block_single_statement() {
    assert_eq!(decode_encode_pretty("foo { a }"), "foo {\n\ta;\n};");
}

#[test]
fn pretty_block_multiple_statements() {
    assert_eq!(
        decode_encode_pretty("foo { a; b }"),
        "foo {\n\ta;\n\tb;\n};"
    );
}

#[test]
fn pretty_nested_blocks() {
    assert_eq!(
        decode_encode_pretty("foo { bar { baz } }"),
        "foo {\n\tbar {\n\t\tbaz;\n\t};\n};"
    );
}

#[test]
fn simple_procedure_constructor_syntax() {
    assert_eq!(
        decode_encode_simple("add : procedure {x : U64; y : U64}, U64 { return x + y; }"),
        "add:procedure {x:U64;y:U64},U64 {return x+y}"
    );
}

#[test]
fn pretty_procedure_constructor_syntax() {
    assert_eq!(
        decode_encode_pretty("add : procedure {x : U64; y : U64}, U64 { return x + y; }"),
        "add : procedure {\n\tx : U64;\n\ty : U64;\n}, U64 {\n\treturn x + y;\n};"
    );
}

#[test]
fn simple_grouped_callee_with_source_block_argument() {
    assert_eq!(decode_encode_simple("(foo bar) {}"), "foo bar {}");
}

#[test]
fn pretty_grouped_callee_with_source_block_argument() {
    assert_eq!(decode_encode_pretty("(foo bar) {}"), "foo bar {};");
}

#[test]
fn simple_grouped_call_member_access() {
    assert_eq!(decode_encode_simple("(foo bar).baz"), "(foo bar).baz");
}

#[test]
fn pretty_grouped_call_member_access() {
    assert_eq!(decode_encode_pretty("(foo bar).baz"), "(foo bar).baz;");
}

#[test]
fn simple_grouped_member_call() {
    assert_eq!(
        decode_encode_simple("(foo bar).baz qux"),
        "(foo bar).baz qux"
    );
}

#[test]
fn pretty_grouped_member_call() {
    assert_eq!(
        decode_encode_pretty("(foo bar).baz qux"),
        "(foo bar).baz qux;"
    );
}

#[test]
fn simple_hello_example_grouped_member_call() {
    assert_eq!(
        decode_encode_simple(r#"(core.getStdoutWriter void).write "Hello world!""#),
        r#"(core.getStdoutWriter void).write "Hello world!""#
    );
}

#[test]
fn pretty_hello_example_grouped_member_call() {
    assert_eq!(
        decode_encode_pretty(r#"(core.getStdoutWriter void).write "Hello world!""#),
        r#"(core.getStdoutWriter void).write "Hello world!";"#
    );
}

#[test]
fn simple_continued_call_application_after_comma_list() {
    assert_eq!(
        decode_encode_simple(
            "for environment.arguments, {argument : String} { writer.write argument; }"
        ),
        "for environment.arguments,{argument:String} {writer.write argument}"
    );
}

#[test]
fn pretty_continued_call_application_after_comma_list() {
    assert_eq!(
        decode_encode_pretty(
            "for environment.arguments, {argument : String} { writer.write argument; }"
        ),
        "for environment.arguments, {\n\targument : String;\n} {\n\twriter.write argument;\n};"
    );
}

#[test]
fn simple_left_associative_return_call_chain() {
    assert_eq!(
        decode_encode_simple("return add 0d3, 0d4"),
        "return add 0d3,0d4"
    );
}

#[test]
fn pretty_left_associative_return_call_chain() {
    assert_eq!(
        decode_encode_pretty("return add 0d3, 0d4"),
        "return add 0d3, 0d4;"
    );
}

#[test]
fn simple_left_associative_long_call_chain() {
    assert_eq!(
        decode_encode_simple("math.subtract math.add 0d5, 0d5, 0d10"),
        "math.subtract math.add 0d5,0d5,0d10"
    );
}

#[test]
fn pretty_left_associative_long_call_chain() {
    assert_eq!(
        decode_encode_pretty("math.subtract math.add 0d5, 0d5, 0d10"),
        "math.subtract math.add 0d5, 0d5, 0d10;"
    );
}

#[test]
fn simple_cat_example_continued_call_application_shape() {
    assert_eq!(
        decode_encode_simple(
            r#"label : for environment.arguments, {argument : String} {
    if argument == "exit", {
        break label;
    };

    writer.write (core.File.read argument);
}"#,
        ),
        r#"label:for environment.arguments,{argument:String} {if argument=="exit",{break label};writer.write (core.File.read argument)}"#
    );
}

#[test]
fn pretty_cat_example_continued_call_application_shape() {
    assert_eq!(
        decode_encode_pretty(
            r#"label : for environment.arguments, {argument : String} {
    if argument == "exit", {
        break label;
    };

    writer.write (core.File.read argument);
}"#,
        ),
        "label : for environment.arguments, {\n\targument : String;\n} {\n\tif argument == \"exit\", {\n\t\tbreak label;\n\t};\n\twriter.write (core.File.read argument);\n};"
    );
}

#[test]
fn precedence_no_parens_needed() {
    assert_eq!(decode_encode_simple("a + b * c"), "a+b*c");
}

#[test]
fn precedence_parens_on_left() {
    assert_eq!(decode_encode_simple("(a + b) * c"), "(a+b)*c");
}

#[test]
fn precedence_parens_on_right() {
    assert_eq!(decode_encode_simple("a * (b + c)"), "a*(b+c)");
}

#[test]
fn precedence_right_associativity_needs_parens() {
    assert_eq!(decode_encode_simple("a - (b - c)"), "a-(b-c)");
}

#[test]
fn precedence_left_associativity_no_parens() {
    assert_eq!(decode_encode_simple("a - b - c"), "a-b-c");
}

#[test]
fn precedence_same_level_no_parens() {
    assert_eq!(decode_encode_simple("a + b - c"), "a+b-c");
}

#[test]
fn precedence_mul_div_no_parens() {
    assert_eq!(decode_encode_simple("a * b / c"), "a*b/c");
}

#[test]
fn precedence_comparison_with_arithmetic() {
    assert_eq!(decode_encode_simple("a + b < c * d"), "a+b<c*d");
}

#[test]
fn precedence_pretty_parens() {
    assert_eq!(decode_encode_pretty("(a + b) * c"), "(a + b) * c;");
}

#[test]
fn call_as_subexpression_gets_parens() {
    let (root, _) = crate::decode("a + (foo bar)").unwrap();
    let simple = Encoder::encode(&root, Format::Simple);
    assert_eq!(simple, "a+(foo bar)");
}

#[test]
fn constant_as_subexpression_gets_parens() {
    let (root, _) = crate::decode("foo (x : 0)").unwrap();
    let simple = Encoder::encode(&root, Format::Simple);
    assert!(simple.contains("(x:0)"), "got: {simple}");
}

#[test]
fn variable_as_subexpression_gets_parens() {
    let (root, _) = crate::decode("foo (x = 0)").unwrap();
    let simple = Encoder::encode(&root, Format::Simple);
    assert!(simple.contains("(x=0)"), "got: {simple}");
}

#[test]
fn implicit_member_encoding() {
    let mut b = Builder::new();
    let field = b.add_string("field");
    let member_expr = b.add_expression(FlatExpression::Member(FlatMember {
        name: FlatIndex::None,
        expression: FlatIndex::Identifier(field),
    }));
    let name = b.add_string("foo");
    let (args_start, args_end) = b.push_indices(&[member_expr]);
    let call_expr = b.add_expression(FlatExpression::Call(FlatCall {
        name: FlatIndex::Identifier(name),
        arguments_start: args_start,
        arguments_end: args_end,
    }));
    b.set_root_source(&[call_expr]);

    assert_eq!(b.encode_simple(), "foo .field");
    assert_eq!(b.encode_pretty(), "foo .field;");
}

#[test]
fn builder_empty_source() {
    let b = Builder::new();
    assert_eq!(b.encode_simple(), "");
    assert_eq!(b.encode_pretty(), "");
}

#[test]
fn builder_single_identifier() {
    let mut b = Builder::new();
    let index = b.add_string("hello");
    b.set_root_source(&[FlatIndex::Identifier(index)]);
    assert_eq!(b.encode_simple(), "hello");
    assert_eq!(b.encode_pretty(), "hello;");
}

#[test]
fn builder_single_number() {
    let mut b = Builder::new();
    let index = b.add_string("0xFF");
    b.set_root_source(&[FlatIndex::Number(index)]);
    assert_eq!(b.encode_simple(), "0xFF");
}

#[test]
fn builder_single_string() {
    let mut b = Builder::new();
    let index = b.add_string(r#""hi""#);
    b.set_root_source(&[FlatIndex::String(index)]);
    assert_eq!(b.encode_simple(), r#""hi""#);
}

#[test]
fn builder_constant() {
    let mut b = Builder::new();
    let name = b.add_string("x");
    let val = b.add_string("0");
    let expression = b.add_expression(FlatExpression::Constant(FlatAssign {
        name: FlatIndex::Identifier(name),
        expression: FlatIndex::Number(val),
    }));
    b.set_root_source(&[expression]);
    assert_eq!(b.encode_simple(), "x:0");
    assert_eq!(b.encode_pretty(), "x : 0;");
}

#[test]
fn builder_variable() {
    let mut b = Builder::new();
    let name = b.add_string("x");
    let val = b.add_string("0");
    let expression = b.add_expression(FlatExpression::Variable(FlatAssign {
        name: FlatIndex::Identifier(name),
        expression: FlatIndex::Number(val),
    }));
    b.set_root_source(&[expression]);
    assert_eq!(b.encode_simple(), "x=0");
    assert_eq!(b.encode_pretty(), "x = 0;");
}

#[test]
fn builder_multiple_variable() {
    let mut b = Builder::new();
    let a = b.add_string("a");
    let b_name = b.add_string("b");
    let val = b.add_string("x");
    let (ns, ne) = b.push_indices(&[FlatIndex::Identifier(a), FlatIndex::Identifier(b_name)]);
    let expression = b.add_expression(FlatExpression::MultipleVariable(FlatMultipleVariable {
        names_start: ns,
        names_end: ne,
        expression: FlatIndex::Identifier(val),
    }));
    b.set_root_source(&[expression]);
    assert_eq!(b.encode_simple(), "a,b=x");
    assert_eq!(b.encode_pretty(), "a, b = x;");
}

#[test]
fn builder_member() {
    let mut b = Builder::new();
    let obj = b.add_string("obj");
    let field = b.add_string("field");
    let expression = b.add_expression(FlatExpression::Member(FlatMember {
        name: FlatIndex::Identifier(obj),
        expression: FlatIndex::Identifier(field),
    }));
    b.set_root_source(&[expression]);
    assert_eq!(b.encode_simple(), "obj.field");
}

#[test]
fn builder_binary_operation() {
    let mut b = Builder::new();
    let a = b.add_string("a");
    let b_name = b.add_string("b");
    let expression = b.add_expression(FlatExpression::BinaryOperation(FlatBinaryOperation {
        kind: FlatBinaryOperationKind::Add,
        left: FlatIndex::Identifier(a),
        right: FlatIndex::Identifier(b_name),
    }));
    b.set_root_source(&[expression]);
    assert_eq!(b.encode_simple(), "a+b");
    assert_eq!(b.encode_pretty(), "a + b;");
}

#[test]
fn builder_call_no_args() {
    let mut b = Builder::new();
    let name = b.add_string("foo");
    let (s, e) = b.push_indices(&[]);
    let expression = b.add_expression(FlatExpression::Call(FlatCall {
        name: FlatIndex::Identifier(name),
        arguments_start: s,
        arguments_end: e,
    }));
    b.set_root_source(&[expression]);
    assert_eq!(b.encode_simple(), "foo");
}

#[test]
fn builder_call_with_args() {
    let mut b = Builder::new();
    let name = b.add_string("foo");
    let arg1 = b.add_string("bar");
    let arg2 = b.add_string("baz");
    let (s, e) = b.push_indices(&[FlatIndex::Identifier(arg1), FlatIndex::Identifier(arg2)]);
    let expression = b.add_expression(FlatExpression::Call(FlatCall {
        name: FlatIndex::Identifier(name),
        arguments_start: s,
        arguments_end: e,
    }));
    b.set_root_source(&[expression]);
    assert_eq!(b.encode_simple(), "foo bar,baz");
    assert_eq!(b.encode_pretty(), "foo bar, baz;");
}

#[test]
fn builder_nested_binary_precedence() {
    let mut b = Builder::new();
    let a = b.add_string("a");
    let b_name = b.add_string("b");
    let c_name = b.add_string("c");
    let addition = b.add_expression(FlatExpression::BinaryOperation(FlatBinaryOperation {
        kind: FlatBinaryOperationKind::Add,
        left: FlatIndex::Identifier(a),
        right: FlatIndex::Identifier(b_name),
    }));
    let multiply = b.add_expression(FlatExpression::BinaryOperation(FlatBinaryOperation {
        kind: FlatBinaryOperationKind::Multiply,
        left: addition,
        right: FlatIndex::Identifier(c_name),
    }));
    b.set_root_source(&[multiply]);
    assert_eq!(b.encode_simple(), "(a+b)*c");
    assert_eq!(b.encode_pretty(), "(a + b) * c;");
}

#[test]
fn builder_nested_binary_no_parens_needed() {
    let mut b = Builder::new();
    let a = b.add_string("a");
    let b_name = b.add_string("b");
    let c_name = b.add_string("c");
    let multiply = b.add_expression(FlatExpression::BinaryOperation(FlatBinaryOperation {
        kind: FlatBinaryOperationKind::Multiply,
        left: FlatIndex::Identifier(b_name),
        right: FlatIndex::Identifier(c_name),
    }));
    let addition = b.add_expression(FlatExpression::BinaryOperation(FlatBinaryOperation {
        kind: FlatBinaryOperationKind::Add,
        left: FlatIndex::Identifier(a),
        right: multiply,
    }));
    b.set_root_source(&[addition]);
    assert_eq!(b.encode_simple(), "a+b*c");
}

#[test]
fn builder_right_associativity_parens() {
    let mut b = Builder::new();
    let a = b.add_string("a");
    let b_name = b.add_string("b");
    let c_name = b.add_string("c");
    let inner = b.add_expression(FlatExpression::BinaryOperation(FlatBinaryOperation {
        kind: FlatBinaryOperationKind::Subtract,
        left: FlatIndex::Identifier(b_name),
        right: FlatIndex::Identifier(c_name),
    }));
    let outer = b.add_expression(FlatExpression::BinaryOperation(FlatBinaryOperation {
        kind: FlatBinaryOperationKind::Subtract,
        left: FlatIndex::Identifier(a),
        right: inner,
    }));
    b.set_root_source(&[outer]);
    assert_eq!(b.encode_simple(), "a-(b-c)");
}

#[test]
fn builder_left_associativity_no_parens() {
    let mut b = Builder::new();
    let a = b.add_string("a");
    let b_name = b.add_string("b");
    let c_name = b.add_string("c");
    let inner = b.add_expression(FlatExpression::BinaryOperation(FlatBinaryOperation {
        kind: FlatBinaryOperationKind::Subtract,
        left: FlatIndex::Identifier(a),
        right: FlatIndex::Identifier(b_name),
    }));
    let outer = b.add_expression(FlatExpression::BinaryOperation(FlatBinaryOperation {
        kind: FlatBinaryOperationKind::Subtract,
        left: inner,
        right: FlatIndex::Identifier(c_name),
    }));
    b.set_root_source(&[outer]);
    assert_eq!(b.encode_simple(), "a-b-c");
}

#[test]
fn builder_empty_block() {
    let mut b = Builder::new();
    let (s, e) = b.push_indices(&[]);
    b.root.sources.push(FlatSource { start: s, end: e });
    let name = b.add_string("foo");
    let (as_, ae) = b.push_indices(&[FlatIndex::Source(1)]);
    let call = b.add_expression(FlatExpression::Call(FlatCall {
        name: FlatIndex::Identifier(name),
        arguments_start: as_,
        arguments_end: ae,
    }));
    b.set_root_source(&[call]);
    assert_eq!(b.encode_simple(), "foo {}");
}

#[test]
fn builder_block_with_content() {
    let mut b = Builder::new();
    let a = b.add_string("a");
    let b_name = b.add_string("b");
    let (s, e) = b.push_indices(&[FlatIndex::Identifier(a), FlatIndex::Identifier(b_name)]);
    b.root.sources.push(FlatSource { start: s, end: e });
    let name = b.add_string("foo");
    let (as_, ae) = b.push_indices(&[FlatIndex::Source(1)]);
    let call = b.add_expression(FlatExpression::Call(FlatCall {
        name: FlatIndex::Identifier(name),
        arguments_start: as_,
        arguments_end: ae,
    }));
    b.set_root_source(&[call]);
    assert_eq!(b.encode_simple(), "foo {a;b}");
    assert_eq!(b.encode_pretty(), "foo {\n\ta;\n\tb;\n};");
}

#[test]
fn builder_multiple_statements_simple() {
    let mut b = Builder::new();
    let a = b.add_string("a");
    let b_name = b.add_string("b");
    let c_name = b.add_string("c");
    b.set_root_source(&[
        FlatIndex::Identifier(a),
        FlatIndex::Identifier(b_name),
        FlatIndex::Identifier(c_name),
    ]);
    assert_eq!(b.encode_simple(), "a;b;c");
    assert_eq!(b.encode_pretty(), "a;\nb;\nc;");
}

#[test]
fn encode_pretty() {
    let (root, _) = crate::decode("x : 0").unwrap();
    assert_eq!(Encoder::encode(&root, Format::Pretty), "x : 0;");
}

#[test]
fn encode_simple() {
    let (root, _) = crate::decode("x : 0").unwrap();
    assert_eq!(Encoder::encode(&root, Format::Simple), "x:0");
}

#[test]
fn left_associative_call_chain_stays_unwrapped() {
    // Left-associative call chains should round-trip without inserting
    // grouping parens around a call used as the next callee.
    let (root, _) = crate::decode("foo bar baz").unwrap();
    assert_eq!(Encoder::encode(&root, Format::Simple), "foo bar baz");
}

#[test]
fn all_binary_operators_pretty_spacing() {
    let cases = [
        ("a+b", "a + b;"),
        ("a-b", "a - b;"),
        ("a*b", "a * b;"),
        ("a/b", "a / b;"),
        ("a%b", "a % b;"),
        ("a<b", "a < b;"),
        ("a>b", "a > b;"),
        ("a<=b", "a <= b;"),
        ("a>=b", "a >= b;"),
        ("a==b", "a == b;"),
        ("a!=b", "a != b;"),
    ];
    for (input, expected) in cases {
        assert_eq!(decode_encode_pretty(input), expected, "input: {input}");
    }
}

#[test]
fn all_binary_operators_simple_no_spacing() {
    let cases = [
        ("a + b", "a+b"),
        ("a - b", "a-b"),
        ("a * b", "a*b"),
        ("a / b", "a/b"),
        ("a % b", "a%b"),
        ("a < b", "a<b"),
        ("a > b", "a>b"),
        ("a <= b", "a<=b"),
        ("a >= b", "a>=b"),
        ("a == b", "a==b"),
        ("a != b", "a!=b"),
    ];
    for (input, expected) in cases {
        assert_eq!(decode_encode_simple(input), expected, "input: {input}");
    }
}

#[test]
fn none_index_produces_no_output() {
    let mut b = Builder::new();
    b.set_root_source(&[FlatIndex::None]);
    assert_eq!(b.encode_simple(), "");
}
