use std::fmt;

pub use serde_text_derive::ToSection;

#[cfg(test)]
mod text_test;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Pretty,
    Simple,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pretty => "pretty",
            Self::Simple => "simple",
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

pub trait Serialize {
    fn serialize(&self, serializer: &mut Serializer) -> fmt::Result;
}

pub trait ToSection {
    fn to_section(&self) -> Section;
}

pub struct Serializer<'a> {
    writer: &'a mut dyn fmt::Write,
    format: Format,
}

impl<'a> Serializer<'a> {
    pub fn new(writer: &'a mut dyn fmt::Write, format: Format) -> Self {
        Self { writer, format }
    }

    pub fn format(&self) -> Format {
        self.format
    }
}

impl fmt::Write for Serializer<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.writer.write_str(text)
    }
}

pub fn to_string<T: Serialize>(value: &T) -> String {
    to_string_with_format(value, Format::Simple)
}

pub fn to_string_pretty<T: Serialize>(value: &T) -> String {
    to_string_with_format(value, Format::Pretty)
}

pub fn to_string_with_format<T: Serialize>(value: &T, format: Format) -> String {
    try_to_string_with_format(value, format)
        .unwrap_or_else(|_| String::from("(serialization failed)"))
}

pub fn try_to_string_with_format<T: Serialize>(
    value: &T,
    format: Format,
) -> Result<String, fmt::Error> {
    let mut output = String::new();
    let mut serializer = Serializer::new(&mut output, format);
    value.serialize(&mut serializer)?;
    Ok(output)
}

const FIELD_INDENT: &str = "  ";
const BLOCK_INDENT: &str = "    ";

enum SectionEntry {
    Inline { key: String, value: String },
    Block { key: String, value: String },
}

/// Structured output builder that produces a named section in both
/// pretty (indented tree) and simple (flat key-value) formats.
///
/// Pretty:
/// ```text
/// compile:
///   bytecode:
///     return 0d1 + 0d1 - 0d2;
/// ```
///
/// Simple:
/// ```text
/// compile:
/// bytecode: return 0d1 + 0d1 - 0d2;
/// ```
pub struct Section {
    section_name: String,
    entries: Vec<SectionEntry>,
}

impl Section {
    pub fn new(section_name: impl Into<String>) -> Self {
        Self {
            section_name: section_name.into(),
            entries: Vec::new(),
        }
    }

    /// Prepends a prefix to the section name separated by `+`.
    pub fn prefix_name(mut self, prefix: &str) -> Self {
        self.section_name = format!("{}+{}", prefix, self.section_name);
        self
    }

    /// Adds a field whose value is rendered on the same line as the key.
    pub fn inline(mut self, key: impl Into<String>, value: impl fmt::Display) -> Self {
        self.entries.push(SectionEntry::Inline {
            key: key.into(),
            value: value.to_string(),
        });
        self
    }

    /// Adds a field whose value is rendered on subsequent indented lines.
    pub fn block(mut self, key: impl Into<String>, value: impl fmt::Display) -> Self {
        self.entries.push(SectionEntry::Block {
            key: key.into(),
            value: value.to_string(),
        });
        self
    }

    pub fn display(&self, format: Format) -> SectionDisplay<'_> {
        SectionDisplay {
            section: self,
            format,
        }
    }

    fn write(&self, writer: &mut dyn fmt::Write, format: Format) -> fmt::Result {
        let (field_indent, block_indent) = match format {
            Format::Pretty => (FIELD_INDENT, BLOCK_INDENT),
            Format::Simple => ("", ""),
        };

        write!(writer, "{}:", self.section_name)?;

        for entry in &self.entries {
            match entry {
                SectionEntry::Inline { key, value } => {
                    write!(writer, "\n{}{}: {}", field_indent, key, value)?;
                }
                SectionEntry::Block { key, value } => {
                    write!(writer, "\n{}{}:", field_indent, key)?;
                    for line in value.lines() {
                        write!(writer, "\n{}{}", block_indent, line)?;
                    }
                }
            }
        }

        Ok(())
    }
}

pub struct SectionDisplay<'a> {
    section: &'a Section,
    format: Format,
}

impl fmt::Display for SectionDisplay<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.section.write(formatter, self.format)
    }
}
