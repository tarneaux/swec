mod state_actor;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use clap::Parser;
use state_actor::StateActorHandle;
use std::collections::BTreeMap;
use swec::ServiceSpec;
use tracing::info;

#[tokio::main]
async fn main() {
    // TODO: env config
    tracing_subscriber::fmt::init();

    // TODO: Load state
    let state_actor_handle = StateActorHandle::new(BTreeMap::new());

    let app = Router::new()
        .route("/:name/spec", post(post_service_spec))
        .with_state(state_actor_handle);

    let cli = Cli::parse();

    info!("Binding to {}", cli.address);
    let listener = tokio::net::TcpListener::bind(cli.address)
        .await
        .expect("Couldn't create TCP listener");
    info!("Starting API server");
    axum::serve(listener, app)
        .await
        .expect("Couldn't start API server");
}

async fn post_service_spec(
    State(state_actor_handle): State<StateActorHandle>,
    Path(name): Path<String>,
    Json(spec): Json<ServiceSpec>,
) -> (StatusCode, String) {
    state_actor_handle
        .create_watcher(name.clone(), spec.clone())
        .await
        .map_or_else(
            |e| (StatusCode::CONFLICT, e.to_string()),
            |()| (StatusCode::CREATED, "Created".to_string()),
        )
}

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Listening address for private API
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    address: String,
}
