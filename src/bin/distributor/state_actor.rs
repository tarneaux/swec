use chrono::{DateTime, Utc};
use std::{
    collections::{BTreeMap, VecDeque},
    error::Error,
    fmt::Display,
};
use swec::{Service, ServiceAction, ServiceSpec, TimedStatus};
use tokio::sync::{broadcast, mpsc, oneshot};

#[derive(Debug)]
struct StateActor {
    receiver: mpsc::UnboundedReceiver<StateActorMessage>,
    services: BTreeMap<String, Service>,
    cap: usize,
}

impl StateActor {
    fn new(
        receiver: mpsc::UnboundedReceiver<StateActorMessage>,
        services: BTreeMap<String, Service>,
        cap: usize,
    ) -> Self {
        Self {
            receiver,
            services,
            cap,
        }
    }

    fn handle_write(&mut self, name: String, msg: ServiceAction) -> Result<(), WriteError> {
        match msg {
            ServiceAction::CreateService(spec) => {
                if self.services.contains_key(&name) {
                    return Err(WriteError::NameConflict);
                }
                self.services.insert(name, Service::new(spec, self.cap));
                Ok(())
            }
            ServiceAction::DeleteService => self
                .services
                .remove(&name)
                .map_or_else(|| Err(WriteError::NotFound), |_| Ok(())),
            ServiceAction::AddStatus(s) => self.services.get_mut(&name).map_or_else(
                || Err(WriteError::NotFound),
                |service| {
                    service.statuses.push_front(s);
                    Ok(())
                },
            ),
        }
    }

    fn handle_get_spec(&mut self, name: &str) -> Result<ServiceSpec, ServiceNotFoundError> {
        self.services
            .get(name)
            .map(|s| s.spec.clone())
            .ok_or(ServiceNotFoundError)
    }

    fn handle_get_statuses(
        &mut self,
        name: &str,
    ) -> Result<VecDeque<TimedStatus>, ServiceNotFoundError> {
        self.services
            .get(name)
            .map(|s| s.statuses.clone())
            .ok_or(ServiceNotFoundError)
    }

    fn handle_get_status_at(
        &mut self,
        name: &str,
        time: DateTime<Utc>,
    ) -> Result<Option<TimedStatus>, ServiceNotFoundError> {
        self.services
            .get(name)
            .map(|s| {
                // TODO: Search through the statuses dichotonomically.
                // Is a VecDeque the right data structure, since the statuses should be ordered ?
                s.statuses
                    .iter()
                    .min_by_key(|status| (status.time - time).abs())
                    .cloned()
            })
            .ok_or(ServiceNotFoundError)
    }

    async fn run(&mut self) {
        while let Some(msg) = self.receiver.recv().await {
            // Errors when sending can happen e.g. if the `select!` macro is used to cancel waiting
            // for the response. We can safely ignore these.
            match msg {
                StateActorMessage::Write {
                    respond_to,
                    name,
                    action,
                } => {
                    let _ = respond_to.send(self.handle_write(name, action));
                }
                StateActorMessage::GetStatuses { name, respond_to } => {
                    let _ = respond_to.send(self.handle_get_statuses(&name));
                }
                StateActorMessage::GetStatusAt {
                    name,
                    time,
                    respond_to,
                } => {
                    let _ = respond_to.send(self.handle_get_status_at(&name, time));
                }
                StateActorMessage::GetSpec { name, respond_to } => {
                    let _ = respond_to.send(self.handle_get_spec(&name));
                }
            };
        }
    }
}

#[derive(Clone)]
pub struct StateActorHandle {
    mpsc_sender: mpsc::UnboundedSender<StateActorMessage>,
    broadcast_sender: broadcast::Sender<(String, ServiceAction)>,
}

impl StateActorHandle {
    /// Create a new state instance and return its handle.
    pub fn new(services: BTreeMap<String, Service>, cap: usize) -> Self {
        let (mpsc_sender, mpsc_receiver) = mpsc::unbounded_channel();
        let mut actor = StateActor::new(mpsc_receiver, services, cap);
        tokio::spawn(async move { actor.run().await });

        let broadcast_sender = broadcast::Sender::new(32);

        Self {
            mpsc_sender,
            broadcast_sender,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<(String, ServiceAction)> {
        self.broadcast_sender.subscribe()
    }

    async fn exchange<R>(&self, msg: StateActorMessage, recv: oneshot::Receiver<R>) -> R {
        // Ignore send errors. If this send fails, so does the
        // recv.await below. There's no reason to check for the
        // same failure twice.
        let _ = self.mpsc_sender.send(msg);
        recv.await.expect("Actor task has been killed")
    }

    /// Run the specified `ServiceAction` on the service with the specified name.
    ///
    /// # Errors
    ///
    /// If there is a name conflict (in the case `CreateService`) or if the specified service
    /// doesn't exist (other cases).
    pub async fn write(&self, name: String, action: ServiceAction) -> Result<(), WriteError> {
        let (send, recv) = oneshot::channel();

        let msg = StateActorMessage::Write {
            name: name.clone(),
            action: action.clone(),
            respond_to: send,
        };

        let resp = self.exchange(msg, recv).await;

        if resp.is_ok() {
            // If this fails, there just aren't any subscribers to send messages to.
            let _ = self.broadcast_sender.send((name, action));
        }

        resp
    }

    pub async fn get_statuses(
        &self,
        name: String,
    ) -> Result<VecDeque<TimedStatus>, ServiceNotFoundError> {
        let (send, recv) = oneshot::channel();

        let msg = StateActorMessage::GetStatuses {
            name,
            respond_to: send,
        };

        self.exchange(msg, recv).await
    }

    pub async fn get_status_at(
        &self,
        name: String,
        time: DateTime<Utc>,
    ) -> Result<Option<TimedStatus>, ServiceNotFoundError> {
        let (send, recv) = oneshot::channel();

        let msg = StateActorMessage::GetStatusAt {
            name,
            time,
            respond_to: send,
        };

        self.exchange(msg, recv).await
    }

    pub async fn get_spec(&self, name: String) -> Result<ServiceSpec, ServiceNotFoundError> {
        let (send, recv) = oneshot::channel();

        let msg = StateActorMessage::GetSpec {
            name,
            respond_to: send,
        };

        self.exchange(msg, recv).await
    }
}

#[derive(Debug)]
enum StateActorMessage {
    Write {
        name: String,
        action: ServiceAction,
        respond_to: oneshot::Sender<Result<(), WriteError>>,
    },
    GetStatuses {
        name: String,
        respond_to: oneshot::Sender<Result<VecDeque<TimedStatus>, ServiceNotFoundError>>,
    },
    GetStatusAt {
        name: String,
        time: DateTime<Utc>,
        respond_to: oneshot::Sender<Result<Option<TimedStatus>, ServiceNotFoundError>>,
    },
    GetSpec {
        name: String,
        respond_to: oneshot::Sender<Result<ServiceSpec, ServiceNotFoundError>>,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum WriteError {
    NotFound,
    NameConflict,
}

impl Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => ServiceNotFoundError.fmt(f),
            Self::NameConflict => write!(f, "Service name conflict"),
        }
    }
}

impl Error for WriteError {}

#[derive(Debug, Clone, Copy)]
pub struct ServiceNotFoundError;

impl Display for ServiceNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Service not found")
    }
}

impl Error for ServiceNotFoundError {}
