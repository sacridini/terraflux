//! Tool calls a model wrote in its text instead of calling them: found and turned into calls.
//!
//! Smaller models served by vLLM or LiteLLM (the HU Berlin's Qwen) often answer with the call written out, the
//! server ending the answer with `finish_reason: stop` and no `tool_calls`: as Python (`edit_node(node="a", ...)`),
//! as JSON (a ```json block with `name` and `arguments`, or Hermes' `<tool_call>{...}</tool_call>`), or as tags
//! (`<function=name><parameter=key>value</parameter></function>`, `<invoke name="...">`, `<list_operators><query>
//! ...</query></list_operators>`). [`calls`] finds the calls of the tools offered in these forms and takes them out
//! of the text; [`announced`] tells when a text promises a call it does not make (for the model to be asked again).

use serde_json::{Map, Value};

use crate::llm::Call;

/// The calls of the tools offered (as [`crate::tools::catalogue`] gives them: `name` and `inputSchema`) written in a
/// text, in their order, and the text without them.
pub fn calls(text: &str, offered: &[Value]) -> (Vec<Call>, String) {
    let tools: Vec<(&str, &Value)> =
        offered.iter().filter_map(|t| Some((t["name"].as_str()?, &t["inputSchema"]))).collect();
    let mut found: Vec<(usize, usize, String, Value)> = Vec::new();
    let taken = |found: &[(usize, usize, String, Value)], at: usize| found.iter().any(|f| at >= f.0 && at < f.1);
    // tags first (they may hold JSON), then JSON, then Python
    let mut i = 0;
    while let Some(k) = text[i..].find('<') {
        let at = i + k;
        match tagged(text, at, &tools) {
            Some((end, calls)) => {
                for (n, (name, args)) in calls.into_iter().enumerate() {
                    // (one span for all the calls of a wrapper)
                    found.push((at, if n == 0 { end } else { at }, name, args));
                }
                i = end;
            }
            None => i = at + 1,
        }
    }
    let mut i = 0;
    while let Some(k) = text[i..].find(['{', '[']) {
        let at = i + k;
        if taken(&found, at) {
            i = at + 1;
            continue;
        }
        let mut stream = serde_json::Deserializer::from_str(&text[at..]).into_iter::<Value>();
        match stream.next() {
            Some(Ok(v)) => {
                let end = at + stream.byte_offset();
                let calls = json_calls(&v, &tools);
                if calls.is_empty() {
                    i = at + 1;
                    continue;
                }
                for (n, (name, args)) in calls.into_iter().enumerate() {
                    found.push((at, if n == 0 { end } else { at }, name, args));
                }
                i = end;
            }
            _ => i = at + 1,
        }
    }
    let mut start = 0;
    for line in text.split_inclusive('\n') {
        let at = start;
        start += line.len();
        if taken(&found, at) {
            continue;
        }
        let lead = line.len() - line.trim_start_matches([' ', '\t', '>', '`', '$', '-', '*']).len();
        if let Some((end, name, args)) = python(text, at + lead, &tools) {
            found.push((at + lead, end, name, args));
        }
    }
    found.sort_by_key(|f| f.0);
    let mut rest = String::new();
    let mut from = 0;
    for f in &found {
        if f.0 >= from {
            rest.push_str(&text[from..f.0]);
            from = f.1.max(f.0);
        }
    }
    rest.push_str(&text[from.min(text.len())..]);
    let calls = found
        .into_iter()
        .enumerate()
        .map(|(k, (_, _, name, args))| Call { id: format!("written_{k}"), name, arguments: args.to_string() })
        .collect();
    (calls, tidy(&rest))
}

