//! The assistant's chat models: a client of the API of OpenAI's chat completions (`/v1/chat/completions`, with
//! tools), which the servers of open models speak (vLLM, LiteLLM, Ollama, llama.cpp, LM Studio, the universities'
//! services) as do the commercial providers (OpenAI, Anthropic's compatible API...). A provider is its address,
//! its model and whether it asks for a key; the key is kept in the system's store of credentials, never in the
//! settings nor in a model.

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A server of chat models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Provider {
    pub name: String,
    /// The API's address, up to its version (`https://.../v1`).
    pub base_url: String,
    pub model: String,
    /// The server asks for a key (kept in the system's store of credentials: [`key`]).
    pub key: bool,
    /// The model thinks before it answers (Qwen 3 and others served by vLLM: `enable_thinking`).
    pub thinking: bool,
    /// Only a few tools offered (explain, change settings, run, check): for small models.
    pub few_tools: bool,
    /// What to know of the server (shown under its settings).
    pub about: String,
}

impl Provider {
    fn new(name: &str, base_url: &str, model: &str, key: bool, about: &str) -> Provider {
        Provider {
            name: name.into(),
            base_url: base_url.into(),
            model: model.into(),
            key,
            thinking: false,
            few_tools: false,
            about: about.into(),
        }
    }
}

/// The providers known, as they first are in the settings.
pub fn presets() -> Vec<Provider> {
    vec![
        Provider::new(
            "HU Berlin (KI-Service)",
            "https://llm3-compute.cms.hu-berlin.de/v1",
            "llm3",
            false,
            "The open models of the Humboldt-Universität's computing centre (Qwen): from the university's network or \
             its VPN, no key. ki.cms.hu-berlin.de/de/apis",
        ),
        Provider::new(
            "GWDG Chat AI (Academic Cloud)",
            "https://chat-ai.academiccloud.de/v1",
            "",
            true,
            "The open models of the GWDG for German universities; the key is asked for at kisski.gwdg.de.",
        ),
        Provider::new(
            "Ollama (this computer)",
            "http://localhost:11434/v1",
            "",
            false,
            "A model run by Ollama on this computer: offline, no key (ollama pull qwen3, for one).",
        ),
        Provider::new(
            "LM Studio (this computer)",
            "http://localhost:1234/v1",
            "",
            false,
            "A model run by LM Studio's server on this computer: offline, no key.",
        ),
        Provider::new("OpenAI", "https://api.openai.com/v1", "", true, "OpenAI's models, with a key of its API."),
        Provider::new(
            "Claude (Anthropic)",
            "https://api.anthropic.com/v1",
            "claude-sonnet-5-5",
            true,
            "Anthropic's models through its API compatible with OpenAI's, with a key of its API.",
        ),
        Provider::new("Other", "http://localhost:8000/v1", "", false, "Any server of OpenAI's chat completions API."),
    ]
}

/// The provider's key in the system's store of credentials (by its address: the key follows the server).
pub fn key(p: &Provider) -> Option<String> {
    keyring::Entry::new("Terraflux", &p.base_url).ok()?.get_password().ok().filter(|k| !k.is_empty())
}

/// Keep the provider's key in the system's store of credentials (empty: forgotten).
pub fn set_key(p: &Provider, key: &str) -> Result<(), String> {
    let entry = keyring::Entry::new("Terraflux", &p.base_url).map_err(|e| e.to_string())?;
    if key.trim().is_empty() {
        return match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        };
    }
    entry.set_password(key.trim()).map_err(|e| e.to_string())
}

/// A provider with its key: what the requests need.
#[derive(Clone)]
pub struct Endpoint {
    pub provider: Provider,
    pub key: Option<String>,
}

impl Endpoint {
    pub fn of(p: &Provider) -> Endpoint {
        Endpoint { provider: p.clone(), key: if p.key { key(p) } else { None } }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{path}", self.provider.base_url.trim().trim_end_matches('/'))
    }

    fn request<B>(&self, r: ureq::RequestBuilder<B>) -> ureq::RequestBuilder<B> {
        // (a server that asks for no key may still want the header)
        let key = self.key.clone().unwrap_or_else(|| "none".into());
        let mut r = r.header("Authorization", &format!("Bearer {key}"));
        if self.provider.base_url.contains("anthropic.com") {
            r = r.header("x-api-key", &key).header("anthropic-version", "2023-06-01");
        }
        r
    }
}

fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        // (a model may think a while before its first word)
        .timeout_recv_response(Some(Duration::from_secs(600)))
        // (a server that stops in the middle of an answer)
        .timeout_recv_body(Some(Duration::from_secs(900)))
        .http_status_as_error(false)
        .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
        .user_agent(format!("Terraflux/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// A failure of a request, said as what to do about it.
fn failed(e: ureq::Error, e_url: &str) -> String {
    let hint = match &e {
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => {
            " (is the server running, and is this computer in its network? A university's server often answers \
             only from its network or its VPN)"
        }
        ureq::Error::Timeout(_) => " (the server did not answer in time)",
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => " (the server's certificate is not trusted)",
        _ => "",
    };
    format!("{e_url}: {e}{hint}")
}

/// What a server's answer of an error says (OpenAI's `{"error": {"message"}}`, or its text).
fn error_of(status: u16, body: &str) -> String {
    let said = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            let e = &v["error"];
            e["message"].as_str().or(e.as_str()).or(v["detail"].as_str()).or(v["message"].as_str()).map(String::from)
        })
        .unwrap_or_else(|| body.chars().take(400).collect());
    let hint = match status {
        401 | 403 => " (the key is missing or wrong)",
        404 => " (the address or the model's name is wrong)",
        429 => " (too many requests: wait a moment)",
        _ => "",
    };
    format!("HTTP {status}: {said}{hint}")
}

/// The models the server has.
pub fn models(e: &Endpoint) -> Result<Vec<String>, String> {
    let url = e.url("models");
    let mut res = e.request(agent().get(&url)).call().map_err(|x| failed(x, &url))?;
    let status = res.status().as_u16();
    let body = res.body_mut().read_to_string().map_err(|x| failed(x, &url))?;
    if status != 200 {
        return Err(error_of(status, &body));
    }
    let v: Value = serde_json::from_str(&body).map_err(|x| format!("{url}: not JSON: {x}"))?;
    let mut ids: Vec<String> =
        v["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str().map(String::from)).collect();
    ids.sort();
    Ok(ids)
}

/// A tool the model asked to call: its id, name and arguments (JSON text).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Call {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// A model's answer: its text, its thinking, the tools it calls.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Answer {
    pub text: String,
    pub thinking: String,
    pub calls: Vec<Call>,
    pub finish: String,
}

impl Answer {
    /// The answer as a message of the conversation.
    pub fn message(&self) -> Value {
        let mut m = json!({ "role": "assistant", "content": self.text });
        if !self.calls.is_empty() {
            m["tool_calls"] = self
                .calls
                .iter()
                .map(|c| json!({ "id": c.id, "type": "function", "function": { "name": c.name, "arguments": c.arguments } }))
                .collect();
        }
        m
    }
}

/// What comes of an answer as it is written.
pub enum Delta {
    Text(String),
    Thinking(String),
}

/// An answer as it streams in: its parts put together (the text, the thinking, the calls' names and arguments in
/// pieces), and the thinking a model writes in its text (`<think>...</think>`) told apart.
#[derive(Default)]
struct Gather {
    answer: Answer,
    /// In the text: within `<think>`; what is held back in case it is the start of a tag.
    thinking: bool,
    held: String,
}

impl Gather {
    fn chunk(&mut self, v: &Value, on: &mut dyn FnMut(Delta)) {
        let Some(choice) = v["choices"].get(0) else { return };
        // (a whole message, when the server did not stream)
        let d = if choice["delta"].is_object() { &choice["delta"] } else { &choice["message"] };
        for key in ["reasoning_content", "reasoning"] {
            if let Some(t) = d[key].as_str().filter(|t| !t.is_empty()) {
                self.answer.thinking.push_str(t);
                on(Delta::Thinking(t.into()));
            }
        }
        if let Some(t) = d["content"].as_str() {
            self.text(t, on);
        }
        for (k, c) in d["tool_calls"].as_array().into_iter().flatten().enumerate() {
            let at = c["index"].as_u64().map_or(k, |i| i as usize);
            while self.answer.calls.len() <= at {
                self.answer.calls.push(Call::default());
            }
            let call = &mut self.answer.calls[at];
            if let Some(id) = c["id"].as_str() {
                call.id = id.into();
            }
            let f = &c["function"];
            if let Some(n) = f["name"].as_str() {
                call.name.push_str(n);
            }
            match &f["arguments"] {
                Value::String(a) => call.arguments.push_str(a),
                Value::Null => {}
                other => call.arguments.push_str(&other.to_string()),
            }
        }
        if let Some(f) = choice["finish_reason"].as_str() {
            self.answer.finish = f.into();
        }
    }

