//! # Mage Compiler
//!
//! Compiles a flat AST ([`mage_ast::FlatRoot`]) into bytecode that the Mage VM
//! can execute.
//!
//! ## Calling Convention
//!
//! The Mage VM uses a stack-based calling convention.  Every procedure
//! (including the root scope) owns a contiguous stack frame.  The `rsp`
//! register always points to the lowest address (byte offset 0) of the active
//! frame.
//!
//! ### Frame Layout
//!
//! From low address (`rsp`) to high address:
//!
//! Memory order in the caller frame:
//!   0 → Return variables → Return address → Argument variables
//!
//! When the callee returns R values, the call area is laid out as:
//!
//! ```text
//!   ┌─────────────────────────────────┐  offset 0
//!   │  Call area                      │  Space reserved for outgoing calls.
//!   │    [0]           return value 0 │  ← callee writes return values here
//!   │    [8]           return value 1 │    (R slots for R return values)
//!   │    …                            │
//!   │    [R×8]         return address │  ← return-site id
//!   │    [(R+1)×8]     argument 0     │  ← caller writes arguments here
//!   │    [(R+2)×8]     argument 1     │
//!   │    …                            │
//!   ├─────────────────────────────────┤  temporaries_start  (= call_area)
//!   │  Allocator-managed slots        │  Variables, constants, and expression
//!   │                                 │  temporaries — all managed uniformly
//!   │                                 │  by TemporaryAllocator.
//!   ├─────────────────────────────────┤  total  (= frame size)
//!   │  Caller's frame (above)         │  For a frame returning R values:
//!   │    [total]       return value 0 │  ← THIS frame writes its results
//!   │    [total + 8]   return value 1 │
//!   │    …                            │
//!   │    [total+R×8]   return address │  ← return-site id written by caller
//!   │    [total+(R+1)×8] parameter 0  │  ← THIS frame reads its params here
//!   │    [total+(R+2)×8] parameter 1  │
//!   │    …                            │
//!   └─────────────────────────────────┘
//! ```
//!
//! ### Call Sequence
//!
//! 1. Caller writes a **return-site id** at offset `R × 8` in its call area
//!    (where R is the callee's return-value count).
//! 2. Caller writes arguments at offsets `(R+1)×8`, `(R+2)×8`, …
//! 3. Caller emits `Call(to=target, take=callee_frame_size)`:
//!    - VM subtracts `callee_frame_size` from `rsp`, creating the callee's
//!      frame.
//!    - VM jumps to `target`.
//! 4. Callee executes; reads parameters at `[total+(R+1)×8]`, …
//! 5. Callee writes its R return values at `[total]`, `[total+8]`, …
//! 6. Callee emits `Return(to=dispatch, free=callee_frame_size)`:
//!    - VM adds `callee_frame_size` to `rsp`, restoring the caller's frame.
//!    - VM jumps to `dispatch` (or directly to the return site when there is
//!      only one call site).

mod control_flow;
mod emit;
mod expressions;
pub(crate) mod layout;
mod procedures;
mod statements;
mod temporaries;
mod two_pass;

#[cfg(test)]
mod compiler_test;

use layout::{
    BlockPatch, ExpressionAnalysis, ExpressionResult, FIELD_2_OFFSET, FrameLayout, LastComparison,
    Procedure, VALUE_SIZE, VariableBinding, WhilePatch,
};
use temporaries::TemporaryAllocator;
use two_pass::TwoPassState;

use mage_ast::{
    BootstrapForm, FlatAssign, FlatCall, FlatIndex, FlatRoot, SourceLocations,
    classify_bootstrap_call_expression, classify_bootstrap_name,
};
use mage_contract::{
    Bytecode, CompileError, ExitCodeImmutable, ExitCodeOffset, Instruction,
    LoadTargetOffsetSourceImmutable, PatchMap, SourceMap, Writer,
};

type Result<T> = std::result::Result<T, CompileError>;

