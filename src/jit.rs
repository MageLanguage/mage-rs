use iced_x86::code_asm::*;
use iced_x86::{BlockEncoderOptions, IcedError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{FlatBinary, FlatExpression, FlatIndex, FlatRoot, FlatSource};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Bytecode {
    pub code: Vec<u8>,
    pub registers_swap: usize,
    pub registers_exit: usize,
    pub main: usize,
    pub root: FlatRoot,
}

pub fn compile_root(root: &FlatRoot) -> Result<Bytecode, crate::Error> {
    let mut compiler = Compiler::new(root);
    let code = compiler
        .compile(root)
        .map_err(|e| crate::Error::CompileError(e.to_string()))?;
    Ok(Bytecode {
        code,
        registers_swap: compiler.registers_swap,
        registers_exit: compiler.registers_exit,
        main: compiler.main_offset,
        root: root.clone(),
    })
}

pub fn compile_lazy(root: &FlatRoot, source_index: usize) -> Result<Vec<u8>, crate::Error> {
    let mut compiler = Compiler::new(root);
    compiler
        .compile_lazy_source(source_index)
        .map_err(|e| crate::Error::CompileError(e.to_string()))
}

struct Compiler<'a> {
    assembler: CodeAssembler,
    root: &'a FlatRoot,
    string_labels: Vec<CodeLabel>,
    identifier_labels: HashMap<String, CodeLabel>,
    is_function: bool,
    exit_label: CodeLabel,
    func_epilogue_label: CodeLabel,
    registers_swap: usize,
    registers_exit: usize,
    main_offset: usize,
}

impl<'a> Compiler<'a> {
    fn new(root: &'a FlatRoot) -> Self {
        let mut assembler = CodeAssembler::new(64).unwrap();
        let exit_label = assembler.create_label();
        let func_epilogue_label = assembler.create_label();
        Self {
            assembler,
            root,
            string_labels: vec![],
            identifier_labels: HashMap::new(),
            is_function: false,
            exit_label,
            func_epilogue_label,
            registers_swap: 0,
            registers_exit: 0,
            main_offset: 0,
        }
    }

    fn get_identifier_label(&mut self, name: &str) -> CodeLabel {
        if let Some(label) = self.identifier_labels.get(name) {
            *label
        } else {
            let label = self.assembler.create_label();
            self.identifier_labels.insert(name.to_string(), label);
            label
        }
    }

