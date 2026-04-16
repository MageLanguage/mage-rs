use crate::Compiler;
use crate::layout::{ExpressionResult, VALUE_SIZE, VariableBinding};

use mage_ast::{FlatAssign, FlatCall, FlatExpression, FlatIndex, FlatMultipleVariable};
use mage_contract::{CompileError, Instruction, LoadTargetOffsetSourceOffset};

type Result<T> = std::result::Result<T, CompileError>;

impl<'a> Compiler<'a> {
    pub(crate) fn compile_source_statements(&mut self, source_index: usize) {
        let source = &self.root.sources[source_index];
        let statement_indices = self.root.get_extra_indices(source.start, source.end);
        let mut last_result: Option<ExpressionResult> = None;

        self.enter_scope();

        for (statement_position, &statement_index) in statement_indices.iter().enumerate() {
            if let Some(source_offset) = self
                .source_locations
                .statement_offset(source_index, statement_position)
            {
                self.current_statement_offset = source_offset;
                let source_end = self
                    .source_locations
                    .statement_offset(source_index, statement_position + 1)
                    .unwrap_or(u32::MAX);
                let bytecode_offset = self.writer.bytecode.len() as u32;
                self.source_map
                    .push(bytecode_offset, source_offset, source_end);
            }

            let FlatIndex::Expression(expression_index) = statement_index else {
                continue;
            };

            if let Some(previous_result) = last_result.take() {
                self.temporaries.free_if_temporary(previous_result);
            }

            last_result = self.compile_statement_expression(expression_index);
        }

        if let Some(result) = last_result {
            self.temporaries.free_if_temporary(result);
        }

        self.exit_scope();
    }

    fn compile_statement_expression(&mut self, expression_index: u32) -> Option<ExpressionResult> {
        match self.root.get_expression(expression_index) {
            FlatExpression::Constant(constant) => {
                if let Err(error) = self.compile_constant(constant) {
                    self.errors.push(error);
                }
                None
            }
            FlatExpression::Variable(assign) => {
                if let Err(error) = self.compile_variable_assignment(assign) {
                    self.errors.push(error);
                }
                None
            }
            FlatExpression::MultipleVariable(multiple) => {
                if let Err(error) = self.compile_multiple_variable_assignment(multiple) {
                    self.errors.push(error);
                }
                None
            }
            FlatExpression::Call(call) => self.compile_call_as_statement(call),
            expression @ (FlatExpression::BinaryOperation(_) | FlatExpression::Member(_)) => {
                match self.compile_expression(expression) {
                    Ok(result) => {
                        self.sync_frame_layout();
                        let target = self.current_layout.return_value_offset(0);
                        self.emit_copy_if_needed(result, target);
                        self.temporaries.free_if_temporary(result);
                        Some(ExpressionResult::variable(target))
                    }
                    Err(error) => {
                        self.errors.push(error);
                        None
                    }
                }
            }
        }
    }

    fn compile_call_as_statement(&mut self, call: &FlatCall) -> Option<ExpressionResult> {
        let procedure_index = match &call.name {
            FlatIndex::Identifier(identifier_index)
                if self.bootstrap_form_name(*identifier_index).is_none() =>
            {
                self.find_procedure_index(*identifier_index)
            }
            _ => None,
        };

        if let Some(procedure_index) = procedure_index {
            if let Err(error) = self.emit_call_sequence(call, procedure_index) {
                self.errors.push(error);
            }
            return Some(ExpressionResult::variable(0));
        }

        match self.compile_call(call) {
            Ok(value) => Some(value),
            Err(error) => {
                self.errors.push(error);
                None
            }
        }
    }

    fn create_variable_binding(&mut self, name_index: u32, is_constant: bool) -> u64 {
        let offset = self.temporaries.allocate();
        self.variables.push(VariableBinding {
            name_index,
            offset,
            is_constant,
        });
        offset
    }

    fn declare_constant_target(&mut self, name_index: u32) -> Result<u64> {
        if self.find_variable_in_current_scope(name_index).is_some() {
            return Err(CompileError::duplicate_constant(
                self.current_statement_offset,
                self.identifier_text(name_index),
            ));
        }

        Ok(self.create_variable_binding(name_index, true))
    }

