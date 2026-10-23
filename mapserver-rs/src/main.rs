use std::sync::Arc;

use mapserver_rs::coordinates::Tile;
use mapserver_rs::mappool::MapPool;
use mapserver_rs::Extent;

use axum::extract::Path;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::Extension;
use axum::{routing::get, Router};
use tokio::sync::Mutex;

#[derive(Debug)]
struct State {
    maplock: Mutex<MapPool>,
    mapfile: String,
    mappath: Option<String>,
}

#[tokio::main]
async fn main() {
    // The mapfile is the primary interface for configuring Mapserver.
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <mapfile>", args[0]);
        eprintln!();
        eprintln!("Render a Mapserver mapfile as a slippy-map tile server.");
        eprintln!("Example: {} example/basic.map", args[0]);
        std::process::exit(1);
    }
    let mapfile_path = &args[1];

    // Validate the mapfile by reading it before starting the server.
    let mapfile = match std::fs::read_to_string(mapfile_path) {
        Ok(contents) if !contents.trim().is_empty() => contents,
        Ok(_) => {
            eprintln!("mapfile is empty: {mapfile_path}");
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("unable to read mapfile {mapfile_path}: {err}");
            std::process::exit(1);
        }
    };

    // Resolve relative paths in the mapfile (DATA, SHAPEPATH, etc.)
    // against the mapfile's directory.
    let mappath = std::path::Path::new(mapfile_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string());

    // Set up shared state
    let map_pool = MapPool::create(24);
    let shared_state = Arc::new(State {
        maplock: Mutex::new(map_pool),
        mapfile,
        mappath,
    });

    // Routes
    let app = Router::new()
        .route("/", get(index))
        .route("/map/{z}/{x}/{y}", get(render_map))
        .layer(Extension(shared_state));

    // Spawn the web handler
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
        println!("Listening on 0.0.0.0:3000");
        axum::serve(listener, app).await.unwrap();
    });

    // And wait for an interrupt signal
    tokio::signal::ctrl_c().await.unwrap();
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn render_map(
    Path((z, x, y)): Path<(u32, u32, u32)>,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    // Render the requested tile extent
    let tile = Tile::from_zxy(z, x, y);
    let extent = Extent::from(tile.bbox_mercator());

    // Get a renderer from the map pool
    let renderer = {
        let mut map_pool = state.maplock.lock().await;
        map_pool.acquire_or_create(state.mapfile.clone(), state.mappath.clone())
    };

    // Yes, we can render concurrently on multiple threads!
    // GDAL may lock things internally though, negating much of the benefit
    let image_bytes = renderer.render(extent);

    ([(header::CONTENT_TYPE, "image/png")], image_bytes)
}
