use crate::{
    ASTDefinition, ASTExpression, ASTIdentifier, ASTIdentifierChain, ASTMath, ASTMathOperation,
    ASTMathSection, ASTMathVariable, ASTNumber, ASTSource, ASTSourceFile, ASTStatement,
    ASTStatementChain, MageError,
};
use hashbrown::HashMap;
use mmap_rs::MmapOptions;
use zydis::{Decoder, EncoderRequest, Mnemonic, Register, VisibleOperands};

pub struct JITCompiler {
    code_buffer: Vec<u8>,
    executable_memory: Option<mmap_rs::Mmap>,
    pub variables: HashMap<String, i32>, // Variable name -> stack offset
    pub stack_offset: i32,               // Current stack offset
    stack_memory: Vec<i64>,              // Simulated stack for variable storage
}

impl JITCompiler {
    pub fn new() -> Result<Self, MageError> {
        let stack_memory = vec![0i64; 1024]; // Allocate 1024 slots for variables

        Ok(Self {
            code_buffer: Vec::new(),
            executable_memory: None,
            variables: HashMap::new(),
            stack_offset: 0,
            stack_memory,
        })
    }

    pub fn compile_source_file(&mut self, source_file: &ASTSourceFile) -> Result<(), MageError> {
        if let Some(ref statement_chain) = source_file.statement_chain {
            self.compile_statement_chain_with_storage(statement_chain)?;
        }
        Ok(())
    }

    pub fn compile_source(&mut self, source: &ASTSource) -> Result<(), MageError> {
        if let Some(ref statement_chain) = source.statement_chain {
            self.compile_statement_chain_with_storage(statement_chain)?;
        }
        Ok(())
    }

    pub fn compile_statement_chain_with_storage(
        &mut self,
        statement_chain: &ASTStatementChain,
    ) -> Result<(), MageError> {
        for statement in &statement_chain.statements {
            self.compile_statement_with_storage(statement)?;
        }
        Ok(())
    }

    pub fn compile_statement_with_storage(
        &mut self,
        statement: &ASTStatement,
    ) -> Result<(), MageError> {
        match statement {
            ASTStatement::Definition(def) => self.compile_definition_with_storage(def),
            ASTStatement::Expression(expr) => self.compile_expression(expr),
        }
    }

    pub fn compile_definition_with_storage(
        &mut self,
        definition: &ASTDefinition,
    ) -> Result<(), MageError> {
        // Compile the expression to get the value
        let value = self.compile_and_get_expression_value(&definition.expression)?;

        // Store the value for each assignment
        for (identifier_chain, _operation) in &definition.assignments {
            let var_name = self.identifier_chain_to_string(identifier_chain);

            // Allocate stack space for the variable
            self.stack_offset += 1; // One slot per variable
            let stack_index = self.stack_offset as usize - 1;

            // Store the variable name -> stack offset mapping
            self.variables.insert(var_name.clone(), self.stack_offset);

            // Store the actual value on our simulated stack
            if stack_index < self.stack_memory.len() {
                self.stack_memory[stack_index] = value;
            }

            // Value is already stored in simulated stack memory

            println!(
                "Defined variable '{}' with value: {} at stack offset: {}",
                var_name, value, self.stack_offset
            );
        }

        Ok(())
    }

