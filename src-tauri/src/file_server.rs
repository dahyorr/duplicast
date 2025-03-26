use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use warp::Filter;

pub async fn start_file_server(ready: Arc<AtomicBool>, port: u16) {
    let preview_dir = warp::fs::dir("./hls-output");
    println!("🗂️  Starting file server...");
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["Content-Type", "Range"])
        .allow_methods(vec!["GET", "HEAD", "OPTIONS"]);

    let routes = preview_dir.with(cors);
    ready.store(true, Ordering::SeqCst);
    warp::serve(routes)
        .run(([127, 0, 0, 1], port)) // or choose your port
        .await;
    println!("File server started at port {}", port)
}
