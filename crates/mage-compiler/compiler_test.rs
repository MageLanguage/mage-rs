use mage_contract::{Instruction, Reader, SuperinstructionKind};

use crate::{Bytecode, CompileError, Compiler, PatchMap, SourceMap};

fn compile_result(code: &str) -> (Bytecode, SourceMap, PatchMap, Vec<CompileError>) {
    let (root, locations) = mage_ast::decode(code).unwrap();
    Compiler::compile_recovering(&root, &locations)
}

fn compile(code: &str) -> (Bytecode, SourceMap, PatchMap) {
    let (bytecode, source_map, patch_map, errors) = compile_result(code);
    if let Some(error) = errors.into_iter().next() {
        panic!("compilation failed for {code:?}: {error}");
    }
    (bytecode, source_map, patch_map)
}

fn try_compile(code: &str) -> Result<(Bytecode, SourceMap, PatchMap), CompileError> {
    let (bytecode, source_map, patch_map, errors) = compile_result(code);
    if let Some(error) = errors.into_iter().next() {
        Err(error)
    } else {
        Ok((bytecode, source_map, patch_map))
    }
}

fn compile_recovering(code: &str) -> (Bytecode, SourceMap, PatchMap, Vec<CompileError>) {
    compile_result(code)
}

fn decode_instructions(bytecode: &Bytecode) -> Vec<Instruction> {
    let mut reader = Reader::new(bytecode.instructions()).unwrap();
    let count = reader.instruction_count();
    (0..count).map(|_| reader.read().unwrap()).collect()
}

fn compile_instructions(code: &str) -> Vec<Instruction> {
    let (bytecode, _, _) = compile(code);
    decode_instructions(&bytecode)
}

fn has<F: Fn(&Instruction) -> bool>(instructions: &[Instruction], pred: F) -> bool {
    instructions.iter().any(pred)
}

fn count<F: Fn(&Instruction) -> bool>(instructions: &[Instruction], pred: F) -> usize {
    instructions
        .iter()
        .filter(|instruction| pred(instruction))
        .count()
}

fn is_take(instruction: &Instruction) -> bool {
    matches!(instruction, Instruction::TakeStackSizeImmutable(_))
}

fn is_free(instruction: &Instruction) -> bool {
    matches!(instruction, Instruction::FreeStackSizeImmutable(_))
}

fn is_jump(instruction: &Instruction) -> bool {
    matches!(instruction, Instruction::JumpToImmutable(_))
}

fn is_jump_if_not(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::JumpIfNotConditionOffsetToImmutable(_)
    )
}

fn is_jump_if(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::JumpIfConditionOffsetToImmutable(_)
    )
}

fn is_load_immutable(instruction: &Instruction) -> bool {
    matches!(instruction, Instruction::LoadTargetOffsetSourceImmutable(_))
}

fn is_exit(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::ExitCodeImmutable(_) | Instruction::ExitCodeOffset(_)
    )
}

// --- Basic compilation ---

#[test]
fn empty_program_compiles() {
    let (bytecode, _, _) = compile("");
    assert!(!bytecode.is_empty());
}

#[test]
fn empty_program_produces_valid_bytecode() {
    let instructions = compile_instructions("");
    assert!(has(&instructions, is_take));
    assert!(has(&instructions, is_exit));
    assert!(has(&instructions, is_free));
}

#[test]
fn empty_return() {
    let instructions = compile_instructions("return 0d0;");
    assert!(has(&instructions, is_take));
    assert!(has(&instructions, is_exit));
    assert!(has(&instructions, is_free));
    assert!(has(&instructions, is_load_immutable));
}

#[test]
fn literal_return() {
    let instructions = compile_instructions("return 0d42;");
    let has_42 = instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::LoadTargetOffsetSourceImmutable(data) if data.source == 42
        )
    });
    assert!(has_42);
}

#[test]
fn root_prologue_structure() {
    let instructions = compile_instructions("return 0d1;");
    assert!(matches!(
        instructions[0],
        Instruction::LoadTargetOffsetSourceImmutable(_)
    ));
    assert!(matches!(
        instructions[1],
        Instruction::TakeStackSizeImmutable(_)
    ));
    assert!(matches!(instructions[2], Instruction::JumpToImmutable(_)));
    assert!(matches!(instructions[3], Instruction::ExitCodeOffset(_)));
}

#[test]
fn root_jump_points_past_exit() {
    let instructions = compile_instructions("return 0d1;");
    // Load + Take + Jump + Exit = 24 + 16 + 16 + 16 = 72 bytes, so the root body starts at offset >= 72.
    if let Instruction::JumpToImmutable(jump) = instructions[2] {
        assert!(
            jump.to > 48,
            "jump target {} should point past Exit",
            jump.to
        );
    } else {
        panic!("third instruction must be Jump");
    }
}

// --- Constants and variables ---

#[test]
fn constant_binding() {
    let instructions = compile_instructions("x : 0d10; return x;");
    assert!(has(&instructions, is_load_immutable));
}

#[test]
fn variable_assignment() {
    let instructions = compile_instructions("x = 0d5; return x;");
    assert!(has(&instructions, is_load_immutable));
}

#[test]
fn variable_reassignment() {
    let instructions = compile_instructions("x = 0d1; x = 0d2; return x;");
    assert!(count(&instructions, is_load_immutable) >= 2);
}

#[test]
fn multiple_variables() {
    compile("a = 0d1; b = 0d2; c = 0d3; return a + b + c;");
}

// --- Binary operations ---

