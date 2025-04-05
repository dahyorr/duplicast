use serde::{Deserialize, Serialize};
use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, FromRow, SqlitePool};
use std::sync::OnceLock;
use tauri::AppHandle;

use crate::config;

static DB_POOL: OnceLock<SqlitePool> = OnceLock::new();
// Path to migrations folder (relative to project root)
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn init_db(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = config::get_data_dir(app).join("app.sqlite");

    // if let Some(parent) = Path::new(db_path.as_os_str()).parent() {
    //     fs::create_dir_all(parent).expect("❌ Failed to create DB directory");
    // }
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
    println!("📦 Creating DB file...");
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
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
pub struct EncoderSettings {
    pub video_bitrate: u32,
    pub audio_bitrate: u32,
    pub video_codec: String,
    pub audio_codec: String,
    pub preset: String,
    pub tune: Option<String>,
    pub bufsize: Option<u32>,
    pub framerate: Option<u32>,
    pub resolution: Option<String>,
    pub use_passthrough: bool, 
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RelayTarget {
    pub id: i64,
    pub tag: String,
    pub stream_key: String,
    pub url: String,
    pub enabled: bool,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RelayTargetPublic {
    pub id: i64,
    pub tag: String,
    pub stream_key: String,
    pub url: String,
    pub enabled: bool,
    pub created_at: Option<String>,
}
impl RelayTargetPublic {
    pub fn from_relay_target(relay_target: &RelayTarget) -> Self {
        Self {
            id: relay_target.id,
            tag: relay_target.tag.clone(),
            stream_key: config::mask_key(&relay_target.stream_key),
            url: relay_target.url.clone(),
            enabled: relay_target.enabled,
            created_at: relay_target.created_at.clone(),
        }
    }
}

pub async fn get_active_relay_targets(pool: &SqlitePool) -> Result<Vec<RelayTarget>, sqlx::Error> {
    sqlx::query_as::<_, RelayTarget>("SELECT * FROM relay_targets WHERE enabled = 1")
        .fetch_all(pool)
        .await
}

pub async fn add_relay_target(
    url: &str,
    stream_key: &str,
    tag: &str,
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO relay_targets (stream_key, url, tag, enabled) VALUES (?, ?, ?, 1)")
        .bind(stream_key)
        .bind(url)
        .bind(tag)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_relay_targets(pool: &SqlitePool) -> Result<Vec<RelayTarget>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM relay_targets ORDER BY enabled DESC")
        .fetch_all(pool)
        .await
}

pub async fn toggle_relay_target(
    id: i64,
    active: bool,
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE relay_targets SET enabled = ? WHERE id = ?")
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

pub async fn get_relay_target(id: i64, pool: &SqlitePool) -> Result<RelayTarget, sqlx::Error> {
    sqlx::query_as::<_, RelayTarget>("SELECT * FROM relay_targets WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn load_encoder_settings(pool: &SqlitePool) -> Result<EncoderSettings, sqlx::Error> {
    sqlx::query_as::<_, EncoderSettings>("SELECT * FROM encoder_settings ORDER BY id DESC LIMIT 1")
        .fetch_one(pool)
        .await
}

pub async fn save_encoder_settings(
    settings: &EncoderSettings,
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO encoder_settings (
            video_bitrate, audio_bitrate, video_codec, audio_codec, preset,
            tune, bufsize, framerate, resolution
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(settings.video_bitrate)
    .bind(settings.audio_bitrate)
    .bind(&settings.video_codec)
    .bind(&settings.audio_codec)
    .bind(&settings.preset)
    .bind(&settings.tune)
    .bind(settings.bufsize)
    .bind(settings.framerate)
    .bind(&settings.resolution)
    .execute(pool)
    .await?;
    Ok(())
}

pub fn default_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        video_bitrate: 6000,
        audio_bitrate: 160,
        video_codec: "libx264".into(),
        audio_codec: "aac".into(),
        preset: "veryfast".into(),
        tune: Some("zerolatency".into()),
        bufsize: Some(8000),
        framerate: None,
        resolution: None,
        use_passthrough: true
    }
}