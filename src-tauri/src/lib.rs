mod config;
mod db;
mod file_server;
mod rtmp;
use std::sync::Arc;
use tauri::{async_runtime, Manager};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn check_if_ready(state: tauri::State<'_, Arc<config::AppState>>) -> bool {
    state.is_ready()
}

#[tauri::command]
fn get_ports(state: tauri::State<'_, Arc<config::AppState>>) -> config::PortInfo {
    state.ports.lock().unwrap().clone()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_sql::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet, check_if_ready, get_ports])
        .setup(|app| {
            // let app_handle = app.handle().clone();
            let app_state = Arc::new(config::AppState::new(0, 0));
            app.manage(app_state.clone());
            let app_state_clone = app_state.clone();
            // let app_handle_clone = app_handle.clone();

            async_runtime::spawn(async move {
                let _ = db::init_db().await.expect("❌ Failed to init DB");
                println!("✅ Database ready");

                let db_pool = db::get_db_pool();
                let port_info = config::get_or_init_ports(db_pool)
                    .await
                    .expect("❌ Failed to init ports");
                let mut ports = app_state_clone.ports.lock().unwrap();
                ports.rtmp_port = port_info.rtmp_port;
                ports.file_port = port_info.file_port;

                let rtmp_notify = app_state_clone.rtmp_ready.clone();
                let file_notify = app_state_clone.file_ready.clone();

                async_runtime::spawn(rtmp::init_rtmp_server(rtmp_notify, port_info.rtmp_port));
                async_runtime::spawn(file_server::start_file_server(
                    file_notify,
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
