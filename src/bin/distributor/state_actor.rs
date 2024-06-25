use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Display};
use swec::{Service, ServiceAction, ServiceSpec};
use tokio::sync::{broadcast, mpsc, oneshot};

struct StateActor {
    receiver: mpsc::UnboundedReceiver<StateActorMessage>,
    services: BTreeMap<String, Service>,
}

impl StateActor {
    fn new(
        receiver: mpsc::UnboundedReceiver<StateActorMessage>,
        services: BTreeMap<String, Service>,
    ) -> Self {
        Self { receiver, services }
    }

    fn handle_message(&mut self, name: String, msg: ServiceAction) -> Result<(), StateActorError> {
        match msg {
            ServiceAction::CreateService(spec) => {
                if self.services.contains_key(&name) {
                    return Err(StateActorError::Conflict);
                }
                self.services.insert(name, Service::new(spec));
                Ok(())
            }
        }
    }

    async fn run(&mut self) {
        while let Some(msg) = self.receiver.recv().await {
            // Errors when sending can happen e.g. if the `select!` macro is used to cancel waiting
            // for the response. We can safely ignore these.
            match msg {
                StateActorMessage::AlterateService {
                    respond_to,
                    name,
                    action,
                } => {
                    let _ = respond_to.send(self.handle_message(name, action));
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct StateActorHandle {
    mpsc_sender: mpsc::UnboundedSender<StateActorMessage>,
    broadcast_sender: broadcast::Sender<(String, ServiceAction)>,
}

impl StateActorHandle {
    pub fn new(services: BTreeMap<String, Service>) -> Self {
        let (mpsc_sender, mpsc_receiver) = mpsc::unbounded_channel();
        let mut actor = StateActor::new(mpsc_receiver, services);
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

    async fn alterate_service(
        &self,
        name: String,
        msg: ServiceAction,
    ) -> Result<(), StateActorError> {
        let (send, recv) = oneshot::channel();

        let encapsulated_msg = StateActorMessage::AlterateService {
            name: name.clone(),
            action: msg.clone(),
            respond_to: send,
        };

        // Ignore send errors. If this send fails, so does the
        // recv.await below. There's no reason to check for the
        // same failure twice.
        let _ = self.mpsc_sender.send(encapsulated_msg);
        let resp = recv.await.expect("Actor task has been killed");

        if resp.is_ok() {
            // If this fails, there just aren't any subscribers to send messages to.
            let _ = self.broadcast_sender.send((name, msg));
        }

        resp
    }

    pub async fn create_watcher(
        &self,
        name: String,
        spec: ServiceSpec,
    ) -> Result<(), StateActorError> {
        self.alterate_service(name, ServiceAction::CreateService(spec))
            .await
    }
}

enum StateActorMessage {
    AlterateService {
        name: String,
        action: ServiceAction,
        respond_to: oneshot::Sender<Result<(), StateActorError>>,
    },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum StateActorError {
    Conflict,
}

impl Display for StateActorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict => write!(f, "Conflict"),
        }
    }
}
