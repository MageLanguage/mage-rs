use tree_sitter::Tree;

mod validate;
pub use validate::*;

mod flatten;
pub use flatten::*;

#[cfg(test)]
mod flatten_tests;

#[derive(Debug)]
pub enum ValidationError {
    InvalidNumberFormat(String),
    EmptyExpression,
    MalformedFunctionCall(String),
    InvalidIdentifierChain(String),
    IncompleteOperatorSequence,
    UnsupportedSourceBlock,
}

#[derive(Debug)]
pub enum Error {
    ValidationError(ValidationError),
    FlattenError(String),
}

pub fn process_tree(tree: &Tree, code: &str) -> Result<FlatRoot, Error> {
    validate_tree(tree, code)?;
    flatten_tree(tree, code)
}
