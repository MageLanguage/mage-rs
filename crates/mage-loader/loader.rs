use mage_contract::{Bytecode, LoadError};
use std::fs;

pub fn load(path: &str) -> Result<String, LoadError> {
    fs::read_to_string(path).map_err(LoadError::from)
}

pub fn load_bytecode(path: &str) -> Result<Bytecode, LoadError> {
    let data = fs::read(path).map_err(LoadError::from)?;
    Ok(Bytecode::new(data))
}
