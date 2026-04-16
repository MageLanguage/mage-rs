use serde_json::{Value, json};

use super::LanguageServer;

fn open_document(language_server: &mut LanguageServer, uri: &str, source: &str) {
    let parameters = json!({
        "textDocument": {
            "uri": uri,
            "languageId": "mage",
            "version": 1,
            "text": source
        }
    });

    let _ = language_server.handle_notification("textDocument/didOpen", &parameters);
}

fn hover_parameters(uri: &str, line: u32, character: u32) -> Value {
    json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character }
    })
}

fn definition_parameters(uri: &str, line: u32, character: u32) -> Value {
    json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character }
    })
}

fn request_id() -> Value {
    Value::from(1)
}

// --- Initialize ---

#[test]
fn initialize_returns_capabilities() {
    let mut language_server = LanguageServer::new();
    let request_id = request_id();

    let response = language_server.handle_request("initialize", &request_id, &json!({}));
    let result = response.result.unwrap();

    assert_eq!(result["capabilities"]["hoverProvider"], true);
    assert_eq!(result["capabilities"]["definitionProvider"], true);
    assert_eq!(result["capabilities"]["textDocumentSync"], 1);
    assert_eq!(result["serverInfo"]["name"], "mage-language-server");
    assert_eq!(result["serverInfo"]["version"], "0.1.0");
}

// --- Shutdown ---

#[test]
fn shutdown_returns_null_result() {
    let mut language_server = LanguageServer::new();
    let request_id = request_id();

    let response = language_server.handle_request("shutdown", &request_id, &json!({}));

    assert_eq!(response.result, Some(Value::Null));
}

// --- Unknown method ---

#[test]
fn unknown_method_returns_error() {
    let mut language_server = LanguageServer::new();
    let request_id = request_id();

    let response = language_server.handle_request("unknownMethod", &request_id, &json!({}));

    assert!(response.error.is_some());
    let response_error = response.error.unwrap();
    assert_eq!(response_error.code, -32601);
    assert!(response_error.message.contains("unknownMethod"));
}

#[test]
fn dollar_prefixed_method_returns_null_without_error() {
    let mut language_server = LanguageServer::new();
    let request_id = request_id();

    let response = language_server.handle_request("$/cancelRequest", &request_id, &json!({}));

    assert_eq!(response.result, Some(Value::Null));
    assert!(response.error.is_none());
}

// --- Hover ---

#[test]
fn hover_returns_null_for_procedure_without_definition() {
    let mut language_server = LanguageServer::new();
    let uri = "file:///test.hex";
    open_document(
        &mut language_server,
        uri,
        "add : procedure {x : U64}, U64 { return x; };",
    );

    let parameters = hover_parameters(uri, 0, 7);
    let response = language_server.handle_request("textDocument/hover", &request_id(), &parameters);

    assert_eq!(response.result, Some(Value::Null));
}

#[test]
fn hover_returns_definition_text_for_user_constant() {
    let mut language_server = LanguageServer::new();
    let uri = "file:///test.hex";
    open_document(&mut language_server, uri, "value : 0d42; return value;");

    let offset = "value : 0d42; return ".len();
    let parameters = hover_parameters(uri, 0, offset as u32);
    let response = language_server.handle_request("textDocument/hover", &request_id(), &parameters);
    let result = response.result.unwrap();

    let contents = result["contents"]["value"].as_str().unwrap();
    assert!(contents.contains("**constant** `value`"));
    assert!(contents.contains("value : 0d42"));
}

#[test]
fn hover_returns_null_for_unknown_position() {
    let mut language_server = LanguageServer::new();
    let uri = "file:///test.hex";
    open_document(&mut language_server, uri, "x = 0d1;");

    let parameters = hover_parameters(uri, 0, 3);
    let response = language_server.handle_request("textDocument/hover", &request_id(), &parameters);

    assert_eq!(response.result, Some(Value::Null));
}

#[test]
fn hover_returns_null_for_unknown_document() {
    let mut language_server = LanguageServer::new();

    let parameters = hover_parameters("file:///missing.hex", 0, 0);
    let response = language_server.handle_request("textDocument/hover", &request_id(), &parameters);

    assert_eq!(response.result, Some(Value::Null));
}

