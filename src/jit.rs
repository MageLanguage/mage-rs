use iced_x86::code_asm::*;
use serde::{Deserialize, Serialize};

use crate::{Error, FlatRoot};

pub fn compile_root(_: FlatRoot) -> Result<Bytecode, Error> {
    compile().map_err(|error| Error::CompileError(format!("{}", error)))
}

fn compile() -> Result<Bytecode, IcedError> {
    let mut assembler = CodeAssembler::new(64)?;

    assembler.mov(rax, 60u64)?;
    assembler.mov(rdi, 40u64)?;
    assembler.syscall()?;

    let code = assembler.assemble(0x1234_5678)?;

    Ok(Bytecode { code: code })
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Bytecode {
    pub code: Vec<u8>,
}
