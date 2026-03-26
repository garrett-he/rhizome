//! LLM 服务：OpenAI 兼容调用与编译响应解析（移植 deprecated core/llm.py）。
//!
//! `LlmClient` 抽象单轮 chat 调用，测试可注入确定性 fake；`OpenAiClient` 用
//! reqwest 调 `{base_url}/chat/completions`。响应解析对齐旧版：容忍外层
//! code fence，行级 YAML front matter 提供元数据（title/summary/language/tags），
//! title 缺失时从正文首个 Markdown 标题恢复。

use serde::Serialize;
use serde_json::{Map, Value};

/// OpenAI 兼容 API 的最小配置。
#[derive(Debug, Clone, PartialEq)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// 单轮 chat 调用：输入完整提示词，返回模型文本响应。
pub trait LlmClient: Send + Sync {
    fn model(&self) -> &str;
    fn chat(&self, prompt: &str) -> Result<String, String>;
}

/// OpenAI 兼容客户端（阻塞调用，请在阻塞线程中使用）。
pub struct OpenAiClient {
    config: LlmConfig,
    http: reqwest::blocking::Client,
}

impl OpenAiClient {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            config,
            // 连接超时 + 总超时：服务挂起时命令最终会返回而不是永久阻塞。
            // 总超时覆盖流式响应全程，取宽值（QA 流式极少超过 10 分钟）。
            http: reqwest::blocking::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
        }
    }
}

impl OpenAiClient {
    /// 连接测试：发送一条 max_tokens=1 的最小请求，只校验 HTTP 成功。
    pub fn test_connection(&self) -> Result<(), String> {
        let url = format!("{}/chat/completions", self.config.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 1,
        });
        let response = match self
            .http
            .post(&url)
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
        {
            Ok(response) => response,
            Err(e) => {
                crate::logging::log("ERROR", &format!("LLM 连接测试失败: {url}: {e}"));
                return Err(format!("LLM 请求失败: {e}"));
            }
        };
        let status = response.status();
        if !status.is_success() {
            let text = response.text().unwrap_or_default();
            let message = format!("LLM 返回 {status}: {}", truncate(&text, 300));
            crate::logging::log("ERROR", &format!("LLM 连接测试失败: {url}: {message}"));
            return Err(message);
        }
        Ok(())
    }
}

impl LlmClient for OpenAiClient {
    fn model(&self) -> &str {
        &self.config.model
    }

    fn chat(&self, prompt: &str) -> Result<String, String> {
        let url = format!("{}/chat/completions", self.config.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [{"role": "user", "content": prompt}],
        });
        let response = match self
            .http
            .post(&url)
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
        {
            Ok(response) => response,
            Err(e) => {
                crate::logging::log("ERROR", &format!("LLM 请求失败: {url}: {e}"));
                return Err(format!("LLM 请求失败: {e}"));
            }
        };
        let status = response.status();
        let text = match response.text() {
            Ok(text) => text,
            Err(e) => {
                crate::logging::log("ERROR", &format!("LLM 响应读取失败: {url}: {e}"));
                return Err(format!("LLM 响应读取失败: {e}"));
            }
        };
        if !status.is_success() {
            let message = format!("LLM 返回 {status}: {}", truncate(&text, 300));
            crate::logging::log("ERROR", &format!("LLM 请求失败: {url}: {message}"));
            return Err(message);
        }
        let json = match serde_json::from_str::<Value>(&text) {
            Ok(json) => json,
            Err(e) => {
                crate::logging::log("ERROR", &format!("LLM 响应不是有效 JSON: {url}: {e}"));
                return Err(format!("LLM 响应不是有效 JSON: {e}"));
            }
        };
        let content = json
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| "LLM 响应缺少 choices[0].message.content".to_string())?;
        Ok(content.to_string())
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let head: String = text.chars().take(max).collect();
    format!("{head}…")
}

/// 流式 chat：每收到一个 delta 回调一次，返回拼接后的完整文本。
pub trait StreamLlmClient: Send + Sync {
    ///
    /// # Arguments
    /// * `prompt` - 完整提示词（单轮 user 消息）
    /// * `on_delta` - 每个增量文本片段回调
    fn chat_stream(&self, prompt: &str, on_delta: &mut dyn FnMut(&str)) -> Result<String, String>;
}

