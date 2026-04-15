use serde::Serialize;

#[derive(Default, Debug, Clone, PartialEq, Serialize)]
pub struct FlatRoot {
    pub sources: Vec<FlatSource>,
    pub strings: Vec<FlatString>,
    pub buffer: String,
    pub expressions: Vec<FlatExpression>,
    pub indices: Vec<FlatIndex>,
}

impl FlatRoot {
    #[inline]
    pub fn get_string(&self, index: u32) -> &str {
        let string = &self.strings[index as usize];
        &self.buffer[string.start as usize..string.end as usize]
    }

    #[inline]
    pub fn get_expression(&self, index: u32) -> &FlatExpression {
        &self.expressions[index as usize]
    }

    #[inline]
    pub fn get_extra_indices(&self, start: u32, end: u32) -> &[FlatIndex] {
        &self.indices[start as usize..end as usize]
    }

    pub fn try_get_string(&self, index: u32) -> Option<&str> {
        let string = self.strings.get(index as usize)?;
        let start = string.start as usize;
        let end = string.end as usize;
        self.buffer.get(start..end)
    }

    #[inline]
    pub fn try_get_expression(&self, index: u32) -> Option<&FlatExpression> {
        self.expressions.get(index as usize)
    }

    #[inline]
    pub fn try_get_extra_indices(&self, start: u32, end: u32) -> Option<&[FlatIndex]> {
        self.indices.get(start as usize..end as usize)
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FlatString {
    pub start: u32,
    pub end: u32,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FlatSource {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FlatExpression {
    Call(FlatCall),
    Constant(FlatAssign),
    Variable(FlatAssign),
    MultipleVariable(FlatMultipleVariable),
    Member(FlatMember),
    BinaryOperation(FlatBinaryOperation),
}

impl Default for FlatExpression {
    fn default() -> Self {
        Self::Call(FlatCall::default())
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FlatCall {
    pub name: FlatIndex,
    pub arguments_start: u32,
    pub arguments_end: u32,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FlatAssign {
    pub name: FlatIndex,
    pub expression: FlatIndex,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FlatMultipleVariable {
    pub names_start: u32,
    pub names_end: u32,
    pub expression: FlatIndex,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FlatMember {
    pub name: FlatIndex,
    pub expression: FlatIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum FlatBinaryOperationKind {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    LessThan,
    GreaterThan,
    LessThanOrEqual,
    GreaterThanOrEqual,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FlatBinaryOperation {
    pub kind: FlatBinaryOperationKind,
    pub left: FlatIndex,
    pub right: FlatIndex,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum FlatIndex {
    #[default]
    None,
    Source(u32),
    Number(u32),
    String(u32),
    Identifier(u32),
    Expression(u32),
}
