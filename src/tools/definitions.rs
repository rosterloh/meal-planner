use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Every tool the agent can call. The `#[serde(tag = "name", content = "arguments")]`
/// layout matches the OpenAI function-calling response format, so we can
/// deserialize tool calls directly from the LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "name", content = "arguments")]
pub enum ToolCall {
    #[serde(rename = "search_recipes")]
    SearchRecipes(SearchRecipesArgs),

    #[serde(rename = "get_recipe")]
    GetRecipe(GetRecipeArgs),

    #[serde(rename = "get_meal_history")]
    GetMealHistory(GetMealHistoryArgs),

    #[serde(rename = "create_meal_plan")]
    CreateMealPlan(CreateMealPlanArgs),

    #[serde(rename = "generate_shopping_list")]
    GenerateShoppingList(GenerateShoppingListArgs),

    #[serde(rename = "log_meal_completed")]
    LogMealCompleted(LogMealCompletedArgs),

    #[serde(rename = "update_preference")]
    UpdatePreference(UpdatePreferenceArgs),
}

// ─── Argument structs ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRecipesArgs {
    pub query: Option<String>,
    pub tags: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub min_rating: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRecipeArgs {
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMealHistoryArgs {
    pub since_days_ago: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMealPlanArgs {
    pub meals: Vec<PlannedMeal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedMeal {
    pub date: NaiveDate,
    pub recipe_slug: String,
    pub recipe_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateShoppingListArgs {
    pub list_name: Option<String>,
    pub recipe_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMealCompletedArgs {
    pub log_id: i64,
    pub was_cooked: bool,
    pub substituted_with: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePreferenceArgs {
    pub key: String,
    pub value: String,
}

// ─── OpenAI function schema generation ───────────────────────────

/// Produces the `tools` array for the OpenAI chat completions request.
/// Hand-written rather than macro-generated for clarity and control
/// over descriptions the LLM sees.
pub fn tool_definitions() -> Vec<serde_json::Value> {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "search_recipes",
                "description": "Search the Mealie recipe database. Returns recipe summaries with names, ratings, and tags. Use this to find candidate meals for planning.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Free-text search query (e.g. 'chicken stir fry', 'pasta')"
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Filter by recipe tags (e.g. ['quick', 'vegetarian'])"
                        },
                        "categories": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Filter by categories (e.g. ['dinner', 'one-pot'])"
                        },
                        "min_rating": {
                            "type": "number",
                            "description": "Minimum star rating (1-5)"
                        }
                    },
                    "additionalProperties": false
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_recipe",
                "description": "Get full details of a specific recipe including ingredients and instructions.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "slug": {
                            "type": "string",
                            "description": "Recipe slug identifier"
                        }
                    },
                    "required": ["slug"],
                    "additionalProperties": false
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_meal_history",
                "description": "Retrieve recent meal planning history. Shows what was planned, whether it was actually cooked, and any substitutions. Use this to avoid repeating meals and to check plan compliance.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "since_days_ago": {
                            "type": "integer",
                            "description": "How many days of history to retrieve (e.g. 30 for last month)"
                        }
                    },
                    "required": ["since_days_ago"],
                    "additionalProperties": false
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "create_meal_plan",
                "description": "Create meal plan entries in Mealie. Call this after the user approves the proposed plan.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "meals": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "date": { "type": "string", "description": "ISO date (YYYY-MM-DD)" },
                                    "recipe_slug": { "type": "string" },
                                    "recipe_name": { "type": "string" }
                                },
                                "required": ["date", "recipe_slug", "recipe_name"]
                            },
                            "description": "List of meals to plan"
                        }
                    },
                    "required": ["meals"],
                    "additionalProperties": false
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "generate_shopping_list",
                "description": "Generate a shopping list in Mealie from the given recipe IDs. Aggregates all ingredients.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "list_name": {
                            "type": "string",
                            "description": "Name for the shopping list (defaults to 'Weekly Shop')"
                        },
                        "recipe_ids": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Recipe IDs to include"
                        }
                    },
                    "required": ["recipe_ids"],
                    "additionalProperties": false
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "log_meal_completed",
                "description": "Log whether a planned meal was actually cooked. Used for tracking plan compliance and improving future recommendations.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "log_id": { "type": "integer", "description": "Meal log entry ID" },
                        "was_cooked": { "type": "boolean", "description": "Whether the meal was actually made" },
                        "substituted_with": {
                            "type": "string",
                            "description": "What was made instead, if substituted"
                        }
                    },
                    "required": ["log_id", "was_cooked"],
                    "additionalProperties": false
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "update_preference",
                "description": "Store a user preference for future meal planning. Examples: 'no_fish_on_monday', 'prefer_quick_meals_weekdays', 'avoid_risotto_for_now'.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "key": { "type": "string", "description": "Preference key" },
                        "value": { "type": "string", "description": "Preference value or description" }
                    },
                    "required": ["key", "value"],
                    "additionalProperties": false
                }
            }
        }
    ])
    .as_array()
    .unwrap()
    .clone()
}
