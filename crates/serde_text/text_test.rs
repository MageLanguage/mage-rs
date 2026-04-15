use std::fmt::{self, Write};

use crate::{
    Format, Section, Serialize, Serializer, to_string, to_string_pretty, to_string_with_format,
};

struct Example;

impl Serialize for Example {
    fn serialize(&self, serializer: &mut Serializer) -> fmt::Result {
        match serializer.format() {
            Format::Pretty => serializer.write_str("pretty"),
            Format::Simple => serializer.write_str("simple"),
        }
    }
}

#[test]
fn format_names_are_stable() {
    assert_eq!(Format::Pretty.name(), "pretty");
    assert_eq!(Format::Simple.name(), "simple");
}

#[test]
fn to_string_with_format_uses_single_pipeline() {
    assert_eq!(to_string(&Example), "simple");
    assert_eq!(to_string_pretty(&Example), "pretty");
    assert_eq!(to_string_with_format(&Example, Format::Simple), "simple");
    assert_eq!(to_string_with_format(&Example, Format::Pretty), "pretty");
}

#[test]
fn prefix_name_uses_same_separator_as_json_output() {
    let output = Section::new("not_found")
        .prefix_name("load")
        .inline("message", "missing")
        .display(Format::Simple)
        .to_string();

    assert_eq!(output, "load+not_found:\nmessage: missing");
}
