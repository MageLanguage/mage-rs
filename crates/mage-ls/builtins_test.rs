use super::{builtin_documentation, canonical_builtin_name};

#[test]
fn returns_documentation_for_procedure() {
    let documentation = builtin_documentation("procedure").unwrap();
    assert!(documentation.contains("defines a procedure"));
    assert!(documentation.contains("name : (procedure"));
}

#[test]
fn returns_documentation_for_if() {
    let documentation = builtin_documentation("if").unwrap();
    assert!(documentation.contains("conditional execution"));
}

#[test]
fn returns_documentation_for_while() {
    let documentation = builtin_documentation("while").unwrap();
    assert!(documentation.contains("loop while condition is true"));
}

#[test]
fn returns_documentation_for_return() {
    let documentation = builtin_documentation("return").unwrap();
    assert!(documentation.contains("return a value"));
}

#[test]
fn returns_documentation_for_break() {
    let documentation = builtin_documentation("break").unwrap();
    assert!(documentation.contains("break out of"));
}

#[test]
fn returns_documentation_for_continue() {
    let documentation = builtin_documentation("continue").unwrap();
    assert!(documentation.contains("continue to the next iteration"));
}

#[test]
fn returns_documentation_for_void() {
    let documentation = builtin_documentation("void").unwrap();
    assert!(documentation.contains("represents nothing"));
}

#[test]
fn returns_documentation_for_comment() {
    let documentation = builtin_documentation("comment").unwrap();
    assert!(documentation.contains("comment"));
}

#[test]
fn returns_documentation_for_class() {
    let documentation = builtin_documentation("Class").unwrap();
    assert!(documentation.contains("class type"));
}

#[test]
fn class_builtin_name_is_supported() {
    assert!(builtin_documentation("Class").is_some());
}

#[test]
fn canonical_builtin_name_keeps_class_name() {
    assert_eq!(canonical_builtin_name("Class"), "Class");
}

#[test]
fn returns_documentation_for_interface() {
    let documentation = builtin_documentation("Interface").unwrap();
    assert!(documentation.contains("interface type"));
}

#[test]
fn canonical_builtin_name_maps_interface_name() {
    assert_eq!(canonical_builtin_name("Interface"), "interface");
}

#[test]
fn returns_documentation_for_enumeration() {
    let documentation = builtin_documentation("Enumeration").unwrap();
    assert!(documentation.contains("enumeration type"));
}

#[test]
fn canonical_builtin_name_maps_enumeration_name() {
    assert_eq!(canonical_builtin_name("Enumeration"), "enumeration");
}

#[test]
fn returns_documentation_for_implement() {
    let documentation = builtin_documentation("implement").unwrap();
    assert!(documentation.contains("implements an interface"));
}

#[test]
fn returns_documentation_for_type() {
    assert!(builtin_documentation("Type").is_some());
}

#[test]
fn canonical_builtin_name_maps_type_name() {
    assert_eq!(canonical_builtin_name("Type"), "type");
}

#[test]
fn returns_documentation_for_link() {
    let documentation = builtin_documentation("Link").unwrap();
    assert!(documentation.contains("referencing memory"));
}

#[test]
fn canonical_builtin_name_maps_link_name() {
    assert_eq!(canonical_builtin_name("Link"), "link");
}

#[test]
fn returns_documentation_for_vector() {
    let documentation = builtin_documentation("Vector").unwrap();
    assert!(documentation.contains("sequence"));
}

#[test]
fn canonical_builtin_name_maps_vector_name() {
    assert_eq!(canonical_builtin_name("Vector"), "vector");
}

#[test]
fn returns_documentation_for_number() {
    let documentation = builtin_documentation("Number").unwrap();
    assert!(documentation.contains("numeric literal"));
}

#[test]
fn canonical_builtin_name_maps_number_name() {
    assert_eq!(canonical_builtin_name("Number"), "number");
}

#[test]
fn returns_documentation_for_source() {
    let documentation = builtin_documentation("Source").unwrap();
    assert!(documentation.contains("source block"));
}

#[test]
fn canonical_builtin_name_maps_source_name() {
    assert_eq!(canonical_builtin_name("Source"), "source");
}

#[test]
fn returns_documentation_for_unsigned_integer_types() {
    for name in ["U8", "U16", "U32", "U64"] {
        let documentation = builtin_documentation(name)
            .unwrap_or_else(|| panic!("missing documentation for {}", name));
        assert!(documentation.contains("unsigned integer"));
    }
}

#[test]
fn returns_documentation_for_signed_integer_types() {
    for name in ["S8", "S16", "S32", "S64"] {
        let documentation = builtin_documentation(name)
            .unwrap_or_else(|| panic!("missing documentation for {}", name));
        assert!(documentation.contains("signed integer"));
    }
}

#[test]
fn returns_documentation_for_floating_point_types() {
    for name in ["F32", "F64"] {
        let documentation = builtin_documentation(name)
            .unwrap_or_else(|| panic!("missing documentation for {}", name));
        assert!(documentation.contains("floating-point"));
    }
}

#[test]
fn returns_documentation_for_string() {
    let documentation = builtin_documentation("String").unwrap();
    assert!(documentation.contains("string type"));
}

#[test]
fn returns_documentation_for_void_type() {
    let documentation = builtin_documentation("Void").unwrap();
    assert!(documentation.contains("nothing"));
}

#[test]
fn returns_none_for_unknown_name() {
    assert_eq!(builtin_documentation("unknown"), None);
}

#[test]
fn canonical_builtin_name_returns_unknown_name_as_is() {
    assert_eq!(canonical_builtin_name("unknown"), "unknown");
}

#[test]
fn returns_none_for_empty_name() {
    assert_eq!(builtin_documentation(""), None);
}

#[test]
fn canonical_builtin_name_returns_empty_name_as_is() {
    assert_eq!(canonical_builtin_name(""), "");
}
