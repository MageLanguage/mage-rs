use crate::Compiler;
use crate::layout::{
    ExpressionResult, FIELD_1_OFFSET, LastComparison, comparison_jump_if_kind,
    comparison_jump_if_not_kind,
};

use mage_ast::{FlatBinaryOperation, FlatBinaryOperationKind, FlatExpression, FlatIndex};
use mage_contract::*;

type Result<T> = std::result::Result<T, CompileError>;

impl<'a> Compiler<'a> {
    pub(crate) fn compile_expression(
        &mut self,
        expression: &FlatExpression,
    ) -> Result<ExpressionResult> {
        match expression {
            FlatExpression::BinaryOperation(binary_operation) => {
                self.compile_binary_operation(binary_operation)
            }
            FlatExpression::Call(call) => self.compile_call(call),
            _ => Err(CompileError::unsupported_expression(
                self.current_statement_offset,
            )),
        }
    }

    pub(crate) fn resolve_number_value(&self, string_index: u32) -> Result<u64> {
        self.parse_number(string_index).ok_or_else(|| {
            CompileError::invalid_number_literal(
                self.current_statement_offset,
                self.get_string(string_index),
            )
        })
    }

    pub(crate) fn compile_index_as_expression(
        &mut self,
        index: &FlatIndex,
    ) -> Result<ExpressionResult> {
        match index {
            FlatIndex::None => Ok(ExpressionResult::temporary(self.emit_load_immutable(0))),
            FlatIndex::Number(string_index) => {
                let value = self.resolve_number_value(*string_index)?;
                Ok(ExpressionResult::temporary(self.emit_load_immutable(value)))
            }
            FlatIndex::Identifier(string_index) => {
                if let Some(binding) = self.find_variable(*string_index) {
                    Ok(ExpressionResult::variable(binding.offset))
                } else if let Some(value) = self.try_resolve_immutable(index) {
                    Ok(ExpressionResult::temporary(self.emit_load_immutable(value)))
                } else {
                    Err(CompileError::undefined_variable(
                        self.current_statement_offset,
                        self.identifier_text(*string_index),
                    ))
                }
            }
            FlatIndex::Expression(expression_index) => {
                self.compile_expression(self.root.get_expression(*expression_index))
            }
            _ => Err(CompileError::unsupported_index(
                self.current_statement_offset,
            )),
        }
    }

    pub(crate) fn compile_expression_to_offset(
        &mut self,
        expression: &FlatExpression,
        target: u64,
    ) -> Result<()> {
        if let FlatExpression::BinaryOperation(binary_operation) = expression {
            return self.compile_binary_operation_to_offset(binary_operation, target);
        }
        let value = self.compile_expression(expression)?;
        self.emit_copy_if_needed(value, target);
        self.temporaries.free_if_temporary(value);
        Ok(())
    }

    pub(crate) fn compile_binary_operation(
        &mut self,
        binary_operation: &FlatBinaryOperation,
    ) -> Result<ExpressionResult> {
        let kind = binary_operation.kind;
        let mut result = self.compile_binary_chain_left(&binary_operation.left, kind)?;
        result = self.compile_binary_chain_step(result, &binary_operation.right, kind)?;

        Ok(result)
    }

    fn compile_binary_operation_to_offset(
        &mut self,
        binary_operation: &FlatBinaryOperation,
        target: u64,
    ) -> Result<()> {
        let kind = binary_operation.kind;
        let result = self.compile_binary_chain_left(&binary_operation.left, kind)?;
        self.compile_binary_chain_step_to_offset(result, &binary_operation.right, kind, target)
    }

    fn compile_binary_chain_left(
        &mut self,
        left: &FlatIndex,
        kind: FlatBinaryOperationKind,
    ) -> Result<ExpressionResult> {
        // Collect right operands while walking the left spine of same-kind
        // operations, then compile left-to-right. This avoids recursion that
        // would overflow the stack on long chains like `a + b + c + ... `.
        let mut operands = Vec::new();
        let mut current = *left;

        while let FlatIndex::Expression(expression_index) = current
            && let FlatExpression::BinaryOperation(left_operation) =
                self.root.get_expression(expression_index)
            && left_operation.kind == kind
        {
            operands.push(left_operation.right);
            current = left_operation.left;
        }

        let mut result = self.compile_index_as_expression(&current)?;

        for operand in operands.into_iter().rev() {
            result = self.compile_binary_chain_step(result, &operand, kind)?;
        }

        Ok(result)
    }

