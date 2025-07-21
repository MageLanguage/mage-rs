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
    pub fn new() -> Result<Self, Error> {
        let mut mage = Mage {
            language: Language::from(LANGUAGE),
            thread: Thread {
                parser: Parser::new(),
            },
        };

        if let Err(error) = mage.thread.parser.set_language(&mage.language) {
            Err(Error::MageError(format!(
                "Unable to set language {}",
                error
            )))
        } else {
            Ok(mage)
        }
    }

    pub fn parse_text(&mut self, text: &str) -> Result<Tree, Error> {
        if let Some(tree) = self.thread.parser.parse(text, None) {
            Ok(tree)
        } else {
            Err(Error::ParseError("Unable to parse".to_string()))
        }
    }
}
