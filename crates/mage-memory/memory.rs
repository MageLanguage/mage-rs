#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BitVec {
    bits: Vec<u64>,
    len: usize,
}

impl BitVec {
    pub fn with_capacity(capacity: usize) -> Self {
        let u64_capacity = capacity.div_ceil(64);
        Self {
            bits: Vec::with_capacity(u64_capacity),
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, value: bool) {
        let u64_index = self.len / 64;
        let bit_index = self.len % 64;

        if u64_index == self.bits.len() {
            self.bits.push(0);
        }

        if value {
            self.bits[u64_index] |= 1 << bit_index;
        } else {
            self.bits[u64_index] &= !(1 << bit_index);
        }

        self.len += 1;
    }

    pub fn resize(&mut self, new_len: usize, value: bool) {
        if new_len <= self.len {
            self.len = new_len;
            let blocks = self.len.div_ceil(64);
            self.bits.truncate(blocks);

            if let Some(remainder) = self.len.checked_rem(64)
                && remainder != 0
                && !self.bits.is_empty()
            {
                let mask = (1 << remainder) - 1;
                if let Some(last) = self.bits.last_mut() {
                    *last &= mask;
                }
            }
            return;
        }

        let old_len = self.len;
        let new_blocks = new_len.div_ceil(64);
        let fill_block: u64 = if value { u64::MAX } else { 0 };

        self.bits.resize(new_blocks, fill_block);
        self.len = new_len;

        if value {
            // The block at the old boundary was kept as-is by resize;
            // set the bits above old_len to 1.
            let old_bit = old_len % 64;
            if old_bit != 0 {
                let block_index = old_len / 64;
                self.bits[block_index] |= !((1u64 << old_bit) - 1);
            }

            // Clear excess bits in the last block so the invariant
            // (bits beyond self.len are zero) holds.
            let new_bit = new_len % 64;
            if new_bit != 0 {
                let block_index = new_len / 64;
                self.bits[block_index] &= (1u64 << new_bit) - 1;
            }
        }
    }

    pub fn clear(&mut self) {
        self.bits.clear();
        self.len = 0;
    }

    pub fn reserve(&mut self, additional: usize) {
        let new_len = self.len + additional;
        let blocks = new_len.div_ceil(64);
        if blocks > self.bits.len() {
            self.bits.reserve(blocks - self.bits.len());
        }
    }

    pub fn get(&self, index: usize) -> Option<bool> {
        if index >= self.len {
            return None;
        }
        let u64_index = index / 64;
        let bit_index = index % 64;
        Some((self.bits[u64_index] & (1 << bit_index)) != 0)
    }

    pub fn set(&mut self, index: usize, value: bool) {
        assert!(index < self.len, "Index out of bounds");
        let u64_index = index / 64;
        let bit_index = index % 64;
        if value {
            self.bits[u64_index] |= 1 << bit_index;
        } else {
            self.bits[u64_index] &= !(1 << bit_index);
        }
    }

    pub fn first_zero(&self) -> Option<usize> {
        for (block_index, &block) in self.bits.iter().enumerate() {
            if block != u64::MAX {
                let bit_index = (!block).trailing_zeros() as usize;
                let index = block_index * 64 + bit_index;
                if index < self.len {
                    return Some(index);
                }
            }
        }
        None
    }
}
