mod state_actor;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use clap::Parser;
use state_actor::{ServiceNotFoundError, StateActorHandle, WriteError};
use std::collections::{BTreeMap, VecDeque};
use swec::{ServiceAction, TimedStatus};
use tokio::spawn;
use tokio::sync::broadcast;
use tracing::info;

#[tokio::main]
async fn main() {
    // TODO: env config
    tracing_subscriber::fmt::init();

    // TODO: Load state
    let state_actor_handle = StateActorHandle::new(BTreeMap::new(), 32);

    let app = Router::new()
        .route("/:name", put(put_action))
        .route("/:name/statuses", get(get_statuses))
        .route("/:name/status", get(get_status_at))
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

async fn put_action(
    State(state_actor_handle): State<StateActorHandle>,
    Path(name): Path<String>,
    Json(action): Json<ServiceAction>,
) -> (StatusCode, String) {
    state_actor_handle.write(name, action).await.map_or_else(
        |e| match e {
            WriteError::NameConflict => (StatusCode::CONFLICT, e.to_string()),
            WriteError::NotFound => (StatusCode::NOT_FOUND, e.to_string()),
        },
        |()| (StatusCode::NO_CONTENT, "Action executed".to_string()),
    )
}

async fn get_statuses(
    State(state_actor_handle): State<StateActorHandle>,
    Path(name): Path<String>,
) -> Result<(StatusCode, Json<VecDeque<TimedStatus>>), (StatusCode, String)> {
    state_actor_handle
        .get_statuses(name)
        .await
        .map(|v| (StatusCode::OK, Json(v)))
        .map_err(|ServiceNotFoundError| (StatusCode::NOT_FOUND, "Not found".to_string()))
}

async fn get_status_at(
    State(state_actor_handle): State<StateActorHandle>,
    Path(name): Path<String>,
    Json(time): Json<DateTime<Utc>>,
) -> Result<(StatusCode, Json<Option<TimedStatus>>), (StatusCode, String)> {
    state_actor_handle
        .get_status_at(name, time)
        .await
        .map(|v| (StatusCode::OK, Json(v)))
        .map_err(|ServiceNotFoundError| (StatusCode::NOT_FOUND, "Not found".to_string()))
}

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Listening address for private API
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    address: String,
}
