use mmap_rs::{MmapFlags, MmapOptions};
use std::mem;

use crate::{Bytecode, Error};

pub fn execute_bytecode(bytecode: Bytecode) -> Result<isize, Error> {
    let stack_map = MmapOptions::new(64 * 1024 * 1024)
        .map_err(|e| Error::ExecuteError(format!("Failed to create memory map: {}", e)))?
        .with_flags(MmapFlags::STACK)
        .map_mut()
        .map_err(|e| Error::ExecuteError(format!("Failed to map memory: {}", e)))?;

    let call = unsafe {
        mem::transmute::<Bytecode, extern "sysv64" fn(stack_ptr: *const u8) -> isize>(bytecode)
    };

    Ok(call(stack_map.as_ptr()))
}
