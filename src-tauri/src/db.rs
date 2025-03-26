use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, SqlitePool};
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
