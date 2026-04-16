use crate::Compiler;
use crate::layout::{BlockPatch, ExpressionResult, WhilePatch};
use crate::two_pass::FixupLabel;

use mage_ast::{FlatCall, FlatExpression, FlatIndex};
use mage_contract::CompileError;

type Result<T> = std::result::Result<T, CompileError>;

impl<'a> Compiler<'a> {
    pub(crate) fn compile_if(&mut self, call: &FlatCall) -> Result<ExpressionResult> {
        let arguments = self
            .root
            .get_extra_indices(call.arguments_start, call.arguments_end);
        let [condition_index, body_index] = arguments else {
            return Err(CompileError::if_argument_count(
                self.current_statement_offset,
                arguments.len(),
            ));
        };
        let condition_index = *condition_index;
        let body_index = *body_index;

        let if_end_label_id = self.two_pass.fresh_label_id();

        let (condition_offset, condition_is_temporary) =
            self.compile_condition(&condition_index)?;

        self.emit_jump_if_not_to_label(
            condition_offset,
            FixupLabel::IfEnd {
                id: if_end_label_id,
            },
        );
        if condition_is_temporary {
            self.temporaries.free(condition_offset);
        }

        let block_name_index = self.extract_condition_name(&condition_index);

        if let Some(name_index) = block_name_index {
            self.block_patches.push(BlockPatch {
                name_index,
                if_end_label_id,
            });
        }

        let FlatIndex::Source(source_index) = &body_index else {
            return Err(CompileError::if_second_argument_not_block(
                self.current_statement_offset,
            ));
        };

        self.compile_source_statements(*source_index as usize);

        let after_then_offset = self.bytecode_offset();

        if block_name_index.is_some() {
            self.block_patches.pop();
        }

        self.two_pass
            .define_label(if_end_label_id, after_then_offset);

        Ok(ExpressionResult::variable(0))
    }

    pub(crate) fn compile_while(&mut self, call: &FlatCall) -> Result<ExpressionResult> {
        let arguments = self
            .root
            .get_extra_indices(call.arguments_start, call.arguments_end);
        let [condition_index, body_index] = arguments else {
            return Err(CompileError::while_argument_count(
                self.current_statement_offset,
                arguments.len(),
            ));
        };
        let condition_index = *condition_index;
        let body_index = *body_index;

        let loop_body_label_id = self.two_pass.fresh_label_id();
        let loop_condition_label_id = self.two_pass.fresh_label_id();
        let loop_end_label_id = self.two_pass.fresh_label_id();

        self.emit_jump_to_label(FixupLabel::WhileTarget {
            id: loop_condition_label_id,
        });

        let body_start_offset = self.bytecode_offset();
        self.two_pass
            .define_label(loop_body_label_id, body_start_offset);

        let FlatIndex::Source(source_index) = &body_index else {
            return Err(CompileError::while_second_argument_not_block(
                self.current_statement_offset,
            ));
        };

        let while_name_index = self.extract_condition_name(&condition_index);

        self.while_patches.push(WhilePatch {
            name_index: while_name_index,
            while_condition_label_id: loop_condition_label_id,
            while_end_label_id: loop_end_label_id,
        });

        self.compile_source_statements(*source_index as usize);

        self.while_patches.pop();

        let condition_check_offset = self.bytecode_offset();
        self.two_pass
            .define_label(loop_condition_label_id, condition_check_offset);

        let (condition_offset, condition_is_temporary) =
            self.compile_condition(&condition_index)?;

        self.emit_jump_if_to_label(
            condition_offset,
            FixupLabel::WhileTarget {
                id: loop_body_label_id,
            },
        );
        if condition_is_temporary {
            self.temporaries.free(condition_offset);
        }

        let after_loop_offset = self.bytecode_offset();
        self.two_pass
            .define_label(loop_end_label_id, after_loop_offset);

        Ok(ExpressionResult::variable(0))
    }

    pub(crate) fn compile_break(&mut self, call: &FlatCall) -> Result<ExpressionResult> {
        let arguments = self
            .root
            .get_extra_indices(call.arguments_start, call.arguments_end);

        let Some(identifier_index) = match arguments.first() {
            Some(FlatIndex::Identifier(index)) => Some(*index),
            _ => None,
        } else {
            return Err(CompileError::break_outside_block(
                self.current_statement_offset,
            ));
        };

        let block_label_id = self
            .block_patches
            .iter()
            .rev()
            .find(|patch| self.string_index_equals(patch.name_index, identifier_index))
            .map(|patch| patch.if_end_label_id);

        if let Some(label_id) = block_label_id {
            self.emit_jump_to_label(FixupLabel::IfEnd { id: label_id });
            return Ok(ExpressionResult::variable(0));
        }

        let while_label_id = self
            .while_patches
            .iter()
            .rev()
            .find(|patch| {
                patch.name_index.is_some_and(|name_index| {
                    self.string_index_equals(name_index, identifier_index)
                })
            })
            .map(|patch| patch.while_end_label_id);

        if let Some(label_id) = while_label_id {
            self.emit_jump_to_label(FixupLabel::WhileEnd { id: label_id });
            return Ok(ExpressionResult::variable(0));
        }

        Err(CompileError::break_unresolved_target(
            self.current_statement_offset,
            self.identifier_text(identifier_index),
        ))
    }

    pub(crate) fn compile_continue(&mut self, call: &FlatCall) -> Result<ExpressionResult> {
        let arguments = self
            .root
            .get_extra_indices(call.arguments_start, call.arguments_end);
        let Some(identifier_index) =
            if let Some(FlatIndex::Identifier(identifier_index)) = arguments.first() {
                Some(*identifier_index)
            } else {
                None
            }
        else {
            return Err(CompileError::continue_outside_while(
                self.current_statement_offset,
            ));
        };

        let while_patch = self.while_patches.iter().rev().find(|patch| {
            patch.name_index.is_some_and(|name_index| {
                self.string_index_equals(name_index, identifier_index)
            })
        });

        let Some(while_patch) = while_patch else {
            return Err(CompileError::continue_unresolved_target(
                self.current_statement_offset,
                self.identifier_text(identifier_index),
            ));
        };

        let while_condition_label_id = while_patch.while_condition_label_id;
        self.emit_jump_to_label(FixupLabel::WhileTarget {
            id: while_condition_label_id,
        });

        Ok(ExpressionResult::variable(0))
    }

    pub(crate) fn compile_condition(&mut self, condition: &FlatIndex) -> Result<(u64, bool)> {
        let result = self.compile_index_as_expression(condition)?;
        Ok((result.offset, result.is_temporary))
    }

    pub(crate) fn extract_condition_name(&self, condition: &FlatIndex) -> Option<u32> {
        match condition {
            FlatIndex::Identifier(identifier_index) => Some(*identifier_index),
            FlatIndex::Expression(expression_index) => {
                let expression = self.root.get_expression(*expression_index);
                match expression {
                    FlatExpression::Variable(variable) => {
                        if let FlatIndex::Identifier(identifier_index) = &variable.name {
                            Some(*identifier_index)
                        } else {
                            None
                        }
                    }
                    FlatExpression::BinaryOperation(binary_operation) => {
                        if let FlatIndex::Identifier(identifier_index) = &binary_operation.left {
                            Some(*identifier_index)
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
