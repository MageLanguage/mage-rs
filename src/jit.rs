use crate::{
    ASTExpression, ASTMath, ASTMathOperation, ASTMathSection, ASTMathVariable, ASTNumber,
    ASTSource, ASTSourceFile, ASTStatement, ASTStatementChain, MageError,
};

pub struct JITCompiler {
    _code_buffer: Vec<u8>,
}

impl JITCompiler {
    pub fn new() -> Result<Self, MageError> {
        Ok(Self {
            _code_buffer: Vec::new(),
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
                let result = self.compile_math(math)?;
                println!("Math result: {}", result);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn compile_math(&mut self, math: &ASTMath) -> Result<i64, MageError> {
        if math.sections.is_empty() {
            return Ok(0);
        }

        let mut result = match &math.sections[0] {
            ASTMathSection::Variable(var) => self.compile_math_variable(var)?,
            ASTMathSection::Operation(_) => {
                return Err(MageError::RuntimeError(
                    "Math expression cannot start with operation".to_string(),
                ));
            }
        };

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

            result = self.apply_math_operation(result, operation, operand)?;
            i += 2;
        }

        Ok(result)
    }

    fn compile_math_variable(&self, variable: &ASTMathVariable) -> Result<i64, MageError> {
        match variable {
            ASTMathVariable::Number(number) => self.parse_number(number),
            ASTMathVariable::IdentifierChain(_) => Err(MageError::RuntimeError(
                "Identifier chains not supported in math expressions yet".to_string(),
            )),
        }
    }

    fn apply_math_operation(
        &self,
        left: i64,
        operation: &ASTMathOperation,
        right: i64,
    ) -> Result<i64, MageError> {
        match operation {
            ASTMathOperation::Add => Ok(left + right),
            ASTMathOperation::Subtract => Ok(left - right),
            ASTMathOperation::Multiply => Ok(left * right),
            ASTMathOperation::Divide => {
                if right == 0 {
                    Err(MageError::RuntimeError("Division by zero".to_string()))
                } else {
                    Ok(left / right)
                }
            }
            ASTMathOperation::Modulo => {
                if right == 0 {
                    Err(MageError::RuntimeError("Modulo by zero".to_string()))
                } else {
                    Ok(left % right)
                }
            }
        }
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
