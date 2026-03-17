use anyhow::Result;
use async_openai::types::chat::{
    ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestMessage,
};
use tracing::{info, warn};

use crate::llm::{self, LlmClient};
use crate::tools::{self, ToolCall, ToolExecutor};

const SYSTEM_PROMPT: &str = r#"You are a helpful meal planning assistant. You plan evening meals for the week based on the user's recipe database in Mealie.

Your workflow:
1. Check recent meal history to avoid repeats (get_meal_history)
2. Search for candidate recipes, preferring higher-rated ones (search_recipes)
3. Propose a week's plan to the user, explaining your choices
4. Once approved, create the plan in Mealie (create_meal_plan)
5. Generate the shopping list (generate_shopping_list)

Guidelines:
- Respect the repeat cooldown: don't suggest a meal that was made recently
- Favour recipes with higher Mealie ratings and higher cook-through rates
- On days marked as "busy", suggest quick meals (look for 'quick' tags or short total_time)
- Always propose the plan and wait for approval before creating it
- When the user wants substitutions, search for alternatives and update the plan
- Store user preferences when they express likes/dislikes (update_preference)
- At the start of a new week, ask about last week's compliance (log_meal_completed)

Be concise and practical. Present plans as a simple day-by-day list.
"#;

/// Maximum tool-call round-trips before we force a text response.
const MAX_TOOL_ROUNDS: usize = 10;

pub struct Agent {
    llm: LlmClient,
    executor: ToolExecutor,
    messages: Vec<ChatCompletionRequestMessage>,
    tool_defs: Vec<serde_json::Value>,
}

impl Agent {
    pub fn new(llm: LlmClient, executor: ToolExecutor) -> Self {
        let messages = vec![llm::system_msg(SYSTEM_PROMPT)];
        let tool_defs = tools::tool_definitions();

        Self {
            llm,
            executor,
            messages,
            tool_defs,
        }
    }

    /// Process a user message through the agent loop.
    /// Returns the assistant's final text response.
    pub async fn chat(&mut self, user_input: &str) -> Result<String> {
        self.messages.push(llm::user_msg(user_input));

        for round in 0..MAX_TOOL_ROUNDS {
            let response = self.llm.chat(&self.messages, &self.tool_defs).await?;

            let choice = response
                .choices
                .first()
                .ok_or_else(|| anyhow::anyhow!("Empty response from LLM"))?;

            let message = &choice.message;

            // Check for tool calls
            if let Some(ref tool_calls) = message.tool_calls {
                if tool_calls.is_empty() {
                    // No tool calls — treat as text response
                    let text = message.content.clone().unwrap_or_default();
                    self.messages.push(llm::assistant_msg(&text));
                    return Ok(text);
                }

                // Reconstruct the assistant message with tool calls for context
                // We need to add the full assistant message to maintain the
                // conversation structure the API expects
                let assistant_msg = ChatCompletionRequestMessage::Assistant(
                    ChatCompletionRequestAssistantMessageArgs::default()
                        .content(message.content.clone().unwrap_or_default())
                        .tool_calls(tool_calls.clone())
                        .build()?,
                );
                self.messages.push(assistant_msg);

                // Execute each tool call and append results
                for tc in tool_calls {
                    let ChatCompletionMessageToolCalls::Function(call) = tc else {
                        continue;
                    };
                    let func_name = &call.function.name;
                    let func_args = &call.function.arguments;

                    info!(round, tool = func_name, "Executing tool call");

                    // Parse the tool call arguments into our typed enum
                    let result = match parse_tool_call(func_name, func_args) {
                        Ok(tc) => match self.executor.execute(&tc).await {
                            Ok(output) => output,
                            Err(e) => {
                                warn!(tool = func_name, error = %e, "Tool execution failed");
                                serde_json::json!({
                                    "error": format!("Tool execution failed: {e}")
                                })
                                .to_string()
                            }
                        },
                        Err(e) => {
                            warn!(tool = func_name, error = %e, "Failed to parse tool args");
                            serde_json::json!({
                                "error": format!("Invalid tool arguments: {e}")
                            })
                            .to_string()
                        }
                    };

                    self.messages.push(llm::tool_result_msg(&call.id, &result));
                }

                // Continue the loop — the LLM will see the tool results and
                // either make more tool calls or produce a text response
                continue;
            }

            // No tool calls — this is a text response
            let text = message.content.clone().unwrap_or_default();
            self.messages.push(llm::assistant_msg(&text));
            return Ok(text);
        }

        anyhow::bail!("Agent exceeded maximum tool-call rounds ({MAX_TOOL_ROUNDS})")
    }

    /// Reset conversation history (keep system prompt).
    pub fn reset(&mut self) {
        self.messages.truncate(1);
    }
}

/// Parse a function name + JSON arguments string into our typed ToolCall enum.
fn parse_tool_call(name: &str, arguments: &str) -> Result<ToolCall> {
    let args: serde_json::Value = serde_json::from_str(arguments)?;

    let call = match name {
        "search_recipes" => ToolCall::SearchRecipes(serde_json::from_value(args)?),
        "get_recipe" => ToolCall::GetRecipe(serde_json::from_value(args)?),
        "get_meal_history" => ToolCall::GetMealHistory(serde_json::from_value(args)?),
        "create_meal_plan" => ToolCall::CreateMealPlan(serde_json::from_value(args)?),
        "generate_shopping_list" => ToolCall::GenerateShoppingList(serde_json::from_value(args)?),
        "log_meal_completed" => ToolCall::LogMealCompleted(serde_json::from_value(args)?),
        "update_preference" => ToolCall::UpdatePreference(serde_json::from_value(args)?),
        _ => anyhow::bail!("Unknown tool: {name}"),
    };

    Ok(call)
}
