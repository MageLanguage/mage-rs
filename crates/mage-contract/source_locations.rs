/// Uses a flat layout: source ranges index into a single contiguous
/// `offsets` array rather than per-source `Vec<u32>` allocations.
#[derive(Default, Debug, Clone)]
pub struct SourceLocations {
    /// (start, end) index pairs into `offsets`, one per source block.
    source_ranges: Vec<(u32, u32)>,
    /// Flat array of all statement byte offsets across all source blocks.
    offsets: Vec<u32>,
    /// Byte offset of each expression, indexed by expression index.
    expressions: Vec<u32>,
}

impl SourceLocations {
    #[inline]
    pub fn get(&self, source_index: usize) -> &[u32] {
        match self.source_ranges.get(source_index) {
            Some(&(start, end)) => &self.offsets[start as usize..end as usize],
            None => &[],
        }
    }

    #[inline]
    pub fn statement_count(&self, source_index: usize) -> usize {
        self.get(source_index).len()
    }

    #[inline]
    pub fn statement_offset(&self, source_index: usize, statement_index: usize) -> Option<u32> {
        self.get(source_index).get(statement_index).copied()
    }

    pub fn statement_range(
        &self,
        source_index: usize,
        statement_index: usize,
        source_end: usize,
    ) -> Option<(usize, usize)> {
        let locations = self.get(source_index);
        let start = *locations.get(statement_index)? as usize;
        let end = locations
            .get(statement_index + 1)
            .copied()
            .map(|offset| offset as usize)
            .unwrap_or(source_end);
        Some((start, end))
    }

    pub fn statement_containing_offset(
        &self,
        source_index: usize,
        offset: usize,
        source_end: usize,
    ) -> Option<usize> {
        let locations = self.get(source_index);
        for (index, &statement_offset) in locations.iter().enumerate() {
            let next_offset = locations
                .get(index + 1)
                .copied()
                .map(|o| o as usize)
                .unwrap_or(source_end);

            if offset >= statement_offset as usize && offset < next_offset {
                return Some(index);
            }
        }
        None
    }

    #[inline]
    pub fn get_expression_offset(&self, expression_index: u32) -> u32 {
        self.expressions[expression_index as usize]
    }

    #[inline]
    pub fn expression_count(&self) -> usize {
        self.expressions.len()
    }

    #[inline]
    pub fn has_expressions(&self) -> bool {
        !self.expressions.is_empty()
    }

    /// Reserves source index 0 (the root source) before parsing;
    /// its offsets are filled in later via [`set_source`].
    #[inline]
    pub fn push_source_empty(&mut self) {
        let pos = self.offsets.len() as u32;
        self.source_ranges.push((pos, pos));
    }

    #[inline]
    pub fn push_source(&mut self, locations: &[u32]) -> u32 {
        let start = self.offsets.len() as u32;
        self.offsets.extend_from_slice(locations);
        let end = self.offsets.len() as u32;
        let index = self.source_ranges.len() as u32;
        self.source_ranges.push((start, end));
        index
    }

    /// Previously stored offsets for this source become unreachable
    /// but remain in the array — acceptable because `set_source` is
    /// called at most once per source (the root source after parsing).
    #[inline]
    pub fn set_source(&mut self, source_index: usize, locations: &[u32]) {
        let start = self.offsets.len() as u32;
        self.offsets.extend_from_slice(locations);
        let end = self.offsets.len() as u32;
        self.source_ranges[source_index] = (start, end);
    }

    #[inline]
    pub fn push_expression(&mut self, offset: u32) {
        self.expressions.push(offset);
    }

    #[inline]
    pub fn save(&self) -> SourceLocationsState {
        SourceLocationsState {
            source_ranges_len: self.source_ranges.len(),
            offsets_len: self.offsets.len(),
            expressions_len: self.expressions.len(),
        }
    }

    #[inline]
    pub fn restore(&mut self, state: &SourceLocationsState) {
        self.source_ranges.truncate(state.source_ranges_len);
        self.offsets.truncate(state.offsets_len);
        self.expressions.truncate(state.expressions_len);
    }
}

#[derive(Clone, Copy)]
pub struct SourceLocationsState {
    source_ranges_len: usize,
    offsets_len: usize,
    expressions_len: usize,
}