/// The tool a text announces it will call without calling it (a tool named in its last paragraph with words of
/// intent, or the text ending with a colon), if any (`Some("")` when it names none).
pub fn announced(text: &str, offered: &[Value]) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let tail = t.rsplit("\n\n").next().unwrap_or(t).to_lowercase();
    let named = offered.iter().filter_map(|o| o["name"].as_str()).find(|n| tail.contains(n)).map(str::to_string);
    const INTENT: [&str; 16] = [
        "let me",
        "i'll",
        "i will",
        "i am going",
        "i'm going",
        "let's",
        "next,",
        "now ",
        "calling",
        "i need to call",
        "vou ",
        "agora",
        "vamos",
        "chamar",
        "chamando",
        "preciso",
    ];
    // (a tool named in what looks like code: a call written in a form not read)
    let as_code = ["<", "```", "{\"", "("].iter().any(|c| tail.contains(c));
    if named.is_some() && (as_code || INTENT.iter().any(|w| tail.contains(w))) {
        return named;
    }
    // (a last sentence promising to act now, the answer ending there: "Let me build this model now.")
    const PROMISE: [&str; 9] =
        ["let me ", "i'll ", "i will ", "now i ", "now, i ", "vou ", "agora vou ", "deixe-me ", "vamos "];
    let promise = tail.len() <= 160
        && !tail.contains('?')
        && tail.matches(". ").count() == 0
        && PROMISE.iter().any(|w| tail.starts_with(w))
        && !["let me know", "i'll be ", "i will be "].iter().any(|w| tail.starts_with(w));
    if promise {
        return Some(named.unwrap_or_default());
    }
    t.trim_end_matches('`').trim_end().ends_with(':').then(|| named.unwrap_or_default())
}

/// The message asking a model again, when it wrote or announced a call without making it.
pub fn ask_again(tool: &str) -> String {
    let what = if tool.is_empty() { "a tool".to_string() } else { format!("`{tool}`") };
    format!(
        "(Nothing was done: your answer announced or wrote a call of {what} as text. A tool runs only when you call \
         it through the tool-calling interface, never written in the message. Call it now; if you are done, give \
         the answer.)"
    )
}

/// The text without empty code blocks and runs of blank lines (what is left where calls were taken out).
fn tidy(t: &str) -> String {
    let mut out = String::new();
    let mut lines = t.lines().peekable();
    while let Some(l) = lines.next() {
        let s = l.trim();
        if s.starts_with("```") {
            // an empty block
            let mut body = Vec::new();
            let mut closed = false;
            for m in lines.by_ref() {
                if m.trim().starts_with("```") {
                    closed = true;
                    break;
                }
                body.push(m);
            }
            // (or not closed: what closed it went with a call)
            if body.iter().all(|b| b.trim().is_empty()) {
                continue;
            }
            out.push_str(l);
            out.push('\n');
            for b in body {
                out.push_str(b);
                out.push('\n');
            }
            if closed {
                out.push_str("```\n");
            }
            continue;
        }
        if s == "``" || s == "`" {
            continue;
        }
        if s.is_empty() && (out.is_empty() || out.ends_with("\n\n")) {
            continue;
        }
        out.push_str(l);
        out.push('\n');
    }
    out.trim().to_string()
}

fn known<'a>(name: &str, tools: &[(&'a str, &'a Value)]) -> Option<(&'a str, &'a Value)> {
    let name = name.trim().trim_start_matches("functions.").trim_start_matches("tools.");
    let name = name.rsplit("__").next().unwrap_or(name);
    tools.iter().find(|t| t.0 == name).copied()
}

/// The calls in a JSON value: `{"name", "arguments"}` (or `parameters`, `args`, `input`), `{"function": {...}}`,
/// `{"tool": ...}`, `{"tool_calls": [...]}` and lists of them.
fn json_calls(v: &Value, tools: &[(&str, &Value)]) -> Vec<(String, Value)> {
    match v {
        Value::Array(a) => {
            let calls: Vec<_> = a.iter().map(|x| json_calls(x, tools)).collect();
            // (all of the list, or none)
            if calls.iter().any(|c| c.is_empty()) { Vec::new() } else { calls.into_iter().flatten().collect() }
        }
        Value::Object(o) => {
            if let Some(list) = o.get("tool_calls") {
                return json_calls(list, tools);
            }
            if let Some(f) = o.get("function").filter(|f| f.is_object()) {
                return json_calls(f, tools);
            }
            let Some(name) = ["name", "tool", "tool_name", "function_name"].iter().find_map(|k| o.get(*k)?.as_str())
            else {
                return Vec::new();
            };
            let Some((name, schema)) = known(name, tools) else { return Vec::new() };
            let args = ["arguments", "parameters", "args", "input", "params"].iter().find_map(|k| o.get(*k));
            let args = match args {
                None => {
                    // only a name (and maybe a type): no arguments
                    if o.keys()
                        .any(|k| !["name", "tool", "tool_name", "function_name", "type", "id"].contains(&k.as_str()))
                    {
                        return Vec::new();
                    }
                    Value::Object(Map::new())
                }
                Some(Value::String(s)) => match serde_json::from_str::<Value>(s) {
                    Ok(v @ Value::Object(_)) => v,
                    _ => return Vec::new(),
                },
                Some(v @ Value::Object(_)) => v.clone(),
                Some(_) => return Vec::new(),
            };
            vec![(name.to_string(), typed(args, schema))]
        }
        _ => Vec::new(),
    }
}

