use anyhow::{Context, Result};
use chrono::NaiveDate;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Typed client for the Mealie REST API.
///
/// Mealie's full OpenAPI spec is at /docs — we model only the endpoints
/// the agent needs. Extend as required.
#[derive(Clone)]
pub struct MealieClient {
    client: Client,
    base_url: String,
}

// ─── Recipe models ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeSummary {
    pub id: Option<String>,
    pub slug: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub rating: Option<f32>,
    pub total_time: Option<String>,
    pub tags: Option<Vec<Tag>>,
    pub recipe_category: Option<Vec<Category>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub rating: Option<f32>,
    pub total_time: Option<String>,
    pub prep_time: Option<String>,
    pub perform_time: Option<String>,
    pub recipe_ingredient: Vec<Ingredient>,
    pub recipe_instructions: Vec<Instruction>,
    pub tags: Vec<Tag>,
    pub recipe_category: Vec<Category>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ingredient {
    pub note: Option<String>,
    pub food: Option<Food>,
    pub quantity: Option<f64>,
    pub unit: Option<Unit>,
    pub display: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Food {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unit {
    pub id: Option<String>,
    pub name: String,
    pub abbreviation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instruction {
    pub id: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: Option<String>,
    pub name: String,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: Option<String>,
    pub name: String,
    pub slug: Option<String>,
}

// ─── Meal plan models ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MealPlanEntry {
    pub date: NaiveDate,
    pub entry_type: String, // "dinner", "lunch", etc.
    pub title: String,
    pub recipe_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMealPlan {
    pub date: NaiveDate,
    #[serde(rename = "entryType")]
    pub entry_type: String,
    pub title: String,
    #[serde(rename = "recipeId")]
    pub recipe_id: Option<String>,
}

// ─── Shopping list models ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoppingList {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoppingListItem {
    pub id: Option<String>,
    pub note: Option<String>,
    pub food: Option<Food>,
    pub quantity: Option<f64>,
    pub unit: Option<Unit>,
    pub checked: bool,
}

// ─── Paginated response wrapper ──────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct Paginated<T> {
    pub page: u32,
    pub per_page: u32,
    pub total: u32,
    pub total_pages: u32,
    pub items: Vec<T>,
}

// ─── Client implementation ───────────────────────────────────────

impl MealieClient {
    pub fn new(base_url: &str, api_token: &str) -> Result<Self> {
        let client = Client::builder()
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    "Authorization",
                    format!("Bearer {api_token}")
                        .parse()
                        .context("invalid api token")?,
                );
                headers
            })
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api{}", self.base_url, path)
    }

    // ── Recipes ──────────────────────────────────────────────────

    /// Search recipes. Mealie supports query-string search + tag/category filtering.
    pub async fn search_recipes(
        &self,
        search: Option<&str>,
        tags: &[String],
        categories: &[String],
        page: u32,
        per_page: u32,
    ) -> Result<Paginated<RecipeSummary>> {
        let mut params = vec![
            ("page".to_string(), page.to_string()),
            ("perPage".to_string(), per_page.to_string()),
        ];

        if let Some(q) = search {
            params.push(("search".to_string(), q.to_string()));
        }
        for tag in tags {
            params.push(("tags".to_string(), tag.clone()));
        }
        for cat in categories {
            params.push(("categories".to_string(), cat.clone()));
        }

        let resp = self
            .client
            .get(self.url("/recipes"))
            .query(&params)
            .send()
            .await?
            .error_for_status()?
            .json::<Paginated<RecipeSummary>>()
            .await?;

        Ok(resp)
    }

    /// Get full recipe details by slug.
    pub async fn get_recipe(&self, slug: &str) -> Result<Recipe> {
        let resp = self
            .client
            .get(self.url(&format!("/recipes/{slug}")))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(resp)
    }

    // ── Meal plans ───────────────────────────────────────────────

    /// Get meal plan entries for a date range.
    pub async fn get_meal_plans(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<MealPlanEntry>> {
        let resp = self
            .client
            .get(self.url("/meal-plans"))
            .query(&[
                ("start_date", start.to_string()),
                ("end_date", end.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<MealPlanEntry>>()
            .await?;

        Ok(resp)
    }

    /// Create a single meal plan entry.
    pub async fn create_meal_plan(&self, plan: &CreateMealPlan) -> Result<MealPlanEntry> {
        let resp = self
            .client
            .post(self.url("/meal-plans"))
            .json(plan)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(resp)
    }

    // ── Shopping lists ───────────────────────────────────────────

    /// List all shopping lists.
    pub async fn get_shopping_lists(&self) -> Result<Vec<ShoppingList>> {
        let resp = self
            .client
            .get(self.url("/groups/shopping/lists"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(resp)
    }

    /// Add recipe ingredients to an existing shopping list.
    pub async fn add_recipe_to_shopping_list(
        &self,
        list_id: &str,
        recipe_id: &str,
    ) -> Result<()> {
        self.client
            .post(self.url(&format!(
                "/groups/shopping/lists/{list_id}/recipe/{recipe_id}"
            )))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}
