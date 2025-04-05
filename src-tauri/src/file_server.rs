use crate::config;
use std::sync::{atomic::Ordering, Arc};
use tauri::{AppHandle, Manager};
use warp::Filter;

pub async fn start_file_server(app: AppHandle, port: u16) {
    let ouptut_dir: std::path::PathBuf = config::hls_output_dir(&app);
    std::fs::create_dir_all(&ouptut_dir).expect("Failed to create output dir");
    let preview_dir = warp::fs::dir(ouptut_dir);
    println!("🗂️  Starting file server...");
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["Content-Type", "Range"])
        .allow_methods(vec!["GET", "HEAD", "OPTIONS"]);

    let routes = preview_dir.with(cors);
    let app_state = app.state::<Arc<config::AppState>>();
    app_state.file_ready.store(true, Ordering::SeqCst);
    warp::serve(routes)
        .run(([127, 0, 0, 1], port)) // or choose your port
        .await;
    println!("File server started at port {}", port)
}
