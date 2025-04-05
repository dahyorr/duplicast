mod config;
mod db;
mod events;
mod file_server;
mod rtmp;
use config::{AppState, StartUpData};
use db::{EncoderSettings, RelayTargetPublic};
use rtmp::relay;
// use rtmp::stop_encoder;
use std::sync::Arc;
use tauri::{async_runtime, AppHandle, Manager};
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn check_if_ready(state: tauri::State<'_, Arc<config::AppState>>) -> bool {
    state.is_ready()
}

#[tauri::command]
fn check_if_stream_active(state: tauri::State<'_, Arc<config::AppState>>) -> bool {
    state
        .source_active
        .load(std::sync::atomic::Ordering::SeqCst)
}

#[tauri::command]
async fn get_startup_data(
    state: tauri::State<'_, Arc<config::AppState>>,
) -> Result<config::StartUpData, String> {
    let ports = state.ports.lock().await.clone();
    Ok(StartUpData {
        ports,
        ips: config::get_ip_addresses().await,
    })
}

#[tauri::command]
async fn start_all_relays(app: AppHandle) -> Result<(), String> {
    let _ = relay::start_relays(&app).await;
    Ok(())
}

#[tauri::command]
async fn stop_all_relays(app: AppHandle) -> Result<(), String> {
    let _ = relay::stop_relays(&app).await;
    Ok(())
}

#[tauri::command]
async fn start_relay(app: AppHandle, id: i64) -> Result<(), String> {
    let pool = db::get_db_pool();
    let relay = db::get_relay_target(id, &pool)
        .await
        .map_err(|e| e.to_string())?;
    let _ = relay::start_relay(&app, &relay).await;
    Ok(())
}

#[tauri::command]
async fn stop_relay(app: AppHandle, id: i64) -> Result<(), String> {
    let _ = relay::stop_relay(&app, id).await;
    Ok(())
}

#[tauri::command]
async fn add_relay_target(stream_key: &str, url: &str, tag: &str) -> Result<(), String> {
    let pool = db::get_db_pool();
    db::add_relay_target(url, stream_key, tag, &pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_relay_targets() -> Result<Vec<db::RelayTargetPublic>, String> {
    let pool = db::get_db_pool();
    let targets = db::get_relay_targets(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let public_targets = targets
        .iter()
        .map(RelayTargetPublic::from_relay_target)
        .collect();
    Ok(public_targets)
}

#[tauri::command]
async fn toggle_relay_target(id: i64, active: bool) -> Result<(), String> {
    let pool = db::get_db_pool();
    db::toggle_relay_target(id, active, &pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_relay_target(id: i64) -> Result<(), String> {
    let pool = db::get_db_pool();
    db::remove_relay_target(id, &pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_encoder_settings(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<EncoderSettings, String> {
    Ok(state.encoder_settings.lock().await.clone())
}

#[tauri::command]
async fn update_encoder_settings(
    state: tauri::State<'_, Arc<AppState>>,
    settings: EncoderSettings,
) -> Result<(), String> {
    let pool = db::get_db_pool();
    db::save_encoder_settings(&settings, &pool)
        .await
        .map_err(|e| e.to_string())?;

    *state.encoder_settings.lock().await = settings;
    Ok(())
}

// async fn cleanup_all(app: &AppHandle) {
//     // Stop all relays
//     let _ = stop_all_relays(app.clone()).await;
//     // Stop encoder if active
//     stop_encoder(app).await;
//     // delete hls_files
//     config::hls_output_dir()
//         .read_dir()
//         .expect("Failed to read directory")
//         .filter_map(|entry| entry.ok())
//         .for_each(|entry| {
//             let path = entry.path();
//             if path.is_file() {
//                 std::fs::remove_file(path).expect("Failed to delete file");
//             }
//         });

//     println!("✅ Cleanup complete. Safe to exit.");
// }

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_sql::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            check_if_ready,
            get_startup_data,
            check_if_stream_active,
            add_relay_target,
            get_relay_targets,
            toggle_relay_target,
            remove_relay_target,
            start_all_relays,
            stop_all_relays,
            stop_relay,
            start_relay,
            get_encoder_settings,
            update_encoder_settings,
        ])
        .setup(|app| {
            let app_handle = app.handle();
            let app_state = Arc::new(config::AppState::new(0, 0));
            // Create the log directory if it doesn't exist
            // let data_dir = app
            //     .path_resolver()
            //     .app_data_dir()
            //     .unwrap_or_else(|| std::env::current_dir().unwrap());

            app.manage(app_state);
            let log_dir = config::log_output_dir(&app_handle);
            if !log_dir.exists() {
                std::fs::create_dir_all(&log_dir).expect("Failed to create log directory");
            }
            //clear hls_output_dir on start
            let hls_dir = config::hls_output_dir(&app_handle);
            if hls_dir.exists() {
                std::fs::remove_dir_all(&hls_dir).expect("Failed to remove hls_output_dir");
            }
            let app = app_handle.clone();
            async_runtime::spawn(async move {
                let _ = db::init_db(&app).await.expect("❌ Failed to init DB");
                println!("✅ Database ready");

                let db_pool = db::get_db_pool();
                let port_info = config::get_or_init_ports(db_pool)
                    .await
                    .expect("❌ Failed to init ports");
                let app_state = app.state::<Arc<config::AppState>>();
                let mut ports = app_state.ports.lock().await;
                ports.rtmp_port = port_info.rtmp_port;
                ports.file_port = port_info.file_port;
                let settings = db::load_encoder_settings(db_pool)
                    .await
                    .unwrap_or_else(|_| db::default_encoder_settings());
                let app_clone_rtmp: tauri::AppHandle = app.clone();
                let app_clone_file: tauri::AppHandle = app.clone();
                *app_state.encoder_settings.lock().await = settings;
                async_runtime::spawn(rtmp::init_rtmp_server(app_clone_rtmp, port_info.rtmp_port));
                async_runtime::spawn(file_server::start_file_server(
                    app_clone_file,
                    port_info.file_port,
                ));

                // tokio::spawn(wait_for_ready(app_handle_clone, app_state));
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// async fn wait_for_ready(app: AppHandle, state: &Arc<config::AppState>) {
//     use tokio::time::{sleep, Duration};

//     // Polling until ready
//     loop {
//         if state.is_ready() {
//             println!("✅ Both RTMP and file servers are ready.");
//             let _ = app.emit("servers-ready", &state.ports);
//             break;
//         }
//         sleep(Duration::from_millis(100)).await;
//     }
// }
