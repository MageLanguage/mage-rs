use serde_text::Format;

use crate::{
    FlatAssign, FlatBinaryOperation, FlatBinaryOperationKind, FlatCall, FlatExpression, FlatIndex,
    FlatMember, FlatMultipleVariable, FlatRoot,
};

struct Separators {
    comma: &'static str,
    colon: &'static str,
    equals: &'static str,
    plus: &'static str,
    minus: &'static str,
    multiply: &'static str,
    divide: &'static str,
    modulo: &'static str,
    less_than: &'static str,
    greater_than: &'static str,
    less_than_or_equal: &'static str,
    greater_than_or_equal: &'static str,
    equal: &'static str,
    not_equal: &'static str,
}

const PRETTY_SEPARATORS: Separators = Separators {
    comma: ", ",
    colon: " : ",
    equals: " = ",
    plus: " + ",
    minus: " - ",
    multiply: " * ",
    divide: " / ",
    modulo: " % ",
    less_than: " < ",
    greater_than: " > ",
    less_than_or_equal: " <= ",
    greater_than_or_equal: " >= ",
    equal: " == ",
    not_equal: " != ",
};

const SIMPLE_SEPARATORS: Separators = Separators {
    comma: ",",
    colon: ":",
    equals: "=",
    plus: "+",
    minus: "-",
    multiply: "*",
    divide: "/",
    modulo: "%",
    less_than: "<",
    greater_than: ">",
    less_than_or_equal: "<=",
    greater_than_or_equal: ">=",
    equal: "==",
    not_equal: "!=",
};

pub struct Encoder<'a> {
    root: &'a FlatRoot,
    separators: &'static Separators,
    depth: usize,
    pretty: bool,
}

impl<'a> Encoder<'a> {
    pub fn encode(root: &'a FlatRoot, format: Format) -> String {
        let pretty = matches!(format, Format::Pretty);
        let mut encoder = Encoder {
            root,
            pretty,
            separators: if pretty {
                &PRETTY_SEPARATORS
            } else {
                &SIMPLE_SEPARATORS
            },
            depth: 0,
        };
        let base = root.buffer.len() + root.expressions.len() * 4 + root.indices.len() * 2;
        let estimate = if pretty { base + base / 4 } else { base };
        let mut output = String::with_capacity(estimate);
        encoder.write_source(0, &mut output);
        output
    }

    #[inline]
    fn write_newline(&self, output: &mut String) {
        if self.pretty {
            output.push('\n');
            output.extend(std::iter::repeat_n('\t', self.depth));
        }
    }

    #[inline]
    fn get_string(&self, index: u32) -> &str {
        self.root.get_string(index)
    }

    fn write_source(&mut self, source_index: u32, output: &mut String) {
        let source = self.root.sources[source_index as usize];
        let indices = self.root.get_extra_indices(source.start, source.end);
        let mut first = true;

        for index in indices {
            if !first {
                output.push(';');
                self.write_newline(output);
            }
            first = false;
            self.write_index_unwrapped(index, output);
        }

        if !indices.is_empty() && self.pretty {
            output.push(';');
        }
    }

    fn write_non_expression_index(&mut self, index: &FlatIndex, output: &mut String) -> bool {
        match index {
            FlatIndex::None => true,
            FlatIndex::Source(source_index) => {
                self.write_source_block(*source_index, output);
                true
            }
            FlatIndex::Number(string_index)
            | FlatIndex::String(string_index)
            | FlatIndex::Identifier(string_index) => {
                output.push_str(self.get_string(*string_index));
                true
            }
            FlatIndex::Expression(_) => false,
        }
    }

    fn write_expression(&mut self, expression: &FlatExpression, output: &mut String) {
        match expression {
            FlatExpression::Call(call) => self.write_call(call, output),
            FlatExpression::Constant(assign) => self.write_constant(assign, output),
            FlatExpression::Variable(assign) => self.write_variable(assign, output),
            FlatExpression::MultipleVariable(multiple) => {
                self.write_multiple_variable(multiple, output)
            }
            FlatExpression::Member(member) => self.write_member(member, output),
            FlatExpression::BinaryOperation(binary_operation) => {
                self.write_binary_operation_expression(binary_operation, output)
            }
        }
    }

    fn write_call(&mut self, call: &FlatCall, output: &mut String) {
        if let Some(implicit_member) = self.try_extract_implicit_member(&call.name) {
            output.push('.');
            self.write_index(&implicit_member, output);
        } else {
            self.write_index(&call.name, output);
        }

        let arguments = self
            .root
            .get_extra_indices(call.arguments_start, call.arguments_end);
        if !arguments.is_empty() {
            output.push(' ');
            self.write_arguments(arguments, output);
        }
    }

    fn write_arguments(&mut self, arguments: &[FlatIndex], output: &mut String) {
        let single_call_argument = arguments.len() == 1
            && matches!(
                arguments.first(),
                Some(FlatIndex::Expression(expression_index)) if matches!(self.root.get_expression(*expression_index), FlatExpression::Call(_))
            );

        for (index, argument) in arguments.iter().enumerate() {
            if index > 0 {
                output.push_str(self.separators.comma);
            }
            if single_call_argument {
                self.write_index_unwrapped(argument, output);
            } else {
                self.write_index(argument, output);
            }
        }
    }

    fn try_extract_implicit_member(&self, index: &FlatIndex) -> Option<FlatIndex> {
        if let FlatIndex::Expression(expression_index) = index
            && let FlatExpression::Member(member) = self.root.get_expression(*expression_index)
            && matches!(member.name, FlatIndex::None)
        {
            return Some(member.expression);
        }

        None
    }

