use tree_sitter::Tree;

use crate::{Error, NodeKinds};

pub fn flatten_tree(_node_kinds: &NodeKinds, _tree: &Tree, _code: &str) -> Result<(), Error> {
    Ok(())
}