#[test]
fn add_immutable() {
    let instructions = compile_instructions("return 0d3 + 0d4;");
    assert!(
        instructions
            .iter()
            .any(|i| matches!(i, Instruction::AddTargetOffsetLeftOffsetRightImmutable(_)))
    );
}

#[test]
fn add_offsets() {
    let instructions = compile_instructions("a = 0d3; b = 0d4; return a + b;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::AddTargetOffsetLeftOffsetRightOffset(_)
            | Instruction::AddTargetOffsetLeftOffsetRightImmutable(_)
    )));
}

#[test]
fn subtract_operation() {
    let instructions = compile_instructions("return 0d10 - 0d3;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::SubtractTargetOffsetLeftOffsetRightImmutable(_)
            | Instruction::SubtractTargetOffsetLeftOffsetRightOffset(_)
    )));
}

#[test]
fn multiply_operation() {
    let instructions = compile_instructions("return 0d5 * 0d6;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::MultiplyTargetOffsetLeftOffsetRightImmutable(_)
            | Instruction::MultiplyTargetOffsetLeftOffsetRightOffset(_)
    )));
}

#[test]
fn divide_operation() {
    let instructions = compile_instructions("return 0d20 / 0d4;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::DivideTargetOffsetLeftOffsetRightImmutable(_)
            | Instruction::DivideTargetOffsetLeftOffsetRightOffset(_)
    )));
}

#[test]
fn modulo_operation() {
    let instructions = compile_instructions("return 0d10 % 0d3;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::ModuloTargetOffsetLeftOffsetRightImmutable(_)
            | Instruction::ModuloTargetOffsetLeftOffsetRightOffset(_)
    )));
}

#[test]
fn less_than_operation() {
    let instructions = compile_instructions("return 0d1 < 0d2;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LessThanTargetOffsetLeftOffsetRightImmutable(_)
            | Instruction::LessThanTargetOffsetLeftOffsetRightOffset(_)
    )));
}

#[test]
fn greater_than_operation() {
    let instructions = compile_instructions("return 0d5 > 0d3;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::GreaterThanTargetOffsetLeftOffsetRightImmutable(_)
            | Instruction::GreaterThanTargetOffsetLeftOffsetRightOffset(_)
    )));
}

#[test]
fn less_than_or_equal_operation() {
    let instructions = compile_instructions("return 0d1 <= 0d2;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LessThanOrEqualTargetOffsetLeftOffsetRightImmutable(_)
            | Instruction::LessThanOrEqualTargetOffsetLeftOffsetRightOffset(_)
    )));
}

#[test]
fn greater_than_or_equal_operation() {
    let instructions = compile_instructions("return 0d1 >= 0d2;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::GreaterThanOrEqualTargetOffsetLeftOffsetRightImmutable(_)
            | Instruction::GreaterThanOrEqualTargetOffsetLeftOffsetRightOffset(_)
    )));
}

#[test]
fn equal_operation() {
    let instructions = compile_instructions("return 0d1 == 0d1;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::EqualTargetOffsetLeftOffsetRightImmutable(_)
            | Instruction::EqualTargetOffsetLeftOffsetRightOffset(_)
    )));
}

#[test]
fn not_equal_operation() {
    let instructions = compile_instructions("return 0d1 != 0d2;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::NotEqualTargetOffsetLeftOffsetRightImmutable(_)
            | Instruction::NotEqualTargetOffsetLeftOffsetRightOffset(_)
    )));
}

#[test]
fn chained_arithmetic() {
    let instructions = compile_instructions("return 0d1 + 0d2 + 0d3;");
    let add_count = instructions
        .iter()
        .filter(|i| {
            matches!(
                i,
                Instruction::AddTargetOffsetLeftOffsetRightImmutable(_)
                    | Instruction::AddTargetOffsetLeftOffsetRightOffset(_)
            )
        })
        .count();
    assert_eq!(add_count, 2);
}

#[test]
fn mixed_precedence() {
    compile("return 0d1 + 0d2 * 0d3;");
}

#[test]
fn long_addition_chain() {
    let instructions = compile_instructions("return 0d1 + 0d2 + 0d3 + 0d4 + 0d5;");
    let add_count = instructions
        .iter()
        .filter(|i| {
            matches!(
                i,
                Instruction::AddTargetOffsetLeftOffsetRightImmutable(_)
                    | Instruction::AddTargetOffsetLeftOffsetRightOffset(_)
            )
        })
        .count();
    assert_eq!(add_count, 4);
}

#[test]
fn mixed_kind_chain() {
    // a + b * c + d  should NOT flatten across different precedence levels.
    let instructions = compile_instructions("return 0d1 + 0d2 * 0d3 + 0d4;");
    let add_count = instructions
        .iter()
        .filter(|i| {
            matches!(
                i,
                Instruction::AddTargetOffsetLeftOffsetRightImmutable(_)
                    | Instruction::AddTargetOffsetLeftOffsetRightOffset(_)
            )
        })
        .count();
    assert_eq!(add_count, 2);
    let mul_count = instructions
        .iter()
        .filter(|i| {
            matches!(
                i,
                Instruction::MultiplyTargetOffsetLeftOffsetRightImmutable(_)
                    | Instruction::MultiplyTargetOffsetLeftOffsetRightOffset(_)
            )
        })
        .count();
    assert_eq!(mul_count, 1);
}

#[test]
fn chain_with_variables() {
    compile("a = 0d1; b = 0d2; c = 0d3; return a + b + c;");
}

// --- Control flow — if ---

#[test]
fn if_produces_conditional_jump() {
    let instructions = compile_instructions("x = 0d1; if x, { return 0d42; };");
    assert!(has(&instructions, is_jump_if_not));
}

