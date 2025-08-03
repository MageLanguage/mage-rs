use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use mmap_rs::{MmapFlags, MmapMut, MmapOptions, UnsafeMmapFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zydis::{EncoderRequest, insn64};

use crate::{
    Error, FlatBinary, FlatExpression, FlatIndex, FlatNumber, FlatOperator, FlatRoot, FlatSource,
    FlatString,
};

pub fn compile_root(_: FlatRoot) -> Result<Bytecode, Error> {
    // let mut jit = JIT::new()?;

    let mut code = Vec::with_capacity(256);
    let mut add = |request: EncoderRequest| request.encode_extend(&mut code);

    add(insn64!(MOV qword ptr [RDI + 0], RSP)).unwrap();
    add(insn64!(MOV RSP, qword ptr [RSI + 0])).unwrap();

    add(insn64!(MOV qword ptr [RSI + 0], RSP)).unwrap();
    add(insn64!(MOV RSP, qword ptr [RDI + 0])).unwrap();

    add(insn64!(RET)).unwrap();

    Ok(Bytecode { code: code })
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Bytecode {
    pub code: Vec<u8>,
}

struct JIT {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: JITModule,
}

impl JIT {
    fn new() -> Result<Self, Error> {
        let mut flag_builder = settings::builder();
        flag_builder
            .set("use_colocated_libcalls", "true")
            .map_err(|e| Error::CompileError(format!("Failed to set flag: {}", e)))?;
        flag_builder
            .set("is_pic", "true")
            .map_err(|e| Error::CompileError(format!("Failed to set flag: {}", e)))?;

        let isa_builder = cranelift_native::builder()
            .map_err(|e| Error::CompileError(format!("Host machine not supported: {}", e)))?;

        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| Error::CompileError(format!("Failed to create ISA: {}", e)))?;

        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        let module = JITModule::new(builder);

        Ok(Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
        })
    }

    fn compile_source(
        &mut self,
        source: &FlatSource,
        numbers: &[FlatNumber],
        strings: &[FlatString],
    ) -> Result<*const u8, Error> {
        // Set up function signature - no parameters, returns i64
        let int = self.module.target_config().pointer_type();
        self.ctx.func.signature.returns.push(AbiParam::new(int));

        // Create function builder
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);

        // Create entry block
        let entry_block = builder.create_block();
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Set up variables for identifiers
        let mut variables = HashMap::new();
        for (i, _identifier) in source.identifiers.iter().enumerate() {
            let var = Variable::new(i);
            builder.declare_var(var, int);
            variables.insert(i, var);
        }

        // Translate expressions
        let mut translator = ExpressionTranslator {
            builder,
            variables,
            int,
            numbers,
            strings,
            source,
        };

        let mut last_value = translator.builder.ins().iconst(int, 0);

        for expression in &source.expressions {
            last_value = translator.translate_expression(expression)?;
        }

        // Return the last expression value
        translator.builder.ins().return_(&[last_value]);
        translator.builder.finalize();

        // Declare and define the function
        let function_name = "main";
        let id = self
            .module
            .declare_function(function_name, Linkage::Export, &self.ctx.func.signature)
            .map_err(|e| Error::CompileError(format!("Failed to declare function: {}", e)))?;

        self.module
            .define_function(id, &mut self.ctx)
            .map_err(|e| Error::CompileError(format!("Failed to define function: {}", e)))?;

        self.module.clear_context(&mut self.ctx);
        self.module
            .finalize_definitions()
            .map_err(|e| Error::CompileError(format!("Failed to finalize: {}", e)))?;

        Ok(self.module.get_finalized_function(id))
    }
}

struct ExpressionTranslator<'a> {
    builder: FunctionBuilder<'a>,
    variables: HashMap<usize, Variable>,
    int: types::Type,
    numbers: &'a [FlatNumber],
    strings: &'a [FlatString],
    source: &'a FlatSource,
}

impl<'a> ExpressionTranslator<'a> {
    fn translate_expression(&mut self, expr: &FlatExpression) -> Result<Value, Error> {
        match expr {
            FlatExpression::Number(index) => {
                let value = self.get_number_value(index)?;
                Ok(self.builder.ins().iconst(self.int, value))
            }

            FlatExpression::String(_index) => {
                // For now, just return 0 for strings
                Ok(self.builder.ins().iconst(self.int, 0))
            }

            FlatExpression::Identifier(index) => {
                if let FlatIndex::Identifier(idx) = index {
                    let var = self.get_variable(idx)?;
                    Ok(self.builder.use_var(var))
                } else {
                    Err(Error::CompileError("Expected identifier index".to_string()))
                }
            }

            FlatExpression::Additive(binary) => self.translate_binary(binary),
            FlatExpression::Multiplicative(binary) => self.translate_binary(binary),
            FlatExpression::Comparison(binary) => self.translate_binary(binary),
            FlatExpression::Logical(binary) => self.translate_binary(binary),
            FlatExpression::Assign(binary) => self.translate_assign(binary),

            FlatExpression::Member(_) => Err(Error::CompileError(
                "Member access not implemented".to_string(),
            )),
            FlatExpression::Call(_) => Err(Error::CompileError(
                "Function calls not implemented".to_string(),
            )),
        }
    }