    fn compile(&mut self, root: &FlatRoot) -> Result<Vec<u8>, IcedError> {
        // Create labels for strings
        for _ in &root.strings {
            self.string_labels.push(self.assembler.create_label());
        }

        let mut swap_label = self.assembler.create_label();
        let mut main_label = self.assembler.create_label();
        let mut exit_label = self.exit_label;

        // 1. Swap Routine
        // Used to switch between coroutines (Old -> New)
        // Entry: rdi = Old (Save here), rsi = New (Restore from here)
        self.assembler.set_label(&mut swap_label)?;

        // Save Callee-Saved Registers to 'Old' (RDI)
        // Offsets match Coroutine struct
        self.assembler.mov(qword_ptr(rdi + 0), rbx)?;
        self.assembler.mov(qword_ptr(rdi + 8), rbp)?;
        self.assembler.mov(qword_ptr(rdi + 16), r12)?;
        self.assembler.mov(qword_ptr(rdi + 24), r13)?;
        self.assembler.mov(qword_ptr(rdi + 32), r14)?;
        self.assembler.mov(qword_ptr(rdi + 40), r15)?;
        self.assembler.mov(qword_ptr(rdi + 56), rsp)?;

        // Restore Callee-Saved Registers from 'New' (RSI)
        self.assembler.mov(rbx, qword_ptr(rsi + 0))?;
        self.assembler.mov(rbp, qword_ptr(rsi + 8))?;
        self.assembler.mov(r12, qword_ptr(rsi + 16))?;
        self.assembler.mov(r13, qword_ptr(rsi + 24))?;
        self.assembler.mov(r14, qword_ptr(rsi + 32))?;
        self.assembler.mov(r15, qword_ptr(rsi + 40))?;
        self.assembler.mov(rsp, qword_ptr(rsi + 56))?;

        // Return (to where 'New' left off)
        self.assembler.ret()?;

        // 2. Exit Routine (Main Script Return)
        // Used when the main script finishes
        self.assembler.set_label(&mut exit_label)?;

        // Restore Rust context (saved in 'Old' coroutine -> r14)
        // r14 holds the 'Old' coroutine pointer passed at entry
        self.assembler.mov(rbx, qword_ptr(r14 + 0))?;
        self.assembler.mov(rbp, qword_ptr(r14 + 8))?;
        self.assembler.mov(r12, qword_ptr(r14 + 16))?;
        self.assembler.mov(r13, qword_ptr(r14 + 24))?;
        // r14 itself is restored last if needed, but we read from it.
        // The return to Rust expects r15 restored too.
        self.assembler.mov(r15, qword_ptr(r14 + 40))?;
        self.assembler.mov(rsp, qword_ptr(r14 + 56))?;
        self.assembler.ret()?;

        // 3. Main Entry Point
        // Called by execute.rs with (old, new, runtime, export)
        // rdi=old, rsi=new, rdx=runtime, rcx=export
        self.assembler.set_label(&mut main_label)?;

        // Initialize Runtime Registers
        self.assembler.mov(r12, rdx)?; // Runtime
        self.assembler.mov(r13, rcx)?; // ExportTable
        self.assembler.mov(r14, rdi)?; // Old Coroutine (Save slot)
        self.assembler.mov(r15, rsi)?; // New Coroutine (Self)

        // Compile Main Script Body
        if let Some(main_source) = root.sources.last() {
            self.compile_source(main_source)?;
        }

        // Jump to Exit
        self.emit_return()?;

        // 4. Data Section
        // Strings
        for (i, string) in root.strings.iter().enumerate() {
            self.assembler.set_label(&mut self.string_labels[i])?;
            self.assembler.db(string.0.as_bytes())?;
            // self.assembler.db(&[0u8])?; // Strings are ptr+len, usually don't need null term unless C interop
        }

        // Identifiers
        let identifiers: Vec<(String, CodeLabel)> = self
            .identifier_labels
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        for (name, label) in identifiers {
            let mut l = label;
            self.assembler.set_label(&mut l)?;
            self.assembler.db(name.as_bytes())?;
            self.assembler.db(&[0u8])?; // Identifiers need null term for C string usage
        }

        let result = self
            .assembler
            .assemble_options(0, BlockEncoderOptions::RETURN_NEW_INSTRUCTION_OFFSETS)?;

        // Resolve Offsets
        self.registers_swap = result.label_ip(&swap_label)? as usize;
        self.registers_exit = result.label_ip(&exit_label)? as usize;
        self.main_offset = result.label_ip(&main_label)? as usize;

        Ok(result.inner.code_buffer)
    }

    fn compile_lazy_source(&mut self, source_index: usize) -> Result<Vec<u8>, IcedError> {
        self.is_function = true;

        // Create labels for strings
        for _ in &self.root.strings {
            self.string_labels.push(self.assembler.create_label());
        }

        // Prologue
        // Save Callee-Saved Registers
        self.assembler.push(rbx)?;
        self.assembler.push(rbp)?;
        self.assembler.push(r12)?;
        self.assembler.push(r13)?;
        self.assembler.push(r14)?;
        self.assembler.push(r15)?;

        // Initialize Runtime Registers from Arguments
        // Standard Mage Call: rdi=Old, rsi=New, rdx=Runtime, rcx=Export
        self.assembler.mov(r14, rdi)?;
        self.assembler.mov(r15, rsi)?;
        self.assembler.mov(r12, rdx)?;
        self.assembler.mov(r13, rcx)?;

        // Compile Body
        let source = &self.root.sources[source_index];
        self.compile_source(source)?;

        // Epilogue Label
        self.assembler.set_label(&mut self.func_epilogue_label)?;

        // Restore Callee-Saved Registers
        self.assembler.pop(r15)?;
        self.assembler.pop(r14)?;
        self.assembler.pop(r13)?;
        self.assembler.pop(r12)?;
        self.assembler.pop(rbp)?;
        self.assembler.pop(rbx)?;
        self.assembler.ret()?;

        // Data Section (Same as compile)
        for (i, string) in self.root.strings.iter().enumerate() {
            self.assembler.set_label(&mut self.string_labels[i])?;
            self.assembler.db(string.0.as_bytes())?;
        }
        let identifiers: Vec<(String, CodeLabel)> = self
            .identifier_labels
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        for (name, label) in identifiers {
            let mut l = label;
            self.assembler.set_label(&mut l)?;
            self.assembler.db(name.as_bytes())?;
            self.assembler.db(&[0u8])?;
        }

        let result = self
            .assembler
            .assemble_options(0, BlockEncoderOptions::RETURN_NEW_INSTRUCTION_OFFSETS)?;

        Ok(result.inner.code_buffer)
    }

