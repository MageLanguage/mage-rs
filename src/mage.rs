use tree_sitter::Parser;

pub struct Mage {
    thread: Thread,
}

impl Mage {
    pub fn new() -> Self {
        Mage {
            thread: Thread {
                parser: Parser::new(),
            },
        }
    }
}

pub struct Thread {
    parser: Parser,
}
