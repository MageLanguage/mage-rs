use serde::{Deserialize, Serialize};

use crate::{Error, FlatRoot};

pub fn compile_root(root: FlatRoot) -> Result<Jit, Error> {
    _ = root;
    Err(Error::JitError(
        "Error: JIT compilation is not implemented.".to_string(),
    ))
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Jit {
    //
}