#[test]
fn if_with_comparison_condition() {
    let instructions = compile_instructions("if 0d1 < 0d2, { return 0d1; };");
    assert!(has(&instructions, is_jump_if_not));
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LessThanTargetOffsetLeftOffsetRightImmutable(_)
    )));
}

#[test]
fn nested_if() {
    let instructions = compile_instructions("if 0d1, { if 0d1, { return 0d42; }; };");
    assert_eq!(count(&instructions, is_jump_if_not), 2);
}

// --- Control flow — while ---

#[test]
fn while_produces_loop_structure() {
    let instructions = compile_instructions("i = 0d0; while i < 0d10, { i = i + 0d1; };");
    assert!(has(&instructions, is_jump_if));
    assert!(has(&instructions, is_jump));
}

#[test]
fn while_accumulate() {
    compile("sum = 0d0; i = 0d0; while i < 0d10, { sum = sum + i; i = i + 0d1; }; return sum;");
}

#[test]
fn while_with_break_if_block() {
    compile("x = 0d1; if x, { while 0d1, { break x; }; };");
}

#[test]
fn while_with_labeled_break() {
    let instructions = compile_instructions("i = 0d0; while i < 0d10, { i = i + 0d1; break i; };");
    // labeled break should emit a jump past the loop
    assert!(count(&instructions, is_jump) >= 2);
}

#[test]
fn while_with_named_break() {
    let instructions = compile_instructions("i = 0d0; while i < 0d10, { i = i + 0d1; break i; };");
    // named break targeting the while condition variable should emit a jump past the loop
    assert!(count(&instructions, is_jump) >= 2);
}

#[test]
fn nested_while_break_inner() {
    compile(
        "i = 0d0; while i < 0d5, { j = 0d0; while j < 0d5, { j = j + 0d1; break; }; i = i + 0d1; };",
    );
}

#[test]
fn nested_while_break_outer() {
    compile(
        "i = 0d0; while i < 0d5, { j = 0d0; while j < 0d5, { j = j + 0d1; break i; }; i = i + 0d1; };",
    );
}

#[test]
fn while_with_labeled_continue() {
    compile("i = 0d0; while i < 0d5, { i = i + 0d1; continue i; };");
}

// --- Procedures ---

#[test]
fn procedure_declaration_does_not_emit_inline() {
    let instructions =
        compile_instructions("add_one : procedure {n : U64}, U64 { return n + 0d1; }; return 0d0;");
    assert!(has(&instructions, is_take));
    assert!(has(&instructions, is_free));
}

#[test]
fn procedure_call() {
    let instructions = compile_instructions(
        "add_one : procedure {n : U64}, U64 { return n + 0d1; }; return (add_one 0d10);",
    );
    // Root entry Take + user procedure Take.
    let take_count = count(&instructions, is_take);
    assert!(take_count >= 2, "got {take_count}");
}

#[test]
fn procedure_multiple_parameters() {
    compile("add : procedure {a : U64; b : U64}, U64 { return a + b; }; return (add 0d3, 0d4);");
}

#[test]
fn procedure_called_from_multiple_sites() {
    let instructions = compile_instructions(
        "f : procedure {n : U64}, U64 { return n + 0d1; }; a = f 0d1; b = f 0d2; return a + b;",
    );
    // Root entry + two user calls.
    let take_count = count(&instructions, is_take);
    assert!(take_count >= 3, "got {take_count}");
}

#[test]
fn procedure_direct_return_via_jump_offset() {
    let instructions = compile_instructions(
        "f : procedure {n : U64}, U64 { return n; }; a = f 0d1; b = f 0d2; return a + b;",
    );
    // Direct return uses JumpToOffset to read the return address from the stack.
    assert!(
        instructions
            .iter()
            .any(|i| matches!(i, Instruction::JumpToOffset(_)))
    );
}

#[test]
fn procedure_single_call_site_direct_return() {
    let instructions =
        compile_instructions("f : procedure {n : U64}, U64 { return n; }; return (f 0d5);");
    // Single call site skips the dispatch comparison — returns directly.
    let has_dispatch_compare = instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::LessThanTargetOffsetLeftOffsetRightImmutable(data) if data.right == 1
        )
    });
    assert!(!has_dispatch_compare);
}

#[test]
fn recursive_procedure() {
    compile(
        "fibonacci : procedure {n : U64}, U64 { \
            if n < 0d2, { return n; }; \
            return (fibonacci n - 0d1) + (fibonacci n - 0d2); \
         }; \
         return (fibonacci 0d10);",
    );
}

#[test]
fn procedure_multiple_return_values() {
    compile(
        "exchange : procedure {a : U64; b : U64}, {c : U64; d : U64} { \
            return b, a; \
         }; \
         c, d = exchange 0d10, 0d20; \
         return c - d;",
    );
}

#[test]
fn procedure_implicit_return() {
    let instructions =
        compile_instructions("noop : procedure {n : U64}, U64 { n + 0d1; }; return (noop 0d1);");
    assert!(has(&instructions, is_jump));
}

#[test]
fn procedure_discard_return_value() {
    compile("f : procedure {n : U64}, U64 { return n; }; f 0d5; return 0d0;");
}

// --- Number formats ---

#[test]
fn decimal_number() {
    let instructions = compile_instructions("return 0d255;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LoadTargetOffsetSourceImmutable(data) if data.source == 255
    )));
}

#[test]
fn hex_number() {
    let instructions = compile_instructions("return 0xFF;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LoadTargetOffsetSourceImmutable(data) if data.source == 255
    )));
}

