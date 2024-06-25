use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Display};
use swec::{Message, Service, ServiceSpec};
use tokio::sync::{mpsc, oneshot};

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

    async fn handle_message(&mut self, msg: Message) -> Result<(), StateActorError> {
        match msg {
            Message::CreateService(name, spec) => {
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
            let _ = msg.respond_to.send(self.handle_message(msg.inner).await);
        }
    }
}

#[derive(Clone)]
pub struct StateActorHandle {
    sender: mpsc::UnboundedSender<StateActorMessage>,
}

impl StateActorHandle {
    pub fn new(services: BTreeMap<String, Service>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut actor = StateActor::new(receiver, services);
        tokio::spawn(async move { actor.run().await });

        Self { sender }
    }

    pub async fn create_watcher(
        &self,
        name: String,
        spec: ServiceSpec,
    ) -> Result<(), StateActorError> {
        let (send, recv) = oneshot::channel();
        let msg = StateActorMessage {
            inner: Message::CreateService(name, spec),
            respond_to: send,
        };

        // Ignore send errors. If this send fails, so does the
        // recv.await below. There's no reason to check for the
        // same failure twice.
        let _ = self.sender.send(msg);
        recv.await.expect("Actor task has been killed")
    }
}

struct StateActorMessage {
    inner: Message,
    respond_to: oneshot::Sender<Result<(), StateActorError>>,
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
