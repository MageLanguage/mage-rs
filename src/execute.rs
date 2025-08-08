use mmap_rs::{MmapFlags, MmapOptions, UnsafeMmapFlags};
use serde::{Deserialize, Serialize};
use std::mem;

use crate::{Bytecode, Error};

#[repr(C)]
struct Coroutine {
    _registers: [usize; 8],
}

#[repr(C)]
struct Main {
    vector: Vector,
    result: Interface,
}

struct Vector {
    _pointer: usize,
    _length: usize,
}

#[repr(C)]
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Interface {
    pub interface_type: InterfaceType,
    pub interface_data: usize,
}

#[repr(usize)]
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum InterfaceType {
    Void,
    Number,
}

pub fn execute_bytecode(bytecode: Bytecode) -> Result<Interface, Error> {
    unsafe {
        let mut executable_map = MmapOptions::new(bytecode.code.len())
            .map_err(|error| {
                Error::ExecuteError(format!("Failed to create memory map: {}", error))
            })?
            .with_unsafe_flags(UnsafeMmapFlags::JIT)
            .map_exec_mut()
            .map_err(|error| Error::ExecuteError(format!("Failed to map memory: {}", error)))?;

        executable_map.copy_from_slice(bytecode.code.as_slice());

        let stack_map = MmapOptions::new(64 * 1024)
            .map_err(|error| {
                Error::ExecuteError(format!("Failed to create memory map: {}", error))
            })?
            .with_flags(MmapFlags::STACK)
            .map_mut()
            .map_err(|error| Error::ExecuteError(format!("Failed to map memory: {}", error)))?;

        let stack_end = stack_map.size() - 8;
        let stack_ptr = stack_map.as_ptr().add(stack_end) as *mut usize;

        *stack_ptr = executable_map.as_ptr().add(bytecode.main) as usize;

        let old = Coroutine { _registers: [0; 8] };
        let new = Coroutine {
            _registers: [0, 0, 0, 0, 0, 0, 0, stack_ptr as usize],
        };

        let call = mem::transmute::<
            *const u8,
            extern "sysv64" fn(old: &Coroutine, new: &Coroutine, main: &Main),
        >(executable_map.as_ptr());

        let hello = "Hello world!\n";

        let main = Main {
            vector: Vector {
                _pointer: hello.as_ptr() as usize,
                _length: hello.len(),
            },
            result: Interface {
                interface_type: InterfaceType::Void,
                interface_data: 0,
            },
        };

        call(&old, &new, &main);

        Ok(main.result)
    }
}
