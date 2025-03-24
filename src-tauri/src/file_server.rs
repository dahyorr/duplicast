use warp::Filter;

pub async fn start_file_server() {
    let preview_dir = warp::fs::dir("./public/preview");
    let port = 8787;

    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["Content-Type", "Range"])
        .allow_methods(vec!["GET", "HEAD", "OPTIONS"]);

    let routes = preview_dir.with(cors);
    warp::serve(routes)
        .run(([127, 0, 0, 1], port)) // or choose your port
        .await;
    println!("File server started at port {}", port)
}
