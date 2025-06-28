use crate::{
    ASTDefinition, ASTExpression, ASTIdentifier, ASTIdentifierChain, ASTMath, ASTMathOperation,
    ASTMathSection, ASTMathVariable, ASTNumber, ASTSource, ASTSourceFile, ASTStatement,
    ASTStatementChain, MageError,
};
use hashbrown::HashMap;

pub struct JITCompiler {
    variables: HashMap<String, i32>, // Variable name -> stack offset
    stack_offset: i32,               // Current stack offset
    variable_values: HashMap<String, i64>, // Variable name -> value
}

impl JITCompiler {
    pub fn new() -> Result<Self, MageError> {
        Ok(Self {
            variables: HashMap::new(),
            stack_offset: 0,
            variable_values: HashMap::new(),
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
            ASTStatement::Definition(def) => self.compile_definition(def),
            ASTStatement::Expression(expr) => self.compile_expression(expr),
        }
    }

    pub fn compile_definition(&mut self, definition: &ASTDefinition) -> Result<(), MageError> {
        // Evaluate the expression first
        let value = self.evaluate_expression(&definition.expression)?;

        // Store the value for each assignment
        for (identifier_chain, _operation) in &definition.assignments {
            let var_name = self.identifier_chain_to_string(identifier_chain);

            // Allocate stack space for the variable
            self.stack_offset += 8; // 8 bytes for i64
            self.variables.insert(var_name.clone(), self.stack_offset);

            println!("Defined variable '{}' with value: {}", var_name, value);
        }

        Ok(())
    }

    pub fn compile_expression(&mut self, expression: &ASTExpression) -> Result<(), MageError> {
        match expression {
            ASTExpression::Math(math) => {
                let result = self.evaluate_math(math)?;
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

    fn evaluate_expression(&mut self, expression: &ASTExpression) -> Result<i64, MageError> {
        match expression {
            ASTExpression::Math(math) => self.evaluate_math(math),
            ASTExpression::Number(number) => self.parse_number(number),
            ASTExpression::IdentifierChain(chain) => {
                let var_name = self.identifier_chain_to_string(chain);
                self.get_variable_value(&var_name)
            }
            _ => Err(MageError::RuntimeError(
                "Unsupported expression type".to_string(),
            )),
        }
    }

    fn evaluate_math(&mut self, math: &ASTMath) -> Result<i64, MageError> {
        if math.sections.is_empty() {
            return Ok(0);
        }

        // Get first operand
        let mut result = match &math.sections[0] {
            ASTMathSection::Variable(var) => self.evaluate_math_variable(var)?,
            ASTMathSection::Operation(_) => {
                return Err(MageError::RuntimeError(
                    "Math expression cannot start with operation".to_string(),
                ));
            }
        };

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
                ASTMathSection::Variable(var) => self.evaluate_math_variable(var)?,
                ASTMathSection::Operation(_) => {
                    return Err(MageError::RuntimeError(
                        "Expected variable, found operation".to_string(),
                    ));
                }
            };

            // Apply the operation
            result = match operation {
                ASTMathOperation::Add => result + operand,
                ASTMathOperation::Subtract => result - operand,
                ASTMathOperation::Multiply => result * operand,
                ASTMathOperation::Divide => {
                    if operand == 0 {
                        return Err(MageError::RuntimeError("Division by zero".to_string()));
                    }
                    result / operand
                }
                ASTMathOperation::Modulo => {
                    if operand == 0 {
                        return Err(MageError::RuntimeError("Modulo by zero".to_string()));
                    }
                    result % operand
                }
            };

            i += 2;
        }

        Ok(result)
    }

    fn evaluate_math_variable(&mut self, variable: &ASTMathVariable) -> Result<i64, MageError> {
        match variable {
            ASTMathVariable::Number(number) => self.parse_number(number),
            ASTMathVariable::IdentifierChain(chain) => {
                let var_name = self.identifier_chain_to_string(chain);
                self.get_variable_value(&var_name)
            }
            ASTMathVariable::Math(math) => self.evaluate_math(math),
        }
    }

    fn identifier_chain_to_string(&self, chain: &ASTIdentifierChain) -> String {
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

    fn get_variable_value(&self, name: &str) -> Result<i64, MageError> {
        self.variable_values
            .get(name)
            .copied()
            .ok_or_else(|| MageError::RuntimeError(format!("Undefined variable: {}", name)))
    }

    fn set_variable_value(&mut self, name: &str, value: i64) {
        self.variable_values.insert(name.to_string(), value);
    }

    pub fn compile_and_execute_math(&mut self, math: &ASTMath) -> Result<(), MageError> {
        let result = self.evaluate_math(math)?;
        println!("Math result: {}", result);
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

    // Override the compile_definition method to actually store values
    fn compile_definition_with_storage(
        &mut self,
        definition: &ASTDefinition,
    ) -> Result<(), MageError> {
        // Evaluate the expression first
        let value = self.evaluate_expression(&definition.expression)?;

        // Store the value for each assignment
        for (identifier_chain, _operation) in &definition.assignments {
            let var_name = self.identifier_chain_to_string(identifier_chain);
            self.set_variable_value(&var_name, value);
            println!("Defined variable '{}' with value: {}", var_name, value);
        }

        Ok(())
    }
}

// Update the compile_statement method to use the new storage method
impl JITCompiler {
    pub fn compile_statement_with_storage(
        &mut self,
        statement: &ASTStatement,
    ) -> Result<(), MageError> {
        match statement {
            ASTStatement::Definition(def) => self.compile_definition_with_storage(def),
            ASTStatement::Expression(expr) => self.compile_expression(expr),
        }
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

    pub fn compile_source_file_with_storage(
        &mut self,
        source_file: &ASTSourceFile,
    ) -> Result<(), MageError> {
        if let Some(ref statement_chain) = source_file.statement_chain {
            self.compile_statement_chain_with_storage(statement_chain)?;
        }
        Ok(())
    }
}
