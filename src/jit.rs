use iced_x86::{BlockEncoderOptions, IcedError, code_asm::*};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{Error, FlatBinary, FlatExpression, FlatIndex, FlatOperator, FlatRoot, FlatSource};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Bytecode {
    pub code: Vec<u8>,
    pub registers_swap: usize,
    pub registers_exit: usize,
    pub main: usize,
    pub root: FlatRoot,
}

pub fn compile_root(root: FlatRoot) -> Result<Bytecode, Error> {
    let compiler = Compiler::new(&root);
    let (code, registers_swap, registers_exit, main) = compiler
        .compile()
        .map_err(|error| Error::CompileError(format!("Failed to compile: {}", error)))?;

    Ok(Bytecode {
        code,
        registers_swap,
        registers_exit,
        main,
        root,
    })
}

pub fn compile_lazy(root: &FlatRoot, source_index: usize) -> Result<Vec<u8>, Error> {
    let compiler = Compiler::new(root);
    compiler
        .compile_lazy_source(source_index)
        .map_err(|error| Error::CompileError(format!("Failed to compile lazy: {}", error)))
}

struct Compiler<'a> {
    assembler: CodeAssembler,
    root: &'a FlatRoot,
    // Store labels for strings to reference them
    string_labels: Vec<CodeLabel>,
    // Store labels for identifiers to reference them
    identifier_labels: HashMap<String, CodeLabel>,
    is_function: bool,
    exit_label: Option<CodeLabel>,
}

