#[cfg(test)]
#[path = "builtins_test.rs"]
mod builtins_test;

pub fn builtin_documentation(name: &str) -> Option<&'static str> {
    match canonical_builtin_name(name) {
        "procedure" => Some(
            "Built-in: defines a procedure.\n\n\
             ```mage\n\
             name : (procedure {params}, ReturnType) {\n\
                 body;\n\
             };\n\
             ```",
        ),
        "if" => Some(
            "Built-in: conditional execution.\n\n\
             ```mage\n\
             if condition, {\n\
                 body;\n\
             };\n\
             ```",
        ),
        "while" => Some(
            "Built-in: loop while condition is true.\n\n\
             ```mage\n\
             while condition, {\n\
                 body;\n\
             };\n\
             ```",
        ),
        "return" => Some(
            "Built-in: return a value from the current procedure.\n\n\
             ```mage\n\
             return expression;\n\
             ```",
        ),
        "break" => Some(
            "Built-in: break out of a named block.\n\n\
             ```mage\n\
             break blockName;\n\
             ```",
        ),
        "continue" => Some(
            "Built-in: continue to the next iteration of a named loop.\n\n\
             ```mage\n\
             continue loopName;\n\
             ```",
        ),
        "void" => Some(
            "Built-in: represents nothing. Used as argument for zero-parameter procedure calls.\n\n\
             ```mage\n\
             result = myProcedure void;\n\
             ```",
        ),
        "comment" => Some(
            "Built-in: adds a comment.\n\n\
             ```mage\n\
             comment \"This is a comment\";\n\
             ```",
        ),
        "Class" => Some(
            "Built-in: defines a class type.\n\n\
             ```mage\n\
             MyType : Class {\n\
                 field : Type;\n\
             };\n\
             ```",
        ),
        "interface" => Some(
            "Built-in: defines an interface type.\n\n\
             ```mage\n\
             Counter : Interface {\n\
                 add : method {counter : Counter}, Void;\n\
                 get : method {counter : Counter}, U64;\n\
             };\n\
             ```",
        ),
        "enumeration" => Some(
            "Built-in: defines an enumeration type that constrains available values.\n\n\
             ```mage\n\
             Power : Enumeration U64, {\n\
                 None  : 0;\n\
                 Water : 1;\n\
             };\n\
             ```",
        ),
        "implement" => Some(
            "Built-in: implements an interface for a class.\n\n\
             ```mage\n\
             newCounter : implement MyClass, Counter, {\n\
                 add : (procedure {self : ^MyClass}, Void) { ... };\n\
             };\n\
             ```",
        ),
        "type" => Some("Built-in: foundation of the Mage type system."),
        "link" => Some(
            "Built-in: special type for referencing memory. Internally a type pointer and data pointer.",
        ),
        "vector" => Some("Built-in: sequence of generic elements."),
        "number" => Some("Built-in: abstract numeric literal type (alias for String)."),
        "source" => {
            Some("Built-in: source block reference for lazy compilation (alias for String).")
        }
        "u8" | "u16" | "u32" | "u64" => Some("Primitive unsigned integer type."),
        "s8" | "s16" | "s32" | "s64" => Some("Primitive signed integer type."),
        "f32" | "f64" => Some("Primitive floating-point type."),
        "string" => Some("Built-in string type (alias for Vector of U8)."),
        "void_type" => Some("Built-in type representing nothing."),
        _ => None,
    }
}

fn canonical_builtin_name(name: &str) -> &str {
    match name {
        "Interface" => "interface",
        "Enumeration" => "enumeration",
        "Type" => "type",
        "Link" => "link",
        "Vector" => "vector",
        "Number" => "number",
        "Source" => "source",
        "U8" => "u8",
        "U16" => "u16",
        "U32" => "u32",
        "U64" => "u64",
        "S8" => "s8",
        "S16" => "s16",
        "S32" => "s32",
        "S64" => "s64",
        "F32" => "f32",
        "F64" => "f64",
        "String" => "string",
        "Void" => "void_type",
        _ => name,
    }
}
