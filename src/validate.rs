use tree_sitter::Tree;

use crate::{Error, NodeKinds};

pub fn validate_tree(_node_kinds: &NodeKinds, _tree: &Tree, _code: &str) -> Result<(), Error> {
    Ok(())
}
