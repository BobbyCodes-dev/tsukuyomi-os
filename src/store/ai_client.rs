use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ai::{AiProvider, ProviderKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

#[async_trait::async_trait]
pub trait AiClient: Send + Sync {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<AiResponse>;
}

pub fn fetch_models_blocking(kind: ProviderKind, endpoint: &str, api_key: &str) -> Result<Vec<String>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()?;
    match kind {
        ProviderKind::Anthropic => {
            let resp = client
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .send()?;
            if !resp.status().is_success() {
                anyhow::bail!("{}", resp.status());
            }
            let data: Value = resp.json()?;
            Ok(data["data"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect())
                .unwrap_or_default())
        }
        ProviderKind::OpenAiCompatible => {
            let base = endpoint.trim_end_matches("/chat/completions").trim_end_matches('/');
            let resp = client.get(format!("{base}/models")).bearer_auth(api_key).send()?;
            if !resp.status().is_success() {
                anyhow::bail!("{}", resp.status());
            }
            let data: Value = resp.json()?;
            Ok(data["data"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect())
                .unwrap_or_default())
        }
        ProviderKind::Gemini => {
            let resp = client.get(format!("{endpoint}?key={api_key}")).send()?;
            if !resp.status().is_success() {
                anyhow::bail!("{}", resp.status());
            }
            let data: Value = resp.json()?;
            Ok(data["models"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["name"].as_str().map(|s| s.trim_start_matches("models/").to_string()))
                        .collect()
                })
                .unwrap_or_default())
        }
        ProviderKind::Ollama => {
            let base = endpoint.trim_end_matches("/api/chat").trim_end_matches('/');
            let resp = client.get(format!("{base}/api/tags")).send()?;
            if !resp.status().is_success() {
                anyhow::bail!("{}", resp.status());
            }
            let data: Value = resp.json()?;
            Ok(data["models"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|m| m["name"].as_str().map(str::to_string)).collect())
                .unwrap_or_default())
        }
        ProviderKind::OllamaCloud => {
            let base = endpoint.trim_end_matches("/api/chat").trim_end_matches('/');
            let resp = client.get(format!("{base}/api/tags")).bearer_auth(api_key).send()?;
            if !resp.status().is_success() {
                anyhow::bail!("{}", resp.status());
            }
            let data: Value = resp.json()?;
            Ok(data["models"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|m| m["name"].as_str().map(str::to_string)).collect())
                .unwrap_or_default())
        }
    }
}

pub fn build_client(provider: &AiProvider, api_key: &str) -> Box<dyn AiClient> {
    match provider.kind {
        ProviderKind::Anthropic => Box::new(AnthropicClient {
            endpoint: provider.endpoint.clone(),
            model: provider.model.clone(),
            api_key: api_key.to_string(),
        }),
        ProviderKind::OpenAiCompatible | ProviderKind::Gemini => Box::new(OpenAiClient {
            endpoint: provider.endpoint.clone(),
            model: provider.model.clone(),
            api_key: api_key.to_string(),
            is_gemini: provider.kind == ProviderKind::Gemini,
        }),
        ProviderKind::Ollama => Box::new(OllamaClient {
            endpoint: provider.endpoint.clone(),
            model: provider.model.clone(),
            api_key: None,
        }),
        ProviderKind::OllamaCloud => Box::new(OllamaClient {
            endpoint: provider.endpoint.clone(),
            model: provider.model.clone(),
            api_key: Some(api_key.to_string()),
        }),
    }
}

struct AnthropicClient {
    endpoint: String,
    model: String,
    api_key: String,
}

#[async_trait::async_trait]
impl AiClient for AnthropicClient {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<AiResponse> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()?;
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": messages,
        });
        if !tools.is_empty() {
            let tool_specs: Vec<Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters
                    })
                })
                .collect();
            body["tools"] = Value::Array(tool_specs);
        }
        let resp = client
            .post(&self.endpoint)
            .header("x-api-key", self.api_key.clone())
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic error: {}", text);
        }
        let data: Value = resp.json().await?;
        parse_anthropic_response(&data)
    }
}

fn parse_anthropic_response(data: &Value) -> Result<AiResponse> {
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    if let Some(arr) = data["content"].as_array() {
        for block in arr {
            match block["type"].as_str() {
                Some("text") => {
                    content.push_str(block["text"].as_str().unwrap_or(""));
                }
                Some("tool_use") => {
                    tool_calls.push(ToolCall {
                        id: block["id"].as_str().unwrap_or("").to_string(),
                        name: block["name"].as_str().unwrap_or("").to_string(),
                        arguments: block["input"].clone(),
                    });
                }
                _ => {}
            }
        }
    }
    Ok(AiResponse { content, tool_calls })
}

struct OpenAiClient {
    endpoint: String,
    model: String,
    api_key: String,
    is_gemini: bool,
}

