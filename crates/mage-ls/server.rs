use serde_json::{Value, json};
use std::{collections::HashMap, io};

use crate::document::{
    self, DocumentState, compute_diagnostics, content_change_text, extract_statement_text,
    find_definition_at_offset, find_identifier_at_offset, lsp_position_to_offset,
    offset_to_lsp_position, text_document_position, text_document_uri,
};
use crate::transport::{self, Message, Notification, Response};

#[cfg(test)]
#[path = "server_test.rs"]
mod server_test;

const SERVER_NAME: &str = "mage-language-server";
const SERVER_VERSION: &str = "0.1.0";
const METHOD_NOT_FOUND: i32 = -32601;
const EXIT_NOTIFICATION: &str = "exit";
const INITIALIZED_NOTIFICATION: &str = "initialized";
const DID_OPEN_NOTIFICATION: &str = "textDocument/didOpen";
const DID_CHANGE_NOTIFICATION: &str = "textDocument/didChange";
const DID_CLOSE_NOTIFICATION: &str = "textDocument/didClose";
const DID_SAVE_NOTIFICATION: &str = "textDocument/didSave";
const INITIALIZE_REQUEST: &str = "initialize";
const SHUTDOWN_REQUEST: &str = "shutdown";
const HOVER_REQUEST: &str = "textDocument/hover";
const DEFINITION_REQUEST: &str = "textDocument/definition";

pub struct LanguageServer {
    documents: HashMap<String, DocumentState>,
}

impl LanguageServer {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut stdin = stdin.lock();

        loop {
            let content = match transport::read_message(&mut stdin) {
                Ok(content) => content,
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(error) => return Err(error),
            };

            let message: Message = match serde_json::from_str(&content) {
                Ok(message) => message,
                Err(_) => continue,
            };

            let Some(method) = message.method.as_deref() else {
                continue;
            };

            if let Some(id) = message.id.as_ref() {
                let response = self.handle_request(method, id, &message.params);
                transport::send_message(&response)?;
                continue;
            }

            self.handle_notification(method, &message.params)?;
            if method == EXIT_NOTIFICATION {
                break;
            }
        }

