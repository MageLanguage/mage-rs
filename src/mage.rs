use tree_sitter::{Language, Parser, Tree};
use tree_sitter_mage::LANGUAGE;

use crate::Error;

pub struct Mage {
    pub language: Language,
    pub thread: Thread,
}

pub struct Thread {
    pub parser: Parser,
}

impl Mage {
    pub fn new() -> Self {
        let mut mage = Mage {
            language: Language::from(LANGUAGE),
            thread: Thread {
                parser: Parser::new(),
            },
        };

        mage.thread.parser.set_language(&mage.language).unwrap();

        mage
    }

    pub fn parse_text(&mut self, text: &str) -> Result<Tree, Error> {
        if let Some(tree) = self.thread.parser.parse(text, None) {
            Ok(tree)
        } else {
            Err(Error::ParseError("Unable to parse".to_string()))
        }
    }
}
