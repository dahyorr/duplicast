use super::relay::RelayHandle;
use crate::config::AppState;
use crate::db;
use crate::events::AppEvents;
use crate::models::RelayTarget;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Configuration for relay reconnection behavior
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            max_retries: 10,
            initial_delay_ms: 1000,
            max_delay_ms: 60000,
            backoff_multiplier: 2.0,
        }
    }
}

/// Manages a relay's lifecycle with automatic reconnection
pub struct RelaySupervisor {
    id: i64,
    config: ReconnectConfig,
    relay_handle: Arc<Mutex<Option<RelayHandle>>>,
    retry_count: Arc<Mutex<u32>>,
    should_run: Arc<AtomicBool>,
    supervisor_task: Option<tokio::task::JoinHandle<()>>,
}

impl RelaySupervisor {
    pub fn new(
        relay_target: &RelayTarget,
        encoder_rx: tokio::sync::broadcast::Receiver<bytes::Bytes>,
        config: Option<ReconnectConfig>,
    ) -> Self {
        let relay_handle = RelayHandle::from_relay_target(relay_target, encoder_rx);

        Self {
            id: relay_target.id,
            config: config.unwrap_or_default(),
            relay_handle: Arc::new(Mutex::new(Some(relay_handle))),
            retry_count: Arc::new(Mutex::new(0)),
            should_run: Arc::new(AtomicBool::new(false)),
            supervisor_task: None,
        }
    }

    /// Start the relay with supervision and automatic reconnection
    pub async fn start(&mut self, app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        // Set flag to keep supervisor running
        self.should_run.store(true, Ordering::SeqCst);

        let app_clone = app.clone();
        let relay_handle = Arc::clone(&self.relay_handle);
        let retry_count = Arc::clone(&self.retry_count);
        let should_run = Arc::clone(&self.should_run);
        let config = self.config.clone();
        let id = self.id;

        let task = tokio::spawn(async move {
            while should_run.load(Ordering::SeqCst) {
                let mut current_retry = retry_count.lock().await;

                if *current_retry >= config.max_retries {
                    eprintln!("🔴 Relay {} exhausted all retries", id);
                    let _ = app_clone.emit(
                        AppEvents::RelayFailed.as_str(),
                        (id.to_string(), "Maximum retry attempts reached".to_string()),
                    );
                    break;
                }

                let mut handle_guard = relay_handle.lock().await;

                if let Some(ref mut handle) = *handle_guard {
                    match handle.start(&app_clone).await {
                        Ok(_) => {
                            println!("✅ Relay {} started successfully", id);
                            *current_retry = 0; // Reset on success
                            drop(current_retry);
                            drop(handle_guard);

                            // Monitor the relay task
                            Self::monitor_relay(
                                id,
                                Arc::clone(&relay_handle),
                                Arc::clone(&should_run),
                            )
                            .await;

                            // If we get here, relay stopped or failed
                            if !should_run.load(Ordering::SeqCst) {
                                break; // Intentional stop
                            }

                            println!("⚠️ Relay {} disconnected, attempting reconnect", id);
                            // Re-acquire locks for next iteration
                            continue;
                        }
                        Err(e) => {
                            eprintln!("❌ Relay {} failed to start: {}", id, e);
                            *current_retry += 1;
                            let retry_num = *current_retry;

                            let _ = app_clone.emit(
                                AppEvents::RelayFailed.as_str(),
                                (
                                    id.to_string(),
                                    format!("Connection failed (attempt {}): {}", retry_num, e),
                                ),
                            );
                        }
                    }
                } else {
                    eprintln!("⚠️ Relay handle is None for relay {}", id);
                    break;
                }

                // Calculate delay before dropping guards
                let delay = Self::calculate_backoff_delay(&config, *current_retry);
                drop(current_retry);
                drop(handle_guard);

                println!("⏳ Relay {} waiting {}ms before retry", id, delay);
                sleep(Duration::from_millis(delay)).await;
            }

            println!("🛑 Supervisor for relay {} stopped", id);
        });

        self.supervisor_task = Some(task);
        Ok(())
    }

    /// Stop the relay and supervisor
    pub async fn stop(&mut self, app: &AppHandle) {
        println!("🛑 Stopping supervisor for relay {}", self.id);
        self.should_run
            .store(false, std::sync::atomic::Ordering::SeqCst);

        // Stop the relay handle
        if let Some(ref mut handle) = *self.relay_handle.lock().await {
            handle.stop(app).await;
        }

        // Abort supervisor task
        if let Some(task) = self.supervisor_task.take() {
            task.abort();
        }

        // Reset retry count
        *self.retry_count.lock().await = 0;
    }

    /// Monitor a running relay and detect when it stops
    async fn monitor_relay(
        id: i64,
        relay_handle: Arc<Mutex<Option<RelayHandle>>>,
        should_run: Arc<AtomicBool>,
    ) {
        loop {
            sleep(Duration::from_secs(5)).await;

            if !should_run.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            let handle_guard = relay_handle.lock().await;
            if let Some(handle) = handle_guard.as_ref() {
                if !handle.is_active() {
                    println!("⚠️ Relay {} is no longer active", id);
                    drop(handle_guard);
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Calculate exponential backoff delay
    fn calculate_backoff_delay(config: &ReconnectConfig, retry_count: u32) -> u64 {
        let delay =
            (config.initial_delay_ms as f64) * config.backoff_multiplier.powi(retry_count as i32);

        delay.min(config.max_delay_ms as f64) as u64
    }

    pub fn is_running(&self) -> bool {
        self.should_run.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Manages all relay supervisors
pub struct RelaySupervisorManager {
    supervisors: Arc<Mutex<std::collections::HashMap<i64, RelaySupervisor>>>,
}

impl RelaySupervisorManager {
    pub fn new() -> Self {
        Self {
            supervisors: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Start all enabled relay targets with supervision
    pub async fn start_all_relays(
        &self,
        app: &AppHandle,
        state: &Arc<AppState>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = db::get_db_pool();
        let relay_targets = db::get_active_relay_targets(&pool).await?;

        for target in relay_targets {
            let encoder_rx = state.encoder_tx.subscribe();
            let mut supervisor = RelaySupervisor::new(&target, encoder_rx, None);

            supervisor.start(app).await?;

            self.supervisors.lock().await.insert(target.id, supervisor);
        }

        Ok(())
    }

    /// Start a specific relay with supervision
    pub async fn start_relay(
        &self,
        id: i64,
        app: &AppHandle,
        state: &Arc<AppState>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = db::get_db_pool();
        let target = db::get_relay_target(id, &pool).await?;

        let encoder_rx = state.encoder_tx.subscribe();
        let mut supervisor = RelaySupervisor::new(&target, encoder_rx, None);

        supervisor.start(app).await?;

        self.supervisors.lock().await.insert(id, supervisor);

        Ok(())
    }

    /// Stop a specific relay
    pub async fn stop_relay(&self, id: i64, app: &AppHandle) -> Result<(), String> {
        let mut supervisors = self.supervisors.lock().await;

        if let Some(mut supervisor) = supervisors.remove(&id) {
            supervisor.stop(app).await;
            Ok(())
        } else {
            Err(format!("Relay {} not found", id))
        }
    }

    /// Stop all relays
    pub async fn stop_all_relays(&self, app: &AppHandle) {
        let mut supervisors = self.supervisors.lock().await;

        for (_, mut supervisor) in supervisors.drain() {
            supervisor.stop(app).await;
        }
    }
}