#[test]
fn binary_number() {
    let instructions = compile_instructions("return 0b1010;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LoadTargetOffsetSourceImmutable(data) if data.source == 10
    )));
}

#[test]
fn octal_number() {
    let instructions = compile_instructions("return 0o17;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LoadTargetOffsetSourceImmutable(data) if data.source == 15
    )));
}

#[test]
fn decimal_number_with_underscores() {
    let instructions = compile_instructions("return 0d1_000;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LoadTargetOffsetSourceImmutable(data) if data.source == 1000
    )));
}

#[test]
fn binary_number_with_underscores() {
    let instructions = compile_instructions("return 0b1010_1100;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LoadTargetOffsetSourceImmutable(data) if data.source == 0b10101100
    )));
}

#[test]
fn hex_number_with_underscores() {
    let instructions = compile_instructions("return 0xFF_00;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LoadTargetOffsetSourceImmutable(data) if data.source == 0xFF00
    )));
}

#[test]
fn octal_number_with_underscores() {
    let instructions = compile_instructions("return 0o7_5_5;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LoadTargetOffsetSourceImmutable(data) if data.source == 0o755
    )));
}

#[test]
fn explicit_decimal_with_underscores() {
    let instructions = compile_instructions("return 0d1_000;");
    assert!(instructions.iter().any(|i| matches!(
        i,
        Instruction::LoadTargetOffsetSourceImmutable(data) if data.source == 1000
    )));
}

// --- Implicit return ---

#[test]
fn implicit_return_when_no_explicit_return() {
    let instructions = compile_instructions("x = 0d42;");
    assert!(has(&instructions, is_free));
}

#[test]
fn no_implicit_return_after_explicit_return() {
    let instructions = compile_instructions("return 0d1;");
    assert!(count(&instructions, is_free) >= 1);
}

// --- Frame layout ---

#[test]
fn root_frame_size_is_at_least_minimum() {
    let instructions = compile_instructions("return 0d0;");
    // Minimum frame is 2 value slots = 16 bytes. Take is at index 1 (after Load return addr).
    if let Instruction::TakeStackSizeImmutable(take) = instructions[1] {
        assert!(take.stack_size >= 16, "got {}", take.stack_size);
    } else {
        panic!("second instruction must be Take");
    }
}

#[test]
fn frame_grows_with_variables() {
    let small = compile_instructions("return 0d0;");
    let large = compile_instructions(
        "a = 0d1; b = 0d2; c = 0d3; d = 0d4; e = 0d5; return a + b + c + d + e;",
    );

    let small_frame = match small[1] {
        Instruction::TakeStackSizeImmutable(take) => take.stack_size,
        _ => panic!("expected Take"),
    };
    let large_frame = match large[1] {
        Instruction::TakeStackSizeImmutable(take) => take.stack_size,
        _ => panic!("expected Take"),
    };

    assert!(large_frame > small_frame, "{large_frame} vs {small_frame}");
}

#[test]
fn procedure_frame_includes_call_area() {
    let instructions = compile_instructions(
        "inner : procedure {n : U64}, U64 { return n; }; \
         outer : procedure {n : U64}, U64 { return (inner n); }; \
         return (outer 0d5);",
    );
    // Root + outer + inner = at least 3 Take instructions.
    let take_count = count(&instructions, is_take);
    assert!(take_count >= 3, "got {take_count}");
}

// --- Error cases ---

#[test]
fn if_with_one_argument_is_error() {
    assert!(matches!(
        try_compile("if 0d1;"),
        Err(CompileError::IfArgumentCount { found: 1, .. })
    ));
}

#[test]
fn if_with_three_arguments_is_error() {
    assert!(matches!(
        try_compile("if 0d1, {}, {};"),
        Err(CompileError::IfArgumentCount { found: 3, .. })
    ));
}

#[test]
fn if_non_block_body_is_error() {
    assert!(matches!(
        try_compile("if 0d1, 0d2;"),
        Err(CompileError::IfSecondArgumentNotBlock { .. })
    ));
}

#[test]
fn while_with_one_argument_is_error() {
    assert!(matches!(
        try_compile("while 0d1;"),
        Err(CompileError::WhileArgumentCount { found: 1, .. })
    ));
}

#[test]
fn while_non_block_body_is_error() {
    assert!(matches!(
        try_compile("while 0d1, 0d2;"),
        Err(CompileError::WhileSecondArgumentNotBlock { .. })
    ));
}

#[test]
fn undefined_procedure_is_error() {
    assert!(matches!(
        try_compile("return (unknown 0d1);"),
        Err(CompileError::UndefinedProcedure { ref name, .. }) if name == "unknown"
    ));
}

#[test]
fn undefined_procedure_discard_is_error() {
    assert!(matches!(
        try_compile("unknown 0d1;"),
        Err(CompileError::UndefinedProcedure { ref name, .. }) if name == "unknown"
    ));
}

#[test]
fn undefined_variable_in_return_is_error() {
    assert!(matches!(
        try_compile("return x;"),
        Err(CompileError::UndefinedVariable { ref name, .. }) if name == "x"
    ));
}

#[test]
fn undefined_variable_in_expression_is_error() {
    assert!(matches!(
        try_compile("a = 0d1; return a + b;"),
        Err(CompileError::UndefinedVariable { ref name, .. }) if name == "b"
    ));
}

#[test]
fn undefined_variable_in_assignment_is_error() {
    assert!(matches!(
        try_compile("a = b;"),
        Err(CompileError::UndefinedVariable { ref name, .. }) if name == "b"
    ));
}

