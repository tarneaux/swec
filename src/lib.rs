use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, fmt::Display};

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

impl Display for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Service with spec {:?} and statuses {:?}",
            self.spec, self.statuses
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimedStatus {
    pub time: DateTime<Utc>,
    pub inner: Status,
}

impl Display for TimedStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Status at {}: {}", self.time, self.inner)
    }
}

/// The status of a `Service`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Status {
    Up(u32),         // Latency, ms
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

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Up(l) => write!(f, "Up, latency {l}ms"),
            Self::Down(r) => write!(f, "Down: {r}"),
            Self::Unknown(r) => write!(f, "Unknown: {r}"),
        }
    }
}

/// Human-readable information about a `Service`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceSpec {
    pub kind: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceAction {
    CreateService(ServiceSpec),
}

impl Display for ServiceAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateService(spec) => {
                write!(f, "Create service with spec: {spec:?}")
            }
        }
    }
}