    /// A piece of the text, the thinking within `<think>` told apart.
    fn text(&mut self, t: &str, on: &mut dyn FnMut(Delta)) {
        self.held.push_str(t);
        loop {
            let tag = if self.thinking { "</think>" } else { "<think>" };
            if let Some(i) = self.held.find(tag) {
                let before: String = self.held[..i].into();
                self.emit(before, on);
                self.held.drain(..i + tag.len());
                self.thinking = !self.thinking;
                continue;
            }
            // what may be the start of a tag is held back
            let keep = (1..tag.len()).rev().find(|&n| self.held.ends_with(&tag[..n])).unwrap_or(0);
            let cut = self.held.len() - keep;
            let out: String = self.held.drain(..cut).collect();
            self.emit(out, on);
            return;
        }
    }

    fn emit(&mut self, t: String, on: &mut dyn FnMut(Delta)) {
        if t.is_empty() {
            return;
        }
        if self.thinking {
            self.answer.thinking.push_str(&t);
            on(Delta::Thinking(t));
        } else {
            self.answer.text.push_str(&t);
            on(Delta::Text(t));
        }
    }

    fn end(mut self, on: &mut dyn FnMut(Delta)) -> Answer {
        let rest = std::mem::take(&mut self.held);
        self.emit(rest, on);
        // (calls without an id, from some servers)
        for (k, c) in self.answer.calls.iter_mut().enumerate() {
            if c.id.is_empty() {
                c.id = format!("call_{k}");
            }
        }
        self.answer.calls.retain(|c| !c.name.is_empty());
        self.answer.text = self.answer.text.trim().to_string();
        self.answer
    }
}

/// Ask the model for the next message of a conversation (the system's message first), with the tools it may call
/// (as [`crate::tools::functions`] gives them): its answer, streamed to `on` as it is written. `stop` ends it.
pub fn complete(
    e: &Endpoint,
    messages: &[Value],
    tools: &[Value],
    on: &mut dyn FnMut(Delta),
    stop: &AtomicBool,
) -> Result<Answer, String> {
    let p = &e.provider;
    if p.model.trim().is_empty() {
        return Err(format!("{}: no model chosen (the assistant's settings list the server's models)", p.name));
    }
    let mut body = json!({ "model": p.model.trim(), "messages": messages, "stream": true });
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools.to_vec());
    }
    if p.thinking {
        body["chat_template_kwargs"] = json!({ "enable_thinking": true });
    }
    let url = e.url("chat/completions");
    let mut res = e
        .request(agent().post(&url))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .send(body.to_string().as_bytes())
        .map_err(|x| failed(x, &url))?;
    let status = res.status().as_u16();
    if status != 200 {
        let text = res.body_mut().read_to_string().unwrap_or_default();
        return Err(error_of(status, &text));
    }
    let mut g = Gather::default();
    let reader = BufReader::new(res.body_mut().as_reader());
    let mut whole = String::new();
    for line in reader.lines() {
        if stop.load(Ordering::Relaxed) {
            return Err("stopped".into());
        }
        let line = line.map_err(|x| format!("{url}: {x}"))?;
        let Some(data) = line.strip_prefix("data:") else {
            // (not streamed: the whole answer at once)
            whole.push_str(&line);
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        if let Ok(v) = serde_json::from_str::<Value>(data) {
            if v["error"].is_object() {
                return Err(error_of(status, data));
            }
            g.chunk(&v, on);
        }
    }
    if !whole.trim().is_empty() {
        let v: Value = serde_json::from_str(&whole).map_err(|x| format!("{url}: not JSON: {x}"))?;
        g.chunk(&v, on);
    }
    Ok(g.end(on))
}

/// What a test of a provider found: each check, passed or not, and what it found.
pub type Checks = Vec<(bool, String)>;