#[test]
fn hover_classifies_procedure_definitions() {
    let mut language_server = LanguageServer::new();
    let uri = "file:///test.hex";
    open_document(
        &mut language_server,
        uri,
        "identity : procedure { x : U64 }, U64 { return x; }; identity 0d1;",
    );

    let offset = "identity : procedure { x : U64 }, U64 { return x; }; ".len();
    let parameters = hover_parameters(uri, 0, offset as u32);
    let response = language_server.handle_request("textDocument/hover", &request_id(), &parameters);
    let result = response.result.unwrap();

    let contents = result["contents"]["value"].as_str().unwrap();
    assert!(contents.contains("**procedure** `identity`"));
}

// --- Definition ---

#[test]
fn definition_returns_location_for_constant() {
    let mut language_server = LanguageServer::new();
    let uri = "file:///test.hex";
    open_document(&mut language_server, uri, "value : 0d1; return value;");

    let offset = "value : 0d1; return ".len();
    let parameters = definition_parameters(uri, 0, offset as u32);
    let response =
        language_server.handle_request("textDocument/definition", &request_id(), &parameters);
    let result = response.result.unwrap();

    assert_eq!(result["uri"], uri);
    assert_eq!(result["range"]["start"]["line"], 0);
    assert_eq!(result["range"]["start"]["character"], 0);
}

#[test]
fn definition_returns_null_for_undefined_name() {
    let mut language_server = LanguageServer::new();
    let uri = "file:///test.hex";
    open_document(&mut language_server, uri, "return unknown;");

    let offset = "return ".len();
    let parameters = definition_parameters(uri, 0, offset as u32);
    let response =
        language_server.handle_request("textDocument/definition", &request_id(), &parameters);

    assert_eq!(response.result, Some(Value::Null));
}

#[test]
fn definition_returns_null_for_unknown_document() {
    let mut language_server = LanguageServer::new();

    let parameters = definition_parameters("file:///missing.hex", 0, 0);
    let response =
        language_server.handle_request("textDocument/definition", &request_id(), &parameters);

    assert_eq!(response.result, Some(Value::Null));
}

// --- Document sync ---

#[test]
fn did_change_updates_document_state() {
    let mut language_server = LanguageServer::new();
    let uri = "file:///test.hex";
    open_document(&mut language_server, uri, "x = 0d1;");

    let change_parameters = json!({
        "textDocument": { "uri": uri },
        "contentChanges": [
            { "text": "new_value : 0d42; return new_value;" }
        ]
    });
    let _ = language_server.handle_notification("textDocument/didChange", &change_parameters);

    let offset = "new_value : 0d42; return ".len();
    let parameters = hover_parameters(uri, 0, offset as u32);
    let response = language_server.handle_request("textDocument/hover", &request_id(), &parameters);
    let result = response.result.unwrap();

    let contents = result["contents"]["value"].as_str().unwrap();
    assert!(contents.contains("new_value"));
}

#[test]
fn did_close_removes_document() {
    let mut language_server = LanguageServer::new();
    let uri = "file:///test.hex";
    open_document(&mut language_server, uri, "x = 0d1;");

    let close_parameters = json!({
        "textDocument": { "uri": uri }
    });
    let _ = language_server.handle_notification("textDocument/didClose", &close_parameters);

    let parameters = hover_parameters(uri, 0, 0);
    let response = language_server.handle_request("textDocument/hover", &request_id(), &parameters);

    assert_eq!(response.result, Some(Value::Null));
}

// --- Hover without hardcoded builtins ---

#[test]
fn hover_returns_null_for_runtime_specific_builtin_like_name_without_definition() {
    let mut language_server = LanguageServer::new();
    let uri = "file:///test.hex";
    open_document(&mut language_server, uri, "x : U64;");

    let offset = "x : ".len();
    let parameters = hover_parameters(uri, 0, offset as u32);
    let response = language_server.handle_request("textDocument/hover", &request_id(), &parameters);

    assert_eq!(response.result, Some(Value::Null));
}

// --- Variable hover ---

#[test]
fn hover_returns_variable_kind_for_assignment() {
    let mut language_server = LanguageServer::new();
    let uri = "file:///test.hex";
    open_document(&mut language_server, uri, "counter = 0d0; return counter;");

    let offset = "counter = 0d0; return ".len();
    let parameters = hover_parameters(uri, 0, offset as u32);
    let response = language_server.handle_request("textDocument/hover", &request_id(), &parameters);
    let result = response.result.unwrap();

    let contents = result["contents"]["value"].as_str().unwrap();
    assert!(contents.contains("**variable** `counter`"));
    assert!(contents.contains("counter = 0d0"));
}
