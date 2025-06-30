#[cfg(test)]
mod flatten_tests {
    use tree_sitter::Parser;
    use tree_sitter_mage::LANGUAGE;

    use crate::{Error, flatten_tree};

    fn setup(code: &str) -> Result<(), Error> {
        let mut parser = Parser::new();
        parser.set_language(&LANGUAGE.into()).unwrap();

        let tree = parser.parse(code, None).unwrap();

        flatten_tree(tree, code)
    }

    #[test]
    fn test_definition_expression() {
        // Test different number formats
        let code = "x : 0d10;";
        setup(code).unwrap();

        let code = "y : 0;";
        setup(code).unwrap();

        let code = "z : 0b1010;";
        setup(code).unwrap();

        let code = "a : 0o777;";
        setup(code).unwrap();

        let code = "b : 0xFF;";
        setup(code).unwrap();

        // Test identifier references
        let code = "result : someVariable;";
        setup(code).unwrap();

        // Test string literals
        let code = "message : \"hello world\";";
        setup(code).unwrap();

        // Test variable assignment (=)
        let code = "counter = 0d42;";
        setup(code).unwrap();
    }

    #[test]
    fn test_definition_expression_with_high_precedence() {
        // Basic multiplication with subtraction
        let code = "y : 0d10 - 0d2 * 0d2;";
        setup(code).unwrap();

        // Basic division with addition
        let code = "z : 0d100 + 0d20 / 0d4;";
        setup(code).unwrap();

        // Multiple multiplications
        let code = "a : 0d1 + 0d2 * 0d3 + 0d4 * 0d5;";
        setup(code).unwrap();

        // Multiple divisions
        let code = "b : 0d100 - 0d20 / 0d4 - 0d8 / 0d2;";
        setup(code).unwrap();

        // Mixed multiplication and division
        let code = "c : 0d10 + 0d6 * 0d2 / 0d3 - 0d1;";
        setup(code).unwrap();

        // Chain of multiplications only (no low precedence ops)
        let code = "d : 0d2 * 0d3 * 0d4;";
        setup(code).unwrap();

        // Complex expression with multiple high precedence ops
        let code = "e : 0d1 + 0d2 * 0d3 + 0d4 / 0d2 * 0d5 - 0d6;";
        setup(code).unwrap();

        // Different number formats in high precedence
        let code = "f : 0xFF + 0b10 * 0o7;";
        setup(code).unwrap();
    }

    #[test]
    fn test_definition_expression_with_prioritize() {
        // Basic prioritize with subtraction
        let code = "x : 0d10 - [0d10 - 0d5];";
        setup(code).unwrap();

        // Prioritize with addition
        let code = "y : 0d5 * [0d2 + 0d3];";
        setup(code).unwrap();

        // Multiple prioritize expressions
        let code = "z : [0d1 + 0d2] * [0d3 + 0d4];";
        setup(code).unwrap();

        // Nested prioritize expressions
        let code = "a : 0d1 + [0d2 + [0d3 * 0d4]];";
        setup(code).unwrap();

        // Deeply nested prioritize
        let code = "b : [0d1 + [0d2 * [0d3 + 0d4]]];";
        setup(code).unwrap();

        // Prioritize with high precedence operations
        let code = "c : 0d10 + [0d2 * 0d3 + 0d4] * 0d5;";
        setup(code).unwrap();

        // Complex mixed case
        let code = "d : 0d1 + [0d2 + 0d3] * 0d4 + [0d5 * 0d6] - 0d7;";
        setup(code).unwrap();

        // Prioritize only (no other operations)
        let code = "e : [0d10 + 0d20];";
        setup(code).unwrap();

        // Multiple levels with different operators
        let code = "f : [0d100 / [0d10 + 0d5]] - [0d2 * 0d3];";
        setup(code).unwrap();
    }
}
