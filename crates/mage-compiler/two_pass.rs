#[derive(Debug, Clone, Copy)]
pub(crate) struct Fixup {
    pub(crate) patch_offset: usize,
    pub(crate) label: FixupLabel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixupLabel {
    WhileTarget { id: u32 },
    WhileEnd { id: u32 },
    IfEnd { id: u32 },
    ProcedureEntry { procedure_index: usize },
    ProcedureFrameSize { procedure_index: usize },
}

#[derive(Debug, Default)]
pub(crate) struct TwoPassState {
    label_offsets: Vec<Option<usize>>,
    pub(crate) fixups: Vec<Fixup>,
}

impl TwoPassState {
    pub(crate) fn fresh_label_id(&mut self) -> u32 {
        let label_id = self.label_offsets.len() as u32;
        self.label_offsets.push(None);
        label_id
    }

    pub(crate) fn define_label(&mut self, label_id: u32, offset: usize) {
        self.label_offsets[label_id as usize] = Some(offset);
    }

    pub(crate) fn resolve_label(&self, label_id: u32) -> Option<usize> {
        self.label_offsets.get(label_id as usize).copied().flatten()
    }
}