/// Test a provider: whether its server answers, has the model, and calls a tool when asked to.
pub fn test(e: &Endpoint) -> Checks {
    let mut found = Vec::new();
    let p = &e.provider;
    if p.key && e.key.is_none() {
        found.push((false, "no key kept for this server: give it in the settings".into()));
    }
    match models(e) {
        Ok(ms) => {
            found.push((true, format!("the server answers ({} model(s): {})", ms.len(), ms.join(", "))));
            if !p.model.is_empty() && !ms.is_empty() && !ms.iter().any(|m| m == p.model.trim()) {
                found.push((false, format!("the server has no model {:?}", p.model)));
                return found;
            }
        }
        Err(x) => {
            found.push((false, x));
            return found;
        }
    }
    let ping = json!([{
        "type": "function",
        "function": {
            "name": "ping",
            "description": "Answer a ping.",
            "parameters": { "type": "object", "properties": { "word": { "type": "string" } }, "required": ["word"] },
        },
    }]);
    let messages = [json!({ "role": "user", "content": "Call the tool ping with the word \"terraflux\"." })];
    let t = std::time::Instant::now();
    match complete(e, &messages, ping.as_array().unwrap(), &mut |_| {}, &AtomicBool::new(false)) {
        Ok(a) if a.calls.iter().any(|c| c.name == "ping") => found.push((
            true,
            format!("the model {} calls tools (answered in {:.1} s)", p.model, t.elapsed().as_secs_f64()),
        )),
        Ok(a) => found.push((
            false,
            format!(
                "the model answered without calling the tool: the server may not let it call tools (vLLM: \
                 --enable-auto-tool-choice and --tool-call-parser). It said: {}",
                a.text.chars().take(200).collect::<String>()
            ),
        )),
        Err(x) => found.push((false, x)),
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gathered(chunks: &[Value]) -> (Answer, String) {
        let mut g = Gather::default();
        let mut shown = String::new();
        let mut on = |d: Delta| {
            if let Delta::Text(t) = d {
                shown.push_str(&t)
            }
        };
        for c in chunks {
            g.chunk(c, &mut on);
        }
        (g.end(&mut on), shown)
    }

    #[test]
    fn a_streamed_answer_is_put_together() {
        let chunk = |d: Value| json!({ "choices": [{ "index": 0, "delta": d }] });
        let (a, _) = gathered(&[
            chunk(json!({ "role": "assistant", "content": "" })),
            chunk(
                json!({ "tool_calls": [{ "id": "c1", "index": 0, "function": { "name": "get", "arguments": "" } }] }),
            ),
            chunk(json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "{\"city\": " } }] })),
            chunk(json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "\"Berlin\"}" } }] })),
            json!({ "choices": [{ "index": 0, "delta": {}, "finish_reason": "tool_calls" }] }),
        ]);
        assert_eq!(
            a.calls,
            vec![Call { id: "c1".into(), name: "get".into(), arguments: "{\"city\": \"Berlin\"}".into() }]
        );
        assert_eq!(a.finish, "tool_calls");
        assert_eq!(a.message()["tool_calls"][0]["function"]["name"], "get");
    }

    #[test]
    fn the_thinking_in_the_text_is_told_apart() {
        let chunk = |t: &str| json!({ "choices": [{ "index": 0, "delta": { "content": t } }] });
        let (a, shown) =
            gathered(&[chunk("<thi"), chunk("nk>let me see</th"), chunk("ink>\n\nHello"), chunk(" there")]);
        assert_eq!(a.thinking, "let me see");
        assert_eq!(a.text, "Hello there");
        assert_eq!(shown.trim(), "Hello there");
        // an answer not streamed
        let (a, _) = gathered(&[json!({ "choices": [{ "message": { "content": "Hi", "reasoning_content": "hm" } }] })]);
        assert_eq!((a.text.as_str(), a.thinking.as_str()), ("Hi", "hm"));
    }

    #[test]
    fn a_server_error_is_said() {
        let e = error_of(401, r#"{"error": {"message": "invalid key"}}"#);
        assert!(e.contains("invalid key") && e.contains("key is missing"), "{e}");
    }

    /// The Humboldt-Universität's server, when this computer reaches it (its network or VPN):
    /// `cargo test -p terraflux-app hu_berlin -- --ignored`.
    #[test]
    #[ignore]
    fn hu_berlin() {
        let p = presets().into_iter().next().unwrap();
        let checks = test(&Endpoint::of(&p));
        for (ok, what) in &checks {
            println!("{} {what}", if *ok { "ok  " } else { "FAIL" });
        }
        assert!(checks.iter().all(|(ok, _)| *ok));
    }
}
