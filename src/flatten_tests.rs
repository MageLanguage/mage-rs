use crate::{BinaryOp, Instruction, Literal, NodeKinds, Operand, Tac, flatten_tree};
use tree_sitter::{Language, Parser};
use tree_sitter_mage::LANGUAGE;

#[test]
fn test_simple_arithmetic() {
    let code = "a * (b + 0d5)";
    let language = Language::from(LANGUAGE);
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(code, None).unwrap();
    let node_kinds = NodeKinds::new(&language);

    let tac = flatten_tree(&node_kinds, &tree, code).unwrap();

    let expected_instructions = vec![
        Instruction::Binary {
            op: BinaryOp::Add,
            dest: Operand::Temp(0),
            src1: Operand::Identifier("b".to_string()),
            src2: Operand::Literal(Literal::Integer(5)),
        },
        Instruction::Binary {
            op: BinaryOp::Mul,
            dest: Operand::Temp(1),
            src1: Operand::Identifier("a".to_string()),
            src2: Operand::Temp(0),
        },
    ];

    assert_eq!(tac.instructions, expected_instructions);
}

#[test]
fn test_unary_negation() {
    let code = "-a";
    let language = Language::from(LANGUAGE);
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(code, None).unwrap();
    let node_kinds = NodeKinds::new(&language);

    let tac = flatten_tree(&node_kinds, &tree, code).unwrap();

    let expected_instructions = vec![crate::Instruction::Unary {
        op: crate::UnaryOp::Negate,
        dest: Operand::Temp(0),
        src: Operand::Identifier("a".to_string()),
    }];

    assert_eq!(tac.instructions, expected_instructions);
}

#[test]
fn test_comparison() {
    let code = "a > b";
    let language = Language::from(LANGUAGE);
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(code, None).unwrap();
    let node_kinds = NodeKinds::new(&language);

    let tac = flatten_tree(&node_kinds, &tree, code).unwrap();

    let expected_instructions = vec![Instruction::Binary {
        op: BinaryOp::Gt,
        dest: Operand::Temp(0),
        src1: Operand::Identifier("a".to_string()),
        src2: Operand::Identifier("b".to_string()),
    }];

    assert_eq!(tac.instructions, expected_instructions);
}

#[test]
fn test_logical_and() {
    let code = "a && b";
    let language = Language::from(LANGUAGE);
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(code, None).unwrap();
    let node_kinds = NodeKinds::new(&language);

    let tac = flatten_tree(&node_kinds, &tree, code).unwrap();

    let expected_instructions = vec![
        Instruction::JumpIfZero {
            label: ".L0".to_string(),
            src: Operand::Identifier("a".to_string()),
        },
        Instruction::Binary {
            op: BinaryOp::Neq,
            dest: Operand::Temp(0),
            src1: Operand::Identifier("b".to_string()),
            src2: Operand::Literal(Literal::Integer(0)),
        },
        Instruction::Jump(".L1".to_string()),
        Instruction::Label(".L0".to_string()),
        Instruction::Assign {
            dest: Operand::Temp(0),
            src: Operand::Literal(Literal::Integer(0)),
        },
        Instruction::Label(".L1".to_string()),
    ];

    assert_eq!(tac.instructions, expected_instructions);
}

#[test]
fn test_logical_or() {
    let code = "a || b";
    let language = Language::from(LANGUAGE);
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(code, None).unwrap();
    let node_kinds = NodeKinds::new(&language);

    let tac = flatten_tree(&node_kinds, &tree, code).unwrap();

    let expected_instructions = vec![
        Instruction::JumpIfZero {
            label: ".L0".to_string(),
            src: Operand::Identifier("a".to_string()),
        },
        Instruction::Assign {
            dest: Operand::Temp(0),
            src: Operand::Literal(Literal::Integer(1)),
        },
        Instruction::Jump(".L1".to_string()),
        Instruction::Label(".L0".to_string()),
        Instruction::Binary {
            op: BinaryOp::Neq,
            dest: Operand::Temp(0),
            src1: Operand::Identifier("b".to_string()),
            src2: Operand::Literal(Literal::Integer(0)),
        },
        Instruction::Label(".L1".to_string()),
    ];

    assert_eq!(tac.instructions, expected_instructions);
}

#[test]
fn test_variable_assignment() {
    let code = "a = b";
    let language = Language::from(LANGUAGE);
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(code, None).unwrap();
    let node_kinds = NodeKinds::new(&language);

    let tac = flatten_tree(&node_kinds, &tree, code).unwrap();

    let expected_instructions = vec![Instruction::Assign {
        dest: Operand::Identifier("a".to_string()),
        src: Operand::Identifier("b".to_string()),
    }];

    assert_eq!(tac.instructions, expected_instructions);
}

#[test]
fn test_constant_assignment() {
    let code = "a : b";
    let language = Language::from(LANGUAGE);
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(code, None).unwrap();
    let node_kinds = NodeKinds::new(&language);

    let tac = flatten_tree(&node_kinds, &tree, code).unwrap();

    let expected_instructions = vec![Instruction::Assign {
        dest: Operand::Identifier("a".to_string()),
        src: Operand::Identifier("b".to_string()),
    }];

    assert_eq!(tac.instructions, expected_instructions);
}

#[test]
fn test_call() {
    let code = "a => b";
    let language = Language::from(LANGUAGE);
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(code, None).unwrap();
    let node_kinds = NodeKinds::new(&language);

    let tac = flatten_tree(&node_kinds, &tree, code).unwrap();

    let expected_instructions = vec![Instruction::Call {
        ret: Some(Operand::Temp(0)),
        target: Operand::Identifier("b".to_string()),
        args: vec![Operand::Identifier("a".to_string())],
    }];

    assert_eq!(tac.instructions, expected_instructions);
}

#[test]
fn test_member_access() {
    let code = "a.b";
    let language = Language::from(LANGUAGE);
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(code, None).unwrap();
    let node_kinds = NodeKinds::new(&language);

    let tac = flatten_tree(&node_kinds, &tree, code).unwrap();

    let expected_instructions = vec![Instruction::Member {
        dest: Operand::Temp(0),
        base: Operand::Identifier("a".to_string()),
        member: Operand::Identifier("b".to_string()),
    }];

    assert_eq!(tac.instructions, expected_instructions);
}
