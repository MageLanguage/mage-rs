use std::io::Cursor;

use super::{Notification, Response, frame_message, read_message};
use serde_json::{Value, json};

fn frame_message_for_test(content: &str) -> String {
    frame_message(content)
}

#[test]
fn read_message_parses_content_length_framed_input() {
    let content = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let input = frame_message_for_test(content);
    let mut reader = Cursor::new(input.as_bytes());

    let result = read_message(&mut reader).unwrap();
    assert_eq!(result, content);
}

#[test]
fn read_message_returns_eof_on_empty_input() {
    let mut reader = Cursor::new(b"" as &[u8]);
    let error = read_message(&mut reader).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
}

#[test]
fn read_message_returns_error_on_missing_content_length() {
    let input = "Some-Header: value\r\n\r\n{}";
    let mut reader = Cursor::new(input.as_bytes());
    let error = read_message(&mut reader).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn read_message_handles_multiple_headers() {
    let content = r#"{"jsonrpc":"2.0"}"#;
    let input = format!(
        "{}{}{}\r\n\r\n{}",
        "Content-Type: application/json\r\nContent-Length: ",
        content.len(),
        "",
        content
    );
    let mut reader = Cursor::new(input.as_bytes());

    let result = read_message(&mut reader).unwrap();
    assert_eq!(result, content);
}

#[test]
fn read_message_reads_two_consecutive_messages() {
    let first = r#"{"jsonrpc":"2.0","id":1}"#;
    let second = r#"{"jsonrpc":"2.0","id":2}"#;
    let input = format!(
        "{}{}",
        frame_message_for_test(first),
        frame_message_for_test(second)
    );
    let mut reader = Cursor::new(input.as_bytes());

    assert_eq!(read_message(&mut reader).unwrap(), first);
    assert_eq!(read_message(&mut reader).unwrap(), second);
}

#[test]
fn response_ok_sets_version_and_result() {
    let response = Response::ok(Value::from(1), json!({"key": "value"}));
    let serialized = serde_json::to_value(&response).unwrap();

    assert_eq!(serialized["jsonrpc"], "2.0");
    assert_eq!(serialized["id"], 1);
    assert_eq!(serialized["result"]["key"], "value");
    assert!(serialized.get("error").is_none());
}

#[test]
fn response_error_sets_version_and_error() {
    let response = Response::error(Value::from(1), -32601, "Method not found");
    let serialized = serde_json::to_value(&response).unwrap();

    assert_eq!(serialized["jsonrpc"], "2.0");
    assert_eq!(serialized["id"], 1);
    assert_eq!(serialized["error"]["code"], -32601);
    assert_eq!(serialized["error"]["message"], "Method not found");
    assert!(serialized.get("result").is_none());
}

#[test]
fn notification_new_sets_version_and_method() {
    let notification = Notification::new(
        "textDocument/publishDiagnostics",
        json!({"uri": "file:///test.hex"}),
    );
    let serialized = serde_json::to_value(&notification).unwrap();

    assert_eq!(serialized["jsonrpc"], "2.0");
    assert_eq!(serialized["method"], "textDocument/publishDiagnostics");
    assert_eq!(serialized["params"]["uri"], "file:///test.hex");
}
