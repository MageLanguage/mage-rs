use tree_sitter::{Language, Parser, Tree};
use tree_sitter_mage::LANGUAGE;

use crate::Error;

pub fn parse_text(text: &str) -> Result<Tree, Error> {
    let language = Language::from(LANGUAGE);

    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();

    if let Some(tree) = parser.parse(text, None) {
        Ok(tree)
    } else {
        Err(Error::ParseError("Unable to parse".to_string()))
    }
}