    fn resolve_variable_target(&mut self, name_index: u32) -> Result<(u64, bool)> {
        if let Some(binding) = self.find_variable(name_index) {
            if binding.is_constant {
                return Err(CompileError::assignment_to_constant(
                    self.current_statement_offset,
                    self.identifier_text(name_index),
                ));
            }
            return Ok((binding.offset, false));
        }

        Ok((self.create_variable_binding(name_index, false), true))
    }

    fn compile_constant(&mut self, constant: &FlatAssign) -> Result<()> {
        let FlatIndex::Identifier(name_index) = &constant.name else {
            return Err(CompileError::invalid_constant_target(
                self.current_statement_offset,
            ));
        };

        if self.is_procedure_declaration(constant) {
            return Ok(());
        }

        let target_offset = self.declare_constant_target(*name_index)?;

        if let Err(error) = self.compile_index_to_offset(&constant.expression, target_offset) {
            self.emit_poison_value(target_offset);
            return Err(error);
        }

        Ok(())
    }

    fn compile_variable_assignment(&mut self, assign: &FlatAssign) -> Result<()> {
        let FlatIndex::Identifier(name_index) = &assign.name else {
            return Err(CompileError::invalid_assignment_target(
                self.current_statement_offset,
            ));
        };

        let (target_offset, is_new_variable) = self.resolve_variable_target(*name_index)?;

        if let Err(error) = self.compile_index_to_offset(&assign.expression, target_offset) {
            if is_new_variable {
                self.emit_poison_value(target_offset);
            }
            return Err(error);
        }
        Ok(())
    }

    fn compile_multiple_variable_assignment(
        &mut self,
        multiple: &FlatMultipleVariable,
    ) -> Result<()> {
        let names = self
            .root
            .get_extra_indices(multiple.names_start, multiple.names_end);

        let (call, procedure_index) = self.resolve_call_target(&multiple.expression)?;
        let return_count = self.procedures[procedure_index].return_count;

        self.resolve_assignment_targets(names)?;

        if let Err(error) = self.emit_call_sequence(call, procedure_index) {
            for target_index in 0..self.target_offsets_scratch.len() {
                let target = self.target_offsets_scratch[target_index];
                self.emit_poison_value(target);
            }
            return Err(error);
        }

        let copy_count = names.len().min(return_count);
        for return_index in 0..copy_count {
            let target = self.target_offsets_scratch[return_index];
            let source_offset = return_index as u64 * VALUE_SIZE;
            if source_offset != target {
                self.emit(&Instruction::LoadTargetOffsetSourceOffset(
                    LoadTargetOffsetSourceOffset {
                        target,
                        source: source_offset,
                    },
                ));
            }
        }

        Ok(())
    }

    fn resolve_call_target(&self, expression: &FlatIndex) -> Result<(&'a FlatCall, usize)> {
        let FlatIndex::Expression(expression_index) = expression else {
            return Err(CompileError::multiple_variable_expression_not_call(
                self.current_statement_offset,
            ));
        };
        let FlatExpression::Call(call) = self.root.get_expression(*expression_index) else {
            return Err(CompileError::multiple_variable_expression_not_call(
                self.current_statement_offset,
            ));
        };
        let FlatIndex::Identifier(identifier_index) = &call.name else {
            return Err(CompileError::unsupported_call_target(
                self.current_statement_offset,
            ));
        };
        let procedure_index = self
            .find_procedure_index(*identifier_index)
            .ok_or_else(|| {
                CompileError::undefined_procedure(
                    self.current_statement_offset,
                    self.identifier_text(*identifier_index),
                )
            })?;
        Ok((call, procedure_index))
    }

    fn resolve_assignment_targets(&mut self, names: &[FlatIndex]) -> Result<()> {
        self.target_offsets_scratch.clear();
        self.target_offsets_scratch.reserve(names.len());

        for name in names {
            let FlatIndex::Identifier(name_index) = name else {
                return Err(CompileError::invalid_multiple_assignment_target(
                    self.current_statement_offset,
                ));
            };

            let (offset, _is_new_variable) = self.resolve_variable_target(*name_index)?;
            self.target_offsets_scratch.push(offset);
        }

        Ok(())
    }
}
