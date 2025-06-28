use crate::{
    ASTExpression, ASTMath, ASTMathOperation, ASTMathSection, ASTMathVariable, ASTNumber,
    ASTSource, ASTSourceFile, ASTStatement, ASTStatementChain, MageError,
};
use mmap_rs::MmapOptions;
use zydis::{Decoder, EncoderRequest, Mnemonic, Register, VisibleOperands};

pub struct JITCompiler {
    code_buffer: Vec<u8>,
    executable_memory: Option<mmap_rs::Mmap>,
}

impl JITCompiler {
    pub fn new() -> Result<Self, MageError> {
        Ok(Self {
            code_buffer: Vec::new(),
            executable_memory: None,
        })
    }

    pub fn compile_source_file(&mut self, source_file: &ASTSourceFile) -> Result<(), MageError> {
        if let Some(ref statement_chain) = source_file.statement_chain {
            self.compile_statement_chain(statement_chain)?;
        }
        Ok(())
    }

    pub fn compile_source(&mut self, source: &ASTSource) -> Result<(), MageError> {
        if let Some(ref statement_chain) = source.statement_chain {
            self.compile_statement_chain(statement_chain)?;
        }
        Ok(())
    }

    pub fn compile_statement_chain(
        &mut self,
        statement_chain: &ASTStatementChain,
    ) -> Result<(), MageError> {
        for statement in &statement_chain.statements {
            self.compile_statement(statement)?;
        }
        Ok(())
    }

    pub fn compile_statement(&mut self, statement: &ASTStatement) -> Result<(), MageError> {
        match statement {
            ASTStatement::Definition(_def) => Ok(()),
            ASTStatement::Expression(expr) => self.compile_expression(expr),
        }
    }

