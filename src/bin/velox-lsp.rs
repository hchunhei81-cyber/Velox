use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use veloxc::lexer::{Lexer, Span, SpannedToken, Tok};
use veloxc::{analyze_source, DiagnosticSeverity};

#[derive(Clone)]
struct Document {
    text: String,
    version: Option<i64>,
}

#[derive(Clone)]
struct Symbol {
    name: String,
    kind: u32,
    detail: String,
    span: Span,
    scope_start: usize,
    scope_end: usize,
    declared_at: usize,
    owner: Option<String>,
}

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
    let mut safe = offset.min(text.len());
    while !text.is_char_boundary(safe) {
        safe = safe.saturating_sub(1);
    }

    let prefix = &text[..safe];
    let line = prefix.bytes().filter(|b| *b == b'\n').count();
    let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let character = prefix[line_start..].encode_utf16().count();
    (line, character)
}

fn lsp_to_byte_offset(text: &str, line: usize, character: usize) -> usize {
    let mut current_line = 0usize;
    let mut line_start = 0usize;

    for (i, byte) in text.bytes().enumerate() {
        if current_line == line {
            break;
        }
        if byte == b'\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }

    if current_line != line {
        return text.len();
    }

    let rest = &text[line_start..];
    let mut units = 0usize;
    for (relative, ch) in rest.char_indices() {
        if ch == '\n' || ch == '\r' {
            return line_start + relative;
        }
        let width = ch.len_utf16();
        if units + width > character {
            return line_start + relative;
        }
        if units + width == character {
            return line_start + relative + ch.len_utf8();
        }
        units += width;
    }
    text.len()
}

fn token_name(token: &SpannedToken) -> Option<&str> {
    if let Tok::Ident(name) = &token.kind {
        Some(name)
    } else {
        None
    }
}

fn type_name(tokens: &[SpannedToken], index: usize) -> String {
    match tokens.get(index).map(|t| &t.kind) {
        Some(Tok::Ident(name)) => name.clone(),
        _ => "unknown".into(),
    }
}

fn matching_braces(tokens: &[SpannedToken]) -> HashMap<usize, usize> {
    let mut stack = Vec::new();
    let mut pairs = HashMap::new();

    for (i, token) in tokens.iter().enumerate() {
        match token.kind {
            Tok::LBrace => stack.push(i),
            Tok::RBrace => {
                if let Some(open) = stack.pop() {
                    pairs.insert(open, i);
                    pairs.insert(i, open);
                }
            }
            _ => {}
        }
    }
    pairs
}

fn enclosing_scope(
    index: usize,
    tokens: &[SpannedToken],
    pairs: &HashMap<usize, usize>,
) -> (usize, usize) {
    let mut stack = Vec::new();
    for (i, token) in tokens.iter().enumerate().take(index.min(tokens.len())) {
        match token.kind {
            Tok::LBrace => stack.push(i),
            Tok::RBrace => {
                stack.pop();
            }
            _ => {}
        }
    }

    if let Some(open) = stack.last() {
        let close = pairs
            .get(open)
            .copied()
            .unwrap_or(tokens.len().saturating_sub(1));
        (tokens[*open].span.start, tokens[close].span.end)
    } else {
        (0, usize::MAX)
    }
}

fn collect_symbols(source: &str, tokens: &[SpannedToken]) -> Vec<Symbol> {
    let pairs = matching_braces(tokens);
    let mut symbols = Vec::new();

    let mut i = 0usize;
    while i < tokens.len() {
        match tokens[i].kind {
            Tok::Fn => {
                if let Some(name_token) = tokens.get(i + 1) {
                    if let Some(name) = token_name(name_token) {
                        let mut body_open = i + 2;
                        while body_open < tokens.len() && tokens[body_open].kind != Tok::LBrace {
                            body_open += 1;
                        }
                        let body_close = pairs
                            .get(&body_open)
                            .copied()
                            .unwrap_or(tokens.len().saturating_sub(1));
                        let signature_end = tokens
                            .get(body_open)
                            .map(|t| t.span.start)
                            .unwrap_or(name_token.span.end);

                        let signature = source
                            .get(name_token.span.start..signature_end)
                            .unwrap_or("")
                            .trim()
                            .to_string();

                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: 12,
                            detail: format!("function {}", signature),
                            span: name_token.span,
                            scope_start: 0,
                            scope_end: usize::MAX,
                            declared_at: name_token.span.start,
                            owner: None,
                        });

                        // Function parameters are visible throughout its body.
                        let mut p = i + 2;
                        while p + 1 < body_open {
                            if let (Some(param), Tok::Colon) =
                                (token_name(&tokens[p]), &tokens[p + 1].kind)
                            {
                                let ty = type_name(tokens, p + 2);
                                symbols.push(Symbol {
                                    name: param.to_string(),
                                    kind: 6,
                                    detail: format!("parameter: {}", ty),
                                    span: tokens[p].span,
                                    scope_start: tokens
                                        .get(body_open)
                                        .map(|t| t.span.start)
                                        .unwrap_or(0),
                                    scope_end: tokens
                                        .get(body_close)
                                        .map(|t| t.span.end)
                                        .unwrap_or(usize::MAX),
                                    declared_at: tokens[p].span.start,
                                    owner: None,
                                });
                                p += 2;
                            }
                            p += 1;
                        }
                    }
                }
            }
            Tok::Struct => {
                if let Some(name_token) = tokens.get(i + 1) {
                    if let Some(name) = token_name(name_token) {
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: 22,
                            detail: format!("struct {}", name),
                            span: name_token.span,
                            scope_start: 0,
                            scope_end: usize::MAX,
                            declared_at: name_token.span.start,
                            owner: None,
                        });

                        let mut open = i + 2;
                        while open < tokens.len() && tokens[open].kind != Tok::LBrace {
                            open += 1;
                        }
                        let close = pairs
                            .get(&open)
                            .copied()
                            .unwrap_or(tokens.len().saturating_sub(1));

                        let mut field = open + 1;
                        while field + 1 < close {
                            if let (Some(field_name), Tok::Colon) =
                                (token_name(&tokens[field]), &tokens[field + 1].kind)
                            {
                                symbols.push(Symbol {
                                    name: field_name.to_string(),
                                    kind: 8,
                                    detail: format!("field: {}", type_name(tokens, field + 2)),
                                    span: tokens[field].span,
                                    scope_start: 0,
                                    scope_end: usize::MAX,
                                    declared_at: tokens[field].span.start,
                                    owner: Some(name.to_string()),
                                });
                            }
                            field += 1;
                        }
                    }
                }
            }
            Tok::Let => {
                let mut name_index = i + 1;
                if tokens.get(name_index).map(|t| &t.kind) == Some(&Tok::Mut) {
                    name_index += 1;
                }

                if let Some(name_token) = tokens.get(name_index) {
                    if let Some(name) = token_name(name_token) {
                        let (scope_start, scope_end) = enclosing_scope(i, tokens, &pairs);

                        let ty = if tokens.get(name_index + 1).map(|t| &t.kind) == Some(&Tok::Colon)
                        {
                            type_name(tokens, name_index + 2)
                        } else {
                            let init = name_index + 1;
                            if tokens.get(init).map(|t| &t.kind) == Some(&Tok::Eq) {
                                match tokens.get(init + 1).map(|t| &t.kind) {
                                    Some(Tok::Int(_)) => "i64".into(),
                                    Some(Tok::Float(_)) => "f64".into(),
                                    Some(Tok::Str(_)) => "str".into(),
                                    Some(Tok::True | Tok::False) => "bool".into(),
                                    Some(Tok::Ident(s)) => format!("inferred from {}", s),
                                    _ => "unknown".into(),
                                }
                            } else {
                                "unknown".into()
                            }
                        };

                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: 13,
                            detail: format!("variable: {}", ty),
                            span: name_token.span,
                            scope_start,
                            scope_end,
                            declared_at: name_token.span.start,
                            owner: None,
                        });
                    }
                }
            }
            Tok::Parallel => {
                if let Some(name_token) = tokens.get(i + 2) {
                    if let Some(name) = token_name(name_token) {
                        let (scope_start, scope_end) = enclosing_scope(i, tokens, &pairs);
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: 6,
                            detail: "parallel loop variable: i64".into(),
                            span: name_token.span,
                            scope_start,
                            scope_end,
                            declared_at: name_token.span.start,
                            owner: None,
                        });
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }

    symbols
}

