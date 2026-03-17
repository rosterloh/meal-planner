use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestToolMessageArgs,
        ChatCompletionTool, ChatCompletionTools,
        CreateChatCompletionRequestArgs, CreateChatCompletionResponse, FunctionObject,
    },
    Client,
};
use serde_json::Value;

use crate::config::LlmConfig;

/// Wraps async-openai to talk to either llama.cpp or a cloud endpoint.
pub struct LlmClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl LlmClient {
    pub fn from_config(config: &LlmConfig) -> Result<Self> {
        let (base_url, model, api_key) = match config.provider.as_str() {
            "local" => (
                config.local.base_url.clone(),
                config.local.model.clone(),
                "not-needed".to_string(), // llama.cpp doesn't require a key
            ),
            "cloud" => {
                let key = std::env::var(&config.cloud.api_key_env)
                    .unwrap_or_else(|_| {
                        panic!("Missing env var: {}", config.cloud.api_key_env)
                    });
                (config.cloud.base_url.clone(), config.cloud.model.clone(), key)
            }
            other => anyhow::bail!("Unknown LLM provider: {other}"),
        };

        let oai_config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key(api_key);

        Ok(Self {
            client: Client::with_config(oai_config),
            model,
        })
    }

    /// Send a chat completion request with tool definitions.
    /// Returns the raw response for the agent loop to process.
    pub async fn chat(
        &self,
        messages: &[ChatCompletionRequestMessage],
        tools: &[Value],
    ) -> Result<CreateChatCompletionResponse> {
        let tool_defs: Vec<ChatCompletionTools> = tools
            .iter()
            .map(|t| {
                let func = &t["function"];
                ChatCompletionTools::Function(ChatCompletionTool {
                    function: FunctionObject {
                        name: func["name"].as_str().unwrap().to_string(),
                        description: func["description"].as_str().map(String::from),
                        parameters: Some(func["parameters"].clone()),
                        strict: None,
                    },
                })
            })
            .collect();

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages.to_vec())
            .tools(tool_defs)
            .build()?;

        let response = self.client.chat().create(request).await?;
        Ok(response)
    }
}

// ─── Message builder helpers ─────────────────────────────────────

pub fn system_msg(content: &str) -> ChatCompletionRequestMessage {
    ChatCompletionRequestMessage::System(
        ChatCompletionRequestSystemMessageArgs::default()
            .content(content)
            .build()
            .unwrap(),
    )
}

pub fn user_msg(content: &str) -> ChatCompletionRequestMessage {
    ChatCompletionRequestMessage::User(
        ChatCompletionRequestUserMessageArgs::default()
            .content(content)
            .build()
            .unwrap(),
    )
}

pub fn assistant_msg(content: &str) -> ChatCompletionRequestMessage {
    ChatCompletionRequestMessage::Assistant(
        ChatCompletionRequestAssistantMessageArgs::default()
            .content(content)
            .build()
            .unwrap(),
    )
}

pub fn tool_result_msg(tool_call_id: &str, content: &str) -> ChatCompletionRequestMessage {
    ChatCompletionRequestMessage::Tool(
        ChatCompletionRequestToolMessageArgs::default()
            .tool_call_id(tool_call_id)
            .content(content)
            .build()
            .unwrap(),
    )
}