    fn compile_binary_chain_step(
        &mut self,
        result: ExpressionResult,
        operand: &FlatIndex,
        kind: FlatBinaryOperationKind,
    ) -> Result<ExpressionResult> {
        if let Some(value) = self.try_resolve_immutable(operand) {
            let target_offset = if result.is_temporary {
                result.offset
            } else {
                self.temporaries.allocate()
            };

            self.emit_binary_immutable(target_offset, result.offset, value, kind);
            return Ok(ExpressionResult::temporary(target_offset));
        }

        let result = if result.is_volatile {
            let preserved = self.temporaries.allocate();
            self.emit_copy_if_needed(result, preserved);
            ExpressionResult::temporary(preserved)
        } else {
            result
        };

        let left_offset = result.offset;

        let right = self.compile_index_as_expression(operand)?;

        let target_offset = if result.is_temporary {
            result.offset
        } else {
            self.temporaries.allocate()
        };

        self.emit_binary_offset(target_offset, left_offset, right.offset, kind);
        self.temporaries.free_if_temporary(right);

        Ok(ExpressionResult::temporary(target_offset))
    }

    fn compile_binary_chain_step_to_offset(
        &mut self,
        result: ExpressionResult,
        operand: &FlatIndex,
        kind: FlatBinaryOperationKind,
        target: u64,
    ) -> Result<()> {
        if let Some(value) = self.try_resolve_immutable(operand) {
            self.emit_binary_immutable(target, result.offset, value, kind);
            self.temporaries.free_if_temporary(result);
            return Ok(());
        }

        let result = if result.is_volatile {
            let preserved = self.temporaries.allocate();
            self.emit_copy_if_needed(result, preserved);
            ExpressionResult::temporary(preserved)
        } else {
            result
        };

        let left_offset = result.offset;

        let right = self.compile_index_as_expression(operand)?;

        self.emit_binary_offset(target, left_offset, right.offset, kind);
        self.temporaries.free_if_temporary(result);
        self.temporaries.free_if_temporary(right);

        Ok(())
    }

