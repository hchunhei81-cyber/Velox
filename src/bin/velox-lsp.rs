use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use veloxc::{analyze_source, DiagnosticSeverity};

fn read_message<R: BufRead>(r: &mut R) -> io::Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("Content-Length") {
                length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
                );
            }
        }
    }

    let length = length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing Content-Length"))?;

    if length > 16 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Message too large",
        ));
    }

    let mut body = vec![0; length];
    r.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn send<W: Write>(w: &mut W, value: Value) -> io::Result<()> {
    let body =
        serde_json::to_vec(&value).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
    w.write_all(&body)?;
    w.flush()
}

fn byte_offset_to_lsp(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let mut line = 0usize;
    let mut line_start = 0usize;

    for (index, byte) in text.bytes().enumerate() {
        if index >= offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
            line_start = index + 1;
        }
    }

    let safe_offset = (line_start..=offset)
        .rev()
        .find(|&candidate| text.is_char_boundary(candidate))
        .unwrap_or(line_start);

    let character = text[line_start..safe_offset].encode_utf16().count();
    (line, character)
}

fn diagnostics<W: Write>(
    w: &mut W,
    uri: &str,
    text: &str,
    version: Option<Value>,
) -> io::Result<()> {
    let items = match analyze_source(text, uri) {
        Ok(()) => vec![],
        Err(error) => {
            let (start_line, start_character, end_line, end_character) =
                match (error.start, error.end) {
                    (Some(start), Some(end)) => {
                        let (sl, sc) = byte_offset_to_lsp(text, start);
                        let (el, ec) = byte_offset_to_lsp(text, end);
                        (sl, sc, el, ec)
                    }
                    _ => {
                        let (line, character) = byte_offset_to_lsp(text, text.len());
                        (line, character, line, character)
                    }
                };

            let mut item = json!({
                "range": {
                    "start": {"line": start_line, "character": start_character},
                    "end": {"line": end_line, "character": end_character}
                },
                "severity": match error.severity {
                    DiagnosticSeverity::Error => 1,
                    DiagnosticSeverity::Warning => 2
                },
                "message": error.message,
                "source": "velox"
            });

            if let Some(code) = error.code {
                item["code"] = json!(code);
            }
            vec![item]
        }
    };

    let mut params = json!({"uri": uri, "diagnostics": items});
    if let Some(v) = version {
        params["version"] = v;
    }

    send(
        w,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": params
        }),
    )
}

fn handle<W: Write>(w: &mut W, msg: Value) -> io::Result<bool> {
    let method = msg["method"].as_str().unwrap_or("");
    let id = msg.get("id").cloned();

    match method {
        "initialize" => {
            if let Some(id) = id {
                send(
                    w,
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "capabilities": {
                                "textDocumentSync": {
                                    "openClose": true,
                                    "change": 1
                                }
                            },
                            "serverInfo": {
                                "name": "Velox Language Server",
                                "version": env!("CARGO_PKG_VERSION")
                            }
                        }
                    }),
                )?;
            }
        }
        "shutdown" => {
            if let Some(id) = id {
                send(w, json!({"jsonrpc": "2.0", "id": id, "result": null}))?;
            }
        }
        "textDocument/didOpen" => {
            let doc = &msg["params"]["textDocument"];
            diagnostics(
                w,
                doc["uri"].as_str().unwrap_or(""),
                doc["text"].as_str().unwrap_or(""),
                doc.get("version").cloned(),
            )?;
        }
        "textDocument/didChange" => {
            let doc = &msg["params"]["textDocument"];
            if let Some(change) = msg["params"]["contentChanges"]
                .as_array()
                .and_then(|changes| changes.last())
            {
                if let Some(text) = change["text"].as_str() {
                    diagnostics(
                        w,
                        doc["uri"].as_str().unwrap_or(""),
                        text,
                        doc.get("version").cloned(),
                    )?;
                }
            }
        }
        "textDocument/didClose" => {
            send(
                w,
                json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/publishDiagnostics",
                    "params": {
                        "uri": msg["params"]["textDocument"]["uri"],
                        "diagnostics": []
                    }
                }),
            )?;
        }
        "exit" => return Ok(false),
        _ => {
            if let Some(id) = id {
                send(w, json!({"jsonrpc": "2.0", "id": id, "result": null}))?;
            }
        }
    }
    Ok(true)
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    while let Some(message) = read_message(&mut reader)? {
        if !handle(&mut writer, message)? {
            break;
        }
    }
    Ok(())
}
