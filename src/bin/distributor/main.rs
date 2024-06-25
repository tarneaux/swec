mod state_actor;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, post},
    Json, Router,
};
use clap::Parser;
use state_actor::StateActorHandle;
use std::collections::BTreeMap;
use swec::{ServiceAction, ServiceSpec, TimedStatus};
use tracing::info;

#[tokio::main]
async fn main() {
    // TODO: env config
    tracing_subscriber::fmt::init();

    // TODO: Load state
    let state_actor_handle = StateActorHandle::new(BTreeMap::new(), 32);

    let app = Router::new()
        .route("/:name/spec", post(post_service_spec))
        .route("/:name", delete(delete_service))
        .route("/:name/status", post(post_status))
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
        .write(name, ServiceAction::CreateService(spec))
        .await
        .map_or_else(
            |()| (StatusCode::CONFLICT, "Conflict".to_string()),
            |()| (StatusCode::CREATED, "Created".to_string()),
        )
}

async fn delete_service(
    State(state_actor_handle): State<StateActorHandle>,
    Path(name): Path<String>,
) -> (StatusCode, String) {
    state_actor_handle
        .write(name, ServiceAction::DeleteService)
        .await
        .map_or_else(
            |()| (StatusCode::NOT_FOUND, "Not found".to_string()),
            |()| (StatusCode::NO_CONTENT, "Deleted".to_string()),
        )
}

async fn post_status(
    State(state_actor_handle): State<StateActorHandle>,
    Path(name): Path<String>,
    Json(status): Json<TimedStatus>,
) -> (StatusCode, String) {
    state_actor_handle
        .write(name, ServiceAction::AddStatus(status))
        .await
        .map_or_else(
            |()| (StatusCode::NOT_FOUND, "Not found".to_string()),
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
