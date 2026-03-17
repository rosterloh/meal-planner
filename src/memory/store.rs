use anyhow::Result;
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Persistent memory for meal history, preferences, and plan compliance.
pub struct MealMemory {
    conn: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MealLog {
    pub id: i64,
    pub recipe_id: String,
    pub recipe_name: String,
    pub planned_date: NaiveDate,
    pub was_cooked: bool,
    pub substituted_with: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preference {
    pub id: i64,
    pub key: String,
    pub value: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeScore {
    pub recipe_id: String,
    pub recipe_name: String,
    pub mealie_rating: f32,
    pub times_planned: u32,
    pub times_cooked: u32,
    pub last_planned: Option<NaiveDate>,
    /// Composite score: higher = better candidate for planning
    pub score: f64,
}

impl MealMemory {
    pub fn open(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let mem = Self { conn };
        mem.init_schema()?;
        Ok(mem)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS meal_log (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                recipe_id       TEXT NOT NULL,
                recipe_name     TEXT NOT NULL,
                planned_date    TEXT NOT NULL,
                was_cooked      BOOLEAN NOT NULL DEFAULT 0,
                substituted_with TEXT,
                notes           TEXT,
                created_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS preferences (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                key         TEXT NOT NULL UNIQUE,
                value       TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_meal_log_recipe
                ON meal_log(recipe_id);
            CREATE INDEX IF NOT EXISTS idx_meal_log_date
                ON meal_log(planned_date);
            ",
        )?;
        Ok(())
    }

    // ── Logging ──────────────────────────────────────────────────

    /// Record a planned meal.
    pub fn log_planned_meal(
        &self,
        recipe_id: &str,
        recipe_name: &str,
        date: NaiveDate,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO meal_log (recipe_id, recipe_name, planned_date)
             VALUES (?1, ?2, ?3)",
            params![recipe_id, recipe_name, date.to_string()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Mark a planned meal as cooked (or substituted).
    pub fn mark_cooked(
        &self,
        log_id: i64,
        was_cooked: bool,
        substituted_with: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE meal_log SET was_cooked = ?1, substituted_with = ?2 WHERE id = ?3",
            params![was_cooked, substituted_with, log_id],
        )?;
        Ok(())
    }

    // ── Queries for the agent ────────────────────────────────────

    /// Get recent meal history within a date range.
    pub fn get_history(
        &self,
        since: NaiveDate,
        until: NaiveDate,
    ) -> Result<Vec<MealLog>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, recipe_id, recipe_name, planned_date, was_cooked,
                    substituted_with, notes
             FROM meal_log
             WHERE planned_date BETWEEN ?1 AND ?2
             ORDER BY planned_date DESC",
        )?;

        let rows = stmt.query_map(params![since.to_string(), until.to_string()], |row| {
            Ok(MealLog {
                id: row.get(0)?,
                recipe_id: row.get(1)?,
                recipe_name: row.get(2)?,
                planned_date: row
                    .get::<_, String>(3)?
                    .parse()
                    .unwrap_or(NaiveDate::default()),
                was_cooked: row.get(4)?,
                substituted_with: row.get(5)?,
                notes: row.get(6)?,
            })
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get the last date a recipe was planned.
    pub fn last_planned(&self, recipe_id: &str) -> Result<Option<NaiveDate>> {
        let result: Option<String> = self
            .conn
            .query_row(
                "SELECT MAX(planned_date) FROM meal_log WHERE recipe_id = ?1",
                params![recipe_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        Ok(result.and_then(|s| s.parse().ok()))
    }

    /// Count how many times a recipe has been planned and actually cooked.
    pub fn recipe_stats(&self, recipe_id: &str) -> Result<(u32, u32)> {
        let (planned, cooked): (u32, u32) = self.conn.query_row(
            "SELECT COUNT(*), SUM(CASE WHEN was_cooked THEN 1 ELSE 0 END)
             FROM meal_log WHERE recipe_id = ?1",
            params![recipe_id],
            |row| Ok((row.get(0)?, row.get::<_, Option<u32>>(1)?.unwrap_or(0))),
        )?;
        Ok((planned, cooked))
    }

    /// Compliance rate for the last N plans (fraction actually cooked).
    pub fn compliance_rate(&self, last_n_days: u32) -> Result<f64> {
        let (total, cooked): (u32, u32) = self.conn.query_row(
            "SELECT COUNT(*), SUM(CASE WHEN was_cooked THEN 1 ELSE 0 END)
             FROM meal_log
             WHERE planned_date >= date('now', ?1)",
            params![format!("-{last_n_days} days")],
            |row| Ok((row.get(0)?, row.get::<_, Option<u32>>(1)?.unwrap_or(0))),
        )?;

        if total == 0 {
            return Ok(1.0);
        }
        Ok(cooked as f64 / total as f64)
    }

    // ── Preferences ──────────────────────────────────────────────

    pub fn set_preference(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO preferences (key, value)
             VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_preference(&self, key: &str) -> Result<Option<String>> {
        let result = self
            .conn
            .query_row(
                "SELECT value FROM preferences WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .ok();
        Ok(result)
    }

    pub fn get_all_preferences(&self) -> Result<Vec<Preference>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, key, value, created_at FROM preferences")?;
        let rows = stmt.query_map([], |row| {
            Ok(Preference {
                id: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}
