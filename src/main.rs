mod agent;
mod config;
mod llm;
mod mealie;
mod memory;
mod tools;

use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use tracing_subscriber::EnvFilter;

use crate::agent::Agent;
use crate::config::AppConfig;
use crate::llm::LlmClient;
use crate::mealie::MealieClient;
use crate::memory::MealMemory;
use crate::tools::ToolExecutor;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialise tracing (set RUST_LOG=info or debug)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Load config
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());
    let config = AppConfig::load(&config_path)?;

    println!("meal-planner v{}", env!("CARGO_PKG_VERSION"));
    println!("  LLM provider: {}", config.llm.provider);
    println!("  Mealie:       {}", config.mealie.base_url);
    println!();

    // Initialise components
    let llm = LlmClient::from_config(&config.llm)?;

    let mealie_token = std::env::var(&config.mealie.api_token_env)
        .unwrap_or_else(|_| panic!("Missing env var: {}", config.mealie.api_token_env));
    let mealie = MealieClient::new(&config.mealie.base_url, &mealie_token)?;

    let memory = MealMemory::open(config.memory.db_path.to_str().unwrap())?;

    let executor = ToolExecutor::new(mealie, memory);
    let mut agent = Agent::new(llm, executor);

    // REPL
    let mut rl = DefaultEditor::new()?;
    let history_path = dirs::data_dir()
        .map(|d: std::path::PathBuf| d.join("meal-planner").join("history.txt"));

    if let Some(ref path) = history_path {
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let _ = rl.load_history(path);
    }

    println!("Ready! Ask me to plan your meals for the week.");
    println!("Commands: /reset (clear conversation), /quit (exit)");
    println!();

    loop {
        let readline = rl.readline("you> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }

                rl.add_history_entry(input)?;

                match input {
                    "/quit" | "/exit" => break,
                    "/reset" => {
                        agent.reset();
                        println!("Conversation reset.\n");
                        continue;
                    }
                    _ => {}
                }

                match agent.chat(input).await {
                    Ok(response) => {
                        println!("\nassistant> {response}\n");
                    }
                    Err(e) => {
                        eprintln!("\nError: {e}\n");
                    }
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("Input error: {e}");
                break;
            }
        }
    }

    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }

    println!("Goodbye!");
    Ok(())
}
