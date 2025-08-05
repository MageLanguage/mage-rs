use iced_x86::code_asm::*;
use serde::{Deserialize, Serialize};

use crate::{Error, FlatRoot};

pub fn compile_root(_: FlatRoot) -> Result<Bytecode, Error> {
    let mut assembler = CodeAssembler::new(64).unwrap();

    assembler.mov(rax, 60u64).unwrap();
    assembler.mov(rdi, 40u64).unwrap();
    assembler.syscall().unwrap();

    let code = assembler.assemble(0x1234_5678).unwrap();

    Ok(Bytecode { code: code })
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Bytecode {
    pub code: Vec<u8>,
}
