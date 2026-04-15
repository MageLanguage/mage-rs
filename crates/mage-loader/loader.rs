use mage_contract::{Bytecode, LoadError};
use std::fs;

pub fn load(path: &str) -> Result<String, LoadError> {
    fs::read_to_string(path).map_err(LoadError::from)
}

pub fn load_bytecode(path: &str) -> Result<Bytecode, LoadError> {
    let data = fs::read(path).map_err(LoadError::from)?;
    Ok(Bytecode::new(data))
}

pub fn save_bytecode(path: &str, bytecode: &Bytecode) -> Result<(), LoadError> {
    fs::write(path, bytecode.data()).map_err(LoadError::from)
}

pub fn load_map(path: &str) -> Result<Vec<u8>, LoadError> {
    fs::read(path).map_err(LoadError::from)
}

pub fn save_map(path: &str, data: &[u8]) -> Result<(), LoadError> {
    fs::write(path, data).map_err(LoadError::from)
}