pub struct Compiler<'a> {
    pub(crate) root: &'a FlatRoot,
    pub(crate) source_locations: &'a SourceLocations,
    pub(crate) writer: Writer,
    pub(crate) temporaries: TemporaryAllocator,
    pub(crate) source_map: SourceMap,
    pub(crate) patch_map: PatchMap,
    pub(crate) block_patches: Vec<BlockPatch>,
    pub(crate) while_patches: Vec<WhilePatch>,
    pub(crate) procedures: Vec<Procedure>,
    pub(crate) parameter_pool: Vec<u32>,
    pub(crate) variables: Vec<VariableBinding>,
    pub(crate) scope_starts: Vec<usize>,
    pub(crate) two_pass: TwoPassState,
    pub(crate) current_layout: FrameLayout,
    pub(crate) current_procedure_index: Option<usize>,
    pub(crate) exit_offset: usize,
    pub(crate) source_analyses: Vec<ExpressionAnalysis>,
    pub(crate) current_statement_offset: u32,
    pub(crate) errors: Vec<CompileError>,
    pub(crate) target_offsets_scratch: Vec<u64>,
    pub(crate) last_comparison: Option<LastComparison>,
    /// Bytecode position and value of the target field of the last
    /// instruction that wrote to an allocator-managed slot. Used by
    /// `compile_return` to retarget the instruction directly to the
    /// return slot, eliminating the copy instruction.
    pub(crate) last_result_target: Option<(usize, u64)>,
}

impl<'a> Compiler<'a> {
    pub fn compile(
        root: &'a FlatRoot,
        source_locations: &'a SourceLocations,
    ) -> Result<(Bytecode, SourceMap)> {
        let (bytecode, source_map, _patch_map, errors) =
            Self::compile_recovering(root, source_locations);
        if let Some(error) = errors.into_iter().next() {
            Err(error)
        } else {
            Ok((bytecode, source_map))
        }
    }

    pub fn compile_recovering(
        root: &'a FlatRoot,
        source_locations: &'a SourceLocations,
    ) -> (Bytecode, SourceMap, PatchMap, Vec<CompileError>) {
        let expression_count = root.expressions.len();
        let source_count = root.sources.len();

        let mut compiler = Self {
            root,
            source_locations,
            writer: Writer::new(),
            temporaries: TemporaryAllocator::default(),
            source_map: SourceMap::with_capacity(expression_count),
            patch_map: PatchMap::with_capacity(
                expression_count + expression_count / 4,
                expression_count / 4,
            ),
            block_patches: Vec::with_capacity(8),
            while_patches: Vec::with_capacity(8),
            procedures: Vec::with_capacity(16),
            parameter_pool: Vec::with_capacity(32),
            variables: Vec::with_capacity(16),
            scope_starts: Vec::with_capacity(8),
            two_pass: TwoPassState::default(),
            current_layout: FrameLayout::default(),
            current_procedure_index: None,
            exit_offset: 0,
            source_analyses: Vec::with_capacity(source_count),
            current_statement_offset: 0,
            errors: Vec::new(),
            target_offsets_scratch: Vec::new(),
            last_comparison: None,
            last_result_target: None,
        };

        compiler.writer.bytecode.reserve(expression_count * 24);

        if let Err(error) = compiler.register_all_procedures() {
            compiler.errors.push(error);
        } else if let Err(error) = compiler.compile_root() {
            compiler.errors.push(error);
        } else {
            compiler.compile_procedures();
            if let Err(error) = compiler.patch_two_pass_fixups() {
                compiler.errors.push(error);
            }
        }

        let bytecode = compiler.patch_map.write_bytecode(&compiler.writer.bytecode);
        (
            bytecode,
            compiler.source_map,
            compiler.patch_map,
            compiler.errors,
        )
    }

    fn compile_root(&mut self) -> Result<()> {
        if self.root.sources.is_empty() {
            self.emit(&Instruction::ExitCodeImmutable(ExitCodeImmutable {
                code: 0,
            }));
            return Ok(());
        }

        let return_count = self.procedures[0].return_count;
        let return_address_offset = return_count as u64 * VALUE_SIZE;

        let load_offset = self.emit(&Instruction::LoadTargetOffsetSourceImmutable(
            LoadTargetOffsetSourceImmutable {
                target: return_address_offset,
                source: 0,
            },
        ));

        self.emit_take_jump_to_procedure(0);

        self.exit_offset = self.bytecode_offset();
        self.emit(&Instruction::ExitCodeOffset(ExitCodeOffset { code: 0 }));

        self.patch_u64(load_offset + FIELD_2_OFFSET, self.exit_offset as u64);
        self.patch_map
            .push_relocation((load_offset + FIELD_2_OFFSET) as u32);

        Ok(())
    }

