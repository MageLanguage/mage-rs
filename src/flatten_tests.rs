use std::{fs, io::Error};

use serde_json;

use tree_sitter::Parser;
use tree_sitter_mage::LANGUAGE;

use crate::{Error as MageError, FlatRoot, flatten_tree, process_tree};

pub struct TestDirectoryIterator {
    path: String,
    current: usize,
    number: usize,
}

pub struct Pair {
    mage: String,
    json: String,
}

fn test_directory_iterator(path: &str, number: usize) -> TestDirectoryIterator {
    TestDirectoryIterator {
        path: path.to_string(),
        current: 0,
        number: number,
    }
}

impl Iterator for TestDirectoryIterator {
    type Item = Result<Pair, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.number {
            self.current += 1;

            let mage = match fs::read_to_string(format!("src/{}/{}.mg", self.path, self.current)) {
                Ok(mage) => mage,
                Err(error) => return Some(Err(error)),
            };

            let json = match fs::read_to_string(format!("src/{}/{}.json", self.path, self.current))
            {
                Ok(json) => json,
                Err(error) => return Some(Err(error)),
            };

            Some(Ok(Pair {
                mage: mage,
                json: json,
            }))
        } else {
            None
        }
    }
}

fn test(pair: &Pair) -> Result<(), Error> {
    let reference = serde_json::from_str::<FlatRoot>(pair.json.as_str()).unwrap();

    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE.into()).unwrap();

    let tree = parser.parse(pair.mage.as_str(), None).unwrap();
    let root = flatten_tree(&tree, pair.mage.as_str()).unwrap();

    assert_eq!(reference, root);
    Ok(())
}

fn test_validation_failure(pair: &Pair) -> Result<(), Error> {
    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE.into()).unwrap();

    let tree = parser.parse(pair.mage.as_str(), None).unwrap();

    // Expect validation to fail
    match process_tree(&tree, pair.mage.as_str()) {
        Err(MageError::ValidationError(_)) => Ok(()), // Expected validation error
        Ok(_) => panic!("Expected validation error but processing succeeded"),
        Err(other) => panic!("Expected validation error but got: {:?}", other),
    }
}

#[test]
fn test_expression() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/expression", 8) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_expression_with_high_precedence() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/expression_with_high_precedence", 8) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_expression_with_leading_zero_omitted() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/expression_with_leading_zero_omitted", 11) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_expression_with_multiple_arithmetic() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/expression_with_multiple_arithmetic", 11) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_expression_with_prioritize() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/expression_with_prioritize", 9) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_expression_with_identifier_chain_with_calls() -> Result<(), Error> {
    for pair in test_directory_iterator(
        "flatten_tests/expression_with_identifier_chain_with_calls",
        6,
    ) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_expression_with_identifier_chain_with_calls_with_arguments() -> Result<(), Error> {
    for pair in test_directory_iterator(
        "flatten_tests/expression_with_identifier_chain_with_calls_with_arguments",
        4,
    ) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_empty_expressions() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/empty_expressions", 2) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_unflattened_function_arguments() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/unflattened_function_arguments", 1) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_multiple_statement_chains() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/multiple_statement_chains", 1) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_invalid_number_formats_validation() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/invalid_number_formats", 1) {
        test_validation_failure(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_inconsistent_call_extraction() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/inconsistent_call_extraction", 1) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_malformed_syntax() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/malformed_syntax", 1) {
        test(&pair?)?;
    }

    Ok(())
}

#[test]
fn test_deeply_nested_calls() -> Result<(), Error> {
    for pair in test_directory_iterator("flatten_tests/deeply_nested_calls", 1) {
        test(&pair?)?;
    }

    Ok(())
}
