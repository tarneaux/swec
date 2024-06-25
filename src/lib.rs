use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A service that is being watched by a checker.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Service {
    pub statuses: VecDeque<TimedStatus>,
    pub spec: ServiceSpec,
}

impl Service {
    pub fn new(spec: ServiceSpec) -> Self {
        Self {
            statuses: VecDeque::new(),
            spec,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimedStatus {
    pub time: DateTime<Utc>,
    pub inner: Status,
}

/// The status of a `Service`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Status {
    Up(u32),         // Latency
    Down(String),    // Reason
    Unknown(String), // Reason
}

impl Status {
    #[must_use]
    pub fn to_map(&self) -> Box<[(String, String)]> {
        Box::new(match self {
            Self::Unknown(reason) => [
                ("kind".to_string(), "unknown".to_string()),
                ("reason".to_string(), reason.to_string()),
            ],
            Self::Down(reason) => [
                ("kind".to_string(), "down".to_string()),
                ("reason".to_string(), reason.to_string()),
            ],
            Self::Up(d) => [
                ("kind".to_string(), "unknown".to_string()),
                ("reason".to_string(), d.to_string()),
            ],
        })
    }
}

/// Human-readable information about a `Service`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceSpec {
    pub kind: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    CreateService(String, ServiceSpec),
}