    pub(crate) fn compile_call(&mut self, call: &FlatCall) -> Result<ExpressionResult> {
        match classify_bootstrap_call_expression(self.root, call) {
            Some(BootstrapForm::Return) => return self.compile_return(call),
            Some(BootstrapForm::If) => return self.compile_if(call),
            Some(BootstrapForm::While) => return self.compile_while(call),
            Some(BootstrapForm::Break) => return self.compile_break(call),
            Some(BootstrapForm::Continue) => return self.compile_continue(call),
            Some(BootstrapForm::Procedure) => return Ok(ExpressionResult::variable(0)),
            None => {}
        }

        let FlatIndex::Identifier(identifier_index) = &call.name else {
            return Err(CompileError::unsupported_call_target(
                self.current_statement_offset,
            ));
        };

        let procedure_index = match self.find_procedure_index(*identifier_index) {
            Some(index) => index,
            None => {
                return Err(CompileError::undefined_procedure(
                    self.current_statement_offset,
                    self.identifier_text(*identifier_index),
                ));
            }
        };

        self.emit_user_procedure_call(call, procedure_index)
    }

    fn compile_return(&mut self, call: &FlatCall) -> Result<ExpressionResult> {
        let return_count = self.current_return_count(self.current_statement_offset)?;

        let arguments = self
            .root
            .get_extra_indices(call.arguments_start, call.arguments_end);

        let _procedure_index =
            self.current_procedure_index
                .ok_or(CompileError::return_outside_procedure(
                    self.current_statement_offset,
                ))?;

        // Single-return: compile the expression, then try to retarget
        // the last instruction to write directly to the return slot,
        // eliminating the copy instruction.
        if return_count == 1 {
            let result = if let Some(argument) = arguments.first() {
                self.compile_index_as_expression(argument)?
            } else {
                ExpressionResult::temporary(self.emit_load_immutable(0))
            };

            self.sync_frame_layout();
            let target = self.current_layout.return_value_offset(0);

            if result.is_temporary
                && let Some((field_offset, field_value)) = self.last_result_target.take()
                && field_value == result.offset
                && result.offset != target
            {
                self.patch_u64(field_offset, target);
            } else {
                self.emit_copy_if_needed(result, target);
            }
            self.temporaries.free_if_temporary(result);

            let return_address_offset = self.current_layout.caller_return_site_id(return_count);
            self.emit_free_jump_offset(self.current_layout.total, return_address_offset);

            return Ok(ExpressionResult::variable(target));
        }

        // Multi-return: compile to temporaries first, then copy to return
        // slots. This avoids aliasing when a later return expression
        // involves a procedure call that overwrites the call area where
        // earlier return values were placed.
        let mut compiled_results: Vec<ExpressionResult> = Vec::with_capacity(return_count);
        for return_index in 0..return_count {
            if let Some(argument) = arguments.get(return_index) {
                compiled_results.push(self.compile_index_as_expression(argument)?);
            } else {
                compiled_results.push(ExpressionResult::temporary(self.emit_load_immutable(0)));
            }
        }

        self.sync_frame_layout();

        for (return_index, result) in compiled_results.into_iter().enumerate() {
            let target = self.current_layout.return_value_offset(return_index);
            self.emit_copy_if_needed(result, target);
            self.temporaries.free_if_temporary(result);
        }

        let return_address_offset = self.current_layout.caller_return_site_id(return_count);
        self.emit_free_jump_offset(self.current_layout.total, return_address_offset);

        Ok(ExpressionResult::variable(
            self.current_layout.return_value_offset(0),
        ))
    }

    pub(crate) fn emit_call_sequence(
        &mut self,
        call: &FlatCall,
        procedure_index: usize,
    ) -> Result<()> {
        let procedure = &self.procedures[procedure_index];
        let parameter_count = (procedure.parameters_end - procedure.parameters_start) as usize;
        let return_count = procedure.return_count;

        for (argument_index, argument) in self
            .root
            .get_extra_indices(call.arguments_start, call.arguments_end)
            .iter()
            .take(parameter_count)
            .enumerate()
        {
            let target = self
                .current_layout
                .argument_offset(return_count, argument_index);
            self.load_index_to_offset(argument, target)?;
        }

        let return_id_offset = self.current_layout.return_site_id_offset(return_count);

        let load_offset = self.emit(&Instruction::LoadTargetOffsetSourceImmutable(
            LoadTargetOffsetSourceImmutable {
                target: return_id_offset,
                source: 0,
            },
        ));

        self.emit_take_jump_to_procedure(procedure_index);

        let return_offset = self.bytecode_offset();

        self.patch_u64(load_offset + FIELD_2_OFFSET, return_offset as u64);
        self.patch_map
            .push_relocation((load_offset + FIELD_2_OFFSET) as u32);

        Ok(())
    }

    fn emit_user_procedure_call(
        &mut self,
        call: &FlatCall,
        procedure_index: usize,
    ) -> Result<ExpressionResult> {
        self.emit_call_sequence(call, procedure_index)?;
        Ok(ExpressionResult::volatile(0))
    }

