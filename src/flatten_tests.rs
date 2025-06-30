use std::{fs, io::Error};

use serde_json;

use tree_sitter::Parser;
use tree_sitter_mage::LANGUAGE;

use crate::{FlatRoot, flatten_tree};

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
    let root = flatten_tree(tree, pair.mage.as_str()).unwrap();

    assert_eq!(reference, root);
    Ok(())
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
