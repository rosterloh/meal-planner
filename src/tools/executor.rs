use anyhow::Result;
use chrono::Local;

use crate::mealie::{CreateMealPlan, MealieClient};
use crate::memory::MealMemory;
use crate::tools::definitions::*;

/// Executes a tool call against the real backends and returns a JSON string
/// result for the LLM to consume.
pub struct ToolExecutor {
    mealie: MealieClient,
    memory: MealMemory,
}

impl ToolExecutor {
    pub fn new(mealie: MealieClient, memory: MealMemory) -> Self {
        Self { mealie, memory }
    }

    /// Dispatch a tool call and return the result as a JSON string.
    pub async fn execute(&self, call: &ToolCall) -> Result<String> {
        match call {
            ToolCall::SearchRecipes(args) => self.search_recipes(args).await,
            ToolCall::GetRecipe(args) => self.get_recipe(args).await,
            ToolCall::GetMealHistory(args) => self.get_meal_history(args),
            ToolCall::CreateMealPlan(args) => self.create_meal_plan(args).await,
            ToolCall::GenerateShoppingList(args) => self.generate_shopping_list(args).await,
            ToolCall::LogMealCompleted(args) => self.log_meal_completed(args),
            ToolCall::UpdatePreference(args) => self.update_preference(args),
        }
    }

    async fn search_recipes(&self, args: &SearchRecipesArgs) -> Result<String> {
        let tags = args.tags.clone().unwrap_or_default();
        let cats = args.categories.clone().unwrap_or_default();

        let results = self
            .mealie
            .search_recipes(args.query.as_deref(), &tags, &cats, 1, 20)
            .await?;

        // Filter by min_rating locally if specified
        let items: Vec<_> = if let Some(min) = args.min_rating {
            results
                .items
                .into_iter()
                .filter(|r| r.rating.unwrap_or(0.0) >= min)
                .collect()
        } else {
            results.items
        };

        // Enrich with memory data: last planned date and cook count
        let mut enriched = Vec::new();
        for recipe in &items {
            let recipe_id = recipe.id.as_deref().unwrap_or("");
            let last = self.memory.last_planned(recipe_id).ok().flatten();
            let (planned, cooked) = self
                .memory
                .recipe_stats(recipe_id)
                .unwrap_or((0, 0));

            enriched.push(serde_json::json!({
                "id": recipe.id,
                "slug": recipe.slug,
                "name": recipe.name,
                "rating": recipe.rating,
                "total_time": recipe.total_time,
                "tags": recipe.tags,
                "last_planned": last,
                "times_planned": planned,
                "times_cooked": cooked,
            }));
        }

        Ok(serde_json::to_string_pretty(&enriched)?)
    }

    async fn get_recipe(&self, args: &GetRecipeArgs) -> Result<String> {
        let recipe = self.mealie.get_recipe(&args.slug).await?;
        Ok(serde_json::to_string_pretty(&recipe)?)
    }

    fn get_meal_history(&self, args: &GetMealHistoryArgs) -> Result<String> {
        let today = Local::now().date_naive();
        let since = today - chrono::Duration::days(args.since_days_ago as i64);
        let history = self.memory.get_history(since, today)?;

        let compliance = self.memory.compliance_rate(args.since_days_ago)?;
        let prefs = self.memory.get_all_preferences()?;

        let result = serde_json::json!({
            "meals": history,
            "compliance_rate": format!("{:.0}%", compliance * 100.0),
            "active_preferences": prefs,
        });

        Ok(serde_json::to_string_pretty(&result)?)
    }

    async fn create_meal_plan(&self, args: &CreateMealPlanArgs) -> Result<String> {
        let mut created = Vec::new();

        for meal in &args.meals {
            // Write to Mealie
            let recipe = self.mealie.get_recipe(&meal.recipe_slug).await?;
            let plan = CreateMealPlan {
                date: meal.date,
                entry_type: "dinner".to_string(),
                title: meal.recipe_name.clone(),
                recipe_id: Some(recipe.id.clone()),
            };
            self.mealie.create_meal_plan(&plan).await?;

            // Log to memory
            let log_id =
                self.memory
                    .log_planned_meal(&recipe.id, &meal.recipe_name, meal.date)?;

            created.push(serde_json::json!({
                "date": meal.date,
                "recipe": meal.recipe_name,
                "log_id": log_id,
            }));
        }

        Ok(serde_json::to_string_pretty(&created)?)
    }

    async fn generate_shopping_list(&self, args: &GenerateShoppingListArgs) -> Result<String> {
        let lists = self.mealie.get_shopping_lists().await?;
        let list = lists.first().ok_or_else(|| {
            anyhow::anyhow!("No shopping lists found in Mealie. Create one first.")
        })?;

        for recipe_id in &args.recipe_ids {
            self.mealie
                .add_recipe_to_shopping_list(&list.id, recipe_id)
                .await?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "shopping_list": list.name,
            "recipes_added": args.recipe_ids.len(),
        })
        .to_string())
    }

    fn log_meal_completed(&self, args: &LogMealCompletedArgs) -> Result<String> {
        self.memory
            .mark_cooked(args.log_id, args.was_cooked, args.substituted_with.as_deref())?;

        Ok(serde_json::json!({
            "status": "ok",
            "log_id": args.log_id,
            "was_cooked": args.was_cooked,
        })
        .to_string())
    }

    fn update_preference(&self, args: &UpdatePreferenceArgs) -> Result<String> {
        self.memory.set_preference(&args.key, &args.value)?;

        Ok(serde_json::json!({
            "status": "ok",
            "preference": { "key": args.key, "value": args.value },
        })
        .to_string())
    }
}
