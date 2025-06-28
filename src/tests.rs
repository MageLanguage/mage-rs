#[cfg(test)]
mod tests {

    use crate::{
        ASTIdentifier, ASTIdentifierChain, ASTName, ASTNode, ASTNumber, JITCompiler, MageError, VM,
    };
    use tree_sitter::Parser;
    use tree_sitter_mage::LANGUAGE;

    fn setup_parser() -> Parser {
        let mut parser = Parser::new();
        parser.set_language(&LANGUAGE.into()).unwrap();
        parser
    }

    fn parse_and_compile(code: &str) -> Result<JITCompiler, MageError> {
        let mut parser = setup_parser();
        let vm = VM::new().unwrap();

        let tree = parser.parse(code, None).unwrap();
        let root_node = tree.root_node();

        match vm.parse_node(&root_node, code) {
            Ok(ast_node) => {
                if let ASTNode::SourceFile(source_file) = ast_node {
                    let mut compiler = JITCompiler::new().unwrap();
                    compiler.compile_source_file(&source_file)?;
                    Ok(compiler)
                } else {
                    Err(MageError::ParseError("Expected source file".to_string()))
                }
            }
            Err(e) => Err(e),
        }
    }

    fn get_variable_value(compiler: &JITCompiler, var_name: &str) -> Option<i64> {
        compiler.get_variable_value(var_name)
    }

    #[test]
    fn test_basic_number_assignment() {
        let code = "x : [0d42];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "x"), Some(42));
    }

    #[test]
    fn test_zero_assignment() {
        let code = "zero_var : [0];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "zero_var"), Some(0));
    }

    #[test]
    fn test_binary_number() {
        let code = "bin_var : [0b1010];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "bin_var"), Some(10));
    }

    #[test]
    fn test_octal_number() {
        let code = "oct_var : [0o17];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "oct_var"), Some(15));
    }

    #[test]
    fn test_hex_number() {
        let code = "hex_var : [0xFF];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "hex_var"), Some(255));
    }

    #[test]
    fn test_basic_addition() {
        let code = "result : [0d10 + 0d5];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "result"), Some(15));
    }

    #[test]
    fn test_basic_subtraction() {
        let code = "result : [0d20 - 0d8];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "result"), Some(12));
    }

    #[test]
    fn test_basic_multiplication() {
        let code = "result : [0d6 * 0d7];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "result"), Some(42));
    }

    #[test]
    fn test_basic_division() {
        let code = "result : [0d20 / 0d4];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "result"), Some(5));
    }

    #[test]
    fn test_basic_modulo() {
        let code = "result : [0d17 % 0d5];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "result"), Some(2));
    }

