use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

#[derive(Debug, Deserialize)]
struct Request {
    seq: i64,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    message_type: String,
    command: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct LaunchArguments {
    #[serde(default)]
    program: Option<String>,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    stop_on_entry: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct AttachArguments {
    #[serde(default)]
    process_id: Option<i32>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    host: Option<String>,
}

#[derive(Debug, Serialize)]
struct Response {
    seq: i64,
    #[serde(rename = "type")]
    message_type: String,
    request_seq: i64,
    success: bool,
    command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Event {
    seq: i64,
    #[serde(rename = "type")]
    message_type: String,
    event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<Value>,
}

struct DebugAdapter {
    seq: i64,
    #[allow(dead_code)]
    initialized: bool,
    configuration_done: bool,
    #[allow(dead_code)]
    launch_args: Option<LaunchArguments>,
    #[allow(dead_code)]
    attach_args: Option<AttachArguments>,
}

impl DebugAdapter {
    fn new() -> Self {
        Self {
            seq: 1,
            initialized: false,
            configuration_done: false,
            launch_args: None,
            attach_args: None,
        }
    }

    fn run(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut stdin = stdin.lock();

        loop {
            let content = match self.read_message(&mut stdin) {
                Ok(content) => content,
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(error) => return Err(error),
            };

            let request: Request = match serde_json::from_str(&content) {
                Ok(request) => request,
                Err(_) => continue,
            };

            let should_exit = request.command == "disconnect";

            let response = self.handle_request(&request);
            self.send_message(&response)?;

            if request.command == "initialize" {
                self.send_event("initialized", None)?;
            }

            if (request.command == "launch" || request.command == "attach")
                && self.configuration_done
            {
                self.send_event("terminated", None)?;
            }

            if should_exit {
                break;
            }
        }

        Ok(())
    }

    fn read_message<R: BufRead>(&self, reader: &mut R) -> io::Result<String> {
        let mut content_length: Option<usize> = None;
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line)?;
            if bytes_read == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF"));
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }

            if let Some(value) = trimmed.strip_prefix("Content-Length: ") {
                content_length = value.parse().ok();
            }
        }

        let content_length = content_length
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing Content-Length"))?;

        let mut content = vec![0u8; content_length];
        reader.read_exact(&mut content)?;

        String::from_utf8(content)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn send_message<T: Serialize>(&mut self, message: &T) -> io::Result<()> {
        let content = serde_json::to_string(message)?;
        let output = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);

        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        stdout.write_all(output.as_bytes())?;
        stdout.flush()
    }

    fn send_event(&mut self, event: &str, body: Option<Value>) -> io::Result<()> {
        let event = Event {
            seq: self.next_seq(),
            message_type: "event".to_string(),
            event: event.to_string(),
            body,
        };
        self.send_message(&event)
    }

    fn next_seq(&mut self) -> i64 {
        let seq = self.seq;
        self.seq += 1;
        seq
    }

    fn make_response(
        &mut self,
        request: &Request,
        success: bool,
        body: Option<Value>,
        message: Option<String>,
    ) -> Response {
        Response {
            seq: self.next_seq(),
            message_type: "response".to_string(),
            request_seq: request.seq,
            success,
            command: request.command.clone(),
            message,
            body,
        }
    }

    fn handle_request(&mut self, request: &Request) -> Response {
        match request.command.as_str() {
            "initialize" => self.handle_initialize(request),
            "launch" => self.handle_launch(request),
            "attach" => self.handle_attach(request),
            "configurationDone" => self.handle_configuration_done(request),
            "setBreakpoints" => self.handle_set_breakpoints(request),
            "setFunctionBreakpoints" => self.handle_set_function_breakpoints(request),
            "setExceptionBreakpoints" => self.handle_set_exception_breakpoints(request),
            "threads" => self.handle_threads(request),
            "stackTrace" => self.handle_stack_trace(request),
            "scopes" => self.handle_scopes(request),
            "variables" => self.handle_variables(request),
            "continue" => self.handle_continue(request),
            "next" | "stepIn" | "stepOut" | "pause" => self.handle_stepping(request),
            "disconnect" | "terminate" => self.handle_disconnect(request),
            _ => self.make_response(
                request,
                false,
                None,
                Some(format!("Unsupported command: {}", request.command)),
            ),
        }
    }