/// The arguments with what was written as text given the type the tool's schema asks for.
fn typed(args: Value, schema: &Value) -> Value {
    let Value::Object(o) = args else { return args };
    Value::Object(
        o.into_iter()
            .map(|(k, v)| {
                let v = match v {
                    Value::String(s) => value_of(&s, &schema["properties"][&k]),
                    v => v,
                };
                (k, v)
            })
            .collect(),
    )
}

/// A value written as text, as the type a property asks for (a string kept a string; else JSON when it is).
fn value_of(s: &str, prop: &Value) -> Value {
    let kind = prop["type"].as_str().unwrap_or("");
    if kind == "string" {
        return Value::String(s.to_string());
    }
    let t = s.trim();
    match serde_json::from_str::<Value>(t) {
        Ok(v) if kind.is_empty() || !v.is_string() => v,
        _ => match t {
            "True" => Value::Bool(true),
            "False" => Value::Bool(false),
            "None" => Value::Null,
            _ => Value::String(s.to_string()),
        },
    }
}

/// A call written as tags at `at` (a `<`): its end and calls. `<tool_call>` (or `<function_calls>`) holding calls,
/// `<function=name>`, `<invoke name="name">` (or `<invoke_name>name</invoke_name>`), `<name>` of a tool offered;
/// the arguments in `<parameter=key>`, `<parameter name="key">` or `<key>` tags, or as a JSON object.
fn tagged(text: &str, at: usize, tools: &[(&str, &Value)]) -> Option<(usize, Vec<(String, Value)>)> {
    let head_end = at + text[at..].find('>')?;
    let head = &text[at + 1..head_end];
    // wrappers
    for wrap in ["tool_call", "tool_calls", "function_calls", "function_call", "tool_use", "tools"] {
        if head.trim() == wrap {
            let close = format!("</{wrap}>");
            let body_end = text[head_end..].find(&close).map(|k| head_end + k).unwrap_or(text.len());
            let end = (body_end + close.len()).min(text.len());
            let body = &text[head_end + 1..body_end];
            let (calls, _) = calls(
                body,
                &tools.iter().map(|t| serde_json::json!({ "name": t.0, "inputSchema": t.1 })).collect::<Vec<_>>(),
            );
            if calls.is_empty() {
                // a form of its own: the tool named in it, the arguments by the names its schema has
                return loose(body, tools).map(|(name, args)| (end, vec![(name, args)]));
            }
            let calls = calls
                .into_iter()
                .map(|c| (c.name, serde_json::from_str(&c.arguments).unwrap_or(Value::Object(Map::new()))))
                .collect();
            return Some((end, calls));
        }
    }
    let (name, close) = if let Some(n) = head.strip_prefix("function=") {
        (n.trim().trim_matches('"').to_string(), "</function>".to_string())
    } else if let Some(rest) = head.strip_prefix("invoke") {
        let rest = rest.trim();
        let name = if let Some(r) = rest.strip_prefix("_name") {
            let r = r.trim_start_matches(['=', ' ']);
            if r.is_empty() {
                // <invoke_name>name</invoke_name>
                let e = text[head_end + 1..].find("</invoke_name>")?;
                text[head_end + 1..head_end + 1 + e].trim().to_string()
            } else {
                r.trim_matches(['"', '\'', '/', ' ']).to_string()
            }
        } else {
            attribute(rest, "name")?
        };
        (name, "</invoke>".to_string())
    } else {
        let n = head.trim().trim_end_matches('/').trim();
        if n.contains(char::is_whitespace) {
            return None;
        }
        (n.to_string(), format!("</{n}>"))
    };
    let (name, schema) = known(&name, tools)?;
    if head.trim_end().ends_with('/') {
        return Some((head_end + 1, vec![(name.to_string(), Value::Object(Map::new()))]));
    }
    let body_from = head_end + 1;
    let (body_end, end) = match text[body_from..].find(close.as_str()) {
        Some(k) => (body_from + k, body_from + k + close.len()),
        // (an unclosed call at the end of the answer)
        None => (text.len(), text.len()),
    };
    let mut body = &text[body_from..body_end];
    if let Some(e) = body.find("</invoke_name>") {
        body = &body[e + "</invoke_name>".len()..];
    }
    let args = parameters(body, schema)?;
    Some((end, vec![(name.to_string(), args)]))
}

