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
        let test_cases = [
            "x : 0d10;",                  // Decimal number
            "y : 0;",                     // Zero
            "z : 0b1010;",                // Binary number
            "a : 0o777;",                 // Octal number
            "b : 0xFF;",                  // Hex number
            "result : someVariable;",     // Identifier reference
            "message : \"hello world\";", // String literal
            "counter = 0d42;",            // Variable assignment (=)
        ];

        for code in test_cases {
            setup(code).unwrap();
        }
    }

    #[test]
    fn test_definition_expression_with_high_precedence() {
        let test_cases = [
            "y : 0d10 - 0d2 * 0d2;",            // Basic multiplication with subtraction
            "z : 0d100 + 0d20 / 0d4;",          // Basic division with addition
            "a : 0d1 + 0d2 * 0d3 + 0d4 * 0d5;", // Multiple multiplications
            "b : 0d100 - 0d20 / 0d4 - 0d8 / 0d2;", // Multiple divisions
            "c : 0d10 + 0d6 * 0d2 / 0d3 - 0d1;", // Mixed multiplication and division
            "d : 0d2 * 0d3 * 0d4;",             // Chain of multiplications only
            "e : 0d1 + 0d2 * 0d3 + 0d4 / 0d2 * 0d5 - 0d6;", // Complex expression with multiple high precedence ops
            "f : 0xFF + 0b10 * 0o7;", // Different number formats in high precedence
        ];

        for code in test_cases {
            setup(code).unwrap();
        }
    }

    #[test]
    fn test_definition_expression_with_prioritize() {
        let test_cases = [
            "x : 0d10 - [0d10 - 0d5];",            // Basic prioritize with subtraction
            "y : 0d5 * [0d2 + 0d3];",              // Prioritize with addition
            "z : [0d1 + 0d2] * [0d3 + 0d4];",      // Multiple prioritize expressions
            "a : 0d1 + [0d2 + [0d3 * 0d4]];",      // Nested prioritize expressions
            "b : [0d1 + [0d2 * [0d3 + 0d4]]];",    // Deeply nested prioritize
            "c : 0d10 + [0d2 * 0d3 + 0d4] * 0d5;", // Prioritize with high precedence operations
            "d : 0d1 + [0d2 + 0d3] * 0d4 + [0d5 * 0d6] - 0d7;", // Complex mixed case
            "e : [0d10 + 0d20];",                  // Prioritize only (no other operations)
            "f : [0d100 / [0d10 + 0d5]] - [0d2 * 0d3];", // Multiple levels with different operators
        ];

        for code in test_cases {
            setup(code).unwrap();
        }
    }

    #[test]
    fn test_definition_expression_with_leading_zero_omitted() {
        let test_cases = [
            "x : - 0d1;",                   // Simple unary minus
            "y : + 0d1;",                   // Simple unary plus
            "z : - 0d1 + 0d2;",             // Unary minus with addition
            "a : + 0d1 - 0d2;",             // Unary plus with subtraction
            "b : - 0d1 * 0d2;",             // Unary minus with multiplication
            "c : - 0d1 + 0d2 * 0d3;",       // Unary with mixed precedence
            "d : - 0d1 + 0d2 + 0d3;",       // Unary with multiple additions
            "e : - 0xFF;",                  // Unary with hex number
            "f : + 0b1010;",                // Unary with binary number
            "g : - [0d1 + 0d2];",           // Unary with prioritized expression
            "h : - 0d1 * 0d2 + 0d3 / 0d4;", // Complex unary expression
        ];

        for code in test_cases {
            setup(code).unwrap();
        }
    }
}
