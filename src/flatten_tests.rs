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
    fn test_definition_basic() {
        let code = "x : 0d10;";
        setup(code).unwrap();
    }

    #[test]
    fn test_definition_complex() {
        let code = "y : 0d10 - 0d2 * 0d2;";
        setup(code).unwrap();
    }
}
