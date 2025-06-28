use tree_sitter::{Node, Tree};

use crate::{JITCompiler, MageError};

pub struct VM {
    jit_compiler: JITCompiler,
}

impl VM {
    pub fn new() -> Result<Self, MageError> {
        let jit_compiler = match JITCompiler::new() {
            Ok(jit_compiler) => jit_compiler,
            Err(error) => return Err(error),
        };

        Ok(Self {
            jit_compiler: jit_compiler,
        })
    }

    pub fn run(&mut self, tree: &Tree, code: &str) {
        //
    }
}

#[derive(Debug, Clone)]
pub enum ASTNode {
    SourceFile(ASTSourceFile),
    Source(ASTSource),

    StatementChain(ASTStatementChain),
    Statement(ASTStatement),
    Definition(ASTDefinition),
    Expression(ASTExpression),
    IdentifierChain(ASTIdentifierChain),
    Identifier(ASTIdentifier),
    Call(ASTCall),
    Math(ASTMath),
    Number(ASTNumber),
    String(ASTString),
    Name(ASTName),
    MathOperation(ASTMathOperation),
}

#[derive(Debug, Clone)]
pub struct ASTSourceFile {
    pub statement_chain: Option<ASTStatementChain>,
}

#[derive(Debug, Clone)]
pub struct ASTSource {
    pub statement_chain: Option<ASTStatementChain>,
}

#[derive(Debug, Clone)]
pub struct ASTStatementChain {
    pub statements: Vec<ASTStatement>,
}

#[derive(Debug, Clone)]
pub enum ASTStatement {
    Definition(ASTDefinition),
    Expression(ASTExpression),
}

#[derive(Debug, Clone)]
pub struct ASTDefinition {
    pub assignments: Vec<(ASTIdentifierChain, ASTDefinitionOperation)>,
    pub expression: ASTExpression,
}

#[derive(Debug, Clone)]
pub enum ASTDefinitionOperation {
    Constant,
    Variable,
}

#[derive(Debug, Clone)]
pub enum ASTExpression {
    IdentifierChain(ASTIdentifierChain),
    Math(ASTMath),
    String(ASTString),
    Number(ASTNumber),
    Source(ASTSource),
}

#[derive(Debug, Clone)]
pub struct ASTIdentifierChain {
    pub identifiers: Vec<ASTIdentifier>,
}

#[derive(Debug, Clone)]
pub enum ASTIdentifier {
    Name(ASTName),
    Call(ASTCall),
}

#[derive(Debug, Clone)]
pub struct ASTCall {
    pub identifier: Box<ASTIdentifier>,
    pub arguments: Vec<ASTStatement>,
}

#[derive(Debug, Clone)]
pub struct ASTMath {
    pub sections: Vec<ASTMathSection>,
}

#[derive(Debug, Clone)]
pub enum ASTMathSection {
    Variable(ASTMathVariable),
    Operation(ASTMathOperation),
}

#[derive(Debug, Clone)]
pub enum ASTMathVariable {
    IdentifierChain(ASTIdentifierChain),
    Number(ASTNumber),
}

#[derive(Debug, Clone)]
pub enum ASTMathOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

#[derive(Debug, Clone)]
pub enum ASTNumber {
    Zero,
    Binary(String),
    Octal(String),
    Decimal(String),
    Hex(String),
}

#[derive(Debug, Clone)]
pub struct ASTString {
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct ASTName {
    pub value: String,
}
