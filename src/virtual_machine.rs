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
        let root_node = tree.root_node();
        match self.parse_node(&root_node, code) {
            Ok(ast_node) => {
                if let ASTNode::SourceFile(source_file) = ast_node {
                    if let Err(e) = self.jit_compiler.compile_source_file(&source_file) {
                        eprintln!("Compilation error: {:?}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Parse error: {:?}", e);
            }
        }
    }

    fn parse_node(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        match node.kind() {
            "source_file" => self.parse_source_file(node, code),
            "source" => self.parse_source(node, code),
            "statement_chain" => self.parse_statement_chain(node, code),
            "statement" => self.parse_statement(node, code),
            "definition" => self.parse_definition(node, code),
            "expression" => self.parse_expression(node, code),
            "identifier_chain" => self.parse_identifier_chain(node, code),
            "identifier" => self.parse_identifier(node, code),
            "call" => self.parse_call(node, code),
            "math" => self.parse_math(node, code),
            "number" => self.parse_number(node, code),
            "string" => self.parse_string(node, code),
            "name" => self.parse_name(node, code),
            "math_operation" => self.parse_math_operation(node, code),
            _ => Err(MageError::ParseError(format!(
                "Unknown node kind: {}",
                node.kind()
            ))),
        }
    }

    fn parse_source_file(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        let mut statement_chain = None;

        for child in node.children(&mut node.walk()) {
            if child.kind() == "statement_chain" {
                if let ASTNode::StatementChain(chain) = self.parse_node(&child, code)? {
                    statement_chain = Some(chain);
                }
            }
        }

        Ok(ASTNode::SourceFile(ASTSourceFile { statement_chain }))
    }

    fn parse_source(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        let mut statement_chain = None;

        for child in node.children(&mut node.walk()) {
            if child.kind() == "statement_chain" {
                if let ASTNode::StatementChain(chain) = self.parse_node(&child, code)? {
                    statement_chain = Some(chain);
                }
            }
        }

        Ok(ASTNode::Source(ASTSource { statement_chain }))
    }

    fn parse_statement_chain(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        let mut statements = Vec::new();

        for child in node.children(&mut node.walk()) {
            if child.kind() == "statement" {
                if let ASTNode::Statement(stmt) = self.parse_node(&child, code)? {
                    statements.push(stmt);
                }
            }
        }

        Ok(ASTNode::StatementChain(ASTStatementChain { statements }))
    }

    fn parse_statement(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "definition" => {
                    if let ASTNode::Definition(def) = self.parse_node(&child, code)? {
                        return Ok(ASTNode::Statement(ASTStatement::Definition(def)));
                    }
                }
                "expression" => {
                    if let ASTNode::Expression(expr) = self.parse_node(&child, code)? {
                        return Ok(ASTNode::Statement(ASTStatement::Expression(expr)));
                    }
                }
                _ => continue,
            }
        }
        Err(MageError::ParseError("Invalid statement".to_string()))
    }

    fn parse_definition(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        let mut assignments = Vec::new();
        let mut expression = None;
        let mut current_identifier_chain = None;

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "identifier_chain" => {
                    if let ASTNode::IdentifierChain(chain) = self.parse_node(&child, code)? {
                        current_identifier_chain = Some(chain);
                    }
                }
                "definition_operation" => {
                    if let Some(chain) = current_identifier_chain.take() {
                        let op = match child.utf8_text(code.as_bytes()).unwrap() {
                            ":" => ASTDefinitionOperation::Constant,
                            "=" => ASTDefinitionOperation::Variable,
                            _ => {
                                return Err(MageError::ParseError(
                                    "Invalid definition operation".to_string(),
                                ));
                            }
                        };
                        assignments.push((chain, op));
                    }
                }
                "expression" => {
                    if let ASTNode::Expression(expr) = self.parse_node(&child, code)? {
                        expression = Some(expr);
                    }
                }
                _ => continue,
            }
        }

        if let Some(expr) = expression {
            Ok(ASTNode::Definition(ASTDefinition {
                assignments,
                expression: expr,
            }))
        } else {
            Err(MageError::ParseError(
                "Definition missing expression".to_string(),
            ))
        }
    }

    fn parse_expression(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "identifier_chain" => {
                    if let ASTNode::IdentifierChain(chain) = self.parse_node(&child, code)? {
                        return Ok(ASTNode::Expression(ASTExpression::IdentifierChain(chain)));
                    }
                }
                "math" => {
                    if let ASTNode::Math(math) = self.parse_node(&child, code)? {
                        return Ok(ASTNode::Expression(ASTExpression::Math(math)));
                    }
                }
                "string" => {
                    if let ASTNode::String(string) = self.parse_node(&child, code)? {
                        return Ok(ASTNode::Expression(ASTExpression::String(string)));
                    }
                }
                "number" => {
                    if let ASTNode::Number(number) = self.parse_node(&child, code)? {
                        return Ok(ASTNode::Expression(ASTExpression::Number(number)));
                    }
                }
                "source" => {
                    if let ASTNode::Source(source) = self.parse_node(&child, code)? {
                        return Ok(ASTNode::Expression(ASTExpression::Source(source)));
                    }
                }
                _ => continue,
            }
        }
        Err(MageError::ParseError("Invalid expression".to_string()))
    }

    fn parse_identifier_chain(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        let mut identifiers = Vec::new();

        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                if let ASTNode::Identifier(ident) = self.parse_node(&child, code)? {
                    identifiers.push(ident);
                }
            }
        }

        Ok(ASTNode::IdentifierChain(ASTIdentifierChain { identifiers }))
    }

    fn parse_identifier(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "name" => {
                    if let ASTNode::Name(name) = self.parse_node(&child, code)? {
                        return Ok(ASTNode::Identifier(ASTIdentifier::Name(name)));
                    }
                }
                "call" => {
                    if let ASTNode::Call(call) = self.parse_node(&child, code)? {
                        return Ok(ASTNode::Identifier(ASTIdentifier::Call(call)));
                    }
                }
                _ => continue,
            }
        }
        Err(MageError::ParseError("Invalid identifier".to_string()))
    }

    fn parse_call(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        let mut identifier = None;
        let mut arguments = Vec::new();

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "identifier" => {
                    if let ASTNode::Identifier(ident) = self.parse_node(&child, code)? {
                        identifier = Some(Box::new(ident));
                    }
                }
                "statement" => {
                    if let ASTNode::Statement(stmt) = self.parse_node(&child, code)? {
                        arguments.push(stmt);
                    }
                }
                _ => continue,
            }
        }

        if let Some(ident) = identifier {
            Ok(ASTNode::Call(ASTCall {
                identifier: ident,
                arguments,
            }))
        } else {
            Err(MageError::ParseError("Call missing identifier".to_string()))
        }
    }

    fn parse_math(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        let mut sections = Vec::new();
        let mut expecting_variable = true;

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "number" => {
                    if expecting_variable {
                        if let ASTNode::Number(number) = self.parse_node(&child, code)? {
                            sections
                                .push(ASTMathSection::Variable(ASTMathVariable::Number(number)));
                            expecting_variable = false;
                        }
                    }
                }
                "math_operation" => {
                    if !expecting_variable {
                        if let ASTNode::MathOperation(op) = self.parse_node(&child, code)? {
                            sections.push(ASTMathSection::Operation(op));
                            expecting_variable = true;
                        }
                    }
                }
                "[" | "]" => continue, // Skip brackets
                _ => continue,
            }
        }

        Ok(ASTNode::Math(ASTMath { sections }))
    }

    fn parse_number(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        for child in node.children(&mut node.walk()) {
            let text = child.utf8_text(code.as_bytes()).unwrap();
            let number = match child.kind() {
                "zero" => ASTNumber::Zero,
                "binary" => ASTNumber::Binary(text.to_string()),
                "octal" => ASTNumber::Octal(text.to_string()),
                "decimal" => ASTNumber::Decimal(text.to_string()),
                "hex" => ASTNumber::Hex(text.to_string()),
                _ => continue,
            };
            return Ok(ASTNode::Number(number));
        }
        Err(MageError::ParseError("Invalid number".to_string()))
    }

    fn parse_string(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        let text = node.utf8_text(code.as_bytes()).unwrap();
        // Remove surrounding quotes
        let value = text[1..text.len() - 1].to_string();
        Ok(ASTNode::String(ASTString { value }))
    }

    fn parse_name(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        let text = node.utf8_text(code.as_bytes()).unwrap();
        Ok(ASTNode::Name(ASTName {
            value: text.to_string(),
        }))
    }

    fn parse_math_operation(&self, node: &Node, code: &str) -> Result<ASTNode, MageError> {
        let text = node.utf8_text(code.as_bytes()).unwrap();
        let op = match text {
            "+" => ASTMathOperation::Add,
            "-" => ASTMathOperation::Subtract,
            "*" => ASTMathOperation::Multiply,
            "/" => ASTMathOperation::Divide,
            "%" => ASTMathOperation::Modulo,
            _ => {
                return Err(MageError::ParseError(format!(
                    "Invalid math operation: {}",
                    text
                )));
            }
        };
        Ok(ASTNode::MathOperation(op))
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