    #[test]
    fn test_variable_reference() {
        let code = r#"
            x : [0d20];
            y : [x];
        "#;
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "x"), Some(20));
        assert_eq!(get_variable_value(&compiler, "y"), Some(20));
    }

    #[test]
    fn test_variable_math_operations() {
        let code = r#"
            x : [0d20];
            y : [0d10];
            add_result : [x + y];
            sub_result : [x - y];
            mul_result : [x * y];
            div_result : [x / y];
        "#;
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "x"), Some(20));
        assert_eq!(get_variable_value(&compiler, "y"), Some(10));
        assert_eq!(get_variable_value(&compiler, "add_result"), Some(30));
        assert_eq!(get_variable_value(&compiler, "sub_result"), Some(10));
        assert_eq!(get_variable_value(&compiler, "mul_result"), Some(200));
        assert_eq!(get_variable_value(&compiler, "div_result"), Some(2));
    }

    #[test]
    fn test_chained_variable_references() {
        let code = r#"
            a : [0d5];
            b : [a];
            c : [b];
            d : [c];
        "#;
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "a"), Some(5));
        assert_eq!(get_variable_value(&compiler, "b"), Some(5));
        assert_eq!(get_variable_value(&compiler, "c"), Some(5));
        assert_eq!(get_variable_value(&compiler, "d"), Some(5));
    }

    #[test]
    fn test_nested_math_expressions() {
        let code = "result : [0d20 - [0d10 - 0d5]];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "result"), Some(15));
    }

    #[test]
    fn test_complex_nested_expressions() {
        let code = r#"
            x : [0d20];
            y : [0d10];
            z : [0d5];
            result : [[x + y] * [y - z]];
        "#;
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "x"), Some(20));
        assert_eq!(get_variable_value(&compiler, "y"), Some(10));
        assert_eq!(get_variable_value(&compiler, "z"), Some(5));
        assert_eq!(get_variable_value(&compiler, "result"), Some(150)); // (20+10) * (10-5) = 30 * 5 = 150
    }

    #[test]
    fn test_deeply_nested_expressions() {
        let code = "result : [[[0d2 + 0d3] * [0d4 - 0d1]] / [0d5 + 0d10]];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "result"), Some(1)); // ((2+3) * (4-1)) / (5+10) = (5 * 3) / 15 = 15 / 15 = 1
    }

    #[test]
    fn test_left_to_right_evaluation() {
        let code = "result : [0d1 + 0d2 * 0d3];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "result"), Some(9)); // Left to right: (1 + 2) * 3 = 3 * 3 = 9
    }

    #[test]
    fn test_sequence_operations() {
        let code = "result : [0d10 - 0d3 + 0d2 * 0d4];";
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "result"), Some(36)); // Left to right: ((10 - 3) + 2) * 4 = (7 + 2) * 4 = 9 * 4 = 36
    }

    #[test]
    fn test_mixed_number_formats() {
        let code = r#"
            bin_num : [0b1010];
            oct_num : [0o12];
            dec_num : [0d10];
            hex_num : [0xA];
            result : [bin_num + oct_num + dec_num + hex_num];
        "#;
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "bin_num"), Some(10));
        assert_eq!(get_variable_value(&compiler, "oct_num"), Some(10));
        assert_eq!(get_variable_value(&compiler, "dec_num"), Some(10));
        assert_eq!(get_variable_value(&compiler, "hex_num"), Some(10));
        assert_eq!(get_variable_value(&compiler, "result"), Some(40));
    }

    #[test]
    fn test_zero_operations() {
        let code = r#"
            zero : [0];
            num : [0d42];
            add_zero : [num + zero];
            mul_zero : [num * zero];
            zero_add : [zero + num];
            zero_sub : [zero - num];
        "#;
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "zero"), Some(0));
        assert_eq!(get_variable_value(&compiler, "num"), Some(42));
        assert_eq!(get_variable_value(&compiler, "add_zero"), Some(42));
        assert_eq!(get_variable_value(&compiler, "mul_zero"), Some(0));
        assert_eq!(get_variable_value(&compiler, "zero_add"), Some(42));
        assert_eq!(get_variable_value(&compiler, "zero_sub"), Some(-42));
    }

    #[test]
    fn test_negative_results() {
        let code = r#"
            small : [0d5];
            large : [0d20];
            negative : [small - large];
        "#;
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "small"), Some(5));
        assert_eq!(get_variable_value(&compiler, "large"), Some(20));
        assert_eq!(get_variable_value(&compiler, "negative"), Some(-15));
    }

    #[test]
    fn test_large_numbers() {
        let code = r#"
            large1 : [0d1000];
            large2 : [0d2000];
            sum : [large1 + large2];
            product : [0d100 * 0d50];
        "#;
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "large1"), Some(1000));
        assert_eq!(get_variable_value(&compiler, "large2"), Some(2000));
        assert_eq!(get_variable_value(&compiler, "sum"), Some(3000));
        assert_eq!(get_variable_value(&compiler, "product"), Some(5000));
    }

    #[test]
    fn test_undefined_variable_error() {
        let code = "result : [undefined_var];";
        let result = parse_and_compile(code);
        assert!(result.is_err());
        if let Err(MageError::RuntimeError(msg)) = result {
            assert!(msg.contains("Undefined variable: undefined_var"));
        } else {
            panic!("Expected RuntimeError for undefined variable");
        }
    }

    #[test]
    fn test_original_example() {
        let code = r#"
            x : [0d20];
            y : [0d10];
            z : [x - y];
        "#;
        let compiler = parse_and_compile(code).unwrap();
        assert_eq!(get_variable_value(&compiler, "x"), Some(20));
        assert_eq!(get_variable_value(&compiler, "y"), Some(10));
        assert_eq!(
            get_variable_value(&compiler, "z"),
            Some(10),
            "z should be 10 (20 - 10)"
        );
    }

    #[test]
    fn test_comprehensive_example() {
        let code = r#"
            x : [0d20];
            y : [0d10];
            z : [x - y];
            a : [0d5];
            b : [0d3];
            result : [a * b];
            division_test : [0d15 / 0d3];
            nested_math : [x - [y - a]];
            complex : [[x + y] * [z - a]];
        "#;
        let compiler = parse_and_compile(code).unwrap();

        assert_eq!(get_variable_value(&compiler, "x"), Some(20));
        assert_eq!(get_variable_value(&compiler, "y"), Some(10));
        assert_eq!(get_variable_value(&compiler, "z"), Some(10));
        assert_eq!(get_variable_value(&compiler, "a"), Some(5));
        assert_eq!(get_variable_value(&compiler, "b"), Some(3));
        assert_eq!(get_variable_value(&compiler, "result"), Some(15));
        assert_eq!(get_variable_value(&compiler, "division_test"), Some(5));
        assert_eq!(get_variable_value(&compiler, "nested_math"), Some(15));
        assert_eq!(get_variable_value(&compiler, "complex"), Some(150));
    }

    #[test]
    fn test_jit_compiler_creation() {
        let compiler = JITCompiler::new();
        assert!(compiler.is_ok());

        let compiler = compiler.unwrap();
        assert_eq!(compiler.stack_offset, 0);
        assert!(compiler.variables.is_empty());
    }

    #[test]
    fn test_vm_creation() {
        let vm = VM::new();
        assert!(vm.is_ok());
    }

    #[test]
    fn test_parser_setup() {
        let parser = setup_parser();
        // Just verify the parser was created successfully
        assert_eq!(parser.language().unwrap().node_kind_count() > 0, true);
    }

    #[test]
    fn test_number_parsing() {
        let compiler = JITCompiler::new().unwrap();

        // Test all number formats
        assert_eq!(compiler.parse_number(&ASTNumber::Zero).unwrap(), 0);
        assert_eq!(
            compiler
                .parse_number(&ASTNumber::Binary("0b1010".to_string()))
                .unwrap(),
            10
        );
        assert_eq!(
            compiler
                .parse_number(&ASTNumber::Octal("0o17".to_string()))
                .unwrap(),
            15
        );
        assert_eq!(
            compiler
                .parse_number(&ASTNumber::Decimal("0d42".to_string()))
                .unwrap(),
            42
        );
        assert_eq!(
            compiler
                .parse_number(&ASTNumber::Hex("0xFF".to_string()))
                .unwrap(),
            255
        );
    }

    #[test]
    fn test_identifier_chain_to_string() {
        let compiler = JITCompiler::new().unwrap();

        let chain = ASTIdentifierChain {
            identifiers: vec![ASTIdentifier::Name(ASTName {
                value: "test_var".to_string(),
            })],
        };

        assert_eq!(compiler.identifier_chain_to_string(&chain), "test_var");
    }

    #[test]
    fn test_stack_offset_tracking() {
        let code = r#"
            var1 : [0d10];
            var2 : [0d20];
            var3 : [0d30];
        "#;
        let compiler = parse_and_compile(code).unwrap();

        // Each variable should allocate 1 slot on stack
        assert_eq!(compiler.stack_offset, 3); // 3 variables
        assert_eq!(compiler.variables.len(), 3);

        // Check stack offsets (slot-based)
        assert_eq!(compiler.variables.get("var1"), Some(&1));
        assert_eq!(compiler.variables.get("var2"), Some(&2));
        assert_eq!(compiler.variables.get("var3"), Some(&3));
    }

    #[test]
    fn test_variable_values_storage() {
        let code = r#"
            test_var : [0d123];
        "#;
        let compiler = parse_and_compile(code).unwrap();

        // Variable should be stored in variables HashMap (for stack tracking)
        // and retrievable from stack
        assert!(compiler.variables.contains_key("test_var"));
        assert_eq!(compiler.get_variable_value("test_var"), Some(123));
    }
}