    fn compile_source(&mut self, source: &FlatSource) -> Result<(), IcedError> {
        let count = source.expressions.len();
        let size = count * 8;

        if size > 0 {
            // Stack Allocation for Tuple
            // Save RBX (used as tuple pointer)
            self.assembler.push(rbx)?;
            // Allocate space
            self.assembler.sub(rsp, size as i32)?;
            // Set RBX to Tuple Start
            self.assembler.mov(rbx, rsp)?;

            for (i, expression) in source.expressions.iter().enumerate() {
                self.compile_expression(source, expression)?;
                // Result in RAX. Store in Tuple.
                self.assembler.mov(qword_ptr(rbx + i * 8), rax)?;
            }

            // Return Tuple Pointer in RAX
            self.assembler.mov(rax, rbx)?;

            // Restore RBX
            // The old RBX is at [rsp + size] because we pushed it then subbed rsp.
            self.assembler.mov(rbx, qword_ptr(rsp + size))?;

            // Note: We leave 'size' bytes on the stack.
            // This is the tuple data which acts as the return value memory.
            // We also leave the saved RBX slot above it.
            // The caller is responsible for stack management if needed,
            // but in Mage linear script execution, we often just accumulate.
        } else {
            // Empty tuple / No expressions
            self.assembler.xor(rax, rax)?;
        }
        Ok(())
    }

