use crate::Compiler;
use crate::layout::{ExpressionAnalysis, FrameLayout, Procedure, VALUE_SIZE, VariableBinding};

use mage_ast::{FlatCall, FlatExpression, FlatIndex};
use mage_contract::{CompileError, Instruction, LoadTargetOffsetSourceImmutable};

type Result<T> = std::result::Result<T, CompileError>;

impl<'a> Compiler<'a> {
    pub(crate) fn register_all_procedures(&mut self) -> Result<()> {
        let Some(source) = self.root.sources.first() else {
            return Ok(());
        };

        self.procedures.push(Procedure {
            name_index: u32::MAX,
            source_index: 0,
            parameters_start: 0,
            parameters_end: 0,
            return_count: 1,
            bytecode_offset: None,
            layout: FrameLayout::default(),
        });

        for &statement_index in self.root.get_extra_indices(source.start, source.end) {
            let FlatIndex::Expression(statement_expression_index) = statement_index else {
                continue;
            };
            let FlatExpression::Constant(constant) =
                self.root.get_expression(statement_expression_index)
            else {
                continue;
            };
            let FlatIndex::Identifier(name_index) = &constant.name else {
                continue;
            };

            let Some((parameter_source_index, return_source_index, body_source_index)) =
                self.extract_procedure_declaration(constant.expression)
            else {
                continue;
            };

            let parameter_source = &self.root.sources[parameter_source_index];
            let parameters_start = self.parameter_pool.len() as u32;

            for &index in self
                .root
                .get_extra_indices(parameter_source.start, parameter_source.end)
            {
                if let FlatIndex::Expression(parameter_expression_index) = index
                    && let FlatExpression::Constant(parameter) =
                        self.root.get_expression(parameter_expression_index)
                    && let FlatIndex::Identifier(parameter_name_index) = &parameter.name
                {
                    self.parameter_pool.push(*parameter_name_index);
                }
            }

            let parameters_end = self.parameter_pool.len() as u32;

            let return_count = return_source_index
                .map(|return_source_index| {
                    let return_source = self.root.sources[return_source_index];
                    self.root
                        .get_extra_indices(return_source.start, return_source.end)
                        .iter()
                        .filter(|index| matches!(index, FlatIndex::Expression(expression_index) if matches!(self.root.get_expression(*expression_index), FlatExpression::Constant(_))))
                        .count()
                        .max(1)
                })
                .unwrap_or(1);

            if self
                .procedures
                .iter()
                .any(|procedure| self.string_index_equals(procedure.name_index, *name_index))
            {
                return Err(CompileError::duplicate_procedure(
                    self.source_locations
                        .get_expression_offset(statement_expression_index),
                    self.identifier_text(*name_index),
                ));
            }

            self.procedures.push(Procedure {
                name_index: *name_index,
                source_index: body_source_index,
                parameters_start,
                parameters_end,
                return_count,
                bytecode_offset: None,
                layout: FrameLayout::default(),
            });
        }

        self.precompute_source_analyses();

        for index in 0..self.procedures.len() {
            let source_index = self.procedures[index].source_index;
            let layout = self.compute_procedure_layout(source_index);
            self.procedures[index].layout = layout;
        }

        Ok(())
    }

    pub(crate) fn extract_procedure_declaration(
        &self,
        expression: FlatIndex,
    ) -> Option<(usize, Option<usize>, usize)> {
        let FlatIndex::Expression(expression_index) = expression else {
            return None;
        };
        let FlatExpression::Call(call) = self.root.get_expression(expression_index) else {
            return None;
        };

        let procedure_arguments = self.procedure_constructor_arguments(call)?;
        let body_source_index =
            self.single_source_argument(call.arguments_start, call.arguments_end)?;
        let (parameter_source_index, return_source_index) =
            self.split_procedure_sources(procedure_arguments)?;

        Some((
            parameter_source_index,
            return_source_index,
            body_source_index,
        ))
    }

