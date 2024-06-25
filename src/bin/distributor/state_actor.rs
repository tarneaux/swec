use std::collections::BTreeMap;
use swec::{Service, ServiceAction};
use tokio::sync::{broadcast, mpsc, oneshot};

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

    fn handle_message(&mut self, name: String, msg: ServiceAction) -> Result<(), ()> {
        match msg {
            ServiceAction::CreateService(spec) => {
                if self.services.contains_key(&name) {
                    return Err(());
                }
                self.services.insert(name, Service::new(spec, self.cap));
                Ok(())
            }
            ServiceAction::DeleteService => self
                .services
                .remove(&name)
                .map_or_else(|| Err(()), |_| Ok(())),
            ServiceAction::AddStatus(s) => self.services.get_mut(&name).map_or_else(
                || Err(()),
                |service| {
                    service.statuses.push_front(s);
                    Ok(())
                },
            ),
        }
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

    /// Run the specified `ServiceAction` on the service with the specified name.
    ///
    /// # Errors
    ///
    /// If there is a name conflict (in the case `CreateService`) or if the specified service
    /// doesn't exist (other cases).
    pub async fn write(&self, name: String, msg: ServiceAction) -> Result<(), ()> {
        let (send, recv) = oneshot::channel();

        let encapsulated_msg = StateActorMessage::Write {
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
}

enum StateActorMessage {
    Write {
        name: String,
        action: ServiceAction,
        respond_to: oneshot::Sender<Result<(), ()>>,
    },
}