    fn compile_expression(
        &mut self,
        source: &FlatSource,
        expression: &FlatExpression,
    ) -> Result<(), IcedError> {
        match expression {
            FlatExpression::Number(index) => {
                if let FlatIndex::Number(idx) = index {
                    let number = &self.root.numbers[*idx];
                    self.assembler.mov(rax, number.0)?;
                }
            }
            FlatExpression::String(index) => {
                if let FlatIndex::String(idx) = index {
                    let string = &self.root.strings[*idx];
                    let label = self.string_labels[*idx];
                    self.assembler.lea(rax, ptr(label))?;
                    self.assembler.mov(rdx, string.0.len() as u64)?;
                }
            }
            FlatExpression::Identifier(index) => {
                if let FlatIndex::Identifier(idx) = index {
                    let name = source.identifiers[*idx].0.clone();
                    let label = self.get_identifier_label(&name);

                    // Call get_var(runtime, name)
                    self.assembler.mov(rdi, r12)?;
                    self.assembler.lea(rsi, ptr(label))?;
                    self.assembler.mov(rax, qword_ptr(r12 + 32))?;
                    self.assembler.call(rax)?;
                }
            }
            FlatExpression::Assign(binary) => {
                if let Some(FlatIndex::Identifier(idx)) = binary.one {
                    let name = source.identifiers[idx].0.clone();
                    let label = self.get_identifier_label(&name);

                    self.compile_index(source, &binary.two)?;

                    self.assembler.mov(rdx, rax)?; // Value
                    self.assembler.mov(rdi, r12)?; // Runtime
                    self.assembler.lea(rsi, ptr(label))?; // Name
                    self.assembler.mov(rax, qword_ptr(r12 + 24))?;
                    self.assembler.call(rax)?;
                }
            }
            FlatExpression::Call(binary) => {
                self.compile_pipe(source, binary)?;
            }
            FlatExpression::Member(binary) => {
                self.compile_member(source, binary)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn compile_index(&mut self, source: &FlatSource, index: &FlatIndex) -> Result<(), IcedError> {
        match index {
            FlatIndex::Expression(idx) => {
                let expr = &source.expressions[*idx];
                self.compile_expression(source, expr)?;
            }
            FlatIndex::Number(idx) => {
                let number = &self.root.numbers[*idx];
                self.assembler.mov(rax, number.0)?;
            }
            FlatIndex::String(idx) => {
                let string = &self.root.strings[*idx];
                let label = self.string_labels[*idx];
                self.assembler.lea(rax, ptr(label))?;
                self.assembler.mov(rdx, string.0.len() as u64)?;
            }
            FlatIndex::Identifier(idx) => {
                let name = source.identifiers[*idx].0.clone();
                let label = self.get_identifier_label(&name);
                self.assembler.mov(rdi, r12)?;
                self.assembler.lea(rsi, ptr(label))?;
                self.assembler.mov(rax, qword_ptr(r12 + 32))?;
                self.assembler.call(rax)?;
            }
            FlatIndex::Source(idx) => {
                let nested_source = &self.root.sources[*idx];
                self.compile_source(nested_source)?;
            }
        }
        Ok(())
    }

    fn compile_member(
        &mut self,
        source: &FlatSource,
        binary: &FlatBinary,
    ) -> Result<(), IcedError> {
        if let Some(lhs) = &binary.one {
            self.compile_index(source, lhs)?;
        }
        // rax = table_ptr

        if let FlatIndex::Identifier(idx) = binary.two {
            let name = source.identifiers[idx].0.clone();
            let label = self.get_identifier_label(&name);

            // get_member(runtime, table, name)
            self.assembler.mov(rsi, rax)?; // table
            self.assembler.mov(rdi, r12)?; // runtime
            self.assembler.lea(rdx, ptr(label))?; // name

            self.assembler.mov(rax, qword_ptr(r12 + 72))?; // get_member offset
            self.assembler.call(rax)?;
        }
        Ok(())
    }

    fn compile_pipe(&mut self, source: &FlatSource, binary: &FlatBinary) -> Result<(), IcedError> {
        let is_syscall = if let FlatIndex::Identifier(id_idx) = binary.two {
            if let Some(ident) = source.identifiers.get(id_idx) {
                ident.0 == "syscall"
            } else {
                false
            }
        } else {
            false
        };

        let is_export = if let FlatIndex::Identifier(id_idx) = binary.two {
            if let Some(ident) = source.identifiers.get(id_idx) {
                ident.0 == "export"
            } else {
                false
            }
        } else {
            false
        };

        let is_import = if let FlatIndex::Identifier(id_idx) = binary.two {
            if let Some(ident) = source.identifiers.get(id_idx) {
                ident.0 == "import"
            } else {
                false
            }
        } else {
            false
        };

        let is_procedure = if let FlatIndex::Identifier(id_idx) = binary.two {
            if let Some(ident) = source.identifiers.get(id_idx) {
                ident.0 == "procedure"
            } else {
                false
            }
        } else {
            false
        };

        let is_class_def = if let FlatIndex::Identifier(id_idx) = binary.two {
            if let Some(ident) = source.identifiers.get(id_idx) {
                let name = &ident.0;
                name == "Class"
                    || name == "Interface"
                    || name == "Enum"
                    || name == "Struct"
                    || name == "Union"
                    || name == "method"
            } else {
                false
            }
        } else {
            false
        };

        let is_return = if let FlatIndex::Identifier(id_idx) = binary.two {
            if let Some(ident) = source.identifiers.get(id_idx) {
                ident.0 == "return"
            } else {
                false
            }
        } else {
            false
        };

        if is_syscall {
            if let Some(lhs_index) = &binary.one {
                self.emit_syscall(source, lhs_index)?;
            }
        } else if is_export {
            if let Some(lhs_index) = &binary.one {
                self.emit_export(source, lhs_index)?;
            }
        } else if is_import {
            if let Some(lhs_index) = &binary.one {
                self.compile_index(source, lhs_index)?;
                self.assembler.mov(rdi, r12)?;
                self.assembler.mov(rsi, rax)?;
                self.assembler.mov(rax, qword_ptr(r12 + 64))?;
                self.assembler.call(rax)?;
            }
        } else if is_procedure {
            if let Some(lhs_index) = &binary.one {
                self.emit_procedure(source, lhs_index)?;
            }
        } else if is_class_def {
            if let Some(lhs_index) = &binary.one {
                self.emit_class(source, lhs_index)?;
            }
        } else if is_return {
            if let Some(left) = &binary.one {
                self.compile_index(source, left)?;
            }
            self.emit_return()?;
        } else {
            // Function Call via Pipe: Arg => Func
            if let Some(left) = &binary.one {
                self.compile_index(source, left)?;
            }
            self.assembler.push(rax)?;

            self.compile_index(source, &binary.two)?;

            self.assembler.mov(rdi, r12)?; // Runtime
            self.assembler.mov(rsi, rax)?; // Proc Ptr
            self.assembler.push(rax)?;

            self.assembler.mov(rax, qword_ptr(r12 + 96))?; // compile_procedure
            self.assembler.call(rax)?;

            self.assembler.mov(r11, rax)?; // Code Ptr
            self.assembler.add(rsp, 8)?; // Pop SavedProcPtr

            // Prepare Call Arguments
            self.assembler.mov(rdi, r14)?; // Old
            self.assembler.mov(rsi, r15)?; // New
            self.assembler.mov(rdx, r12)?; // Runtime
            self.assembler.mov(rcx, r13)?; // Export
            self.assembler.mov(r8, 0u64)?; // Type
            self.assembler.pop(r9)?; // Argument (Pop from stack)

            // Call
            self.assembler.call(r11)?;
        }
        Ok(())
    }

    fn emit_syscall(
        &mut self,
        _source: &FlatSource,
        args_index: &FlatIndex,
    ) -> Result<(), IcedError> {
        if let FlatIndex::Source(src_idx) = args_index {
            let arg_source = &self.root.sources[*src_idx];
            if arg_source.expressions.len() >= 2 {
                let mut start_label = self.assembler.create_label();
                let mut end_label = self.assembler.create_label();

                self.assembler.jmp(end_label)?;
                self.assembler.set_label(&mut start_label)?;

                // 1. Syscall Number
                self.compile_expression(arg_source, &arg_source.expressions[0])?;
                // Result in RAX. Syscall number goes in RAX.

                // 2. Arguments
                let args_expr = &arg_source.expressions[1];
                let mut args_count = 0;
                match args_expr {
                    FlatExpression::Member(bin) | FlatExpression::Call(bin) => {
                        if let Some(FlatIndex::Source(tuple_idx)) = bin.one {
                            let tuple_source = &self.root.sources[tuple_idx];
                            args_count = tuple_source.expressions.len();
                        }
                    }
                    _ => {}
                }

                if args_count > 1 {
                    self.assembler.mov(rdi, qword_ptr(r9))?;
                    self.assembler.mov(rsi, qword_ptr(r9 + 8))?;
                    if args_count > 2 {
                        self.assembler.mov(rdx, qword_ptr(r9 + 16))?;
                    }
                } else {
                    // Dereference R9 to get scalar arg
                    self.assembler.mov(rdi, qword_ptr(r9))?;
                }

                self.assembler.syscall()?;
                self.assembler.ret()?;

                self.assembler.set_label(&mut end_label)?;

                // Return pointer to trampoline (Code Ptr)
                self.assembler.lea(rsi, ptr(start_label))?;

                // Call make_syscall_procedure(rt, code_ptr)
                self.assembler.mov(rdi, r12)?;
                self.assembler.mov(rax, qword_ptr(r12 + 120))?;
                self.assembler.call(rax)?;
            }
        }
        Ok(())
    }

    fn emit_export(
        &mut self,
        _source: &FlatSource,
        args_index: &FlatIndex,
    ) -> Result<(), IcedError> {
        if let FlatIndex::Source(idx) = args_index {
            let export_source = &self.root.sources[*idx];
            for expr in &export_source.expressions {
                if let FlatExpression::Assign(bin) = expr {
                    if let Some(FlatIndex::Identifier(id_idx)) = bin.one {
                        let name = &export_source.identifiers[id_idx].0;
                        self.compile_index(export_source, &bin.two)?;
                        let label = self.get_identifier_label(name);
                        self.assembler.mov(rdx, rax)?;
                        self.assembler.mov(rdi, r12)?;
                        self.assembler.lea(rsi, ptr(label))?;
                        self.assembler.mov(rax, qword_ptr(r12 + 48))?;
                        self.assembler.call(rax)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn emit_procedure(
        &mut self,
        _source: &FlatSource,
        args_index: &FlatIndex,
    ) -> Result<(), IcedError> {
        if let FlatIndex::Source(idx) = args_index {
            self.assembler.mov(rdi, r12)?;
            self.assembler.mov(rsi, *idx as u64)?;
            self.assembler.mov(rax, qword_ptr(r12 + 88))?;
            self.assembler.call(rax)?;
        }
        Ok(())
    }

    fn emit_class(&mut self, source: &FlatSource, args_index: &FlatIndex) -> Result<(), IcedError> {
        // 1. push_scope(rt)
        self.assembler.mov(rdi, r12)?;
        self.assembler.mov(rax, qword_ptr(r12 + 104))?;
        self.assembler.call(rax)?;

        // 2. Compile Body
        self.compile_index(source, args_index)?;

        // 3. pop_scope(rt) -> ExportTable*
        self.assembler.mov(rdi, r12)?;
        self.assembler.mov(rax, qword_ptr(r12 + 112))?;
        self.assembler.call(rax)?;

        Ok(())
    }

    fn emit_return(&mut self) -> Result<(), IcedError> {
        if self.is_function {
            self.assembler.jmp(self.func_epilogue_label)?;
        } else {
            self.assembler.jmp(self.exit_label)?;
        }
        Ok(())
    }
}
