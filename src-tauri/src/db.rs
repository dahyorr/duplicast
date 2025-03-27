use serde::{Deserialize, Serialize};
use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, FromRow, SqlitePool};
use std::{fs, path::Path, sync::OnceLock};

static DB_POOL: OnceLock<SqlitePool> = OnceLock::new();
// Path to migrations folder (relative to project root)
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn init_db() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = "./data/app.db";

    if let Some(parent) = Path::new(db_path).parent() {
        fs::create_dir_all(parent).expect("❌ Failed to create DB directory");
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_path)
        .await?;

    // Run migrations
    MIGRATOR.run(&pool).await?;

    DB_POOL.set(pool).ok(); // Cache connection
    Ok(())
}

pub fn get_db_pool() -> &'static SqlitePool {
    DB_POOL.get().expect("DB not initialized")
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RelayTarget {
    pub id: i64,
    pub stream_key: String,
    pub url: String,
    pub active: bool,
    pub created_at: Option<String>,
}

pub async fn get_active_relay_targets(pool: &SqlitePool) -> Result<Vec<RelayTarget>, sqlx::Error> {
    sqlx::query_as::<_, RelayTarget>("SELECT * FROM relay_targets WHERE active = 1")
        .fetch_all(pool)
        .await
}

pub async fn add_relay_target(
    url: &str,
    stream_key: &str,
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO relay_targets (stream_key, url, active) VALUES (?, ?, 1)")
        .bind(stream_key)
        .bind(url)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_relay_targets(pool: &SqlitePool) -> Result<Vec<RelayTarget>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM relay_targets")
        .fetch_all(pool)
        .await
}

pub async fn toggle_relay_target(
    id: i64,
    active: bool,
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE relay_targets SET active = ? WHERE id = ?")
        .bind(active)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn remove_relay_target(id: i64, pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM relay_targets WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}