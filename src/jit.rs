use iced_x86::{BlockEncoderOptions, IcedError, code_asm::*};
use serde::{Deserialize, Serialize};

use crate::{Error, FlatBinary, FlatExpression, FlatIndex, FlatOperator, FlatRoot, FlatSource};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Bytecode {
    pub code: Vec<u8>,
    pub registers_swap: usize,
    pub registers_exit: usize,
    pub main: usize,
}

pub fn compile_root(root: FlatRoot) -> Result<Bytecode, Error> {
    let mut compiler = Compiler::new(root);
    compiler
        .compile()
        .map_err(|error| Error::CompileError(format!("Failed to compile: {}", error)))
}

struct Compiler {
    assembler: CodeAssembler,
    root: FlatRoot,
    // Store labels for strings to reference them
    string_labels: Vec<CodeLabel>,
}

impl Compiler {
    fn new(root: FlatRoot) -> Self {
        Self {
            assembler: CodeAssembler::new(64).unwrap(),
            root,
            string_labels: vec![],
        }
    }

    fn compile(&mut self) -> Result<Bytecode, IcedError> {
        let assembler = &mut self.assembler;

        // Create labels for all strings upfront
        for _ in &self.root.strings {
            self.string_labels.push(assembler.create_label());
        }

        let mut registers_swap_label = assembler.create_label();
        let mut registers_exit_label = assembler.create_label();

        // --- Context Switch Prologue ---
        assembler.set_label(&mut registers_swap_label)?;

        // Save Callee-saved registers to 'old' coroutine (RDI)
        assembler.mov(qword_ptr(rdi + 8), rbx)?;
        assembler.mov(qword_ptr(rdi + 16), rbp)?;
        assembler.mov(qword_ptr(rdi + 24), r12)?;
        assembler.mov(qword_ptr(rdi + 32), r13)?;
        assembler.mov(qword_ptr(rdi + 40), r14)?;
        assembler.mov(qword_ptr(rdi + 48), r15)?;
        assembler.mov(qword_ptr(rdi + 56), rsp)?;

        // --- Context Switch Epilogue ---
        assembler.set_label(&mut registers_exit_label)?;

        // Restore Callee-saved registers from 'new' coroutine (RSI)
        assembler.mov(rbx, qword_ptr(rsi + 8))?;
        assembler.mov(rbp, qword_ptr(rsi + 16))?;
        assembler.mov(r12, qword_ptr(rsi + 24))?;
        assembler.mov(r13, qword_ptr(rsi + 32))?;
        assembler.mov(r14, qword_ptr(rsi + 40))?;
        assembler.mov(r15, qword_ptr(rsi + 48))?;
        assembler.mov(rsp, qword_ptr(rsi + 56))?;

        assembler.ret()?;

        // --- Main Entry Point ---
        let mut main_label = assembler.create_label();
        assembler.set_label(&mut main_label)?;

        // Save arguments pointer (rdx) if needed, though we don't use it yet in this simple JIT
        // But the calling convention says RDX has &mut Runtime.
        // We might want to preserve it or use it.
        assembler.push(rdi)?;
        assembler.push(rsi)?;

        // Compile the last source (assumed to be the main file content)
        if let Some(source) = self.root.sources.last() {
            // We clone the source to avoid borrow checker issues while mutating assembler
            // In a real implementation we might pass references carefully.
            let source = source.clone();
            Self::compile_source(assembler, &self.root, &self.string_labels, &source)?;
        }

        assembler.pop(rdi)?;
        assembler.pop(rsi)?;

        // Jump to exit to restore old context and return to Rust
        assembler.jmp(registers_swap_label)?;

        // --- Data Section (Strings) ---
        // Emit string data after the code
        for (i, string) in self.root.strings.iter().enumerate() {
            let label = &mut self.string_labels[i];
            assembler.set_label(label)?;
            // Emit bytes
            assembler.db(string.0.as_bytes())?;
            // Null terminator just in case, though we usually use length
            assembler.db(&[0])?;
        }

        // --- Final Assembly ---
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

    fn compile_source(
        assembler: &mut CodeAssembler,
        root: &FlatRoot,
        string_labels: &[CodeLabel],
        source: &FlatSource,
    ) -> Result<(), IcedError> {
        for expression in &source.expressions {
            Self::compile_expression(assembler, root, string_labels, source, expression)?;
        }
        Ok(())
    }

    fn compile_expression(
        assembler: &mut CodeAssembler,
        root: &FlatRoot,
        string_labels: &[CodeLabel],
        source: &FlatSource,
        expression: &FlatExpression,
    ) -> Result<(), IcedError> {
        match expression {
            FlatExpression::Number(index) => {
                if let FlatIndex::Number(idx) = index {
                    let number = &root.numbers[*idx];
                    assembler.mov(rax, number.0)?;
                }
            }
            FlatExpression::String(index) => {
                if let FlatIndex::String(idx) = index {
                    let string = &root.strings[*idx];
                    let label = string_labels[*idx];
                    // Load address of string into RAX
                    assembler.lea(rax, ptr(label))?;
                    // Load length of string into RDX
                    assembler.mov(rdx, string.0.len() as u64)?;
                }
            }
            FlatExpression::Call(binary) | FlatExpression::Member(binary) => {
                // Handle Pipe (=>) specially for syscalls
                if binary.operator == FlatOperator::Pipe {
                    Self::compile_pipe(assembler, root, string_labels, source, binary)?;
                } else {
                    // Generic binary op compilation (simplified)
                    if let Some(left) = &binary.one {
                        Self::compile_index(assembler, root, string_labels, source, left)?;
                    }
                    // For now, ignore RHS unless it's special.
                }
            }
            // Handle other expressions...
            _ => {}
        }
        Ok(())
    }

    fn compile_index(
        assembler: &mut CodeAssembler,
        root: &FlatRoot,
        string_labels: &[CodeLabel],
        source: &FlatSource,
        index: &FlatIndex,
    ) -> Result<(), IcedError> {
        match index {
            FlatIndex::Expression(idx) => {
                let expr = &source.expressions[*idx];
                Self::compile_expression(assembler, root, string_labels, source, expr)?;
            }
            FlatIndex::Number(idx) => {
                let number = &root.numbers[*idx];
                assembler.mov(rax, number.0)?;
            }
            FlatIndex::String(idx) => {
                let string = &root.strings[*idx];
                let label = string_labels[*idx];
                assembler.lea(rax, ptr(label))?;
                assembler.mov(rdx, string.0.len() as u64)?;
            }
            FlatIndex::Source(idx) => {
                // If we encounter a nested source (block), we compile it.
                // This executes the block.
                let nested_source = &root.sources[*idx];
                Self::compile_source(assembler, root, string_labels, nested_source)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn compile_pipe(
        assembler: &mut CodeAssembler,
        root: &FlatRoot,
        string_labels: &[CodeLabel],
        source: &FlatSource,
        binary: &FlatBinary,
    ) -> Result<(), IcedError> {
        // Check for "syscall" identifier on RHS
        let is_syscall = if let FlatIndex::Identifier(id_idx) = binary.two {
            if let Some(ident) = source.identifiers.get(id_idx) {
                ident.0 == "syscall"
            } else {
                false
            }
        } else {
            false
        };

        if is_syscall {
            // LHS should be the arguments structure: {sys_num; {args...}}
            if let Some(lhs_index) = &binary.one {
                Self::emit_syscall(assembler, root, string_labels, source, lhs_index)?;
            }
        } else {
            // Standard pipe: compile LHS, then RHS.
            if let Some(left) = &binary.one {
                Self::compile_index(assembler, root, string_labels, source, left)?;
            }
            // For now, we don't support calling other functions via JIT in this snippet
            // properly without resolving exports, but we can compile RHS.
            Self::compile_index(assembler, root, string_labels, source, &binary.two)?;
        }
        Ok(())
    }

    fn emit_syscall(
        assembler: &mut CodeAssembler,
        root: &FlatRoot,
        string_labels: &[CodeLabel],
        _source: &FlatSource,
        args_index: &FlatIndex,
    ) -> Result<(), IcedError> {
        // We expect args_index to point to a Source (Block) like {sys_num; args_tuple}
        // In Flattened tree, `{...}` is a FlatSource.
        if let FlatIndex::Source(src_idx) = args_index {
            let arg_source = &root.sources[*src_idx];
            // Expect at least 2 expressions: [sys_num, args_tuple]
            if arg_source.expressions.len() >= 2 {
                // 1. Syscall Number
                // Compile the first expression. Result in RAX.
                Self::compile_expression(
                    assembler,
                    root,
                    string_labels,
                    arg_source,
                    &arg_source.expressions[0],
                )?;
                // Syscall number goes to RAX.
                // But we need to compile arguments next, which clobbers RAX.
                assembler.push(rax)?;

                // 2. Arguments Tuple
                // The second expression should be another Source (tuple of args).
                let args_expr = &arg_source.expressions[1];

                // Expect the structure {sys_num; {arg1; arg2; arg3}}
                // We manually unpack the second expression if it corresponds to a nested source.

                let mut tuple_source_opt = None;
                match args_expr {
                    FlatExpression::Member(bin) | FlatExpression::Call(bin) => {
                        if let Some(FlatIndex::Source(tuple_idx)) = bin.one {
                            tuple_source_opt = Some(&root.sources[tuple_idx]);
                        }
                    }
                    _ => {}
                }

                if let Some(tuple_source) = tuple_source_opt {
                    // Compile args
                    // Arg 1 -> RDI
                    if tuple_source.expressions.len() > 0 {
                        Self::compile_expression(
                            assembler,
                            root,
                            string_labels,
                            tuple_source,
                            &tuple_source.expressions[0],
                        )?;
                        assembler.mov(rdi, rax)?;
                    }
                    // Arg 2 -> RSI
                    if tuple_source.expressions.len() > 1 {
                        Self::compile_expression(
                            assembler,
                            root,
                            string_labels,
                            tuple_source,
                            &tuple_source.expressions[1],
                        )?;
                        assembler.mov(rsi, rax)?;
                        // If arg 2 was a string, RDX has len.
                        // syscall write needs: RDI(fd), RSI(ptr), RDX(len).
                        // If the expression was a String, RAX=ptr, RDX=len.
                        // We moved RAX to RSI. RDX is already correct!
                    }
                    // Arg 3 -> RDX
                    if tuple_source.expressions.len() > 2 {
                        assembler.push(rdx)?; // Save previous RDX (len) just in case?
                        Self::compile_expression(
                            assembler,
                            root,
                            string_labels,
                            tuple_source,
                            &tuple_source.expressions[2],
                        )?;
                        // If this overrides RDX...
                        // For `write`, the 3rd arg is length.
                        // But for `String` expression, we set RDX implicitly.
                        // If the user explicitly passed length as 3rd arg:
                        assembler.mov(rdx, rax)?;
                        // Discard saved RDX
                        assembler.add(rsp, 8)?;
                    }
                }

                // Restore syscall number to RAX
                assembler.pop(rax)?;
                assembler.syscall()?;
            }
        }
        Ok(())
    }
}