fn visible_symbol<'a>(symbols: &'a [Symbol], name: &str, offset: usize) -> Option<&'a Symbol> {
    symbols
        .iter()
        .filter(|s| {
            s.name == name
                && s.declared_at <= offset
                && s.scope_start <= offset
                && offset <= s.scope_end
                && s.owner.is_none()
        })
        .max_by_key(|s| s.declared_at)
        .or_else(|| {
            symbols
                .iter()
                .find(|s| s.name == name && s.owner.is_none() && s.scope_start == 0)
        })
}

fn identifier_at(source: &str, offset: usize) -> Option<(String, Span)> {
    let mut start = offset.min(source.len());
    while start > 0 {
        let ch = source[..start].chars().next_back()?;
        if ch.is_ascii_alphanumeric() || ch == '_' {
            start -= ch.len_utf8();
        } else {
            break;
        }
    }

    let mut end = offset.min(source.len());
    while end < source.len() {
        let ch = source[end..].chars().next()?;
        if ch.is_ascii_alphanumeric() || ch == '_' {
            end += ch.len_utf8();
        } else {
            break;
        }
    }

    if start == end {
        return None;
    }

    Some((source[start..end].to_string(), Span { start, end }))
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
            let (sl, sc, el, ec) = match (error.start, error.end) {
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
                    "start": {"line": sl, "character": sc},
                    "end": {"line": el, "character": ec}
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

fn completion(source: &str, offset: usize) -> Value {
    let tokens = match Lexer::new(source).tokenize_spanned() {
        Ok(tokens) => tokens,
        Err(_) => return json!({"isIncomplete": false, "items": []}),
    };

    let symbols = collect_symbols(source, &tokens);
    let mut items = Vec::new();

    // Struct field completion after a dot.
    let before = &source[..offset.min(source.len())];
    let field_context = before.trim_end().ends_with('.');
    let base_name = if field_context {
        let without_dot = before.trim_end().trim_end_matches('.');
        identifier_at(without_dot, without_dot.len()).map(|v| v.0)
    } else {
        None
    };

    if field_context {
        if let Some(base) = base_name {
            if let Some(base_symbol) = visible_symbol(&symbols, &base, offset) {
                if let Some(ty) = base_symbol.detail.split(':').nth(1) {
                    let struct_name = ty.trim();
                    for symbol in symbols
                        .iter()
                        .filter(|s| s.kind == 8 && s.owner.as_deref() == Some(struct_name))
                    {
                        items.push(json!({
                            "label": symbol.name,
                            "kind": 5,
                            "detail": symbol.detail
                        }));
                    }
                }
            }
        }

        // Member access should not be polluted by global symbols or built-ins.
        return json!({"isIncomplete": false, "items": items});
    }

    for symbol in &symbols {
        if symbol.owner.is_some() {
            continue;
        }
        if symbol.scope_start != 0
            && !(symbol.declared_at <= offset
                && symbol.scope_start <= offset
                && offset <= symbol.scope_end)
        {
            continue;
        }

        let kind = match symbol.kind {
            6 => 6,
            12 => 3,
            13 => 6,
            22 => 22,
            _ => 1,
        };

        items.push(json!({
            "label": symbol.name,
            "kind": kind,
            "detail": symbol.detail
        }));
    }

    for builtin in ["print", "println", "write"] {
        if !items.iter().any(|v| v["label"] == builtin) {
            items.push(json!({
                "label": builtin,
                "kind": 3,
                "detail": "Velox built-in function"
            }));
        }
    }

    for keyword in [
        "fn", "struct", "let", "mut", "return", "if", "else", "while", "for", "in", "true",
        "false", "parallel", "pub", "export", "import",
    ] {
        if !items.iter().any(|item| item["label"] == keyword) {
            items.push(json!({
                "label": keyword,
                "kind": 14,
                "detail": "Velox keyword"
            }));
        }
    }

    json!({"isIncomplete": false, "items": items})
}

fn semantic_tokens(source: &str) -> Value {
    let tokens = match Lexer::new(source).tokenize_spanned() {
        Ok(tokens) => tokens,
        Err(_) => return json!({"data": []}),
    };

    let symbols = collect_symbols(source, &tokens);
    // namespace, type, function, variable, parameter, property,
    // number, string, operator, keyword, boolean
    let mut data: Vec<u32> = Vec::new();
    let mut previous_line = 0usize;
    let mut previous_start = 0usize;

    for (index, token) in tokens.iter().enumerate() {
        if token.span.start >= token.span.end {
            continue;
        }

        let text = &source[token.span.start..token.span.end];
        let previous = index.checked_sub(1).and_then(|i| tokens.get(i));
        let next = tokens.get(index + 1);

        let token_type = match &token.kind {
            Tok::Fn
            | Tok::Struct
            | Tok::Let
            | Tok::Mut
            | Tok::Return
            | Tok::If
            | Tok::Else
            | Tok::While
            | Tok::For
            | Tok::In
            | Tok::Parallel
            | Tok::Public
            | Tok::Export
            | Tok::Break
            | Tok::Continue => Some(9),

            Tok::True | Tok::False => Some(10),
            Tok::Int(_) | Tok::Float(_) => Some(6),
            Tok::Str(_) => Some(7),

            Tok::Plus
            | Tok::Minus
            | Tok::Star
            | Tok::Slash
            | Tok::Percent
            | Tok::Eq
            | Tok::EqEq
            | Tok::NotEq
            | Tok::Lt
            | Tok::Gt
            | Tok::Le
            | Tok::Ge
            | Tok::AndAnd
            | Tok::OrOr
            | Tok::Bang
            | Tok::Arrow
            | Tok::FatArrow
            | Tok::DotDot
            | Tok::PlusPlus
            | Tok::PlusEq
            | Tok::MinusMinus
            | Tok::MinusEq
            | Tok::StarEq
            | Tok::SlashEq
            | Tok::Caret
            | Tok::Question
            | Tok::Shl
            | Tok::Shr
            | Tok::Amp
            | Tok::Pipe => Some(8),

            Tok::Ident(name) => {
                if name == "import" {
                    Some(9)
                } else if name == "py"
                    || previous.is_some_and(|p| matches!(&p.kind, Tok::Ident(s) if s == "import"))
                {
                    Some(0)
                } else if matches!(name.as_str(), "i64" | "f64" | "bool" | "str" | "void")
                    || name.chars().next().is_some_and(char::is_uppercase)
                {
                    Some(1)
                } else if previous.is_some_and(|p| matches!(&p.kind, Tok::Dot)) {
                    Some(5)
                } else if let Some(symbol) = symbols.iter().find(|s| s.span == token.span) {
                    match symbol.kind {
                        22 => Some(1),
                        12 => Some(2),
                        6 => Some(4),
                        8 => Some(5),
                        13 => Some(3),
                        _ => Some(3),
                    }
                } else if next.is_some_and(|n| matches!(&n.kind, Tok::LParen)) {
                    Some(2)
                } else {
                    Some(3)
                }
            }

            Tok::LParen
            | Tok::RParen
            | Tok::LBrace
            | Tok::RBrace
            | Tok::LBracket
            | Tok::RBracket
            | Tok::Comma
            | Tok::Semi
            | Tok::Colon
            | Tok::Dot
            | Tok::Eof => None,
        };

        let Some(token_type) = token_type else {
            continue;
        };

        let (line, start) = byte_offset_to_lsp(source, token.span.start);
        let length = text.encode_utf16().count();
        if length == 0 {
            continue;
        }

        let delta_line = line - previous_line;
        let delta_start = if delta_line == 0 {
            start - previous_start
        } else {
            start
        };

        data.extend_from_slice(&[
            delta_line as u32,
            delta_start as u32,
            length as u32,
            token_type,
            0,
        ]);

        previous_line = line;
        previous_start = start;
    }

    json!({"data": data})
}

fn hover(source: &str, offset: usize) -> Value {
    let Some((name, _)) = identifier_at(source, offset) else {
        return Value::Null;
    };

    let Ok(tokens) = Lexer::new(source).tokenize_spanned() else {
        return Value::Null;
    };
    let symbols = collect_symbols(source, &tokens);

    let Some(symbol) = visible_symbol(&symbols, &name, offset) else {
        return Value::Null;
    };

    json!({
        "contents": {
            "kind": "markdown",
            "value": format!("```velox\n{}\n```\n\n{}", symbol.name, symbol.detail)
        },
        "range": {
            "start": {
                "line": byte_offset_to_lsp(source, symbol.span.start).0,
                "character": byte_offset_to_lsp(source, symbol.span.start).1
            },
            "end": {
                "line": byte_offset_to_lsp(source, symbol.span.end).0,
                "character": byte_offset_to_lsp(source, symbol.span.end).1
            }
        }
    })
}

fn definition(uri: &str, source: &str, offset: usize) -> Value {
    let Some((name, _)) = identifier_at(source, offset) else {
        return Value::Null;
    };

    let Ok(tokens) = Lexer::new(source).tokenize_spanned() else {
        return Value::Null;
    };
    let symbols = collect_symbols(source, &tokens);

    let Some(symbol) = visible_symbol(&symbols, &name, offset) else {
        return Value::Null;
    };

    let (sl, sc) = byte_offset_to_lsp(source, symbol.span.start);
    let (el, ec) = byte_offset_to_lsp(source, symbol.span.end);

    json!({
        "uri": uri,
        "range": {
            "start": {"line": sl, "character": sc},
            "end": {"line": el, "character": ec}
        }
    })
}

fn document_position(msg: &Value, text: &str) -> usize {
    let position = &msg["params"]["position"];
    lsp_to_byte_offset(
        text,
        position["line"].as_u64().unwrap_or(0) as usize,
        position["character"].as_u64().unwrap_or(0) as usize,
    )
}

fn handle<W: Write>(
    w: &mut W,
    msg: Value,
    documents: &mut HashMap<String, Document>,
) -> io::Result<bool> {
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
                                },
                                "completionProvider": {
                                    "resolveProvider": false,
                                    "triggerCharacters": ["."]
                                },
                                "hoverProvider": true,
                                "definitionProvider": true,
                                "semanticTokensProvider": {
                                    "legend": {
                                        "tokenTypes": [
                                            "namespace", "type", "function",
                                            "variable", "parameter", "property",
                                            "number", "string", "operator",
                                            "keyword", "boolean"
                                        ],
                                        "tokenModifiers": []
                                    },
                                    "full": true
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
        "initialized" => {}
        "shutdown" => {
            if let Some(id) = id {
                send(w, json!({"jsonrpc": "2.0", "id": id, "result": null}))?;
            }
        }
        "textDocument/didOpen" => {
            let doc = &msg["params"]["textDocument"];
            let uri = doc["uri"].as_str().unwrap_or("").to_string();
            let text = doc["text"].as_str().unwrap_or("").to_string();
            let version = doc["version"].as_i64();

            documents.insert(
                uri.clone(),
                Document {
                    text: text.clone(),
                    version,
                },
            );
            diagnostics(w, &uri, &text, doc.get("version").cloned())?;
        }
        "textDocument/didChange" => {
            let doc = &msg["params"]["textDocument"];
            let uri = doc["uri"].as_str().unwrap_or("").to_string();
            let version = doc["version"].as_i64();

            let stale = documents
                .get(&uri)
                .and_then(|d| d.version)
                .zip(version)
                .map(|(old, new)| new <= old)
                .unwrap_or(false);

            if !stale {
                if let Some(text) = msg["params"]["contentChanges"]
                    .as_array()
                    .and_then(|changes| changes.last())
                    .and_then(|change| change["text"].as_str())
                {
                    let text = text.to_string();
                    documents.insert(
                        uri.clone(),
                        Document {
                            text: text.clone(),
                            version,
                        },
                    );
                    diagnostics(w, &uri, &text, doc.get("version").cloned())?;
                }
            }
        }
        "textDocument/didClose" => {
            let uri = msg["params"]["textDocument"]["uri"]
                .as_str()
                .unwrap_or("")
                .to_string();
            documents.remove(&uri);
            send(
                w,
                json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/publishDiagnostics",
                    "params": {"uri": uri, "diagnostics": []}
                }),
            )?;
        }
        "textDocument/completion" => {
            if let Some(id) = id {
                let uri = msg["params"]["textDocument"]["uri"].as_str().unwrap_or("");
                let result = documents
                    .get(uri)
                    .map(|doc| completion(&doc.text, document_position(&msg, &doc.text)))
                    .unwrap_or_else(|| json!({"isIncomplete": false, "items": []}));
                send(w, json!({"jsonrpc": "2.0", "id": id, "result": result}))?;
            }
        }
        "textDocument/semanticTokens/full" => {
            if let Some(id) = id {
                let uri = msg["params"]["textDocument"]["uri"].as_str().unwrap_or("");
                let result = documents
                    .get(uri)
                    .map(|doc| semantic_tokens(&doc.text))
                    .unwrap_or_else(|| json!({"data": []}));
                send(w, json!({"jsonrpc": "2.0", "id": id, "result": result}))?;
            }
        }
        "textDocument/hover" => {
            if let Some(id) = id {
                let uri = msg["params"]["textDocument"]["uri"].as_str().unwrap_or("");
                let result = documents
                    .get(uri)
                    .map(|doc| hover(&doc.text, document_position(&msg, &doc.text)))
                    .unwrap_or(Value::Null);
                send(w, json!({"jsonrpc": "2.0", "id": id, "result": result}))?;
            }
        }
        "textDocument/definition" => {
            if let Some(id) = id {
                let uri = msg["params"]["textDocument"]["uri"].as_str().unwrap_or("");
                let result = documents
                    .get(uri)
                    .map(|doc| definition(uri, &doc.text, document_position(&msg, &doc.text)))
                    .unwrap_or(Value::Null);
                let result = if result.is_null() {
                    Value::Null
                } else {
                    json!([result])
                };
                send(w, json!({"jsonrpc": "2.0", "id": id, "result": result}))?;
            }
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
    let mut documents = HashMap::new();

    while let Some(message) = read_message(&mut reader)? {
        if !handle(&mut writer, message, &mut documents)? {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_positions_support_chinese_and_emoji() {
        let text = "中😀x";
        assert_eq!(byte_offset_to_lsp(text, 3), (0, 1));
        assert_eq!(byte_offset_to_lsp(text, 7), (0, 3));
        assert_eq!(byte_offset_to_lsp(text, text.len()), (0, 4));
    }

    #[test]
    fn utf16_positions_support_crlf() {
        let text = "abc\r\ndef";
        assert_eq!(byte_offset_to_lsp(text, 5), (1, 0));
        assert_eq!(byte_offset_to_lsp(text, 8), (1, 3));
    }

    #[test]
    fn lsp_positions_convert_back_to_utf8_offsets() {
        let text = "中😀x\nnext";
        assert_eq!(lsp_to_byte_offset(text, 0, 1), 3);
        assert_eq!(lsp_to_byte_offset(text, 0, 3), 7);
        assert_eq!(lsp_to_byte_offset(text, 1, 2), 11);
    }

    #[test]
    fn symbol_index_finds_functions_structs_and_variables() {
        let source = r#"
struct Point { x: i64, y: i64 }
fn add(a: i64, b: i64) -> i64 {
    let result: i64 = a + b;
    return result;
}
"#;
        let tokens = Lexer::new(source).tokenize_spanned().unwrap();
        let symbols = collect_symbols(source, &tokens);

        assert!(symbols.iter().any(|s| s.name == "Point" && s.kind == 22));
        assert!(symbols.iter().any(|s| s.name == "add" && s.kind == 12));
        assert!(symbols.iter().any(|s| s.name == "a" && s.kind == 6));
        assert!(symbols.iter().any(|s| s.name == "result" && s.kind == 13));
        assert!(symbols
            .iter()
            .any(|s| s.name == "x" && s.owner.as_deref() == Some("Point")));
    }

    #[test]
    fn completion_includes_visible_symbols_and_builtins() {
        let source = "fn main() -> void { let count: i64 = 1; print(count); }";
        let offset = source.find("print").unwrap();
        let result = completion(source, offset);
        let labels: Vec<String> = result["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["label"].as_str().map(str::to_string))
            .collect();

        assert!(labels.contains(&"count".to_string()));
        assert!(labels.contains(&"main".to_string()));
        assert!(labels.contains(&"println".to_string()));
    }

    #[test]
    fn member_completion_only_returns_fields_of_receiver_struct() {
        let source = r#"
struct Point { x: i64, y: i64 }
fn main() -> void {
    let point: Point = Point { x: 1, y: 2 };
    point.
}
"#;
        let offset = source.find("point.").unwrap() + "point.".len();
        let result = completion(source, offset);
        let labels: Vec<String> = result["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["label"].as_str().map(str::to_string))
            .collect();

        assert!(labels.contains(&"x".to_string()));
        assert!(labels.contains(&"y".to_string()));
        assert!(!labels.contains(&"main".to_string()));
        assert!(!labels.contains(&"print".to_string()));
        assert!(!labels.contains(&"println".to_string()));
    }

    #[test]
    fn hover_resolves_local_variable() {
        let source = "fn main() -> void { let count: i64 = 1; print(count); }";
        let offset = source.rfind("count").unwrap() + 2;
        let result = hover(source, offset);
        assert!(result["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("variable: i64"));
    }

    #[test]
    fn definition_resolves_local_variable_declaration() {
        let source = "fn main() -> void { let count: i64 = 1; print(count); }";
        let offset = source.rfind("count").unwrap() + 2;
        let result = definition("file:///test.vx", source, offset);
        assert_eq!(result["uri"], "file:///test.vx");
        assert_eq!(result["range"]["start"]["character"], 24);
    }

    #[test]
    fn completion_includes_velox_keywords() {
        let source = "fn main() -> void {}";
        let result = completion(source, source.len());
        let labels: Vec<String> = result["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["label"].as_str().map(str::to_string))
            .collect();

        for keyword in [
            "fn", "struct", "let", "mut", "return", "if", "else", "while", "for", "in", "parallel",
            "pub", "export", "import",
        ] {
            assert!(
                labels.iter().any(|label| label == keyword),
                "missing keyword completion: {keyword}"
            );
        }
        assert!(!labels.iter().any(|label| label == "python"));
        assert!(!labels.iter().any(|label| label == "public"));
        assert!(!labels.iter().any(|label| label == "private"));
    }

    #[test]
    fn semantic_tokens_classify_velox_constructs() {
        let source = "fn main() -> void { let value: i64 = 42; println(true); }";
        let result = semantic_tokens(source);
        let data = result["data"].as_array().unwrap();
        assert_eq!(data.len() % 5, 0);

        let types: Vec<u64> = data
            .chunks(5)
            .map(|chunk| chunk[3].as_u64().unwrap())
            .collect();

        for expected in [1, 2, 3, 6, 9, 10] {
            assert!(
                types.contains(&expected),
                "missing semantic token type {expected}"
            );
        }
    }

    #[test]
    fn semantic_tokens_support_short_python_imports() {
        let source = "import math; fn main() { let x: f64 = py.math.sqrt(9.0); }";
        let result = semantic_tokens(source);
        let data = result["data"].as_array().unwrap();

        let types: Vec<u64> = data
            .chunks(5)
            .map(|chunk| chunk[3].as_u64().unwrap())
            .collect();

        assert!(types.contains(&0), "module namespace token missing");
    }

    #[test]
    fn initialize_advertises_language_features() {
        let mut output = Vec::new();
        let mut docs = HashMap::new();
        handle(
            &mut output,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            &mut docs,
        )
        .unwrap();

        let response = String::from_utf8(output).unwrap();
        assert!(response.contains("completionProvider"));
        assert!(response.contains("hoverProvider"));
        assert!(response.contains("definitionProvider"));
        assert!(response.contains("semanticTokensProvider"));
    }
}