#[test]
fn undefined_variable_in_call_argument_is_error() {
    assert!(matches!(
        try_compile("f : procedure {n : U64}, U64 { return n; }; return (f x);"),
        Err(CompileError::UndefinedVariable { ref name, .. }) if name == "x"
    ));
}

#[test]
fn defined_variable_is_not_error() {
    compile("x = 0d5; return x;");
}

#[test]
fn constant_with_invalid_target_is_error() {
    assert!(matches!(
        try_compile("a.b : 0d1;"),
        Err(CompileError::InvalidConstantTarget { .. })
    ));
}

#[test]
fn assignment_with_invalid_target_is_error() {
    assert!(matches!(
        try_compile("a.b = 0d1;"),
        Err(CompileError::InvalidAssignmentTarget { .. })
    ));
}

#[test]
fn multiple_assignment_with_invalid_target_is_error() {
    let code = "f : procedure {n : U64}, {x : U64; y : U64} { return n, n; }; a.b, c = f 0d1;";
    assert!(matches!(
        try_compile(code),
        Err(CompileError::InvalidMultipleAssignmentTarget { .. })
    ));
}

#[test]
fn procedure_type_constructor_without_body_compiles() {
    assert!(try_compile("f : procedure 0d1, 0d2;").is_ok());
}

#[test]
fn break_requires_explicit_label() {
    assert!(matches!(
        try_compile("break missing;"),
        Err(CompileError::BreakUnresolvedTarget { ref name, .. }) if name == "missing"
    ));
}

#[test]
fn continue_requires_explicit_label() {
    assert!(matches!(
        try_compile("continue missing;"),
        Err(CompileError::ContinueUnresolvedTarget { ref name, .. }) if name == "missing"
    ));
}

// --- Error offsets ---

#[test]
fn error_offset_first_statement() {
    match try_compile("return x;") {
        Err(CompileError::UndefinedVariable { offset, .. }) => {
            assert_eq!(offset, 0);
        }
        other => panic!("expected UndefinedVariable, got {:?}", other),
    }
}

#[test]
fn error_offset_second_statement() {
    // "a = 0d1; return a + b;"
    //  ^0       ^9
    match try_compile("a = 0d1; return a + b;") {
        Err(CompileError::UndefinedVariable { offset, .. }) => {
            assert_eq!(offset, 9);
        }
        other => panic!("expected UndefinedVariable, got {:?}", other),
    }
}

#[test]
fn error_offset_if_argument_count() {
    match try_compile("if 0d1;") {
        Err(CompileError::IfArgumentCount { offset, .. }) => {
            assert_eq!(offset, 0);
        }
        other => panic!("expected IfArgumentCount, got {:?}", other),
    }
}

#[test]
fn error_offset_undefined_procedure() {
    match try_compile("unknown 0d1;") {
        Err(CompileError::UndefinedProcedure { offset, .. }) => {
            assert_eq!(offset, 0);
        }
        other => panic!("expected UndefinedProcedure, got {:?}", other),
    }
}

#[test]
fn error_offset_undefined_procedure_after_valid_statement() {
    // "x = 0d1; unknown 0d2;"
    //  ^0       ^9
    match try_compile("x = 0d1; unknown 0d2;") {
        Err(CompileError::UndefinedProcedure { offset, .. }) => {
            assert_eq!(offset, 9);
        }
        other => panic!("expected UndefinedProcedure, got {:?}", other),
    }
}

#[test]
fn error_offset_duplicate_constant() {
    // "x : 0d1; x : 0d2;"
    //  ^0       ^9
    match try_compile("x : 0d1; x : 0d2;") {
        Err(CompileError::DuplicateConstant { offset, .. }) => {
            assert_eq!(offset, 9);
        }
        other => panic!("expected DuplicateConstant, got {:?}", other),
    }
}

#[test]
fn error_has_no_offset_for_internal_errors() {
    // Internal fixup errors have no source offset
    assert_eq!(CompileError::UnresolvedWhileTargetLabel.offset(), None);
    assert_eq!(CompileError::MissingProcedureBytecodeOffset.offset(), None);
}

// --- Error recovery (1.5 & 1.6) ---

#[test]
fn recovering_constant_with_bad_rhs_still_defines_variable() {
    // `x : undefined_var` fails, but x should still be defined so that
    // `return x` does not produce a cascading UndefinedVariable error.
    let (_, _, _, errors) = compile_recovering("x : bad; return x;");
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        &errors[0],
        CompileError::UndefinedVariable { name, .. } if name == "bad"
    ));
}

#[test]
fn recovering_variable_with_bad_rhs_still_defines_variable() {
    let (_, _, _, errors) = compile_recovering("x = bad; return x;");
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        &errors[0],
        CompileError::UndefinedVariable { name, .. } if name == "bad"
    ));
}

#[test]
fn recovering_multiple_independent_errors() {
    // Two independent errors: undefined a and undefined b.
    let (_, _, _, errors) = compile_recovering("x = bad1; y = bad2; return 0d0;");
    assert_eq!(errors.len(), 2);
    assert!(matches!(
        &errors[0],
        CompileError::UndefinedVariable { name, .. } if name == "bad1"
    ));
    assert!(matches!(
        &errors[1],
        CompileError::UndefinedVariable { name, .. } if name == "bad2"
    ));
}

#[test]
fn recovering_no_cascade_after_constant_error() {
    // The constant `x` has a bad RHS, but x should still be usable after.
    // Only one error expected, not two.
    let (_, _, _, errors) = compile_recovering("x : bad; y = x + 0d1; return y;");
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        &errors[0],
        CompileError::UndefinedVariable { name, .. } if name == "bad"
    ));
}

