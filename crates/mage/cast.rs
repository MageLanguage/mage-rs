mod arguments;

use arguments::{Output, ParsedArguments, parse};
use mage_ast::encode::Encoder;
use mage_compiler::Compiler;
use mage_contract::{Bytecode, Code, FlatRoot, LoadError, Reader, Stage};
use mage_native::VirtualMachine;
use serde::Serialize;
use serde_text::{Format, Section};
use std::{path::Path, process::ExitCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputKind {
    Hex,
    Bytecode,
}

impl InputKind {
    fn from_path(path: &str) -> Result<Self, mage::Error> {
        match Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("hex") => Ok(Self::Hex),
            Some("bytecode") => Ok(Self::Bytecode),
            _ => Err(LoadError::UnsupportedExtension { path: path.into() }.into()),
        }
    }

    fn supports_stage(self, stage: Stage) -> bool {
        match self {
            Self::Hex => true,
            Self::Bytecode => matches!(stage, Stage::Load | Stage::Execute),
        }
    }
}

const JSON_SERIALIZATION_FAILED: &str = r#"{"error":"json serialization failed"}"#;

struct JsonOutputField<'a, T> {
    section_name: &'a str,
    key: &'a str,
    value: T,
}

impl<T: Serialize> Serialize for JsonOutputField<'_, T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("type", self.section_name)?;
        map.serialize_entry(self.key, &self.value)?;
        map.end()
    }
}

struct JsonError<'a> {
    type_name: &'a str,
    fields: &'a serde_json::Map<String, serde_json::Value>,
}