        Ok(())
    }

    fn handle_notification(&mut self, method: &str, parameters: &Value) -> io::Result<()> {
        match method {
            INITIALIZED_NOTIFICATION | EXIT_NOTIFICATION => Ok(()),
            DID_OPEN_NOTIFICATION => self.handle_did_open(parameters),
            DID_CHANGE_NOTIFICATION => self.handle_did_change(parameters),
            DID_CLOSE_NOTIFICATION => self.handle_did_close(parameters),
            DID_SAVE_NOTIFICATION => self.handle_did_save(parameters),
            _ => Ok(()),
        }
    }

    fn handle_request(&mut self, method: &str, id: &Value, parameters: &Value) -> Response {
        match method {
            INITIALIZE_REQUEST => self.ok_response(id, self.initialize_result()),
            SHUTDOWN_REQUEST => self.null_response(id),
            HOVER_REQUEST => {
                self.ok_response(id, self.handle_hover(parameters).unwrap_or(Value::Null))
            }
            DEFINITION_REQUEST => self.ok_response(
                id,
                self.handle_definition(parameters).unwrap_or(Value::Null),
            ),
            _ if method.starts_with("$/") => self.null_response(id),
            _ => Response::error(
                id.clone(),
                METHOD_NOT_FOUND,
                format!("Method not found: {}", method),
            ),
        }
    }

    fn handle_did_open(&mut self, parameters: &Value) -> io::Result<()> {
        let Some(uri) = text_document_uri(parameters) else {
            return Ok(());
        };
        let Some(text) = document::nested_string(parameters, &["textDocument", "text"]) else {
            return Ok(());
        };

        let document = DocumentState::new(text);
        let diagnostics = compute_diagnostics(&document);
        self.documents.insert(uri.clone(), document);
        self.publish_diagnostics(&uri, diagnostics)
    }

    fn handle_did_change(&mut self, parameters: &Value) -> io::Result<()> {
        let Some(uri) = text_document_uri(parameters) else {
            return Ok(());
        };
        let Some(text) = content_change_text(parameters) else {
            return Ok(());
        };
        let Some(document) = self.documents.get_mut(&uri) else {
            return Ok(());
        };

        document.update(text);
        let diagnostics = compute_diagnostics(document);
        self.publish_diagnostics(&uri, diagnostics)
    }

    fn handle_did_close(&mut self, parameters: &Value) -> io::Result<()> {
        let Some(uri) = text_document_uri(parameters) else {
            return Ok(());
        };

        self.documents.remove(&uri);
        self.publish_diagnostics(&uri, Vec::new())
    }

    fn handle_did_save(&mut self, parameters: &Value) -> io::Result<()> {
        let Some(uri) = text_document_uri(parameters) else {
            return Ok(());
        };
        let Some(document) = self.documents.get(&uri) else {
            return Ok(());
        };

        let diagnostics = compute_diagnostics(document);
        self.publish_diagnostics(&uri, diagnostics)
    }

    fn initialize_result(&self) -> Value {
        json!({
            "capabilities": {
                "textDocumentSync": 1,
                "hoverProvider": true,
                "definitionProvider": true,
                "completionProvider": Value::Null,
                "referencesProvider": false,
                "documentSymbolProvider": false,
                "workspaceSymbolProvider": false,
                "codeActionProvider": false,
                "documentFormattingProvider": false,
                "renameProvider": false
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION
            }
        })
    }

    fn ok_response(&self, id: &Value, result: Value) -> Response {
        Response::ok(id.clone(), result)
    }

    fn null_response(&self, id: &Value) -> Response {
        self.ok_response(id, Value::Null)
    }

    fn markdown_response(&self, value: impl Into<String>) -> Value {
        json!({
            "contents": {
                "kind": "markdown",
                "value": value.into()
            }
        })
    }

    fn handle_hover(&self, parameters: &Value) -> Option<Value> {
        let uri = text_document_uri(parameters)?;
        let (line, character) = text_document_position(parameters)?;

        let document = self.documents.get(&uri)?;
        let offset = lsp_position_to_offset(&document.line_index, line, character)?;
        let identifier = find_identifier_at_offset(&document.source, offset)?;

        let definition = find_definition_at_offset(document, identifier, offset)?;
        let statement_text = extract_statement_text(
            document,
            definition.source_index,
            definition.statement_index,
        );

        let hover_text = format!(
            "**{}** `{}`\n\n```mage\n{}\n```",
            definition.kind, identifier, statement_text
        );

        Some(self.markdown_response(hover_text))
    }

    fn handle_definition(&self, parameters: &Value) -> Option<Value> {
        let uri = text_document_uri(parameters)?;
        let (line, character) = text_document_position(parameters)?;

        let document = self.documents.get(&uri)?;
        let offset = lsp_position_to_offset(&document.line_index, line, character)?;
        let identifier = find_identifier_at_offset(&document.source, offset)?;

        let definition = find_definition_at_offset(document, identifier, offset)?;

        let definition_offset = definition.source_offset as usize;
        let (definition_line, definition_character) =
            offset_to_lsp_position(&document.line_index, definition_offset);
        let end_character = definition_character + identifier.len() as u32;

        Some(json!({
            "uri": uri,
            "range": {
                "start": { "line": definition_line, "character": definition_character },
                "end": { "line": definition_line, "character": end_character }
            }
        }))
    }

    fn publish_diagnostics(&self, uri: &str, diagnostics: Vec<Value>) -> io::Result<()> {
        transport::send_message(&Notification::new(
            "textDocument/publishDiagnostics",
            json!({
                "uri": uri,
                "diagnostics": diagnostics
            }),
        ))
    }
}