impl StreamLlmClient for OpenAiClient {
    /// `stream: true` 请求 + SSE 逐行解析（blocking Response 逐块迭代）。
    /// 行解析容错：单行 JSON 失败仅 log 跳过，不中断整个流。
    fn chat_stream(&self, prompt: &str, on_delta: &mut dyn FnMut(&str)) -> Result<String, String> {
        let url = format!("{}/chat/completions", self.config.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [{"role": "user", "content": prompt}],
            "stream": true,
        });
        let response = match self
            .http
            .post(&url)
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
        {
            Ok(response) => response,
            Err(e) => {
                crate::logging::log("ERROR", &format!("LLM 流式请求失败: {url}: {e}"));
                return Err(format!("LLM 请求失败: {e}"));
            }
        };
        let status = response.status();
        if !status.is_success() {
            let text = response.text().unwrap_or_default();
            let message = format!("LLM 返回 {status}: {}", truncate(&text, 300));
            crate::logging::log("ERROR", &format!("LLM 流式请求失败: {url}: {message}"));
            return Err(message);
        }

        let mut full = String::new();
        // blocking Response 实现 io::Read：BufRead::read_line 逐行读取，实现真流式
        let mut reader = std::io::BufReader::new(response);
        loop {
            let mut line = String::new();
            match std::io::BufRead::read_line(&mut reader, &mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if let Some(delta) = parse_sse_line(line.trim_end_matches(['\n', '\r'])) {
                        on_delta(&delta);
                        full.push_str(&delta);
                    }
                }
                Err(e) => {
                    crate::logging::log("ERROR", &format!("LLM 流式读取失败: {url}: {e}"));
                    return Err(format!("LLM 响应读取失败: {e}"));
                }
            }
        }
        if full.is_empty() {
            let message = "LLM 流式响应为空".to_string();
            crate::logging::log("ERROR", &format!("LLM 流式请求失败: {url}: {message}"));
            return Err(message);
        }
        Ok(full)
    }
}

/// 解析一行 SSE：`data: {...}` 取 `choices[0].delta.content`；`[DONE]` 返回 None。
fn parse_sse_line(line: &str) -> Option<String> {
    let payload = line.strip_prefix("data: ")?;
    let payload = payload.trim();
    if payload == "[DONE]" || payload.is_empty() {
        return None;
    }
    match serde_json::from_str::<Value>(payload) {
        Ok(json) => json
            .pointer("/choices/0/delta/content")
            .and_then(Value::as_str)
            .map(str::to_string),
        Err(e) => {
            crate::logging::log("ERROR", &format!("SSE 行解析失败（已跳过）: {e}"));
            None
        }
    }
}

/// 单个文档的编译结果。
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct CompileOutcome {
    pub document_id: String,
    pub title: Option<String>,
    pub tags: Option<Vec<String>>,
    pub ok: bool,
    pub error: Option<String>,
}

/// 解析 LLM 响应为（元数据 JSON 对象, 正文）。
///
/// 剥掉外层 code fence 后按行解析 front matter（`---` 围栏，行级 `key: value`，
/// 方括号/逗号值转为字符串列表，无法解析的行拼接到上一个字段）；front matter
/// 缺失或为空时整个文本即正文，title 从首个 Markdown 标题恢复。
pub fn parse_response(content: &str) -> Result<(Value, String), String> {
    let text = strip_code_fence(content).trim();
    if text.is_empty() {
        return Err("LLM 返回了空响应".to_string());
    }
    let (mut metadata, remainder) = split_front_matter(text);
    if !metadata.contains_key("title") {
        if let Some(title) = first_heading_title(&remainder) {
            metadata.insert("title".to_string(), Value::String(title));
        }
    }
    Ok((Value::Object(metadata), remainder))
}