    pub(crate) fn emit_binary_immutable(
        &mut self,
        target: u64,
        left: u64,
        right: u64,
        operation: FlatBinaryOperationKind,
    ) {
        use FlatBinaryOperationKind::*;
        let offset = match operation {
            Add => self.emit(&Instruction::AddTargetOffsetLeftOffsetRightImmutable(
                AddTargetOffsetLeftOffsetRightImmutable {
                    target,
                    left,
                    right,
                },
            )),
            Subtract => self.emit(&Instruction::SubtractTargetOffsetLeftOffsetRightImmutable(
                SubtractTargetOffsetLeftOffsetRightImmutable {
                    target,
                    left,
                    right,
                },
            )),
            Multiply => self.emit(&Instruction::MultiplyTargetOffsetLeftOffsetRightImmutable(
                MultiplyTargetOffsetLeftOffsetRightImmutable {
                    target,
                    left,
                    right,
                },
            )),
            Divide => self.emit(&Instruction::DivideTargetOffsetLeftOffsetRightImmutable(
                DivideTargetOffsetLeftOffsetRightImmutable {
                    target,
                    left,
                    right,
                },
            )),
            Modulo => self.emit(&Instruction::ModuloTargetOffsetLeftOffsetRightImmutable(
                ModuloTargetOffsetLeftOffsetRightImmutable {
                    target,
                    left,
                    right,
                },
            )),
            LessThan => self.emit(&Instruction::LessThanTargetOffsetLeftOffsetRightImmutable(
                LessThanTargetOffsetLeftOffsetRightImmutable {
                    target,
                    left,
                    right,
                },
            )),
            GreaterThan => self.emit(
                &Instruction::GreaterThanTargetOffsetLeftOffsetRightImmutable(
                    GreaterThanTargetOffsetLeftOffsetRightImmutable {
                        target,
                        left,
                        right,
                    },
                ),
            ),
            LessThanOrEqual => self.emit(
                &Instruction::LessThanOrEqualTargetOffsetLeftOffsetRightImmutable(
                    LessThanOrEqualTargetOffsetLeftOffsetRightImmutable {
                        target,
                        left,
                        right,
                    },
                ),
            ),
            GreaterThanOrEqual => self.emit(
                &Instruction::GreaterThanOrEqualTargetOffsetLeftOffsetRightImmutable(
                    GreaterThanOrEqualTargetOffsetLeftOffsetRightImmutable {
                        target,
                        left,
                        right,
                    },
                ),
            ),
            Equal => self.emit(&Instruction::EqualTargetOffsetLeftOffsetRightImmutable(
                EqualTargetOffsetLeftOffsetRightImmutable {
                    target,
                    left,
                    right,
                },
            )),
            NotEqual => self.emit(&Instruction::NotEqualTargetOffsetLeftOffsetRightImmutable(
                NotEqualTargetOffsetLeftOffsetRightImmutable {
                    target,
                    left,
                    right,
                },
            )),
        };
        // Only comparisons can fuse with a following jump; for arithmetic
        // operations `emit()` already cleared `last_comparison`.
        if let (Some(jifn), Some(jif)) = (
            comparison_jump_if_not_kind(operation, true),
            comparison_jump_if_kind(operation, true),
        ) {
            self.last_comparison = Some(LastComparison {
                opcode_offset: offset,
                target,
                jump_if_not_kind: jifn,
                jump_if_kind: jif,
            });
        }
        self.last_result_target = Some((offset + FIELD_1_OFFSET, target));
    }

    pub(crate) fn emit_binary_offset(
        &mut self,
        target: u64,
        left: u64,
        right: u64,
        operation: FlatBinaryOperationKind,
    ) {
        use FlatBinaryOperationKind::*;
        let offset = match operation {
            Add => self.emit(&Instruction::AddTargetOffsetLeftOffsetRightOffset(
                AddTargetOffsetLeftOffsetRightOffset {
                    target,
                    left,
                    right,
                },
            )),
            Subtract => self.emit(&Instruction::SubtractTargetOffsetLeftOffsetRightOffset(
                SubtractTargetOffsetLeftOffsetRightOffset {
                    target,
                    left,
                    right,
                },
            )),
            Multiply => self.emit(&Instruction::MultiplyTargetOffsetLeftOffsetRightOffset(
                MultiplyTargetOffsetLeftOffsetRightOffset {
                    target,
                    left,
                    right,
                },
            )),
            Divide => self.emit(&Instruction::DivideTargetOffsetLeftOffsetRightOffset(
                DivideTargetOffsetLeftOffsetRightOffset {
                    target,
                    left,
                    right,
                },
            )),
            Modulo => self.emit(&Instruction::ModuloTargetOffsetLeftOffsetRightOffset(
                ModuloTargetOffsetLeftOffsetRightOffset {
                    target,
                    left,
                    right,
                },
            )),
            LessThan => self.emit(&Instruction::LessThanTargetOffsetLeftOffsetRightOffset(
                LessThanTargetOffsetLeftOffsetRightOffset {
                    target,
                    left,
                    right,
                },
            )),
            GreaterThan => self.emit(&Instruction::GreaterThanTargetOffsetLeftOffsetRightOffset(
                GreaterThanTargetOffsetLeftOffsetRightOffset {
                    target,
                    left,
                    right,
                },
            )),
            LessThanOrEqual => self.emit(
                &Instruction::LessThanOrEqualTargetOffsetLeftOffsetRightOffset(
                    LessThanOrEqualTargetOffsetLeftOffsetRightOffset {
                        target,
                        left,
                        right,
                    },
                ),
            ),
            GreaterThanOrEqual => self.emit(
                &Instruction::GreaterThanOrEqualTargetOffsetLeftOffsetRightOffset(
                    GreaterThanOrEqualTargetOffsetLeftOffsetRightOffset {
                        target,
                        left,
                        right,
                    },
                ),
            ),
            Equal => self.emit(&Instruction::EqualTargetOffsetLeftOffsetRightOffset(
                EqualTargetOffsetLeftOffsetRightOffset {
                    target,
                    left,
                    right,
                },
            )),
            NotEqual => self.emit(&Instruction::NotEqualTargetOffsetLeftOffsetRightOffset(
                NotEqualTargetOffsetLeftOffsetRightOffset {
                    target,
                    left,
                    right,
                },
            )),
        };
        // Only comparisons can fuse with a following jump; for arithmetic
        // operations `emit()` already cleared `last_comparison`.
        if let (Some(jifn), Some(jif)) = (
            comparison_jump_if_not_kind(operation, false),
            comparison_jump_if_kind(operation, false),
        ) {
            self.last_comparison = Some(LastComparison {
                opcode_offset: offset,
                target,
                jump_if_not_kind: jifn,
                jump_if_kind: jif,
            });
        }
        self.last_result_target = Some((offset + FIELD_1_OFFSET, target));
    }

