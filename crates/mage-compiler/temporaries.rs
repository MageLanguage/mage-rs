use crate::layout::{ExpressionResult, VALUE_SIZE};

/// Manages stack frame slot allocation for both expression temporaries and
/// user-declared variables/constants.
///
/// # Aliasing Safety Invariant
///
/// Variable and constant bindings are allocated through this same allocator
/// (via `allocate()`), so they occupy real slots in the `positions` bitmap.
/// The critical invariant is:
///
/// **A variable's slot is never freed while the variable is still live
/// (i.e. reachable by name lookup in `Compiler::variables`).**
///
/// This is maintained by the `Compiler::enter_scope` / `exit_scope` pairing:
/// - `enter_scope` records the current `variables.len()` as a checkpoint.
/// - `exit_scope` frees (via this allocator) the slots of all variables
///   declared since the checkpoint, then truncates the variables list.
///
/// Because `exit_scope` is the *only* path that frees variable slots, and it
/// always removes the corresponding bindings from `variables` in the same
/// call, the allocator will never hand out a slot that is still bound to a
/// live variable. Any refactor that introduces a new path for freeing
/// variable slots or for removing variable bindings must preserve this
/// coupling, or expression temporaries may silently alias live variables.
#[derive(Default)]
pub(crate) struct TemporaryAllocator {
    positions: mage_memory::BitVec,
    free_stack: Vec<u64>,
    high_water_mark: u64,
}

impl TemporaryAllocator {
    pub(crate) fn reset(&mut self) {
        self.positions.clear();
        self.free_stack.clear();
        self.high_water_mark = 0;
    }

    pub(crate) fn reserve_below(&mut self, byte_offset: u64) {
        let min_index = (byte_offset / VALUE_SIZE) as usize;
        if self.positions.len() < min_index {
            self.positions.resize(min_index, true);
        } else {
            for position in 0..min_index {
                self.positions.set(position, true);
            }
        }
    }

    pub(crate) fn allocate(&mut self) -> u64 {
        let offset = if let Some(offset) = self.free_stack.pop() {
            let index = (offset / VALUE_SIZE) as usize;
            self.positions.set(index, true);
            offset
        } else if let Some(index) = self.positions.first_zero() {
            self.positions.set(index, true);
            index as u64 * VALUE_SIZE
        } else {
            let index = self.positions.len();
            self.positions.push(true);
            index as u64 * VALUE_SIZE
        };

        let end = offset + VALUE_SIZE;
        if end > self.high_water_mark {
            self.high_water_mark = end;
        }

        offset
    }

    pub(crate) fn high_water_mark(&self) -> u64 {
        self.high_water_mark
    }

    pub(crate) fn free(&mut self, byte_offset: u64) {
        let index = (byte_offset / VALUE_SIZE) as usize;
        if index < self.positions.len() {
            debug_assert!(
                self.positions.get(index) == Some(true),
                "double-free of temporary at byte offset {}",
                byte_offset
            );
            self.positions.set(index, false);
            self.free_stack.push(byte_offset);
        }
    }

    pub(crate) fn free_if_temporary(&mut self, result: ExpressionResult) {
        if result.is_temporary {
            self.free(result.offset);
        }
    }
}
