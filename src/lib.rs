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

/// Process a tree by validating it first, then flattening it
/// This is the recommended way to process Mage code
pub fn process_tree(tree: Tree, code: &str) -> Result<FlatRoot, Error> {
    // Validate the tree first to catch errors early
    validate_tree(tree.clone(), code)?;
    // If validation passes, proceed with flattening
    flatten_tree(tree, code)
}
