use mage_contract::Bytecode;
use mmap_rs::{MmapFlags, MmapOptions};

unsafe extern "C" {
    fn execute(bytecode: *mut u8, bytecode_size: u64, stack_top: *mut u8) -> u64;
}

pub struct VirtualMachine;

impl VirtualMachine {
    /// # Safety
    ///
    /// `bytecode` must be a valid Mage bytecode buffer starting with
    /// a 16-byte header followed by instructions and patch arrays.
    /// The native patcher reads the header, mutates the instruction
    /// stream in place, and begins execution.
    pub fn execute(bytecode: &mut Bytecode) -> u64 {
        let mut stack = MmapOptions::new(64 * 1024 * 1024)
            .expect("failed to create VM stack mapping")
            .with_flags(MmapFlags::STACK)
            .map_mut()
            .expect("failed to allocate VM stack");

        let stack_top = stack.as_mut_ptr().wrapping_add(stack.len());
        let data = bytecode.data_mut();

        unsafe { execute(data.as_mut_ptr(), data.len() as u64, stack_top) }
    }
}
