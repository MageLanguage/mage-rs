use mmap_rs::{MmapFlags, MmapOptions, UnsafeMmapFlags};
use std::mem;

use crate::{Bytecode, Error};

struct Coroutine {
    _registers: [usize; 8],
}

pub fn execute_bytecode(bytecode: Bytecode) -> Result<isize, Error> {
    let mut executable_map = unsafe {
        MmapOptions::new(bytecode.code.len())
            .map_err(|error| {
                Error::ExecuteError(format!("Failed to create memory map: {}", error))
            })?
            .with_unsafe_flags(UnsafeMmapFlags::JIT)
            .map_exec_mut()
            .map_err(|error| Error::ExecuteError(format!("Failed to map memory: {}", error)))?
    };

    executable_map.copy_from_slice(bytecode.code.as_slice());

    let call = unsafe {
        mem::transmute::<
            *const u8,
            extern "sysv64" fn(old_coroutine: *const Coroutine, new_coroutine: *const Coroutine),
        >(executable_map.as_ptr())
    };

    let stack_map = MmapOptions::new(64 * 1024)
        .map_err(|error| Error::ExecuteError(format!("Failed to create memory map: {}", error)))?
        .with_flags(MmapFlags::STACK)
        .map_mut()
        .map_err(|error| Error::ExecuteError(format!("Failed to map memory: {}", error)))?;

    let old_coroutine = Coroutine { _registers: [0; 8] };
    let new_coroutine = Coroutine {
        _registers: [stack_map.end(), 0, 0, 0, 0, 0, 0, 0],
    };

    call(&old_coroutine, &new_coroutine);

    Ok(0)
}