#[async_trait::async_trait]
impl AiClient for OpenAiClient {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<AiResponse> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()?;
        let url = if self.is_gemini {
            format!("{}/{}", self.endpoint, self.model)
        } else {
            self.endpoint.clone()
        };
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
        });
        if !tools.is_empty() {
            let tool_specs: Vec<Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect();
            body["tools"] = Value::Array(tool_specs);
            body["tool_choice"] = "auto".into();
        }
        let builder = if self.is_gemini {
            client
                .post(&format!("{}:generateContent?key={}", url, self.api_key))
                .json(&gemini_body(messages, tools))
        } else {
            client
                .post(&url)
                .bearer_auth(&self.api_key)
                .json(&body)
        };
        let resp = builder.send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI-compatible error: {}", text);
        }
        let data: Value = resp.json().await?;
        if self.is_gemini {
            parse_gemini_response(&data)
        } else {
            parse_openai_response(&data)
        }
    }
}

fn gemini_body(messages: &[ChatMessage], tools: &[ToolDefinition]) -> Value {
    let contents: Vec<Value> = messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "role": if m.role == "assistant" { "model" } else { &m.role },
                "parts": [{"text": m.content}]
            })
        })
        .collect();
    let mut body = serde_json::json!({ "contents": contents });
    if !tools.is_empty() {
        let declarations: Vec<Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters
                })
            })
            .collect();
        body["tools"] = serde_json::json!([{ "function_declarations": declarations }]);
    }
    body
}

fn parse_gemini_response(data: &Value) -> Result<AiResponse> {
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    if let Some(candidates) = data["candidates"].as_array() {
        for candidate in candidates {
            if let Some(parts) = candidate["content"]["parts"].as_array() {
                for part in parts {
                    if let Some(text) = part["text"].as_str() {
                        content.push_str(text);
                    }
                    if let Some(call) = part["functionCall"].as_object() {
                        tool_calls.push(ToolCall {
                            id: format!("gemini-{}", tool_calls.len()),
                            name: call["name"].as_str().unwrap_or("").to_string(),
                            arguments: call["args"].clone(),
                        });
                    }
                }
            }
        }
    }
    Ok(AiResponse { content, tool_calls })
}

fn parse_openai_response(data: &Value) -> Result<AiResponse> {
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    if let Some(choices) = data["choices"].as_array() {
        if let Some(choice) = choices.first() {
            if let Some(msg) = choice["message"].as_object() {
                content.push_str(msg["content"].as_str().unwrap_or(""));
                if let Some(calls) = msg["tool_calls"].as_array() {
                    for call in calls {
                        tool_calls.push(ToolCall {
                            id: call["id"].as_str().unwrap_or("").to_string(),
                            name: call["function"]["name"].as_str().unwrap_or("").to_string(),
                            arguments: serde_json::from_str(
                                call["function"]["arguments"].as_str().unwrap_or("{}")
                            )?,
                        });
                    }
                }
            }
        }
    }
    Ok(AiResponse { content, tool_calls })
}

struct OllamaClient {
    endpoint: String,
    model: String,
    api_key: Option<String>,
}

#[async_trait::async_trait]
impl AiClient for OllamaClient {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<AiResponse> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
        });
        if !tools.is_empty() {
            let tool_specs: Vec<Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect();
            body["tools"] = Value::Array(tool_specs);
        }
        let mut req = client.post(&self.endpoint).header("content-type", "application/json");
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req.json(&body).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Ollama error: {}", text);
        }
        let data: Value = resp.json().await?;
        parse_ollama_response(&data)
    }
}

fn parse_ollama_response(data: &Value) -> Result<AiResponse> {
    let content = data["message"]["content"].as_str().unwrap_or("").to_string();
    let mut tool_calls = Vec::new();
    if let Some(calls) = data["message"]["tool_calls"].as_array() {
        for (i, call) in calls.iter().enumerate() {
            tool_calls.push(ToolCall {
                id: format!("ollama-{i}"),
                name: call["function"]["name"].as_str().unwrap_or("").to_string(),
                arguments: call["function"]["arguments"].clone(),
            });
        }
    }
    Ok(AiResponse { content, tool_calls })
}

#[allow(dead_code)]
pub fn tool_definition(name: &str, description: &str, params: &[(&str, &str)]) -> ToolDefinition {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for (pname, pdesc) in params {
        properties.insert(
            pname.to_string(),
            serde_json::json!({"type": "string", "description": pdesc}),
        );
        required.push(pname.to_string());
    }
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_anthropic_block_text() {
        let data = serde_json::json!({
            "content": [{"type": "text", "text": "Hello, world."}]
        });
        let resp = parse_anthropic_response(&data).unwrap();
        assert_eq!(resp.content, "Hello, world.");
        assert!(resp.tool_calls.is_empty());
    }

    #[test]
    fn parse_openai_text_and_tool() {
        let data = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "Using tool...",
                    "tool_calls": [{
                        "id": "call_1",
                        "function": {"name": "add_finding", "arguments": "{\"title\": \"xss\"}"}
                    }]
                }
            }]
        });
        let resp = parse_openai_response(&data).unwrap();
        assert_eq!(resp.content, "Using tool...");
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "add_finding");
    }
}
