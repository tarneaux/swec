mod api_util;
mod state_actor;

use api_util::ApiError;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use clap::Parser;
use state_actor::StateActorHandle;
use std::collections::{BTreeMap, VecDeque};
use swec::{ServiceAction, ServiceSpec, TimedStatus};
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
        .route("/:name/spec", get(get_spec))
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
) -> Result<(StatusCode, String), ApiError> {
    state_actor_handle.write(name, action).await?;
    Ok((StatusCode::NO_CONTENT, "Action executed".to_string()))
}

async fn get_statuses(
    State(state_actor_handle): State<StateActorHandle>,
    Path(name): Path<String>,
) -> Result<(StatusCode, Json<VecDeque<TimedStatus>>), ApiError> {
    let statuses = state_actor_handle.get_statuses(name).await?;
    Ok((StatusCode::OK, Json(statuses)))
}

async fn get_status_at(
    State(state_actor_handle): State<StateActorHandle>,
    Path(name): Path<String>,
    Json(time): Json<DateTime<Utc>>,
) -> Result<(StatusCode, Json<Option<TimedStatus>>), ApiError> {
    let status = state_actor_handle.get_status_at(name, time).await?;
    Ok((StatusCode::OK, Json(status)))
}

async fn get_spec(
    State(state_actor_handle): State<StateActorHandle>,
    Path(name): Path<String>,
) -> Result<(StatusCode, Json<ServiceSpec>), ApiError> {
    let spec = state_actor_handle.get_spec(name).await?;
    Ok((StatusCode::OK, Json(spec)))
}

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Listening address for private API
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    address: String,
}