    fn translate_binary(&mut self, binary: &FlatBinary) -> Result<Value, Error> {
        let right_val = self.translate_index(&binary.two)?;

        let left_val = if let Some(ref left_idx) = binary.one {
            self.translate_index(left_idx)?
        } else {
            // Unary operation - use 0 as left operand
            self.builder.ins().iconst(self.int, 0)
        };

        match binary.operator {
            FlatOperator::Add => Ok(self.builder.ins().iadd(left_val, right_val)),
            FlatOperator::Subtract => Ok(self.builder.ins().isub(left_val, right_val)),
            FlatOperator::Multiply => Ok(self.builder.ins().imul(left_val, right_val)),
            FlatOperator::Divide => Ok(self.builder.ins().udiv(left_val, right_val)),
            FlatOperator::Modulo => Ok(self.builder.ins().urem(left_val, right_val)),

            FlatOperator::Equal => {
                let cmp = self.builder.ins().icmp(IntCC::Equal, left_val, right_val);
                Ok(self.builder.ins().uextend(self.int, cmp))
            }
            FlatOperator::NotEqual => {
                let cmp = self
                    .builder
                    .ins()
                    .icmp(IntCC::NotEqual, left_val, right_val);
                Ok(self.builder.ins().uextend(self.int, cmp))
            }
            FlatOperator::LessThan => {
                let cmp = self
                    .builder
                    .ins()
                    .icmp(IntCC::SignedLessThan, left_val, right_val);
                Ok(self.builder.ins().uextend(self.int, cmp))
            }
            FlatOperator::GreaterThan => {
                let cmp = self
                    .builder
                    .ins()
                    .icmp(IntCC::SignedGreaterThan, left_val, right_val);
                Ok(self.builder.ins().uextend(self.int, cmp))
            }
            FlatOperator::LessEqual => {
                let cmp =
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedLessThanOrEqual, left_val, right_val);
                Ok(self.builder.ins().uextend(self.int, cmp))
            }
            FlatOperator::GreaterEqual => {
                let cmp =
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThanOrEqual, left_val, right_val);
                Ok(self.builder.ins().uextend(self.int, cmp))
            }

            FlatOperator::And => Ok(self.builder.ins().band(left_val, right_val)),
            FlatOperator::Or => Ok(self.builder.ins().bor(left_val, right_val)),

            _ => Err(Error::CompileError(format!(
                "Operator {:?} not supported in binary expression",
                binary.operator
            ))),
        }
    }

    fn translate_assign(&mut self, binary: &FlatBinary) -> Result<Value, Error> {
        // For assignment, left operand should be an identifier, right is the value
        let value = self.translate_index(&binary.two)?;

        if let Some(FlatIndex::Identifier(var_idx)) = &binary.one {
            let var = self.get_variable(var_idx)?;
            self.builder.def_var(var, value);
            Ok(value)
        } else {
            Err(Error::CompileError(
                "Assignment left side must be an identifier".to_string(),
            ))
        }
    }

    fn translate_index(&mut self, index: &FlatIndex) -> Result<Value, Error> {
        match index {
            FlatIndex::Number(_idx) => {
                let value = self.get_number_value(index)?;
                Ok(self.builder.ins().iconst(self.int, value))
            }
            FlatIndex::String(_idx) => Ok(self.builder.ins().iconst(self.int, 0)),
            FlatIndex::Identifier(idx) => {
                let var = self.get_variable(idx)?;
                Ok(self.builder.use_var(var))
            }
            FlatIndex::Expression(idx) => {
                if let Some(expr) = self.source.expressions.get(*idx) {
                    self.translate_expression(expr)
                } else {
                    Err(Error::CompileError("Invalid expression index".to_string()))
                }
            }
            FlatIndex::Source(_) => Err(Error::CompileError(
                "Source references not supported in expressions".to_string(),
            )),
        }
    }

    fn get_number_value(&self, index: &FlatIndex) -> Result<i64, Error> {
        if let FlatIndex::Number(idx) = index {
            if let Some(FlatNumber(num_str)) = self.numbers.get(*idx) {
                self.parse_number(num_str)
            } else {
                Err(Error::CompileError("Invalid number index".to_string()))
            }
        } else {
            Err(Error::CompileError("Expected number index".to_string()))
        }
    }

    fn parse_number(&self, num_str: &str) -> Result<i64, Error> {
        if let Some(hex_str) = num_str
            .strip_prefix("0x")
            .or_else(|| num_str.strip_prefix("0X"))
        {
            i64::from_str_radix(hex_str, 16)
                .map_err(|e| Error::CompileError(format!("Invalid hex number: {}", e)))
        } else if let Some(bin_str) = num_str
            .strip_prefix("0b")
            .or_else(|| num_str.strip_prefix("0B"))
        {
            i64::from_str_radix(bin_str, 2)
                .map_err(|e| Error::CompileError(format!("Invalid binary number: {}", e)))
        } else if let Some(oct_str) = num_str
            .strip_prefix("0o")
            .or_else(|| num_str.strip_prefix("0O"))
        {
            i64::from_str_radix(oct_str, 8)
                .map_err(|e| Error::CompileError(format!("Invalid octal number: {}", e)))
        } else if let Some(dec_str) = num_str
            .strip_prefix("0d")
            .or_else(|| num_str.strip_prefix("0D"))
        {
            dec_str
                .parse::<i64>()
                .map_err(|e| Error::CompileError(format!("Invalid decimal number: {}", e)))
        } else {
            // Fallback to decimal parsing
            num_str
                .parse::<i64>()
                .map_err(|e| Error::CompileError(format!("Invalid number: {}", e)))
        }
    }

    fn get_variable(&self, idx: &usize) -> Result<Variable, Error> {
        self.variables
            .get(idx)
            .copied()
            .ok_or_else(|| Error::CompileError("Undefined variable".to_string()))
    }
}