/// 剥掉环绕整个响应的 Markdown code fence（可带语言标签）。
fn strip_code_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let Some(after) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let rest = after.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    // 语言标签后必须先有空白行再换行进入正文
    let Some(newline) = rest.find('\n') else {
        return trimmed;
    };
    if !rest[..newline].chars().all(char::is_whitespace) {
        return trimmed;
    }
    let Some(content) = rest[newline + 1..].strip_suffix("```") else {
        return trimmed;
    };
    content.strip_suffix('\n').unwrap_or(content)
}

/// 拆出开头的 front matter 块；无法解析时返回 (空对象, 原文)。
fn split_front_matter(text: &str) -> (Map<String, Value>, String) {
    let lines: Vec<&str> = text.lines().collect();
    if lines.first().map(|l| l.trim()) != Some("---") {
        return (Map::new(), text.to_string());
    }
    let Some(closing) = lines[1..].iter().position(|l| *l == "---") else {
        return (Map::new(), text.to_string());
    };
    let closing = closing + 1;
    let mut metadata = Map::new();
    let mut last_key: Option<String> = None;
    for line in &lines[1..closing] {
        match front_matter_field(line) {
            Some((key, value)) => {
                last_key = Some(key.to_string());
                metadata.insert(key.to_string(), parse_value(key, value));
            }
            None => {
                if let (Some(key), true) = (last_key.as_deref(), !line.trim().is_empty()) {
                    let joined = match metadata.get(key).and_then(Value::as_str) {
                        Some(previous) => format!("{previous} {}", line.trim()),
                        None => line.trim().to_string(),
                    };
                    metadata.insert(key.to_string(), Value::String(joined));
                }
            }
        }
    }
    if metadata.is_empty() {
        return (Map::new(), text.to_string());
    }
    let body = lines[closing + 1..].join("\n");
    (metadata, body.trim_start_matches('\n').to_string())
}

/// 匹配 front matter 字段行 `key: value`（key 首字符为字母或下划线，其余为字母/数字/下划线/连字符）。
fn front_matter_field(line: &str) -> Option<(&str, &str)> {
    let idx = line.find(':')?;
    let key = &line[..idx];
    let mut chars = key.chars();
    let first = chars.next()?;
    if !(first.is_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    Some((key, &line[idx + 1..]))
}

/// 值解析：方括号值恒为列表；裸逗号分隔值仅对 `tags` 转列表；其余为标量字符串。
fn parse_value(key: &str, value: &str) -> Value {
    let value = value.trim();
    if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        return Value::Array(split_scalars(inner));
    }
    if key == "tags" {
        return Value::Array(split_scalars(value));
    }
    Value::String(parse_scalar(value))
}

fn split_scalars(value: &str) -> Vec<Value> {
    value
        .split(',')
        .map(parse_scalar)
        .filter(|s| !s.is_empty())
        .map(Value::String)
        .collect()
}

fn parse_scalar(value: &str) -> String {
    value.trim().trim_matches('"').trim_matches('\'').to_string()
}