    pub fn compile_expression(&mut self, expression: &ASTExpression) -> Result<(), MageError> {
        match expression {
            ASTExpression::Math(math) => {
                let result = self.compile_and_get_math_value(math)?;
                println!("Math result: {}", result);
                Ok(())
            }
            ASTExpression::Number(number) => {
                let value = self.parse_number(number)?;
                println!("Number: {}", value);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn compile_and_get_expression_value(
        &mut self,
        expression: &ASTExpression,
    ) -> Result<i64, MageError> {
        match expression {
            ASTExpression::Math(math) => self.compile_and_get_math_value(math),
            ASTExpression::Number(number) => self.parse_number(number),
            ASTExpression::IdentifierChain(chain) => {
                let var_name = self.identifier_chain_to_string(chain);
                self.get_variable_from_stack(&var_name)
            }
            _ => Err(MageError::RuntimeError(
                "Unsupported expression type".to_string(),
            )),
        }
    }

    fn compile_and_get_math_value(&mut self, math: &ASTMath) -> Result<i64, MageError> {
        if math.sections.is_empty() {
            return Ok(0);
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

        // Stack access will be handled through simulated stack for now

        // Load first operand into RAX
        match &math.sections[0] {
            ASTMathSection::Variable(var) => {
                self.compile_math_variable_to_register(var, Register::RAX)?;
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

            // Load operand into RCX
            match &math.sections[i + 1] {
                ASTMathSection::Variable(var) => {
                    self.compile_math_variable_to_register(var, Register::RCX)?;
                }
                ASTMathSection::Operation(_) => {
                    return Err(MageError::RuntimeError(
                        "Expected variable, found operation".to_string(),
                    ));
                }
            }

            // Emit the operation
            match operation {
                ASTMathOperation::Add => {
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::ADD)
                            .add_operand(Register::RAX)
                            .add_operand(Register::RCX),
                    )?;
                }
                ASTMathOperation::Subtract => {
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::SUB)
                            .add_operand(Register::RAX)
                            .add_operand(Register::RCX),
                    )?;
                }
                ASTMathOperation::Multiply => {
                    self.emit_instruction(
                        EncoderRequest::new64(Mnemonic::IMUL)
                            .add_operand(Register::RAX)
                            .add_operand(Register::RCX),
                    )?;
                }
                ASTMathOperation::Divide => {
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

        Ok(result)
    }

    fn compile_math_variable_to_register(
        &mut self,
        variable: &ASTMathVariable,
        register: Register,
    ) -> Result<(), MageError> {
        match variable {
            ASTMathVariable::Number(number) => {
                // Load constant value into register
                let value = self.parse_number(number)?;
                self.emit_instruction(
                    EncoderRequest::new64(Mnemonic::MOV)
                        .add_operand(register)
                        .add_operand(value),
                )?;
            }
            ASTMathVariable::IdentifierChain(chain) => {
                let var_name = self.identifier_chain_to_string(chain);
                // For now, load from our simulated stack memory
                let value = self.get_variable_from_stack(&var_name)?;
                self.emit_instruction(
                    EncoderRequest::new64(Mnemonic::MOV)
                        .add_operand(register)
                        .add_operand(value),
                )?;
            }
            ASTMathVariable::Math(nested_math) => {
                // Save current code buffer state
                let saved_buffer = self.code_buffer.clone();

                // Compile nested math expression
                let value = self.compile_and_get_math_value(nested_math)?;

                // Restore code buffer state to avoid interference
                self.code_buffer = saved_buffer;

                // Load the computed value into register
                self.emit_instruction(
                    EncoderRequest::new64(Mnemonic::MOV)
                        .add_operand(register)
                        .add_operand(value),
                )?;
            }
        }
        Ok(())
    }

    fn get_variable_from_stack(&self, name: &str) -> Result<i64, MageError> {
        if let Some(&stack_offset) = self.variables.get(name) {
            let stack_index = stack_offset as usize - 1;
            if stack_index < self.stack_memory.len() {
                Ok(self.stack_memory[stack_index])
            } else {
                Err(MageError::RuntimeError(format!(
                    "Stack index out of bounds for variable: {}",
                    name
                )))
            }
        } else {
            Err(MageError::RuntimeError(format!(
                "Undefined variable: {}",
                name
            )))
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

    pub fn identifier_chain_to_string(&self, chain: &ASTIdentifierChain) -> String {
        chain
            .identifiers
            .iter()
            .map(|identifier| match identifier {
                ASTIdentifier::Name(name) => name.value.clone(),
                ASTIdentifier::Call(_) => "call".to_string(), // Simplified for now
            })
            .collect::<Vec<String>>()
            .join(".")
    }

    pub fn parse_number(&self, number: &ASTNumber) -> Result<i64, MageError> {
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

    // Method to get variable value for testing purposes
    pub fn get_variable_value(&self, name: &str) -> Option<i64> {
        self.get_variable_from_stack(name).ok()
    }
}