impl Serialize for JsonError<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1 + self.fields.len()))?;
        map.serialize_entry("type", self.type_name)?;
        for (key, value) in self.fields {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct Printer {
    output: Output,
    format: Format,
}

impl Printer {
    fn fail(&self, error: mage::Error) -> ExitCode {
        match self.output {
            Output::Text => self.write_text(error.to_section()),
            Output::Json => self.write_json_error(&error),
        }

        ExitCode::FAILURE
    }

    fn write_json_error(&self, error: &mage::Error) {
        let inner_value = match error {
            mage::Error::Load(error) => serde_json::to_value(error),
            mage::Error::Flatten(error) => serde_json::to_value(error),
            mage::Error::Compile(error) => serde_json::to_value(error),
        };

        let Ok(serde_json::Value::Object(mut map)) = inner_value else {
            self.write_json(error);
            return;
        };

        let inner_type_name = match map.remove("type") {
            Some(serde_json::Value::String(inner_type_name)) => inner_type_name,
            _ => String::new(),
        };

        let type_name = Self::prefixed_json_type_name(error.stage().name(), &inner_type_name);

        self.write_json(&JsonError {
            type_name: &type_name,
            fields: &map,
        });
    }

    fn write_json<T: Serialize>(&self, value: &T) {
        let result = match self.format {
            Format::Pretty => serde_json::to_string_pretty(value),
            Format::Simple => serde_json::to_string(value),
        };

        match result {
            Ok(encoded) => println!("{}", encoded),
            Err(error) => {
                eprintln!("failed to serialize JSON output: {}", error);
                println!("{}", JSON_SERIALIZATION_FAILED);
            }
        }
    }

    fn write_text(&self, section: Section) {
        println!("{}", section.display(self.format));
    }

    fn write_json_field<T: Serialize>(&self, section_name: &str, key: &str, value: T) {
        self.write_json(&JsonOutputField {
            section_name,
            key,
            value,
        });
    }

    fn write_inline_field<T: Serialize>(
        &self,
        section_name: &str,
        key: &str,
        text_value: impl std::fmt::Display,
        json_value: T,
    ) {
        match self.output {
            Output::Text => self.write_text(Section::new(section_name).inline(key, text_value)),
            Output::Json => self.write_json_field(section_name, key, json_value),
        }
    }

    fn write_block_field<D: std::fmt::Display, T: Serialize>(
        &self,
        section_name: &str,
        key: &str,
        text_value: impl FnOnce() -> D,
        json_value: T,
    ) {
        match self.output {
            Output::Text => self.write_text(Section::new(section_name).block(key, text_value())),
            Output::Json => self.write_json_field(section_name, key, json_value),
        }
    }

    fn prefixed_json_type_name(prefix: &str, json_type_name: &str) -> String {
        if json_type_name.is_empty() {
            prefix.to_string()
        } else {
            format!("{}+{}", prefix, json_type_name)
        }
    }

    fn print_source(&self, code: &Code) {
        self.write_block_field("load", "code", || code.source(), code.source());
    }

    fn print_root(&self, root: &FlatRoot) {
        self.write_block_field(
            "flatten",
            "root",
            || Encoder::encode(root, self.format),
            root,
        );
    }

    fn print_bytecode(&self, section_name: &str, bytecode: &Bytecode) -> Result<(), mage::Error> {
        let reader = Reader::new(bytecode.instructions()).map_err(LoadError::from)?;

        self.write_block_field(
            section_name,
            "bytecode",
            || serde_text::to_string_with_format(&reader, self.format),
            &reader,
        );

        Ok(())
    }

    fn print_exit_code(&self, exit_code: u64) {
        self.write_inline_field("execute", "code", exit_code, exit_code);
    }
}

fn run_hex(path: &str, stage: Stage, printer: &Printer) -> ExitCode {
    let code = match mage_loader::load(path) {
        Ok(source) => Code::new(source),
        Err(error) => return printer.fail(error.into()),
    };

    if stage == Stage::Load {
        printer.print_source(&code);
        return ExitCode::SUCCESS;
    }

    let (root, source_locations) = match mage_ast::decode(code.source()) {
        Ok(pair) => pair,
        Err(error) => return printer.fail(error.into()),
    };

    if stage == Stage::Flatten {
        printer.print_root(&root);
        return ExitCode::SUCCESS;
    }

    let (mut bytecode, _) = match Compiler::compile(&root, &source_locations) {
        Ok(pair) => pair,
        Err(error) => return printer.fail(error.into()),
    };

    match stage {
        Stage::Compile => {
            if let Err(error) = printer.print_bytecode("compile", &bytecode) {
                return printer.fail(error);
            }
            ExitCode::SUCCESS
        }
        Stage::Execute => {
            let exit_code = VirtualMachine::execute(&mut bytecode);
            printer.print_exit_code(exit_code);
            ExitCode::from(exit_code as u8)
        }
        Stage::Load | Stage::Flatten => ExitCode::SUCCESS,
    }
}

fn run_bytecode(path: &str, stage: Stage, printer: &Printer) -> ExitCode {
    let mut bytecode = match mage_loader::load_bytecode(path) {
        Ok(bytecode) => bytecode,
        Err(error) => return printer.fail(error.into()),
    };

    match stage {
        Stage::Load => {
            if let Err(error) = printer.print_bytecode("load", &bytecode) {
                return printer.fail(error);
            }
            ExitCode::SUCCESS
        }
        Stage::Execute => {
            let exit_code = VirtualMachine::execute(&mut bytecode);
            printer.print_exit_code(exit_code);
            ExitCode::from(exit_code as u8)
        }
        Stage::Flatten | Stage::Compile => ExitCode::SUCCESS,
    }
}

fn main() -> ExitCode {
    let ParsedArguments {
        path,
        stage,
        output,
        format,
        ..
    } = parse();

    let printer = Printer { output, format };

    let input_kind = match InputKind::from_path(&path) {
        Ok(kind) => kind,
        Err(error) => return printer.fail(error),
    };

    if !input_kind.supports_stage(stage) {
        return printer.fail(
            LoadError::UnsupportedStage {
                stage: stage.name().into(),
            }
            .into(),
        );
    }

    match input_kind {
        InputKind::Hex => run_hex(&path, stage, &printer),
        InputKind::Bytecode => run_bytecode(&path, stage, &printer),
    }
}