impl<'a> Compiler<'a> {
    fn new(root: &'a FlatRoot) -> Self {
        Self {
            assembler: CodeAssembler::new(64).unwrap(),
            root,
            string_labels: vec![],
            identifier_labels: HashMap::new(),
            is_function: false,
            exit_label: None,
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

    fn compile(mut self) -> Result<(Vec<u8>, usize, usize, usize), IcedError> {
        // Create labels for all strings upfront
        for _ in &self.root.strings {
            self.string_labels.push(self.assembler.create_label());
        }

        let mut registers_swap_label = self.assembler.create_label();
        self.exit_label = Some(registers_swap_label);
        let mut registers_exit_label = self.assembler.create_label();

        // --- Context Switch Prologue ---
        self.assembler.set_label(&mut registers_swap_label)?;

        // Save Callee-saved registers to 'old' coroutine (RDI)
        self.assembler.mov(qword_ptr(rdi + 8), rbx)?;
        self.assembler.mov(qword_ptr(rdi + 16), rbp)?;
        self.assembler.mov(qword_ptr(rdi + 24), r12)?;
        self.assembler.mov(qword_ptr(rdi + 32), r13)?;
        self.assembler.mov(qword_ptr(rdi + 40), r14)?;
        self.assembler.mov(qword_ptr(rdi + 48), r15)?;
        self.assembler.mov(qword_ptr(rdi + 56), rsp)?;

        // --- Context Switch Epilogue ---
        self.assembler.set_label(&mut registers_exit_label)?;

        // Restore Callee-saved registers from 'new' coroutine (RSI)
        self.assembler.mov(rbx, qword_ptr(rsi + 8))?;
        self.assembler.mov(rbp, qword_ptr(rsi + 16))?;
        self.assembler.mov(r12, qword_ptr(rsi + 24))?;
        self.assembler.mov(r13, qword_ptr(rsi + 32))?;
        self.assembler.mov(r14, qword_ptr(rsi + 40))?;
        self.assembler.mov(r15, qword_ptr(rsi + 48))?;
        self.assembler.mov(rsp, qword_ptr(rsi + 56))?;

        self.assembler.ret()?;

        // --- Main Entry Point ---
        let mut main_label = self.assembler.create_label();
        self.assembler.set_label(&mut main_label)?;

        // Incoming arguments:
        // RDI = old coroutine
        // RSI = new coroutine
        // RDX = runtime
        // RCX = export_table

        // Move Runtime to R12 (Callee-saved)
        self.assembler.mov(r12, rdx)?;
        // Move ExportTable to R13 (Callee-saved)
        self.assembler.mov(r13, rcx)?;
        // Save Old Coroutine to R14
        self.assembler.mov(r14, rdi)?;
        // Save New Coroutine to R15
        self.assembler.mov(r15, rsi)?;

        // Compile the last source (assumed to be the main file content)
        if let Some(source) = self.root.sources.last() {
            let source = source.clone();
            self.compile_source(&source)?;
        }

        // Return to Rust
        // Prepare swap: Old -> New (current state), Restore -> Old (Rust state)
        // RDI = New (R15)
        // RSI = Old (R14)
        self.assembler.mov(rdi, r15)?;
        self.assembler.mov(rsi, r14)?;
        self.assembler.jmp(registers_swap_label)?;

        // --- Data Section (Strings) ---
        for (i, string) in self.root.strings.iter().enumerate() {
            let label = &mut self.string_labels[i];
            self.assembler.set_label(label)?;
            self.assembler.db(string.0.as_bytes())?;
            self.assembler.db(&[0])?;
        }

        // --- Data Section (Identifiers) ---
        let identifiers: Vec<(String, CodeLabel)> = self
            .identifier_labels
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        for (name, _) in identifiers {
            let label_mut = self.identifier_labels.get_mut(&name).unwrap();
            self.assembler.set_label(label_mut)?;
            self.assembler.db(name.as_bytes())?;
            self.assembler.db(&[0])?;
        }

        let result = self
            .assembler
            .assemble_options(0, BlockEncoderOptions::RETURN_NEW_INSTRUCTION_OFFSETS)?;

        let registers_swap = result.label_ip(&registers_swap_label)?;
        let registers_exit = result.label_ip(&registers_exit_label)?;
        let main = result.label_ip(&main_label)?;

        Ok((
            result.inner.code_buffer,
            registers_swap as usize,
            registers_exit as usize,
            main as usize,
        ))
    }

    fn compile_lazy_source(mut self, source_index: usize) -> Result<Vec<u8>, IcedError> {
        self.is_function = true;

        // Create labels for all strings upfront
        for _ in &self.root.strings {
            self.string_labels.push(self.assembler.create_label());
        }

        // Prologue: Save Callee-Saved Registers used by JIT context
        self.assembler.push(r12)?;
        self.assembler.push(r13)?;
        self.assembler.push(r14)?;
        self.assembler.push(r15)?;

        // Setup JIT Context from Arguments
        // RDI (Old), RSI (New), RDX (Runtime), RCX (Export)
        self.assembler.mov(r14, rdi)?; // Old
        self.assembler.mov(r15, rsi)?; // New
        self.assembler.mov(r12, rdx)?; // Runtime
        self.assembler.mov(r13, rcx)?; // Export

        let source = &self.root.sources[source_index];
        self.compile_source(source)?;

        // Epilogue: Restore Callee-Saved Registers
        self.assembler.pop(r15)?;
        self.assembler.pop(r14)?;
        self.assembler.pop(r13)?;
        self.assembler.pop(r12)?;
        self.assembler.ret()?;

        // --- Data Section (Strings) ---
        for (i, string) in self.root.strings.iter().enumerate() {
            let label = &mut self.string_labels[i];
            self.assembler.set_label(label)?;
            self.assembler.db(string.0.as_bytes())?;
            self.assembler.db(&[0])?;
        }

        // --- Data Section (Identifiers) ---
        let identifiers: Vec<(String, CodeLabel)> = self
            .identifier_labels
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        for (name, _) in identifiers {
            let label_mut = self.identifier_labels.get_mut(&name).unwrap();
            self.assembler.set_label(label_mut)?;
            self.assembler.db(name.as_bytes())?;
            self.assembler.db(&[0])?;
        }

        let result = self
            .assembler
            .assemble_options(0, BlockEncoderOptions::RETURN_NEW_INSTRUCTION_OFFSETS)?;

        Ok(result.inner.code_buffer)
    }

    fn compile_source(&mut self, source: &FlatSource) -> Result<(), IcedError> {
        for expression in &source.expressions {
            self.compile_expression(source, expression)?;
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
                // Get Variable
                if let FlatIndex::Identifier(idx) = index {
                    let name = source.identifiers[*idx].0.clone();
                    let label = self.get_identifier_label(&name);

                    // Call get_var(runtime, name)
                    self.assembler.mov(rdi, r12)?;
                    self.assembler.lea(rsi, ptr(label))?;

                    // Load func ptr from runtime offset 32 (get_var)
                    self.assembler.mov(rax, qword_ptr(r12 + 32))?;
                    self.assembler.call(rax)?;
                }
            }
            FlatExpression::Assign(binary) => {
                // Handle Assignment: Identifier : Expression
                // LHS should be identifier
                if let Some(FlatIndex::Identifier(idx)) = binary.one {
                    let name = source.identifiers[idx].0.clone();
                    let label = self.get_identifier_label(&name);

                    // Compile RHS (Value)
                    // Since compile_index might clobber volatile registers, we do it first.
                    // The result is usually in RAX.
                    self.compile_index(source, &binary.two)?;

                    // Call set_var(runtime, name, value)
                    self.assembler.mov(rdx, rax)?; // Value
                    self.assembler.mov(rdi, r12)?; // Runtime
                    self.assembler.lea(rsi, ptr(label))?; // Name

                    // Load func ptr from runtime offset 24 (set_var)
                    self.assembler.mov(rax, qword_ptr(r12 + 24))?;
                    self.assembler.call(rax)?;
                } else if let Some(FlatIndex::Expression(expr_idx)) = binary.one {
                    if let FlatExpression::Member(member_binary) = &source.expressions[expr_idx] {
                        // 1. Compile Object (LHS of Member)
                        if let Some(obj_idx) = &member_binary.one {
                            self.compile_index(source, obj_idx)?;
                        }
                        // Save Object to Stack
                        self.assembler.push(rax)?;

                        // 2. Compile RHS Value (of Assignment)
                        self.compile_index(source, &binary.two)?;
                        // Move Value to RCX (4th arg)
                        self.assembler.mov(rcx, rax)?;

                        // 3. Restore Object to RSI (2nd arg)
                        self.assembler.pop(rsi)?;

                        // 4. Get Property Name (RHS of Member)
                        if let FlatIndex::Identifier(id_idx) = member_binary.two {
                            let name = source.identifiers[id_idx].0.clone();
                            let label = self.get_identifier_label(&name);
                            // Load Name to RDX (3rd arg)
                            self.assembler.lea(rdx, ptr(label))?;

                            // 5. Setup Runtime (RDI - 1st arg)
                            self.assembler.mov(rdi, r12)?;

                            // 6. Call set_member (offset 80)
                            self.assembler.mov(rax, qword_ptr(r12 + 80))?;
                            self.assembler.call(rax)?;
                        }
                    }
                }
            }
            FlatExpression::Member(binary) => {
                self.compile_member(source, binary)?;
            }
            FlatExpression::Call(binary) => {
                if binary.operator == FlatOperator::Pipe {
                    self.compile_pipe(source, binary)?;
                } else {
                    // TODO: Standard Call syntax
                }
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
        // LHS: Object (Compiled to RAX)
        if let Some(left) = &binary.one {
            self.compile_index(source, left)?;
        }
        // Save Object (RAX) to R15 (Wait, R15 is New Coroutine).
        // Use Stack.
        self.assembler.push(rax)?;

        // RHS: Identifier (String Name)
        if let FlatIndex::Identifier(id_idx) = binary.two {
            let name = source.identifiers[id_idx].0.clone();
            let label = self.get_identifier_label(&name);

            // Call get_member(runtime, object, name)
            // RDI = Runtime (R12)
            // RSI = Object (Pop)
            // RDX = Name (Label)

            self.assembler.mov(rdx, r12)?; // Runtime (Wait, Signature?)
            // Signature: get_member(rt, obj, name) -> RDI, RSI, RDX
            self.assembler.mov(rdi, r12)?;
            self.assembler.pop(rsi)?;
            self.assembler.lea(rdx, ptr(label))?;

            // Call get_member at offset 72
            self.assembler.mov(rax, qword_ptr(r12 + 72))?;
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
            // 1. Compile Argument (LHS)
            if let Some(left) = &binary.one {
                self.compile_index(source, left)?;
            }
            // Result in RAX. Push it as Argument value.
            self.assembler.push(rax)?;

            // 2. Compile Function (RHS)
            self.compile_index(source, &binary.two)?;
            // Result in RAX (Procedure Pointer).

            // Resolve Code Pointer from Procedure Pointer
            // call compile_procedure(rt, proc_ptr)
            self.assembler.mov(rdi, r12)?; // Runtime
            self.assembler.mov(rsi, rax)?; // Proc Ptr
            self.assembler.push(rax)?; // Save Proc Ptr (needed? No, we need code ptr)

            // We need to save regs? No, sysv clobbers, but we are about to call result.
            // We pushed Arg on stack. It is safe.
            self.assembler.mov(rax, qword_ptr(r12 + 96))?; // compile_procedure
            self.assembler.call(rax)?;

            self.assembler.mov(r11, rax)?; // Code Ptr
            // Stack has [Arg, SavedProcPtr].
            // Wait, I pushed rax (ProcPtr) above.
            self.assembler.add(rsp, 8)?; // Pop SavedProcPtr (discard)

            // 3. Prepare Call Arguments
            self.assembler.mov(rdi, r14)?; // Old
            self.assembler.mov(rsi, r15)?; // New
            self.assembler.mov(rdx, r12)?; // Runtime
            self.assembler.mov(rcx, r13)?; // Export
            self.assembler.mov(r8, 0u64)?; // Type
            self.assembler.pop(r9)?; // Argument (Pop from stack)

            // 4. Call
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

                // Jump over the trampoline
                self.assembler.jmp(end_label)?;
                self.assembler.set_label(&mut start_label)?;

                // 1. Syscall Number (Constant)
                self.compile_expression(arg_source, &arg_source.expressions[0])?;

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
                    // Unpack from pointer in R9
                    // Arg 1 -> RDI
                    self.assembler.mov(rdi, qword_ptr(r9))?;
                    // Arg 2 -> RSI
                    self.assembler.mov(rsi, qword_ptr(r9 + 8))?;
                    // Arg 3 -> RDX
                    if args_count > 2 {
                        self.assembler.mov(rdx, qword_ptr(r9 + 16))?;
                    }
                } else {
                    // Single arg in R9 -> RDI
                    self.assembler.mov(rdi, r9)?;
                }

                self.assembler.syscall()?;
                self.assembler.ret()?;

                self.assembler.set_label(&mut end_label)?;
                // Return pointer to trampoline
                self.assembler.lea(rsi, ptr(start_label))?;
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
        if let FlatIndex::Source(src_idx) = args_index {
            let export_source = &self.root.sources[*src_idx];
            for expr in &export_source.expressions {
                if let FlatExpression::Assign(binary) = expr {
                    if let Some(FlatIndex::Identifier(id_idx)) = binary.one {
                        let name = export_source.identifiers[id_idx].0.clone();
                        let label = self.get_identifier_label(&name);

                        self.compile_index(export_source, &binary.two)?;

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
        if let FlatIndex::Source(src_idx) = args_index {
            // call make_procedure(rt, source_index)
            self.assembler.mov(rdi, r12)?;
            self.assembler.mov(rsi, *src_idx as u64)?;
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
            // Restore Callee-Saved Registers (Epilogue)
            self.assembler.pop(r15)?;
            self.assembler.pop(r14)?;
            self.assembler.pop(r13)?;
            self.assembler.pop(r12)?;
            self.assembler.ret()?;
        } else if let Some(label) = self.exit_label {
            // Main script return
            // RDI = New (R15)
            // RSI = Old (R14)
            self.assembler.mov(rdi, r15)?;
            self.assembler.mov(rsi, r14)?;
            self.assembler.jmp(label)?;
        }
        Ok(())
    }
}
