/// Loaded source text from a `.hex` file.
#[derive(Debug, Clone, Default)]
pub struct Code {
    source: String,
}

impl Code {
    pub fn new(source: String) -> Self {
        Self { source }
    }

    #[inline]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.source.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.source.is_empty()
    }
}