    fn handle_initialize(&mut self, request: &Request) -> Response {
        self.initialized = true;
        self.make_response(
            request,
            true,
            Some(json!({
                "supportsConfigurationDoneRequest": true,
                "supportsFunctionBreakpoints": false,
                "supportsConditionalBreakpoints": false,
                "supportsHitConditionalBreakpoints": false,
                "supportsEvaluateForHovers": false,
                "supportsStepBack": false,
                "supportsSetVariable": false,
                "supportsRestartFrame": false,
                "supportsGotoTargetsRequest": false,
                "supportsStepInTargetsRequest": false,
                "supportsCompletionsRequest": false,
                "supportsModulesRequest": false,
                "supportsExceptionOptions": false,
                "supportsValueFormattingOptions": false,
                "supportsExceptionInfoRequest": false,
                "supportTerminateDebuggee": true,
                "supportsDelayedStackTraceLoading": false,
                "supportsLoadedSourcesRequest": false,
                "supportsLogPoints": false,
                "supportsTerminateThreadsRequest": false,
                "supportsSetExpression": false,
                "supportsTerminateRequest": true,
                "supportsDataBreakpoints": false,
                "supportsReadMemoryRequest": false,
                "supportsDisassembleRequest": false,
                "supportsCancelRequest": false,
                "supportsBreakpointLocationsRequest": false,
                "supportsClipboardContext": false,
                "supportsSteppingGranularity": false,
                "supportsInstructionBreakpoints": false,
                "supportsExceptionFilterOptions": false
            })),
            None,
        )
    }

    fn handle_launch(&mut self, request: &Request) -> Response {
        let args: LaunchArguments =
            serde_json::from_value(request.arguments.clone()).unwrap_or_default();

        self.launch_args = Some(args);
        self.attach_args = None;

        self.make_response(request, true, None, None)
    }

    fn handle_attach(&mut self, request: &Request) -> Response {
        let args: AttachArguments =
            serde_json::from_value(request.arguments.clone()).unwrap_or_default();

        self.attach_args = Some(args);
        self.launch_args = None;

        self.make_response(request, true, None, None)
    }

    fn handle_configuration_done(&mut self, request: &Request) -> Response {
        self.configuration_done = true;
        self.make_response(request, true, None, None)
    }

    fn handle_set_breakpoints(&mut self, request: &Request) -> Response {
        self.make_response(request, true, Some(json!({ "breakpoints": [] })), None)
    }

    fn handle_set_function_breakpoints(&mut self, request: &Request) -> Response {
        self.make_response(request, true, Some(json!({ "breakpoints": [] })), None)
    }

    fn handle_set_exception_breakpoints(&mut self, request: &Request) -> Response {
        self.make_response(request, true, None, None)
    }

    fn handle_threads(&mut self, request: &Request) -> Response {
        self.make_response(
            request,
            true,
            Some(json!({
                "threads": [
                    {
                        "id": 1,
                        "name": "main"
                    }
                ]
            })),
            None,
        )
    }

    fn handle_stack_trace(&mut self, request: &Request) -> Response {
        self.make_response(
            request,
            true,
            Some(json!({
                "stackFrames": [],
                "totalFrames": 0
            })),
            None,
        )
    }

    fn handle_scopes(&mut self, request: &Request) -> Response {
        self.make_response(request, true, Some(json!({ "scopes": [] })), None)
    }

    fn handle_variables(&mut self, request: &Request) -> Response {
        self.make_response(request, true, Some(json!({ "variables": [] })), None)
    }

    fn handle_continue(&mut self, request: &Request) -> Response {
        self.make_response(
            request,
            true,
            Some(json!({ "allThreadsContinued": true })),
            None,
        )
    }

    fn handle_stepping(&mut self, request: &Request) -> Response {
        self.make_response(request, true, None, None)
    }

    fn handle_disconnect(&mut self, request: &Request) -> Response {
        self.make_response(request, true, None, None)
    }
}

fn main() {
    let mut adapter = DebugAdapter::new();
    if let Err(error) = adapter.run() {
        eprintln!("Debug adapter error: {}", error);
        std::process::exit(1);
    }
}
