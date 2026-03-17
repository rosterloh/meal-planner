use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub llm: LlmConfig,
    pub mealie: MealieConfig,
    pub memory: MemoryConfig,
    pub planning: PlanningConfig,
    pub telemetry: TelemetryConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    /// "local" or "cloud"
    pub provider: String,
    pub local: LocalLlmConfig,
    pub cloud: CloudLlmConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LocalLlmConfig {
    /// e.g. "http://localhost:8080/v1"
    pub base_url: String,
    /// Model name as reported by llama.cpp
    pub model: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CloudLlmConfig {
    /// e.g. "https://api.anthropic.com" or an OpenAI-compatible endpoint
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MealieConfig {
    /// e.g. "http://mealie.local:9925"
    pub base_url: String,
    pub api_token_env: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MemoryConfig {
    pub db_path: PathBuf,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PlanningConfig {
    /// Minimum days before a meal can repeat
    pub repeat_cooldown_days: u32,
    /// Minimum star rating to include (1-5)
    pub min_rating: f32,
    /// Number of days to plan
    pub plan_days: u32,
    /// Tags/categories for quick weeknight meals
    pub quick_meal_tags: Vec<String>,
}


#[derive(Debug, Deserialize, Clone)]
pub struct TelemetryConfig {
    /// OTLP endpoint for exporting traces
    pub otlp_endpoint: String,
    /// Application tag for telemetry
    pub app_name: String,
}

impl AppConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}