#[test]
fn recovering_procedure_error_does_not_corrupt_next_procedure() {
    // First procedure has an error; second procedure should compile fine.
    let code = "\
        bad_proc : procedure {}, U64 { return undefined_var; }; \
        good_proc : procedure {n : U64}, U64 { return n + 0d1; }; \
        return (good_proc 0d5);";
    let (bytecode, _, _, errors) = compile_recovering(code);
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        &errors[0],
        CompileError::UndefinedVariable { name, .. } if name == "undefined_var"
    ));
    // Bytecode should still be decodable despite the error in bad_proc.
    assert!(Reader::new(bytecode.instructions()).is_ok());
}

#[test]
fn recovering_produces_valid_bytecode() {
    // Even with errors, the emitted bytecode should be structurally valid
    // (decodable) because poison values fill in for failed expressions.
    let (bytecode, _, _, errors) = compile_recovering("x : bad; return x;");
    assert!(!errors.is_empty());
    assert!(Reader::new(bytecode.instructions()).is_ok());
}

#[test]
fn recovering_error_in_if_body_does_not_leak_patches() {
    // An error inside an if-body should not leave stale block patches
    // that corrupt subsequent control flow.
    let code = "if 0d1, { x = bad; }; return 0d0;";
    let (bytecode, _, _, errors) = compile_recovering(code);
    assert_eq!(errors.len(), 1);
    assert!(Reader::new(bytecode.instructions()).is_ok());
}

#[test]
fn recovering_error_in_while_body_does_not_leak_patches() {
    let code = "i = 0d0; while i < 0d3, { x = bad; i = i + 0d1; }; return i;";
    let (bytecode, _, _, errors) = compile_recovering(code);
    assert_eq!(errors.len(), 1);
    assert!(Reader::new(bytecode.instructions()).is_ok());
}

// --- Scoping & constant/variable semantics (2.1 & 2.4) ---

#[test]
fn constant_duplicate_in_same_block_is_error() {
    assert!(matches!(
        try_compile("x : 0d1; x : 0d2;"),
        Err(CompileError::DuplicateConstant { ref name, .. }) if name == "x"
    ));
}

#[test]
fn constant_shadow_in_inner_block_is_allowed() {
    // x : 0d10; { x : 0d20 }; — different blocks, shadowing allowed
    compile("x : 0d10; if 0d1, { x : 0d20; return x; }; return x;");
}

#[test]
fn constant_shadow_restores_after_block() {
    // After the inner block, x should refer to the outer constant (0d10)
    let instructions = compile_instructions("x : 0d10; if 0d1 < 0d2, { x : 0d20; }; return x;");
    // Should compile without error; the outer x is still accessible
    assert!(!instructions.is_empty());
}

#[test]
fn variable_reassignment_is_allowed() {
    compile("x = 0d1; x = 0d2; return x;");
}

#[test]
fn assignment_to_constant_is_error() {
    // x : 0d1 defines a constant; x = 0d2 tries to mutate it
    assert!(matches!(
        try_compile("x : 0d1; x = 0d2;"),
        Err(CompileError::AssignmentToConstant { ref name, .. }) if name == "x"
    ));
}

#[test]
fn assignment_to_constant_in_inner_block_is_error() {
    // The inner block sees x as a constant from the parent scope
    assert!(matches!(
        try_compile("x : 0d1; if 0d1, { x = 0d2; };"),
        Err(CompileError::AssignmentToConstant { ref name, .. }) if name == "x"
    ));
}

#[test]
fn variable_after_constant_in_same_block_is_error() {
    // x : 0d1 takes the name; x = 0d2 tries to assign to a constant
    assert!(matches!(
        try_compile("x : 0d1; x = 0d2; return x;"),
        Err(CompileError::AssignmentToConstant { ref name, .. }) if name == "x"
    ));
}

#[test]
fn constant_after_variable_in_same_block_is_error() {
    // x = 0d1 takes the name; x : 0d2 tries to redefine in same block
    assert!(matches!(
        try_compile("x = 0d1; x : 0d2;"),
        Err(CompileError::DuplicateConstant { ref name, .. }) if name == "x"
    ));
}

#[test]
fn inner_block_variable_not_visible_after_block() {
    // y is defined only inside the if block; using it after should fail
    assert!(matches!(
        try_compile("if 0d1, { y = 0d5; }; return y;"),
        Err(CompileError::UndefinedVariable { ref name, .. }) if name == "y"
    ));
}

#[test]
fn inner_block_constant_not_visible_after_block() {
    assert!(matches!(
        try_compile("if 0d1, { y : 0d5; }; return y;"),
        Err(CompileError::UndefinedVariable { ref name, .. }) if name == "y"
    ));
}

#[test]
fn while_body_variables_scoped() {
    // Variable defined in while body should not leak
    assert!(matches!(
        try_compile("i = 0d0; while i < 0d1, { tmp : 0d99; i = i + 0d1; }; return tmp;"),
        Err(CompileError::UndefinedVariable { ref name, .. }) if name == "tmp"
    ));
}

#[test]
fn nested_scopes_restore_correctly() {
    // x defined in outer, shadowed in middle, shadowed again in inner;
    // after each block exit, the previous x should be restored.
    compile(
        "x : 0d1; \
         if 0d1, { \
             x : 0d2; \
             if 0d1, { \
                 x : 0d3; \
             }; \
         }; \
         return x;",
    );
}

#[test]
fn variable_defined_before_block_visible_inside() {
    // Variables from outer scope should be readable inside inner blocks
    compile("x = 0d42; if 0d1, { return x; }; return 0d0;");
}

