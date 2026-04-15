use mage_ast::FlatBinaryOperationKind;
use mage_contract::SuperinstructionKind;

pub(crate) const VALUE_SIZE: u64 = 8;
pub(crate) const FIELD_1_OFFSET: usize = 8;
pub(crate) const FIELD_2_OFFSET: usize = 16;

pub(crate) const COMPARISON_INSTRUCTION_SIZE: usize = 32;

#[derive(Clone, Copy, Default)]
pub(crate) struct FrameLayout {
    pub(crate) temporaries_start: u64,
    pub(crate) total: u64,
}

impl FrameLayout {
    pub(crate) fn new(call_area: u64, allocator_capacity: u64) -> Self {
        let temporaries_start = call_area;
        let total = temporaries_start + allocator_capacity;
        Self {
            temporaries_start,
            total,
        }
    }

    #[inline]
    pub(crate) fn argument_offset(&self, callee_return_count: usize, index: usize) -> u64 {
        (callee_return_count as u64 + 1 + index as u64) * VALUE_SIZE
    }

    #[inline]
    pub(crate) fn return_site_id_offset(&self, callee_return_count: usize) -> u64 {
        callee_return_count as u64 * VALUE_SIZE
    }

    #[inline]
    pub(crate) fn return_value_offset(&self, index: usize) -> u64 {
        self.total + index as u64 * VALUE_SIZE
    }

    #[inline]
    pub(crate) fn parameter_offset(&self, own_return_count: usize, index: usize) -> u64 {
        self.total + (own_return_count as u64 + 1 + index as u64) * VALUE_SIZE
    }

    #[inline]
    pub(crate) fn caller_return_site_id(&self, own_return_count: usize) -> u64 {
        self.total + own_return_count as u64 * VALUE_SIZE
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VariableBinding {
    pub(crate) name_index: u32,
    pub(crate) offset: u64,
    pub(crate) is_constant: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ExpressionAnalysis {
    pub(crate) has_procedure_call: bool,
    pub(crate) max_call_area_slots: u16,
    pub(crate) temp_estimate: u16,
}

impl ExpressionAnalysis {
    pub(crate) fn merge_binary(left: Self, right: Self, adds_temp: bool) -> Self {
        Self {
            has_procedure_call: left.has_procedure_call || right.has_procedure_call,
            max_call_area_slots: left.max_call_area_slots.max(right.max_call_area_slots),
            temp_estimate: if adds_temp {
                1 + left.temp_estimate.max(right.temp_estimate)
            } else {
                left.temp_estimate.max(right.temp_estimate)
            },
        }
    }

    pub(crate) fn merge_max(self, other: Self) -> Self {
        Self {
            has_procedure_call: self.has_procedure_call || other.has_procedure_call,
            max_call_area_slots: self.max_call_area_slots.max(other.max_call_area_slots),
            temp_estimate: self.temp_estimate.max(other.temp_estimate),
        }
    }

    pub(crate) fn with_procedure_call(mut self, call_area_slots: u16) -> Self {
        self.has_procedure_call = true;
        self.max_call_area_slots = self.max_call_area_slots.max(call_area_slots);
        self.temp_estimate = self.temp_estimate.max(1);
        self
    }
}

pub(crate) struct Procedure {
    pub(crate) name_index: u32,
    pub(crate) source_index: usize,
    pub(crate) parameters_start: u32,
    pub(crate) parameters_end: u32,
    pub(crate) return_count: usize,
    pub(crate) bytecode_offset: Option<usize>,
    pub(crate) layout: FrameLayout,
}

#[derive(Clone, Copy)]
pub(crate) struct ExpressionResult {
    pub(crate) offset: u64,
    pub(crate) is_temporary: bool,
    /// True when the result lives in the call area (e.g. return value at
    /// offset 0). Volatile results are clobbered by any subsequent call
    /// and must be copied before evaluating expressions that might call.
    pub(crate) is_volatile: bool,
}

impl ExpressionResult {
    pub(crate) fn temporary(offset: u64) -> Self {
        Self {
            offset,
            is_temporary: true,
            is_volatile: false,
        }
    }

    pub(crate) fn variable(offset: u64) -> Self {
        Self {
            offset,
            is_temporary: false,
            is_volatile: false,
        }
    }

    pub(crate) fn volatile(offset: u64) -> Self {
        Self {
            offset,
            is_temporary: false,
            is_volatile: true,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct LastComparison {
    pub(crate) opcode_offset: usize,
    pub(crate) target: u64,
    pub(crate) jump_if_not_kind: SuperinstructionKind,
    pub(crate) jump_if_kind: SuperinstructionKind,
}

pub(crate) struct BlockPatch {
    pub(crate) name_index: u32,
    pub(crate) if_end_label_id: u32,
}

pub(crate) struct WhilePatch {
    pub(crate) name_index: Option<u32>,
    pub(crate) while_condition_label_id: u32,
    pub(crate) while_end_label_id: u32,
}

pub(crate) fn comparison_jump_if_not_kind(
    operation: FlatBinaryOperationKind,
    is_immutable: bool,
) -> Option<SuperinstructionKind> {
    use FlatBinaryOperationKind::*;
    match (operation, is_immutable) {
        (LessThan, false) => Some(SuperinstructionKind::LessThanTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable),
        (LessThan, true) => Some(SuperinstructionKind::LessThanTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable),
        (GreaterThan, false) => Some(SuperinstructionKind::GreaterThanTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable),
        (GreaterThan, true) => Some(SuperinstructionKind::GreaterThanTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable),
        (LessThanOrEqual, false) => Some(SuperinstructionKind::LessThanOrEqualTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable),
        (LessThanOrEqual, true) => Some(SuperinstructionKind::LessThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable),
        (GreaterThanOrEqual, false) => Some(SuperinstructionKind::GreaterThanOrEqualTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable),
        (GreaterThanOrEqual, true) => Some(SuperinstructionKind::GreaterThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable),
        (Equal, false) => Some(SuperinstructionKind::EqualTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable),
        (Equal, true) => Some(SuperinstructionKind::EqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable),
        (NotEqual, false) => Some(SuperinstructionKind::NotEqualTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable),
        (NotEqual, true) => Some(SuperinstructionKind::NotEqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable),
        _ => None,
    }
}

pub(crate) fn comparison_jump_if_kind(
    operation: FlatBinaryOperationKind,
    is_immutable: bool,
) -> Option<SuperinstructionKind> {
    use FlatBinaryOperationKind::*;
    match (operation, is_immutable) {
        (LessThan, false) => Some(SuperinstructionKind::LessThanTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable),
        (LessThan, true) => Some(SuperinstructionKind::LessThanTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable),
        (GreaterThan, false) => Some(SuperinstructionKind::GreaterThanTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable),
        (GreaterThan, true) => Some(SuperinstructionKind::GreaterThanTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable),
        (LessThanOrEqual, false) => Some(SuperinstructionKind::LessThanOrEqualTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable),
        (LessThanOrEqual, true) => Some(SuperinstructionKind::LessThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable),
        (GreaterThanOrEqual, false) => Some(SuperinstructionKind::GreaterThanOrEqualTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable),
        (GreaterThanOrEqual, true) => Some(SuperinstructionKind::GreaterThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable),
        (Equal, false) => Some(SuperinstructionKind::EqualTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable),
        (Equal, true) => Some(SuperinstructionKind::EqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable),
        (NotEqual, false) => Some(SuperinstructionKind::NotEqualTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable),
        (NotEqual, true) => Some(SuperinstructionKind::NotEqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable),
        _ => None,
    }
}