/// A call in tags of a form of its own, within a wrapper (`<function_calls><function_call><parameter
/// name="list_operators"><parameter argument_name="query">...`): the first tool offered named in it (a tag's
/// name, an attribute's value or a tag's text), and as its arguments the tags whose name or attribute is a property
/// of the tool's schema, their text the value.
fn loose(body: &str, tools: &[(&str, &Value)]) -> Option<(String, Value)> {
    // the tags in order: (head, start of the text after it)
    let mut tags = Vec::new();
    let mut i = 0;
    while let Some(k) = body[i..].find('<') {
        let at = i + k;
        let he = at + body[at..].find('>')?;
        tags.push((body[at + 1..he].trim(), he + 1));
        i = he + 1;
    }
    let words = |head: &str| -> Vec<String> {
        let mut w = vec![head.split_whitespace().next().unwrap_or("").to_string()];
        let mut r = head;
        while let Some(q) = r.find('"') {
            let Some(e) = r[q + 1..].find('"') else { break };
            w.push(r[q + 1..q + 1 + e].to_string());
            r = &r[q + 2 + e..];
        }
        w
    };
    let text_after = |from: usize| body[from..].split('<').next().unwrap_or("").trim().to_string();
    let (tool, schema) = tags.iter().find_map(|(head, from)| {
        let mut named = words(head);
        named.push(text_after(*from));
        named.iter().find_map(|w| known(w, tools))
    })?;
    let props = schema["properties"].as_object()?;
    let mut args = Map::new();
    for (head, from) in &tags {
        if head.starts_with('/') {
            continue;
        }
        let Some(key) = words(head).into_iter().find(|w| props.contains_key(w.as_str())) else { continue };
        let v = text_after(*from);
        if !v.is_empty() {
            args.insert(key.clone(), value_of(&v, &props[&key]));
        }
    }
    Some((tool.to_string(), Value::Object(args)))
}

/// An attribute's value in a tag's head (`name="x"`).
fn attribute(head: &str, key: &str) -> Option<String> {
    let i = head.find(&format!("{key}="))? + key.len() + 1;
    let r = &head[i..];
    let q = r.chars().next()?;
    if q == '"' || q == '\'' {
        let e = r[1..].find(q)?;
        Some(r[1..1 + e].to_string())
    } else {
        Some(r.split([' ', '/', '>']).next()?.to_string())
    }
}