/// 返回正文中首个 Markdown 标题的文本（去首尾空白与围井号）。
fn first_heading_title(text: &str) -> Option<String> {
    for line in text.lines() {
        let hashes = line.chars().take_while(|&c| c == '#').count();
        if !(1..=6).contains(&hashes) {
            continue;
        }
        let rest = &line[hashes..];
        let Some(after) = rest.strip_prefix(|c: char| c.is_whitespace()) else {
            continue;
        };
        if after.is_empty() {
            continue;
        }
        return Some(after.trim().trim_matches('#').trim().to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    // ---------- parse_response ----------

    #[test]
    fn test_parse_response_full_front_matter() {
        let content = "---\ntitle: 笔记\nsummary: 一句话\nlanguage: zh\ntags: [a, b, c]\n---\n\n# 笔记\n\n<!-- chunk: 正文 -->\n内容";
        let (metadata, body) = parse_response(content).unwrap();

        assert_eq!(metadata["title"].clone(), json!("笔记"));
        assert_eq!(metadata["summary"].clone(), json!("一句话"));
        assert_eq!(metadata["language"].clone(), json!("zh"));
        assert_eq!(metadata["tags"].clone(), json!(["a", "b", "c"]));
        assert!(body.starts_with("# 笔记"));
        assert!(body.contains("<!-- chunk: 正文 -->"));
    }

    #[test]
    fn test_parse_response_bare_tags_become_list() {
        let content = "---\ntags: 甲,乙, 丙\n---\n正文";
        let (metadata, _) = parse_response(content).unwrap();

        assert_eq!(metadata["tags"].clone(), json!(["甲", "乙", "丙"]));
    }

    #[test]
    fn test_parse_response_no_front_matter_recovers_title_from_heading() {
        let content = "# 我的标题\n\n正文内容";
        let (metadata, body) = parse_response(content).unwrap();

        assert_eq!(metadata["title"].clone(), json!("我的标题"));
        assert_eq!(body, content);
    }

    #[test]
    fn test_parse_response_code_fence_is_stripped() {
        let content = "```markdown\n---\ntitle: T\n---\n\n# T\n\n正文\n```";
        let (metadata, body) = parse_response(content).unwrap();

        assert_eq!(metadata["title"].clone(), json!("T"));
        assert!(body.starts_with("# T"));
        assert!(body.ends_with("正文"));
    }

    #[test]
    fn test_parse_response_continuation_line_joins_previous_field() {
        let content = "---\nsummary: 第一行\n  第二行\n---\n正文";
        let (metadata, _) = parse_response(content).unwrap();

        assert_eq!(metadata["summary"].clone(), json!("第一行 第二行"));
    }

    #[test]
    fn test_parse_response_empty_is_error() {
        assert!(parse_response("   ").is_err());
        assert!(parse_response("").is_err());
    }

    #[test]
    fn test_parse_response_quoted_scalars_are_unquoted() {
        let content = "---\ntitle: \"带引号\"\nlanguage: 'zh'\n---\n正文";
        let (metadata, _) = parse_response(content).unwrap();

        assert_eq!(metadata["title"].clone(), json!("带引号"));
        assert_eq!(metadata["language"].clone(), json!("zh"));
    }

    // ---------- strip_code_fence ----------

    #[test]
    fn test_strip_code_fence_variants() {
        assert_eq!(strip_code_fence("```\n内容\n```"), "内容");
        assert_eq!(strip_code_fence("```md\n内容\n```"), "内容");
        assert_eq!(strip_code_fence("```\n内容```"), "内容");
        assert_eq!(strip_code_fence("无围栏"), "无围栏");
        // 内部围栏不剥
        assert_eq!(strip_code_fence("``` \n a ```"), " a ");
    }

    // ---------- front matter 边界 ----------

    #[test]
    fn test_split_front_matter_requires_delimiters() {
        let (metadata, body) = split_front_matter("没有围栏的文本");
        assert!(metadata.is_empty());
        assert_eq!(body, "没有围栏的文本");

        // 只有开头围栏
        let (metadata, body) = split_front_matter("---\ntitle: T\n正文");
        assert!(metadata.is_empty());
        assert_eq!(body, "---\ntitle: T\n正文");
    }

    #[test]
    fn test_front_matter_field_validation() {
        assert_eq!(front_matter_field("title: x"), Some(("title", " x")));
        assert_eq!(front_matter_field("_a-b1: x"), Some(("_a-b1", " x")));
        assert_eq!(front_matter_field("1key: x"), None);
        assert_eq!(front_matter_field("no colon"), None);
    }

    #[test]
    fn test_first_heading_title_variants() {
        assert_eq!(first_heading_title("# 标题 #\n"), Some("标题".to_string()));
        assert_eq!(first_heading_title("### 深层"), Some("深层".to_string()));
        assert_eq!(first_heading_title("####### 七个井号不算"), None);
        assert_eq!(first_heading_title("#无空格"), None);
        assert_eq!(first_heading_title(""), None);
    }

    // ---------- OpenAiClient（本地一次性 HTTP 服务，不发真实外网请求） ----------

    /// 启动只处理一次请求的本地 HTTP 服务，返回 base_url。
    fn spawn_once(status_line: &'static str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 2048];
                let _ = stream.read(&mut buffer);
                let response =
                    format!("{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    fn client(base_url: String) -> OpenAiClient {
        OpenAiClient::new(LlmConfig {
            base_url,
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
        })
    }

    #[test]
    fn test_chat_success_returns_content() {
        let url = spawn_once("HTTP/1.1 200 OK", r#"{"choices":[{"message":{"content":"你好"}}]}"#);
        assert_eq!(client(url).chat("提示词").unwrap(), "你好");
    }

    #[test]
    fn test_chat_http_error_includes_status_and_body() {
        let url = spawn_once("HTTP/1.1 429 Too Many Requests", "rate limited");
        let error = client(url).chat("p").unwrap_err();
        assert!(error.contains("LLM 返回 429"), "{error}");
        assert!(error.contains("rate limited"), "{error}");
    }

    #[test]
    fn test_chat_invalid_json() {
        let url = spawn_once("HTTP/1.1 200 OK", "not json");
        assert!(client(url).chat("p").unwrap_err().contains("有效 JSON"));
    }

    #[test]
    fn test_chat_missing_content_field() {
        let url = spawn_once("HTTP/1.1 200 OK", "{}");
        assert!(client(url).chat("p").unwrap_err().contains("choices"));
    }

    #[test]
    fn test_chat_connection_refused() {
        // 绑定后立即释放端口，保证连接失败
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let error = client(format!("http://{addr}")).chat("p").unwrap_err();
        assert!(error.contains("LLM 请求失败"), "{error}");
    }

    #[test]
    fn test_connection_success_is_ok() {
        let url = spawn_once("HTTP/1.1 200 OK", r#"{"choices":[{"message":{"content":"p"}}]}"#);
        assert!(client(url).test_connection().is_ok());
    }

    #[test]
    fn test_connection_http_error_includes_status_and_body() {
        let url = spawn_once("HTTP/1.1 401 Unauthorized", "bad key");
        let error = client(url).test_connection().unwrap_err();
        assert!(error.contains("LLM 返回 401"), "{error}");
        assert!(error.contains("bad key"), "{error}");
    }

    #[test]
    fn test_connection_refused_is_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let error = client(format!("http://{addr}")).test_connection().unwrap_err();
        assert!(error.contains("LLM 请求失败"), "{error}");
    }

    #[test]
    fn test_model_returns_configured_name() {
        assert_eq!(
            LlmClient::model(&client("http://127.0.0.1:1".to_string())),
            "test-model"
        );
    }

    // ---------- chat_stream（SSE 流式） ----------

    #[test]
    fn test_chat_stream_collects_deltas_in_order() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n\
                    data: [DONE]\n\n";
        let url = spawn_once("HTTP/1.1 200 OK", body);
        let mut deltas: Vec<String> = Vec::new();
        let full = client(url)
            .chat_stream("p", &mut |d| deltas.push(d.to_string()))
            .unwrap();
        assert_eq!(full, "你好");
        assert_eq!(deltas, vec!["你".to_string(), "好".to_string()]);
    }

    #[test]
    fn test_chat_stream_skips_malformed_line() {
        let body = "data: not-json\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n\
                    data: [DONE]\n\n";
        let url = spawn_once("HTTP/1.1 200 OK", body);
        let full = client(url).chat_stream("p", &mut |_| {}).unwrap();
        assert_eq!(full, "好");
    }

    #[test]
    fn test_chat_stream_http_error() {
        let url = spawn_once("HTTP/1.1 500 Internal Server Error", "boom");
        let error = client(url).chat_stream("p", &mut |_| {}).unwrap_err();
        assert!(error.contains("LLM 返回 500"), "{error}");
    }

    #[test]
    fn test_chat_stream_empty_body_is_error() {
        let url = spawn_once("HTTP/1.1 200 OK", "");
        assert!(client(url).chat_stream("p", &mut |_| {}).unwrap_err().contains("为空"));
    }

    #[test]
    fn test_chat_stream_connection_refused() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let error = client(format!("http://{addr}"))
            .chat_stream("p", &mut |_| {})
            .unwrap_err();
        assert!(error.contains("LLM 请求失败"), "{error}");
    }
}
