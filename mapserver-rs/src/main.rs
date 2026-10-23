use std::sync::Arc;

use mapserver_rs::coordinates::Tile;
use mapserver_rs::mappool::MapPool;
use mapserver_rs::Extent;

use axum::extract::Path;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::Extension;
use axum::{routing::get, Router};
use clap::Parser;
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(version, about = "Serve a MapServer mapfile as a ZXY tile server.")]
struct Cli {
    /// The mapfile to serve.
    mapfile: String,

    /// Enable standard web request logging.
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug)]
struct State {
    maplock: Mutex<MapPool>,
    mapfile: String,
    mappath: Option<String>,
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // The mapfile is the primary interface for configuring Mapserver.
    // Without that DSL, this code would be a lot longer
    let mapfile_path = &cli.mapfile;

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

    // Fail fast: parse the mapfile and smoke-render it at its own EXTENT/SIZE
    // before binding the socket, so a bad mapfile aborts startup with a clear
    // MapServer error instead of failing on the first request.
    match mapserver_rs::mappool::Map::load(mapfile.clone(), mappath.clone())
        .and_then(|map| map.validate())
    {
        Ok(()) => {}
        Err(err) => {
            eprintln!("invalid mapfile {mapfile_path}: {err}");
            std::process::exit(1);
        }
    }

    // Set up shared state
    let map_pool = MapPool::create(24);
    let shared_state = Arc::new(State {
        maplock: Mutex::new(map_pool),
        mapfile,
        mappath,
    });

    // Enable standard web request logging when verbose is set.
    if cli.verbose {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "tower_http=debug".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    // Routes
    let app = Router::new()
        .route("/", get(index))
        .route("/map/{z}/{x}/{y}", get(render_map))
        .layer(Extension(shared_state));

    let app = if cli.verbose {
        app.layer(TraceLayer::new_for_http())
    } else {
        app
    };

    // Spawn the web handler
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
        println!("Listening on http://0.0.0.0:3000");
        axum::serve(listener, app).await.unwrap();
    });

    // And wait for an interrupt signal
    tokio::signal::ctrl_c().await.unwrap();
}