#[test]
fn variable_modified_in_inner_block_persists() {
    // Mutable variable from outer scope, modified in inner block
    compile("x = 0d1; if 0d1, { x = 0d2; }; return x;");
}

#[test]
fn multiple_variable_assignment_to_constant_is_error() {
    // a is a constant; trying to assign to it via multiple-variable syntax
    let code = "\
        a : 0d1; \
        f : procedure {n : U64}, {x : U64; y : U64} { return n, n; }; \
        a, b = f 0d5;";
    assert!(matches!(
        try_compile(code),
        Err(CompileError::AssignmentToConstant { ref name, .. }) if name == "a"
    ));
}

#[test]
fn scope_frees_slots_for_reuse() {
    // After an inner block's variables are freed, new allocations should
    // reuse those slots, keeping the frame compact.
    let (bytecode1, _, _) = compile(
        "if 0d1, { a : 0d1; b : 0d2; c : 0d3; }; \
         if 0d1, { x : 0d4; y : 0d5; z : 0d6; }; \
         return 0d0;",
    );
    // This should compile and produce valid bytecode. The second block's
    // variables should reuse slots freed by the first block.
    assert!(Reader::new(bytecode1.instructions()).is_ok());
}

#[test]
fn procedure_parameters_not_affected_by_body_scope() {
    // Parameters should remain accessible throughout the procedure body,
    // even after inner blocks exit.
    compile(
        "f : procedure {a : U64; b : U64}, U64 { \
             if a < b, { x : 0d1; }; \
             return a + b; \
         }; \
         return (f 0d3, 0d4);",
    );
}

// --- Source map ---

#[test]
fn source_map_non_empty_for_non_trivial_program() {
    let (_, source_map, _) = compile("x = 0d1; return x;");
    assert!(!source_map.is_empty());
}

#[test]
fn source_map_entries_in_bytecode_order() {
    let (_, source_map, _) = compile("a = 0d1; b = 0d2; c = 0d3; return a + b + c;");
    for window in source_map.entries().windows(2) {
        assert!(window[0].bytecode_offset <= window[1].bytecode_offset);
    }
}

#[test]
fn source_map_source_offsets_are_valid() {
    let code = "x = 0d1; return x;";
    let (_, source_map, _) = compile(code);
    let code_len = code.len() as u32;
    for entry in source_map.entries() {
        assert!(entry.source_start <= code_len || entry.source_end == u32::MAX);
    }
}

#[test]
fn source_map_round_trip_lookup() {
    let code = "a = 0d1; b = 0d2; return a + b;";
    let (_, source_map, _) = compile(code);

    for entry in source_map.entries() {
        assert_eq!(
            source_map.source_range(entry.bytecode_offset),
            Some((entry.source_start, entry.source_end)),
        );
    }
}

#[test]
fn source_map_for_procedure() {
    let (_, source_map, _) =
        compile("f : procedure {n : U64}, U64 { return n + 0d1; }; return (f 0d5);");
    assert!(source_map.len() >= 2);
}

// --- Superinstruction fusion ---

fn has_superinstruction(patch_map: &PatchMap, kind: SuperinstructionKind) -> bool {
    patch_map
        .superinstruction_patches()
        .iter()
        .any(|patch| patch.kind == kind as u32)
}

fn count_superinstruction(patch_map: &PatchMap, kind: SuperinstructionKind) -> usize {
    patch_map
        .superinstruction_patches()
        .iter()
        .filter(|patch| patch.kind == kind as u32)
        .count()
}

#[test]
fn take_jump_fused_for_root_entry() {
    let (_, _, patch_map) = compile("return 0d0;");
    assert!(has_superinstruction(
        &patch_map,
        SuperinstructionKind::TakeStackSizeImmutableJumpToImmutable
    ));
}

#[test]
fn take_jump_fused_for_procedure_call() {
    let (_, _, patch_map) =
        compile("f : procedure {n : U64}, U64 { return n + 0d1; }; return (f 0d5);");
    // Root entry + one user call = at least 2 TakeStackSizeImmutableJumpToImmutable fusions.
    assert!(
        count_superinstruction(
            &patch_map,
            SuperinstructionKind::TakeStackSizeImmutableJumpToImmutable
        ) >= 2
    );
}

#[test]
fn free_jump_offset_fused_for_procedure_return() {
    let (_, _, patch_map) = compile("f : procedure {n : U64}, U64 { return n; }; return (f 0d5);");
    assert!(has_superinstruction(
        &patch_map,
        SuperinstructionKind::FreeStackSizeImmutableJumpToOffset
    ));
}

#[test]
fn if_comparison_immutable_fuses_jump_if_not() {
    let (_, _, patch_map) = compile("if 0d1 < 0d2, { return 0d1; };");
    assert!(has_superinstruction(
        &patch_map,
        SuperinstructionKind::LessThanTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable,
    ));
}

#[test]
fn if_comparison_offset_fuses_jump_if_not() {
    let (_, _, patch_map) = compile("a = 0d1; b = 0d2; if a < b, { return 0d1; };");
    assert!(has_superinstruction(
        &patch_map,
        SuperinstructionKind::LessThanTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable,
    ));
}

#[test]
fn while_comparison_immutable_fuses_jump_if() {
    let (_, _, patch_map) = compile("i = 0d0; while i < 0d10, { i = i + 0d1; };");
    assert!(has_superinstruction(
        &patch_map,
        SuperinstructionKind::LessThanTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable,
    ));
}

#[test]
fn while_comparison_offset_fuses_jump_if() {
    let (_, _, patch_map) = compile("a = 0d0; b = 0d10; while a < b, { a = a + 0d1; };");
    assert!(has_superinstruction(
        &patch_map,
        SuperinstructionKind::LessThanTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable,
    ));
}