/// The arguments within a call's tags: a JSON object, or tags of their own.
fn parameters(body: &str, schema: &Value) -> Option<Value> {
    let b = body.trim();
    if b.is_empty() {
        return Some(Value::Object(Map::new()));
    }
    if b.starts_with('{') {
        return match serde_json::from_str::<Value>(b) {
            Ok(v @ Value::Object(_)) => Some(typed(v, schema)),
            _ => None,
        };
    }
    let mut out = Map::new();
    let mut i = 0;
    while let Some(k) = body[i..].find('<') {
        let at = i + k;
        let he = at + body[at..].find('>')?;
        let head = body[at + 1..he].trim();
        if head.starts_with('/') {
            i = he + 1;
            continue;
        }
        let (key, close) = if let Some(n) = head.strip_prefix("parameter=") {
            (n.trim().trim_matches('"').to_string(), "</parameter>".to_string())
        } else if head.starts_with("parameter ") {
            (attribute(head, "name")?, "</parameter>".to_string())
        } else {
            (head.to_string(), format!("</{head}>"))
        };
        let from = he + 1;
        let e = body[from..].find(close.as_str()).map(|k| from + k).unwrap_or(body.len());
        out.insert(key.clone(), value_of(body[from..e].trim_matches(['\n', '\r']), &schema["properties"][&key]));
        i = (e + close.len()).min(body.len());
    }
    (!out.is_empty()).then_some(Value::Object(out))
}

/// A call written as Python at `at`: `name(key="value", ...)` (or `name({...})`, `name("value")`) of a tool
/// offered, its end, name and arguments.
fn python(text: &str, at: usize, tools: &[(&str, &Value)]) -> Option<(usize, String, Value)> {
    let s = &text[at..];
    let open = s.find('(')?;
    let name = s[..open].trim_end();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
        return None;
    }
    let (name, schema) = known(name, tools)?;
    let mut p = Py { s: &s[open + 1..], i: 0 };
    let mut out = Map::new();
    let mut positional = Vec::new();
    loop {
        p.space();
        if p.eat(')') {
            break;
        }
        // key=value, or a value
        let save = p.i;
        let key = p.ident();
        p.space();
        if let Some(k) = key.filter(|_| p.peek() == Some('=') && p.s[p.i..].chars().nth(1) != Some('=')) {
            p.i += 1;
            let v = p.value()?;
            out.insert(k, v);
        } else {
            p.i = save;
            positional.push(p.value()?);
        }
        p.space();
        if p.eat(',') {
            continue;
        }
        p.space();
        if !p.eat(')') {
            return None;
        }
        break;
    }
    let required: Vec<&str> =
        schema["required"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    for (k, v) in positional.into_iter().enumerate() {
        match v {
            Value::Object(o) if k == 0 && out.is_empty() => out = o,
            v => {
                let key = required.iter().find(|r| !out.contains_key(**r))?;
                out.insert(key.to_string(), v);
            }
        }
    }
    let end = at + open + 1 + p.i;
    Some((end, name.to_string(), typed(Value::Object(out), schema)))
}

/// A reader of Python's literals.
struct Py<'a> {
    s: &'a str,
    i: usize,
}

