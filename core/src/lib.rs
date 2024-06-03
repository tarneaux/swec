use chrono::DateTime;

/// A service that is being watched by a checker.
pub struct Service<B: StatusBuffer> {
    statuses: B,
}

pub trait StatusBuffer {
    fn push(&mut self, status: DatedStatus);
    fn get(&self, index: usize) -> Option<DatedStatus>;
}

struct DatedStatus(dateTime<Utc>, Status);

/// The status of a `Service`.
pub enum Status {
    Up(u64),
    Down(String),
    Unknown(String),
}