    fn procedure_constructor_arguments<'b>(&'b self, call: &FlatCall) -> Option<&'b [FlatIndex]> {
        let FlatIndex::Expression(callee_expression_index) = call.name else {
            return None;
        };
        let FlatExpression::Call(inner_call) = self.root.get_expression(callee_expression_index)
        else {
            return None;
        };
        let FlatIndex::Identifier(call_name_index) = inner_call.name else {
            return None;
        };
        if self.get_string(call_name_index) != "procedure" {
            return None;
        }
        Some(
            self.root
                .get_extra_indices(inner_call.arguments_start, inner_call.arguments_end),
        )
    }

    fn single_source_argument(&self, start: u32, end: u32) -> Option<usize> {
        let arguments = self.root.get_extra_indices(start, end);
        if arguments.len() != 1 {
            return None;
        }
        let FlatIndex::Source(source_index) = arguments[0] else {
            return None;
        };
        Some(source_index as usize)
    }

    fn split_procedure_sources(&self, arguments: &[FlatIndex]) -> Option<(usize, Option<usize>)> {
        let mut source_indices = arguments.iter().filter_map(|argument| match argument {
            FlatIndex::Source(source_index) => Some(*source_index as usize),
            _ => None,
        });

        let parameter_source_index = source_indices.next()?;
        let return_source_index = source_indices.next();

        if source_indices.next().is_some() {
            return None;
        }

        Some((parameter_source_index, return_source_index))
    }

    fn compute_procedure_layout(&self, source_index: usize) -> FrameLayout {
        let analysis = self.source_analyses[source_index];

        let call_area = if analysis.has_procedure_call {
            analysis.max_call_area_slots as u64 * VALUE_SIZE
        } else {
            0
        };

        let allocator_estimate = analysis.temp_estimate as u64 * VALUE_SIZE;

        let min_total = 2 * VALUE_SIZE;
        let allocator_capacity = if call_area + allocator_estimate < min_total {
            min_total - call_area
        } else {
            allocator_estimate
        };

        FrameLayout::new(call_area, allocator_capacity)
    }

    pub(crate) fn precompute_source_analyses(&mut self) {
        let source_count = self.root.sources.len();
        let mut source_cache = vec![ExpressionAnalysis::default(); source_count];
        let mut expression_cache = vec![None::<ExpressionAnalysis>; self.root.expressions.len()];

        for source_index in (0..source_count).rev() {
            let source = &self.root.sources[source_index];
            let mut result = ExpressionAnalysis::default();
            for &index in self.root.get_extra_indices(source.start, source.end) {
                result = result.merge_max(self.analyze_index_with_cache(
                    &index,
                    &source_cache,
                    &mut expression_cache,
                ));
            }
            source_cache[source_index] = result;
        }

        self.source_analyses = source_cache;
    }

    fn analyze_expression_with_cache(
        &self,
        expression: &FlatExpression,
        source_cache: &[ExpressionAnalysis],
        expression_cache: &mut [Option<ExpressionAnalysis>],
    ) -> ExpressionAnalysis {
        match expression {
            FlatExpression::Call(call) => {
                let FlatIndex::Identifier(name_index) = &call.name else {
                    return ExpressionAnalysis::default();
                };
                let arguments = self
                    .root
                    .get_extra_indices(call.arguments_start, call.arguments_end);
                if self.is_builtin_keyword(*name_index) {
                    let mut result = ExpressionAnalysis::default();
                    for argument in arguments {
                        result = result.merge_max(self.analyze_index_with_cache(
                            argument,
                            source_cache,
                            expression_cache,
                        ));
                    }
                    return result;
                }

                let call_area_slots = self
                    .procedures
                    .iter()
                    .find(|procedure| self.string_index_equals(procedure.name_index, *name_index))
                    .map(|procedure| {
                        (procedure.return_count
                            + 1
                            + (procedure.parameters_end - procedure.parameters_start) as usize)
                            as u16
                    })
                    .unwrap_or(2);

                let mut result = ExpressionAnalysis::default();
                for argument in arguments {
                    result = result.merge_max(self.analyze_index_with_cache(
                        argument,
                        source_cache,
                        expression_cache,
                    ));
                }
                result.with_procedure_call(call_area_slots)
            }
            FlatExpression::Variable(assign) => {
                self.analyze_index_with_cache(&assign.expression, source_cache, expression_cache)
            }
            FlatExpression::Constant(assign) => {
                self.analyze_index_with_cache(&assign.expression, source_cache, expression_cache)
            }
            FlatExpression::MultipleVariable(multiple) => {
                self.analyze_index_with_cache(&multiple.expression, source_cache, expression_cache)
            }
            FlatExpression::BinaryOperation(binary_operation) => {
                let left = self.analyze_index_with_cache(
                    &binary_operation.left,
                    source_cache,
                    expression_cache,
                );
                let right = self.analyze_index_with_cache(
                    &binary_operation.right,
                    source_cache,
                    expression_cache,
                );
                ExpressionAnalysis::merge_binary(left, right, true)
            }
            FlatExpression::Member(_) => ExpressionAnalysis::default(),
        }
    }

    fn analyze_index_with_cache(
        &self,
        index: &FlatIndex,
        source_cache: &[ExpressionAnalysis],
        expression_cache: &mut [Option<ExpressionAnalysis>],
    ) -> ExpressionAnalysis {
        match index {
            FlatIndex::Expression(expression_index) => {
                let index = *expression_index as usize;
                if let Some(cached) = expression_cache[index] {
                    return cached;
                }
                let result = self.analyze_expression_with_cache(
                    self.root.get_expression(*expression_index),
                    source_cache,
                    expression_cache,
                );
                expression_cache[index] = Some(result);
                result
            }
            FlatIndex::Source(source_index) => source_cache[*source_index as usize],
            _ => ExpressionAnalysis::default(),
        }
    }

    pub(crate) fn compile_procedures(&mut self) {
        let procedure_count = self.procedures.len();

        for procedure_index in 0..procedure_count {
            let source_index = self.procedures[procedure_index].source_index;
            let mut layout = self.procedures[procedure_index].layout;

            let procedure_offset = self.bytecode_offset();
            self.procedures[procedure_index].bytecode_offset = Some(procedure_offset);

            let previous_layout = self.current_layout;
            let previous_procedure_index = self.current_procedure_index;

            self.current_layout = layout;
            self.current_procedure_index = Some(procedure_index);

            self.temporaries.reset();
            self.temporaries.reserve_below(layout.temporaries_start);

            let return_count = self.procedures[procedure_index].return_count;

            self.variables.clear();
            self.scope_starts.clear();
            self.block_patches.clear();
            self.while_patches.clear();
            let parameters_start = self.procedures[procedure_index].parameters_start as usize;
            let parameters_end = self.procedures[procedure_index].parameters_end as usize;
            for parameter_index in 0..(parameters_end - parameters_start) {
                let parameter_name_index = self.parameter_pool[parameters_start + parameter_index];
                let offset = layout.parameter_offset(return_count, parameter_index);
                self.variables.push(VariableBinding {
                    name_index: parameter_name_index,
                    offset,
                    is_constant: false,
                });
            }

            self.compile_source_statements(source_index);

            let actual_high_water = self.temporaries.high_water_mark();
            if actual_high_water > layout.total {
                layout.total = actual_high_water;
                self.procedures[procedure_index].layout = layout;
                self.current_layout = layout;
            }

            let source = &self.root.sources[source_index];
            let needs_implicit_return = match self
                .root
                .get_extra_indices(source.start, source.end)
                .last()
            {
                Some(&FlatIndex::Expression(expression_index)) => {
                    !matches!(self.root.get_expression(expression_index), FlatExpression::Call(call) if self.is_return_call(call))
                }
                _ => true,
            };

            if needs_implicit_return {
                self.sync_frame_layout();
                for return_index in 0..return_count {
                    self.emit(&Instruction::LoadTargetOffsetSourceImmutable(
                        LoadTargetOffsetSourceImmutable {
                            target: self.current_layout.return_value_offset(return_index),
                            source: 0,
                        },
                    ));
                }
                let return_address_offset = self.current_layout.caller_return_site_id(return_count);
                self.emit_free_jump_offset(self.current_layout.total, return_address_offset);
            }

            self.variables.clear();
            self.scope_starts.clear();
            self.current_layout = previous_layout;
            self.current_procedure_index = previous_procedure_index;
        }
    }
}
