use crate::Compiler;
use crate::layout::{COMPARISON_INSTRUCTION_SIZE, FIELD_1_OFFSET, FIELD_2_OFFSET};
use crate::two_pass::{Fixup, FixupLabel};

use mage_contract::*;

type Result<T> = std::result::Result<T, CompileError>;

impl<'a> Compiler<'a> {
    #[inline]
    pub(crate) fn bytecode_offset(&self) -> usize {
        self.writer.bytecode.len()
    }

    pub(crate) fn sync_frame_layout(&mut self) {
        let high_water = self.temporaries.high_water_mark();
        if high_water > self.current_layout.total {
            self.current_layout.total = high_water;
            if let Some(procedure_index) = self.current_procedure_index {
                self.procedures[procedure_index].layout = self.current_layout;
            }
        }
    }

    pub(crate) fn emit(&mut self, instruction: &Instruction) -> usize {
        self.last_comparison = None;
        self.last_result_target = None;
        let offset = self.bytecode_offset();
        self.patch_map.push_opcode(offset as u32);
        self.writer.write(instruction);
        offset
    }

    pub(crate) fn emit_jump(&mut self, to: u64) -> usize {
        let offset = self.emit(&Instruction::JumpToImmutable(JumpToImmutable { to }));
        self.patch_map
            .push_relocation((offset + FIELD_1_OFFSET) as u32);
        offset
    }

    fn emit_conditional_jump(
        &mut self,
        condition: u64,
        instruction: Instruction,
        superinstruction_kind: Option<SuperinstructionKind>,
    ) -> usize {
        if let Some(superinstruction_kind) = superinstruction_kind {
            self.patch_comparison_superinstruction(condition, superinstruction_kind);
        }
        let offset = self.emit(&instruction);
        self.patch_map
            .push_relocation((offset + FIELD_2_OFFSET) as u32);
        offset
    }

    fn patch_comparison_superinstruction(
        &mut self,
        condition: u64,
        superinstruction_kind: SuperinstructionKind,
    ) {
        if let Some(comparison) = self.last_comparison.take()
            && comparison.target == condition
            && comparison.opcode_offset + COMPARISON_INSTRUCTION_SIZE == self.bytecode_offset()
        {
            self.patch_map
                .push_superinstruction(comparison.opcode_offset as u32, superinstruction_kind);
        }
    }

    pub(crate) fn emit_jump_if_not(&mut self, condition: u64, to: u64) -> usize {
        let superinstruction_kind = self
            .last_comparison
            .as_ref()
            .map(|comparison| comparison.jump_if_not_kind);
        self.emit_conditional_jump(
            condition,
            Instruction::JumpIfNotConditionOffsetToImmutable(JumpIfNotConditionOffsetToImmutable {
                condition,
                to,
            }),
            superinstruction_kind,
        )
    }

    pub(crate) fn emit_jump_if(&mut self, condition: u64, to: u64) -> usize {
        let superinstruction_kind = self
            .last_comparison
            .as_ref()
            .map(|comparison| comparison.jump_if_kind);
        self.emit_conditional_jump(
            condition,
            Instruction::JumpIfConditionOffsetToImmutable(JumpIfConditionOffsetToImmutable {
                condition,
                to,
            }),
            superinstruction_kind,
        )
    }

    pub(crate) fn emit_jump_to_label(&mut self, label: FixupLabel) {
        let offset = self.emit_jump(0);
        self.two_pass.fixups.push(Fixup {
            patch_offset: offset + FIELD_1_OFFSET,
            label,
        });
    }

    pub(crate) fn emit_jump_if_not_to_label(&mut self, condition: u64, label: FixupLabel) {
        let offset = self.emit_jump_if_not(condition, 0);
        self.two_pass.fixups.push(Fixup {
            patch_offset: offset + FIELD_2_OFFSET,
            label,
        });
    }

