use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Write};

#[cfg(test)]
#[path = "transport_test.rs"]
mod transport_test;

const JSON_RPC_VERSION: &str = "2.0";
const CONTENT_LENGTH_HEADER: &str = "Content-Length: ";
const HEADER_TERMINATOR: &str = "\r\n\r\n";
const MISSING_CONTENT_LENGTH: &str = "Missing Content-Length";
const UNEXPECTED_EOF: &str = "EOF";

#[derive(Debug, Deserialize)]
pub struct Message {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    pub id: Option<Value>,
    pub method: Option<String>,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct Response {
    jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
}

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION,
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION,
            id,
            result: None,
            error: Some(ResponseError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct Notification {
    jsonrpc: &'static str,
    pub method: String,
    pub params: Value,
}

impl Notification {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION,
            method: method.into(),
            params,
        }
    }
}

pub fn read_message(reader: &mut impl BufRead) -> io::Result<String> {
    let content_length = read_message_content_length(reader)?;
    let mut content = vec![0u8; content_length];
    reader.read_exact(&mut content)?;
    decode_message_content(content)
}

pub fn send_message(message: &impl Serialize) -> io::Result<()> {
    let content = serde_json::to_string(message)?;
    let framed_message = frame_message(&content);

    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(framed_message.as_bytes())?;
    stdout.flush()
}

fn read_message_content_length(reader: &mut impl BufRead) -> io::Result<usize> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, UNEXPECTED_EOF));
        }

        let header_line = line.trim();
        if header_line.is_empty() {
            break;
        }

        if let Some(value) = header_line.strip_prefix(CONTENT_LENGTH_HEADER) {
            content_length = value.parse().ok();
        }
    }

    content_length.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, MISSING_CONTENT_LENGTH))
}

fn decode_message_content(bytes: Vec<u8>) -> io::Result<String> {
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn frame_message(content: &str) -> String {
    format!(
        "{}{}{}{}",
        CONTENT_LENGTH_HEADER,
        content.len(),
        HEADER_TERMINATOR,
        content
    )
}