impl Py<'_> {
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += self.peek().map_or(1, char::len_utf8);
        }
    }

    fn ident(&mut self) -> Option<String> {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            self.i += self.peek().map_or(1, char::len_utf8);
        }
        (self.i > start && !self.s[start..].starts_with(|c: char| c.is_ascii_digit()))
            .then(|| self.s[start..self.i].to_string())
            .or_else(|| {
                self.i = start;
                None
            })
    }

    fn value(&mut self) -> Option<Value> {
        self.space();
        match self.peek()? {
            '"' | '\'' => self.string().map(Value::String),
            '[' | '(' => {
                let close = if self.eat('[') {
                    ']'
                } else {
                    self.i += 1;
                    ')'
                };
                let mut a = Vec::new();
                loop {
                    self.space();
                    if self.eat(close) {
                        return Some(Value::Array(a));
                    }
                    a.push(self.value()?);
                    self.space();
                    if !self.eat(',') {
                        self.space();
                        return self.eat(close).then_some(Value::Array(a));
                    }
                }
            }
            '{' => {
                self.i += 1;
                let mut o = Map::new();
                loop {
                    self.space();
                    if self.eat('}') {
                        return Some(Value::Object(o));
                    }
                    let k = match self.value()? {
                        Value::String(s) => s,
                        v => v.to_string(),
                    };
                    self.space();
                    if !self.eat(':') {
                        return None;
                    }
                    let v = self.value()?;
                    o.insert(k, v);
                    self.space();
                    if !self.eat(',') {
                        self.space();
                        return self.eat('}').then_some(Value::Object(o));
                    }
                }
            }
            _ => {
                let start = self.i;
                while self.peek().is_some_and(|c| c.is_alphanumeric() || "._-+".contains(c)) {
                    self.i += self.peek().map_or(1, char::len_utf8);
                }
                let w = &self.s[start..self.i];
                match w {
                    "" => None,
                    "True" | "true" => Some(Value::Bool(true)),
                    "False" | "false" => Some(Value::Bool(false)),
                    "None" | "null" => Some(Value::Null),
                    _ => match serde_json::from_str::<Value>(w) {
                        Ok(v @ Value::Number(_)) => Some(v),
                        // (a bare word: a name)
                        _ => Some(Value::String(w.to_string())),
                    },
                }
            }
        }
    }

    fn string(&mut self) -> Option<String> {
        let q = self.peek()?;
        let triple: String = std::iter::repeat_n(q, 3).collect();
        let long = self.s[self.i..].starts_with(&triple);
        self.i += if long { 3 } else { 1 };
        let mut out = String::new();
        loop {
            if long && self.s[self.i..].starts_with(&triple) {
                self.i += 3;
                return Some(out);
            }
            let c = self.peek()?;
            self.i += c.len_utf8();
            match c {
                '\\' => {
                    let e = self.peek()?;
                    self.i += e.len_utf8();
                    out.push(match e {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        e => e,
                    });
                }
                c if c == q && !long => return Some(out),
                c => out.push(c),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn offered() -> Vec<Value> {
        vec![
            json!({ "name": "list_operators", "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } } } }),
            json!({ "name": "edit_node", "inputSchema": { "type": "object", "required": ["node"], "properties": {
                "node": { "type": "string" }, "settings": { "type": "object" }, "note": { "type": "string" } } } }),
            json!({ "name": "run_model", "inputSchema": { "type": "object", "properties": {} } }),
            json!({ "name": "add_node", "inputSchema": { "type": "object", "properties": {
                "id": { "type": "string" }, "op": { "type": "string" }, "settings": { "type": "object" } } } }),
        ]
    }

    fn one(text: &str) -> (String, Value, String) {
        let (calls, rest) = calls(text, &offered());
        assert_eq!(calls.len(), 1, "{text}: {calls:?}");
        let c = &calls[0];
        (c.name.clone(), serde_json::from_str(&c.arguments).unwrap(), rest)
    }

    #[test]
    fn a_call_written_as_python_is_found() {
        let (name, args, rest) =
            one("I will set it.\n\nedit_node(node=\"a\", settings={\"threshold\": 0.5, \"keep\": True})\n");
        assert_eq!(name, "edit_node");
        assert_eq!(args, json!({ "node": "a", "settings": { "threshold": 0.5, "keep": true } }));
        assert_eq!(rest, "I will set it.");
        let (_, args, _) = one("```python\nedit_node('a', note='it\\'s the forest')\n```");
        assert_eq!(args, json!({ "node": "a", "note": "it's the forest" }));
        let (name, args, rest) = one("`run_model()`");
        assert_eq!((name.as_str(), args), ("run_model", json!({})));
        assert_eq!(rest, "");
        // in a sentence, or not a tool offered: no call
        assert!(calls("You may call edit_node(node=\"a\") later.", &offered()).0.is_empty());
        assert!(calls("print(\"x\")", &offered()).0.is_empty());
    }

    #[test]
    fn a_call_written_as_json_is_found() {
        let text = "Let me look.\n```json\n{\"name\": \"list_operators\", \"arguments\": {\"query\": \"edge density\"}}\n```\n";
        let (name, args, rest) = one(text);
        assert_eq!((name.as_str(), args), ("list_operators", json!({ "query": "edge density" })));
        assert_eq!(rest, "Let me look.");
        // arguments as a string, Hermes' tags, a list of two, an MCP name
        let (_, args, _) = one("{\"name\": \"edit_node\", \"arguments\": \"{\\\"node\\\": \\\"b\\\"}\"}");
        assert_eq!(args, json!({ "node": "b" }));
        let (name, _, rest) = one("<tool_call>\n{\"name\": \"run_model\", \"arguments\": {}}\n</tool_call>");
        assert_eq!((name.as_str(), rest.as_str()), ("run_model", ""));
        let (two, _) = calls(
            "[{\"name\": \"run_model\"}, {\"function\": {\"name\": \"mcp__terraflux__list_operators\", \"parameters\": {\"query\": \"x\"}}}]",
            &offered(),
        );
        assert_eq!(two.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["run_model", "list_operators"]);
        // JSON that is not a call stays
        let (none, rest) = calls("The settings: {\"threshold\": 0.5}", &offered());
        assert!(none.is_empty());
        assert_eq!(rest, "The settings: {\"threshold\": 0.5}");
    }

    #[test]
    fn a_call_written_as_tags_is_found() {
        let (name, args, rest) = one("Searching.\n<list_operators>\n<query>edge density</query>\n</list_operators>");
        assert_eq!(
            (name.as_str(), args, rest.as_str()),
            ("list_operators", json!({ "query": "edge density" }), "Searching.")
        );
        let (name, args, _) = one(
            "<tool_call>\n<function=edit_node>\n<parameter=node>\na\n</parameter>\n<parameter=settings>\n{\"n\": 3}\n</parameter>\n</function>\n</tool_call>",
        );
        assert_eq!((name.as_str(), args), ("edit_node", json!({ "node": "a", "settings": { "n": 3 } })));
        let (name, args, _) = one(
            "<invoke name=\"add_node\">\n<parameter name=\"id\">grow</parameter>\n<parameter name=\"op\">raster.calc</parameter>\n</invoke>",
        );
        assert_eq!((name.as_str(), args), ("add_node", json!({ "id": "grow", "op": "raster.calc" })));
        let (name, args, _) = one("<invoke_name>edit_node</invoke_name>\n<node>a</node>\n</invoke>");
        assert_eq!((name.as_str(), args), ("edit_node", json!({ "node": "a" })));
        let (name, _, _) = one("<run_model/>");
        assert_eq!(name, "run_model");
        // a form of the model's own (Qwen, 2026-11-03), its closing tags wrong
        let (name, args, rest) = one(
            "Primeiro, vou verificar.\n\n```xml\n<function_calls>\n<function_call>\n<parameter name=\"list_operators\">\n<parameter argument_name=\"query\">\nrandom cluster landscape\n</parameter>\n</function_call>\n</function_call>\n```",
        );
        assert_eq!((name.as_str(), args), ("list_operators", json!({ "query": "random cluster landscape" })));
        assert_eq!(rest, "Primeiro, vou verificar.");
        // tags of no tool stay
        assert!(calls("A <b>bold</b> word, x < 3", &offered()).0.is_empty());
    }

    #[test]
    fn a_call_announced_and_not_made_is_told() {
        let o = offered();
        assert_eq!(announced("Now let me run the model with run_model.", &o).as_deref(), Some("run_model"));
        assert_eq!(announced("Vou chamar o edit_node agora", &o).as_deref(), Some("edit_node"));
        assert_eq!(announced("The next step is:", &o).as_deref(), Some(""));
        assert_eq!(
            announced("The model ran: 3 maps were written. check it with run_model if you change it.", &o),
            None
        );
        assert_eq!(announced("Done.", &o), None);
        // the answer ending on a promise to act
        assert_eq!(
            announced("Sure. I'll use `control.repeat`.\n\nLet me build this model now.\n\n", &o).as_deref(),
            Some("")
        );
        assert_eq!(announced("The number is 10.\n\nLet me know if you want more steps?", &o), None);
        assert_eq!(announced("The number is 10.\n\nLet me know if you want more steps.", &o), None);
        assert_eq!(announced("It ran.\n\nI'll leave the rest to you. The maps are in out/.", &o), None);
        // a tool named in what looks like code
        assert_eq!(
            announced("Searching.\n\n<call tool=\"list_operators\" q=\"x\"/>", &o).as_deref(),
            Some("list_operators")
        );
        assert!(ask_again("run_model").contains("`run_model`"));
    }
}
