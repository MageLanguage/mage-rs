use crate::{
    ASTDefinition, ASTExpression, ASTMath, ASTSource, ASTSourceFile, ASTStatement,
    ASTStatementChain, MageError,
};

pub struct JITCompiler {}

impl JITCompiler {
    pub fn new() -> Result<Self, MageError> {
        Ok(Self {})
    }

    pub fn compile_source_file(&self, source_file: &ASTSourceFile) {}
    pub fn compile_source(&self, source: &ASTSource) {}

    pub fn compile_statement_chain(&self, statmenet_chain: &ASTStatementChain) {}
    pub fn compile_statement(&self, statmenet: &ASTStatement) {}

    pub fn compile_expression(&self, expression: &ASTExpression) {}

    pub fn compile_math(&self, math: &ASTMath) {}
}