    pub fn compile_expression(&mut self, expression: &ASTExpression) -> Result<(), MageError> {
        match expression {
            ASTExpression::Math(math) => {
                self.compile_and_execute_math(math)?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn compile_and_execute_math(&mut self, math: &ASTMath) -> Result<(), MageError> {
        if math.sections.is_empty() {
            println!("Math result: 0");
            return Ok(());
        }

        // Clear previous code
        self.code_buffer.clear();

        // Generate function prologue
        self.emit_instruction(EncoderRequest::new64(Mnemonic::PUSH).add_operand(Register::RBP))?;
        self.emit_instruction(
            EncoderRequest::new64(Mnemonic::MOV)
                .add_operand(Register::RBP)
                .add_operand(Register::RSP),
        )?;

        // Load first operand into RAX
        match &math.sections[0] {
            ASTMathSection::Variable(var) => {
                let value = self.compile_math_variable(var)?;
                self.emit_instruction(
                    EncoderRequest::new64(Mnemonic::MOV)
                        .add_operand(Register::RAX)
                        .add_operand(value),
                )?;
            }
            ASTMathSection::Operation(_) => {
                return Err(MageError::RuntimeError(
                    "Math expression cannot start with operation".to_string(),
                ));
            }
        }

        // Process remaining sections in pairs (operation, variable)
        let mut i = 1;
        while i < math.sections.len() {
            if i + 1 >= math.sections.len() {
                return Err(MageError::RuntimeError(
                    "Math expression ends with operation".to_string(),
                ));
            }

            let operation = match &math.sections[i] {
                ASTMathSection::Operation(op) => op,
                ASTMathSection::Variable(_) => {
                    return Err(MageError::RuntimeError(
                        "Expected operation, found variable".to_string(),
                    ));
                }
            };

            let operand = match &math.sections[i + 1] {
                ASTMathSection::Variable(var) => self.compile_math_variable(var)?,
                ASTMathSection::Operation(_) => {
                    return Err(MageError::RuntimeError(
                        "Expected variable, found operation".to_string(),
                    ));
                }
            };

            // Emit the operation
            match operation {
                ASTMathOperation::Add => {
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::ADD)
                            .add_operand(Register::RAX)
                            .add_operand(operand),
                    )?;
                }
                ASTMathOperation::Subtract => {
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::SUB)
                            .add_operand(Register::RAX)
                            .add_operand(operand),
                    )?;
                }
                ASTMathOperation::Multiply => {
                    // Load operand into RCX for multiplication
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::MOV)
                            .add_operand(Register::RCX)
                            .add_operand(operand),
                    )?;
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::IMUL)
                            .add_operand(Register::RAX)
                            .add_operand(Register::RCX),
                    )?;
                }
                ASTMathOperation::Divide => {
                    // Check for division by zero
                    if operand == 0 {
                        return Err(MageError::RuntimeError("Division by zero".to_string()));
                    }
                    // Load operand into RCX, clear RDX, then divide
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::MOV)
                            .add_operand(Register::RCX)
                            .add_operand(operand),
                    )?;
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::XOR)
                            .add_operand(Register::RDX)
                            .add_operand(Register::RDX),
                    )?;
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::IDIV).add_operand(Register::RCX),
                    )?;
                }
                ASTMathOperation::Modulo => {
                    // Check for modulo by zero
                    if operand == 0 {
                        return Err(MageError::RuntimeError("Modulo by zero".to_string()));
                    }
                    // Load operand into RCX, clear RDX, divide, then move remainder to RAX
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::MOV)
                            .add_operand(Register::RCX)
                            .add_operand(operand),
                    )?;
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::XOR)
                            .add_operand(Register::RDX)
                            .add_operand(Register::RDX),
                    )?;
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::IDIV).add_operand(Register::RCX),
                    )?;
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::MOV)
                            .add_operand(Register::RAX)
                            .add_operand(Register::RDX),
                    )?;
                }
            }

            i += 2;
        }

        // Generate function epilogue
        self.emit_instruction(EncoderRequest::new64(Mnemonic::POP).add_operand(Register::RBP))?;
        self.emit_instruction(EncoderRequest::new64(Mnemonic::RET))?;

        // Print generated assembly for debugging
        self.print_assembly()?;

        // Execute the generated code
        let result = self.execute_code()?;
        println!("Math result: {}", result);

        Ok(())
    }

    fn compile_math_variable(&self, variable: &ASTMathVariable) -> Result<i64, MageError> {
        match variable {
            ASTMathVariable::Number(number) => self.parse_number(number),
            ASTMathVariable::IdentifierChain(_) => Err(MageError::RuntimeError(
                "Identifier chains not supported in math expressions yet".to_string(),
            )),
        }
    }

    fn emit_instruction(&mut self, request: EncoderRequest) -> Result<(), MageError> {
        request
            .encode_extend(&mut self.code_buffer)
            .map_err(|e| MageError::JitError(e))
            .map(|_| ())
    }

    fn execute_code(&mut self) -> Result<i64, MageError> {
        if self.code_buffer.is_empty() {
            return Ok(0);
        }

        // Create executable memory mapping
        let page_size = MmapOptions::page_size();
        let code_size = self.code_buffer.len();
        let map_size = ((code_size + page_size - 1) / page_size) * page_size;

        let mut mmap = MmapOptions::new(map_size)
            .map_err(|e| MageError::RuntimeError(format!("Failed to create memory map: {}", e)))?
            .map_mut()
            .map_err(|e| MageError::RuntimeError(format!("Failed to map memory: {}", e)))?;

        // Copy code to the mapping
        mmap[..code_size].copy_from_slice(&self.code_buffer);

        // Make it executable
        let exec_mmap = mmap.make_exec().map_err(|(_, e)| {
            MageError::RuntimeError(format!("Failed to make memory executable: {}", e))
        })?;

        // Get function pointer and execute
        let func_ptr: unsafe extern "C" fn() -> i64 =
            unsafe { std::mem::transmute(exec_mmap.as_ptr()) };
        let result = unsafe { func_ptr() };

        // Store the executable memory to keep it alive
        self.executable_memory = Some(exec_mmap);

        Ok(result)
    }

    fn print_assembly(&self) -> Result<(), MageError> {
        if self.code_buffer.is_empty() {
            return Ok(());
        }

        println!("Generated assembly:");
        let decoder = Decoder::new64();
        for insn in decoder.decode_all::<VisibleOperands>(&self.code_buffer, 0) {
            let (offs, bytes, insn) = insn.map_err(|e| MageError::JitError(e))?;
            let bytes: String = bytes.iter().map(|x| format!("{x:02x} ")).collect();
            println!("  0x{:04X}: {:<24} {}", offs, bytes, insn);
        }
        println!();
        Ok(())
    }

    fn parse_number(&self, number: &ASTNumber) -> Result<i64, MageError> {
        match number {
            ASTNumber::Zero => Ok(0),
            ASTNumber::Binary(s) => {
                let num_str = &s[2..];
                i64::from_str_radix(num_str, 2)
                    .map_err(|e| MageError::ParseError(format!("Invalid binary number: {}", e)))
            }
            ASTNumber::Octal(s) => {
                let num_str = &s[2..];
                i64::from_str_radix(num_str, 8)
                    .map_err(|e| MageError::ParseError(format!("Invalid octal number: {}", e)))
            }
            ASTNumber::Decimal(s) => {
                let num_str = &s[2..];
                num_str
                    .parse::<i64>()
                    .map_err(|e| MageError::ParseError(format!("Invalid decimal number: {}", e)))
            }
            ASTNumber::Hex(s) => {
                let num_str = &s[2..];
                i64::from_str_radix(num_str, 16)
                    .map_err(|e| MageError::ParseError(format!("Invalid hex number: {}", e)))
            }
        }
    }
}