    /// Must be paired with exactly one `exit_scope`;
    /// see [`TemporaryAllocator`] for the aliasing invariant.
    pub(crate) fn enter_scope(&mut self) {
        self.scope_starts.push(self.variables.len());
    }

    /// Only code path that frees variable slots — keeping it as the
    /// single point of free-and-remove ensures the allocator never
    /// hands out a slot still bound to a live variable.
    pub(crate) fn exit_scope(&mut self) {
        let scope_start = self.scope_starts.pop().unwrap_or(0);
        for variable_index in scope_start..self.variables.len() {
            self.temporaries.free(self.variables[variable_index].offset);
        }
        self.variables.truncate(scope_start);
    }

    pub(crate) fn find_variable(&self, name_index: u32) -> Option<&VariableBinding> {
        self.variables
            .iter()
            .rev()
            .find(|binding| self.string_index_equals(binding.name_index, name_index))
    }

    pub(crate) fn find_variable_in_current_scope(
        &self,
        name_index: u32,
    ) -> Option<&VariableBinding> {
        let scope_start = self.scope_starts.last().copied().unwrap_or(0);
        self.variables[scope_start..]
            .iter()
            .find(|binding| self.string_index_equals(binding.name_index, name_index))
    }

    pub(crate) fn is_procedure_declaration(&self, constant: &FlatAssign) -> bool {
        self.extract_procedure_declaration(constant.expression)
            .is_some()
    }

    fn current_return_count(&self, offset: u32) -> Result<usize> {
        let index = self
            .current_procedure_index
            .ok_or(CompileError::return_outside_procedure(offset))?;
        Ok(self.procedures[index].return_count)
    }

    pub(crate) fn bootstrap_form_name(&self, name_index: u32) -> Option<BootstrapForm> {
        classify_bootstrap_name(self.get_string(name_index))
    }

    pub(crate) fn is_return_call(&self, call: &FlatCall) -> bool {
        classify_bootstrap_call_expression(self.root, call) == Some(BootstrapForm::Return)
    }

    pub(crate) fn find_procedure_index(&self, identifier_index: u32) -> Option<usize> {
        self.procedures
            .iter()
            .position(|procedure| self.string_index_equals(procedure.name_index, identifier_index))
    }

    pub(crate) fn try_resolve_immutable(&self, index: &FlatIndex) -> Option<u64> {
        match index {
            FlatIndex::Number(string_index) => self.parse_number(*string_index),
            FlatIndex::Identifier(string_index) => {
                if self.find_variable(*string_index).is_some() {
                    None
                } else {
                    self.parse_number(*string_index)
                }
            }
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_string(&self, string_index: u32) -> &str {
        self.root.get_string(string_index)
    }

    #[inline]
    pub(crate) fn identifier_text(&self, name_index: u32) -> &str {
        self.get_string(name_index)
    }

    #[inline]
    pub(crate) fn string_index_equals(&self, a: u32, b: u32) -> bool {
        if a == b {
            return true;
        }
        if a == u32::MAX || b == u32::MAX {
            return false;
        }
        // Identical identifiers typically map to the exact same `string_index` via
        // AST deduplication. However, programmatic or non-interned ASTs might not,
        // so we fall back to a full string comparison.
        self.get_string(a) == self.get_string(b)
    }

    #[inline]
    pub(crate) fn parse_number(&self, string_index: u32) -> Option<u64> {
        let text = self.get_string(string_index);
        Self::parse_unsigned_integer(text)
    }

    fn parse_unsigned_integer(text: &str) -> Option<u64> {
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return None;
        }

        let (start, radix): (usize, u64) = if bytes.len() >= 2 && bytes[0] == b'0' {
            match bytes[1] {
                b'b' => (2, 2),
                b'o' => (2, 8),
                b'd' => (2, 10),
                b'x' => (2, 16),
                _ => (0, 10),
            }
        } else {
            (0, 10)
        };

        if start >= bytes.len() {
            return None;
        }

        let mut value: u64 = 0;
        let mut has_digits = false;

        for &byte in &bytes[start..] {
            if byte == b'_' {
                continue;
            }
            let digit = match byte {
                b'0'..=b'9' => (byte - b'0') as u64,
                b'a'..=b'f' => (byte - b'a') as u64 + 10,
                b'A'..=b'F' => (byte - b'A') as u64 + 10,
                _ => return None,
            };
            if digit >= radix {
                return None;
            }
            value = value.checked_mul(radix)?;
            value = value.checked_add(digit)?;
            has_digits = true;
        }

        if !has_digits {
            return None;
        }

        Some(value)
    }
}