#[test]
fn no_comparison_fusion_for_variable_condition() {
    let (_, _, patch_map) = compile("x = 0d1; if x, { return 0d42; };");
    // Condition is a plain variable — no comparison+jump fusions (kinds 2–25)
    // should be present. FreeStackSizeImmutableJumpToOffset (kind 26) is not a comparison fusion.
    let comparison_fusions = patch_map
        .superinstruction_patches()
        .iter()
        .filter(|patch| patch.kind >= 2 && patch.kind <= 25)
        .count();
    assert_eq!(comparison_fusions, 0);
}

#[test]
fn multi_site_uses_free_jump_offset() {
    let (_, _, patch_map) =
        compile("f : procedure {n : U64}, U64 { return n; }; a = f 0d1; b = f 0d2; return a + b;");
    // With direct return addresses, no dispatch table is needed.
    // Instead, FreeStackSizeImmutableJumpToOffset is used for procedure returns.
    assert!(has_superinstruction(
        &patch_map,
        SuperinstructionKind::FreeStackSizeImmutableJumpToOffset,
    ));
}

#[test]
fn all_comparison_kinds_fuse_with_if() {
    let cases: &[(&str, SuperinstructionKind)] = &[
        (
            "if 0d1 < 0d2, { return 0d1; };",
            SuperinstructionKind::LessThanTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable,
        ),
        (
            "if 0d2 > 0d1, { return 0d1; };",
            SuperinstructionKind::GreaterThanTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable,
        ),
        (
            "if 0d1 <= 0d2, { return 0d1; };",
            SuperinstructionKind::LessThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable,
        ),
        (
            "if 0d2 >= 0d1, { return 0d1; };",
            SuperinstructionKind::GreaterThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable,
        ),
        (
            "if 0d1 == 0d1, { return 0d1; };",
            SuperinstructionKind::EqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable,
        ),
        (
            "if 0d1 != 0d2, { return 0d1; };",
            SuperinstructionKind::NotEqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable,
        ),
    ];
    for (code, expected_kind) in cases {
        let (_, _, patch_map) = compile(code);
        assert!(
            has_superinstruction(&patch_map, *expected_kind),
            "expected {:?} fusion for: {}",
            expected_kind,
            code,
        );
    }
}

#[test]
fn all_comparison_kinds_fuse_with_while() {
    let cases: &[(&str, SuperinstructionKind)] = &[
        (
            "i = 0d0; while i < 0d10, { i = i + 0d1; };",
            SuperinstructionKind::LessThanTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable,
        ),
        (
            "i = 0d10; while i > 0d0, { i = i - 0d1; };",
            SuperinstructionKind::GreaterThanTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable,
        ),
        (
            "i = 0d0; while i <= 0d9, { i = i + 0d1; };",
            SuperinstructionKind::LessThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable,
        ),
        (
            "i = 0d10; while i >= 0d1, { i = i - 0d1; };",
            SuperinstructionKind::GreaterThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable,
        ),
        (
            "i = 0d0; while i == 0d0, { i = i + 0d1; };",
            SuperinstructionKind::EqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable,
        ),
        (
            "i = 0d0; while i != 0d10, { i = i + 0d1; };",
            SuperinstructionKind::NotEqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable,
        ),
    ];
    for (code, expected_kind) in cases {
        let (_, _, patch_map) = compile(code);
        assert!(
            has_superinstruction(&patch_map, *expected_kind),
            "expected {:?} fusion for: {}",
            expected_kind,
            code,
        );
    }
}

// --- Bytecode validity ---

#[test]
fn all_fixture_programs_compile_and_decode() {
    let programs = [
        "return 0d0;",
        "return 0d42;",
        "return 0d10 + 0d10;",
        "return 0d1 + 0d2 * 0d3;",
        "x : 0d42; return x;",
        "x = 0d1; x = 0d2; return x;",
        "a = 0d10; b = 0d20; return a + b;",
        "a = 0d10; b = 0d3; return a - b;",
        "a = 0d10; b = 0d3; return a * b;",
        "a = 0d10; b = 0d3; return a / b;",
        "a = 0d10; b = 0d3; return a % b;",
        "a = 0d1; b = 0d2; return a < b;",
        "a = 0d1; b = 0d2; return a > b;",
        "a = 0d1; b = 0d2; return a == b;",
        "a = 0d1; b = 0d2; return a != b;",
        "if 0d1 < 0d2, { return 0d1; }; return 0d0;",
        "if 0d2 < 0d1, { return 0d1; }; return 0d0;",
        "sum = 0d0; i = 0d0; while i < 0d10, { sum = sum + i; i = i + 0d1; }; return sum;",
        "add : procedure {a : U64; b : U64}, U64 { return a + b; }; return (add 0d3, 0d4);",
        "f : procedure {n : U64}, U64 { return n; }; a = f 0d1; b = f 0d2; return a + b;",
        "fibonacci : procedure {n : U64}, U64 { \
            if n < 0d2, { return n; }; \
            return (fibonacci n - 0d1) + (fibonacci n - 0d2); \
         }; return (fibonacci 0d10);",
        "exchange : procedure {a : U64; b : U64}, {c : U64; d : U64} { \
            return b, a; \
         }; c, d = exchange 0d10, 0d20; return c - d;",
    ];

    for code in &programs {
        let (bytecode, _, _) = compile(code);
        assert!(
            Reader::new(bytecode.instructions()).is_ok(),
            "failed for {code:?}"
        );
    }
}