    pub(crate) fn compile_index_to_offset(&mut self, index: &FlatIndex, target: u64) -> Result<()> {
        match index {
            FlatIndex::Expression(expression_index) => self
                .compile_expression_to_offset(self.root.get_expression(*expression_index), target),
            _ => self.load_index_to_offset(index, target),
        }
    }

    pub(crate) fn load_index_to_offset(&mut self, index: &FlatIndex, target: u64) -> Result<()> {
        match index {
            FlatIndex::None => {
                self.emit(&Instruction::LoadTargetOffsetSourceImmutable(
                    LoadTargetOffsetSourceImmutable { target, source: 0 },
                ));
            }
            FlatIndex::Number(string_index) => {
                let value = self.resolve_number_value(*string_index)?;
                self.emit(&Instruction::LoadTargetOffsetSourceImmutable(
                    LoadTargetOffsetSourceImmutable {
                        target,
                        source: value,
                    },
                ));
            }
            FlatIndex::Identifier(string_index) => {
                if let Some(binding) = self.find_variable(*string_index) {
                    if binding.offset != target {
                        self.emit(&Instruction::LoadTargetOffsetSourceOffset(
                            LoadTargetOffsetSourceOffset {
                                target,
                                source: binding.offset,
                            },
                        ));
                    }
                } else if let Some(value) = self.try_resolve_immutable(index) {
                    self.emit(&Instruction::LoadTargetOffsetSourceImmutable(
                        LoadTargetOffsetSourceImmutable {
                            target,
                            source: value,
                        },
                    ));
                } else {
                    return Err(CompileError::undefined_variable(
                        self.current_statement_offset,
                        self.identifier_text(*string_index),
                    ));
                }
            }
            FlatIndex::Expression(expression_index) => {
                self.compile_expression_to_offset(
                    self.root.get_expression(*expression_index),
                    target,
                )?;
            }
            _ => {
                let value = self.compile_index_as_expression(index)?;
                self.emit_copy_if_needed(value, target);
                self.temporaries.free_if_temporary(value);
            }
        }
        Ok(())
    }

    pub(crate) fn emit_load_immutable(&mut self, value: u64) -> u64 {
        let offset = self.temporaries.allocate();
        let instruction_offset = self.emit(&Instruction::LoadTargetOffsetSourceImmutable(
            LoadTargetOffsetSourceImmutable {
                target: offset,
                source: value,
            },
        ));
        self.last_result_target = Some((instruction_offset + FIELD_1_OFFSET, offset));
        offset
    }

    pub(crate) fn emit_copy_if_needed(&mut self, source: ExpressionResult, target: u64) {
        if source.offset == target {
            return;
        }
        self.emit(&Instruction::LoadTargetOffsetSourceOffset(
            LoadTargetOffsetSourceOffset {
                target,
                source: source.offset,
            },
        ));
    }

    pub(crate) fn emit_poison_value(&mut self, target: u64) {
        self.emit(&Instruction::LoadTargetOffsetSourceImmutable(
            LoadTargetOffsetSourceImmutable { target, source: 0 },
        ));
    }
}