    pub(crate) fn emit_jump_if_to_label(&mut self, condition: u64, label: FixupLabel) {
        let offset = self.emit_jump_if(condition, 0);
        self.two_pass.fixups.push(Fixup {
            patch_offset: offset + FIELD_2_OFFSET,
            label,
        });
    }

    pub(crate) fn emit_take_jump_to_procedure(&mut self, procedure_index: usize) {
        let (take_offset, jump_offset) = self.emit_take_jump(0, 0);
        self.two_pass.fixups.push(Fixup {
            patch_offset: take_offset + FIELD_1_OFFSET,
            label: FixupLabel::ProcedureFrameSize { procedure_index },
        });
        // The relocation for the jump target is already pushed by
        // emit_take_jump → emit_jump → push_relocation.
        self.two_pass.fixups.push(Fixup {
            patch_offset: jump_offset + FIELD_1_OFFSET,
            label: FixupLabel::ProcedureEntry { procedure_index },
        });
    }

    pub(crate) fn emit_take_jump(&mut self, stack_size: u64, to: u64) -> (usize, usize) {
        let take_offset = self.emit(&Instruction::TakeStackSizeImmutable(
            TakeStackSizeImmutable { stack_size },
        ));
        let jump_offset = self.emit_jump(to);
        self.patch_map.push_superinstruction(
            take_offset as u32,
            SuperinstructionKind::TakeStackSizeImmutableJumpToImmutable,
        );
        (take_offset, jump_offset)
    }

    pub(crate) fn emit_free_jump_offset(
        &mut self,
        stack_size: u64,
        return_address_offset: u64,
    ) -> (usize, usize) {
        let free_offset = self.emit(&Instruction::FreeStackSizeImmutable(
            FreeStackSizeImmutable { stack_size },
        ));
        let jump_offset = self.emit(&Instruction::JumpToOffset(JumpToOffset {
            to: return_address_offset,
        }));
        self.patch_map.push_superinstruction(
            free_offset as u32,
            SuperinstructionKind::FreeStackSizeImmutableJumpToOffset,
        );
        (free_offset, jump_offset)
    }

    pub(crate) fn patch_u64(&mut self, offset: usize, value: u64) {
        let end = offset + 8;
        debug_assert!(
            end <= self.writer.bytecode.len(),
            "patch offset out of bounds: {offset}..{end} (len={})",
            self.writer.bytecode.len()
        );
        let bytes = value.to_le_bytes();
        self.writer.bytecode[offset..end].copy_from_slice(&bytes);
    }

    pub(crate) fn patch_two_pass_fixups(&mut self) -> Result<()> {
        for fixup_index in 0..self.two_pass.fixups.len() {
            let fixup = self.two_pass.fixups[fixup_index];
            let value = self.resolve_fixup_label(&fixup.label)?;
            self.patch_u64(fixup.patch_offset, value as u64);
        }
        self.two_pass.fixups.clear();
        Ok(())
    }

    fn resolve_fixup_label(&self, label: &FixupLabel) -> Result<usize> {
        match label {
            FixupLabel::WhileTarget { id } => self
                .two_pass
                .resolve_label(*id)
                .ok_or(CompileError::UnresolvedWhileTargetLabel),
            FixupLabel::WhileEnd { id } => self
                .two_pass
                .resolve_label(*id)
                .ok_or(CompileError::UnresolvedWhileEndLabel),
            FixupLabel::IfEnd { id } => self
                .two_pass
                .resolve_label(*id)
                .ok_or(CompileError::UnresolvedIfEndLabel),
            FixupLabel::ProcedureEntry { procedure_index } => self.procedures[*procedure_index]
                .bytecode_offset
                .ok_or(CompileError::MissingProcedureBytecodeOffset),
            FixupLabel::ProcedureFrameSize { procedure_index } => {
                Ok(self.procedures[*procedure_index].layout.total as usize)
            }
        }
    }
}
