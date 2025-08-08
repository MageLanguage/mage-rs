use iced_x86::{BlockEncoderOptions, IcedError, code_asm::*};
use serde::{Deserialize, Serialize};

use crate::{Error, FlatRoot};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Bytecode {
    pub code: Vec<u8>,
    pub registers_swap: usize,
    pub registers_exit: usize,
    pub main: usize,
}

pub fn compile_root(_root: FlatRoot) -> Result<Bytecode, Error> {
    let compiler = Compiler::new();
    compiler
        .compile()
        .map_err(|error| Error::CompileError(format!("Failed to compile: {}", error)))
}

struct Compiler {
    assembler: CodeAssembler,
}

impl Compiler {
    fn new() -> Self {
        Self {
            assembler: CodeAssembler::new(64).unwrap(),
        }
    }

    fn compile(self) -> Result<Bytecode, IcedError> {
        let mut assembler = self.assembler;

        let mut registers_swap_label = assembler.create_label();
        let mut registers_exit_label = assembler.create_label();

        assembler.set_label(&mut registers_swap_label)?;

        assembler.mov(qword_ptr(rdi + 8), rbx)?;
        assembler.mov(qword_ptr(rdi + 16), rbp)?;
        assembler.mov(qword_ptr(rdi + 24), r12)?;
        assembler.mov(qword_ptr(rdi + 32), r13)?;
        assembler.mov(qword_ptr(rdi + 40), r14)?;
        assembler.mov(qword_ptr(rdi + 48), r15)?;
        assembler.mov(qword_ptr(rdi + 56), rsp)?;

        assembler.set_label(&mut registers_exit_label)?;

        assembler.mov(rbx, qword_ptr(rsi + 8))?;
        assembler.mov(rbp, qword_ptr(rsi + 16))?;
        assembler.mov(r12, qword_ptr(rsi + 24))?;
        assembler.mov(r13, qword_ptr(rsi + 32))?;
        assembler.mov(r14, qword_ptr(rsi + 40))?;
        assembler.mov(r15, qword_ptr(rsi + 48))?;
        assembler.mov(rsp, qword_ptr(rsi + 56))?;

        assembler.ret()?;

        let mut main_label = assembler.create_label();

        assembler.set_label(&mut main_label)?;

        assembler.push(rdi)?;
        assembler.push(rdx)?;

        assembler.mov(rax, 1u64)?;
        assembler.mov(rdi, 1u64)?;
        assembler.mov(rsi, qword_ptr(rdx + 0))?;
        assembler.mov(rdx, qword_ptr(rdx + 8))?;
        assembler.syscall()?;

        assembler.pop(rdx)?;
        assembler.pop(rsi)?;

        assembler.mov(qword_ptr(rdx + 16), 1)?;
        assembler.mov(qword_ptr(rdx + 24), rax)?;

        assembler.jmp(registers_exit_label)?;

        let result =
            assembler.assemble_options(0, BlockEncoderOptions::RETURN_NEW_INSTRUCTION_OFFSETS)?;

        let registers_swap = result.label_ip(&registers_swap_label)?;
        let registers_exit = result.label_ip(&registers_exit_label)?;
        let main = result.label_ip(&main_label)?;

        Ok(Bytecode {
            code: result.inner.code_buffer,
            registers_swap: registers_swap as usize,
            registers_exit: registers_exit as usize,
            main: main as usize,
        })
    }
}