    fn write_constant(&mut self, assign: &FlatAssign, output: &mut String) {
        self.write_index(&assign.name, output);
        output.push_str(self.separators.colon);
        self.write_index_unwrapped(&assign.expression, output);
    }

    fn write_variable(&mut self, assign: &FlatAssign, output: &mut String) {
        self.write_index(&assign.name, output);
        output.push_str(self.separators.equals);
        self.write_index_unwrapped(&assign.expression, output);
    }

    fn write_multiple_variable(&mut self, multiple: &FlatMultipleVariable, output: &mut String) {
        let names = self
            .root
            .get_extra_indices(multiple.names_start, multiple.names_end);
        for (i, name) in names.iter().enumerate() {
            if i > 0 {
                output.push_str(self.separators.comma);
            }
            self.write_index(name, output);
        }
        output.push_str(self.separators.equals);
        self.write_index_unwrapped(&multiple.expression, output);
    }

    fn write_member(&mut self, member: &FlatMember, output: &mut String) {
        if !matches!(member.name, FlatIndex::None) {
            self.write_index(&member.name, output);
        }
        output.push('.');
        self.write_index(&member.expression, output);
    }

    fn precedence(kind: FlatBinaryOperationKind) -> u8 {
        match kind {
            FlatBinaryOperationKind::Multiply
            | FlatBinaryOperationKind::Divide
            | FlatBinaryOperationKind::Modulo => 3,
            FlatBinaryOperationKind::Add | FlatBinaryOperationKind::Subtract => 2,
            FlatBinaryOperationKind::LessThan
            | FlatBinaryOperationKind::GreaterThan
            | FlatBinaryOperationKind::LessThanOrEqual
            | FlatBinaryOperationKind::GreaterThanOrEqual
            | FlatBinaryOperationKind::Equal
            | FlatBinaryOperationKind::NotEqual => 1,
        }
    }

    fn needs_precedence_wrap(
        &self,
        index: &FlatIndex,
        parent_precedence: u8,
        is_left: bool,
    ) -> bool {
        if let FlatIndex::Expression(expression_index) = index
            && let FlatExpression::BinaryOperation(child_operation) =
                self.root.get_expression(*expression_index)
        {
            let child_precedence = Self::precedence(child_operation.kind);
            return if is_left {
                child_precedence < parent_precedence
            } else {
                child_precedence <= parent_precedence
            };
        }
        false
    }

    fn write_binary_operation_expression(
        &mut self,
        binary_operation: &FlatBinaryOperation,
        output: &mut String,
    ) {
        let separator = match binary_operation.kind {
            FlatBinaryOperationKind::Add => self.separators.plus,
            FlatBinaryOperationKind::Subtract => self.separators.minus,
            FlatBinaryOperationKind::Multiply => self.separators.multiply,
            FlatBinaryOperationKind::Divide => self.separators.divide,
            FlatBinaryOperationKind::Modulo => self.separators.modulo,
            FlatBinaryOperationKind::LessThan => self.separators.less_than,
            FlatBinaryOperationKind::GreaterThan => self.separators.greater_than,
            FlatBinaryOperationKind::LessThanOrEqual => self.separators.less_than_or_equal,
            FlatBinaryOperationKind::GreaterThanOrEqual => self.separators.greater_than_or_equal,
            FlatBinaryOperationKind::Equal => self.separators.equal,
            FlatBinaryOperationKind::NotEqual => self.separators.not_equal,
        };

        let parent_precedence = Self::precedence(binary_operation.kind);

        if self.needs_precedence_wrap(&binary_operation.left, parent_precedence, true) {
            output.push('(');
            self.write_index_unwrapped(&binary_operation.left, output);
            output.push(')');
        } else {
            self.write_index(&binary_operation.left, output);
        }

        output.push_str(separator);

        if self.needs_precedence_wrap(&binary_operation.right, parent_precedence, false) {
            output.push('(');
            self.write_index_unwrapped(&binary_operation.right, output);
            output.push(')');
        } else {
            self.write_index(&binary_operation.right, output);
        }
    }

    fn write_source_block(&mut self, source_index: u32, output: &mut String) {
        let source = self.root.sources[source_index as usize];
        if source.start == source.end {
            output.push_str("{}");
            return;
        }
        output.push('{');
        self.depth += 1;
        self.write_newline(output);
        self.write_source(source_index, output);
        self.depth -= 1;
        self.write_newline(output);
        output.push('}');
    }

    fn write_index_unwrapped(&mut self, index: &FlatIndex, output: &mut String) {
        if self.write_non_expression_index(index, output) {
            return;
        }

        let FlatIndex::Expression(expression_index) = index else {
            return;
        };
        let expression = self.root.get_expression(*expression_index);
        self.write_expression(expression, output);
    }

    fn write_index(&mut self, index: &FlatIndex, output: &mut String) {
        if self.write_non_expression_index(index, output) {
            return;
        }

        let FlatIndex::Expression(expression_index) = index else {
            return;
        };
        let expression = self.root.get_expression(*expression_index);
        let needs_wrap = matches!(
            expression,
            FlatExpression::Call(_) | FlatExpression::Constant(_) | FlatExpression::Variable(_)
        );
        if needs_wrap {
            output.push('(');
        }
        self.write_expression(expression, output);
        if needs_wrap {
            output.push(')');
        }
    }
}
