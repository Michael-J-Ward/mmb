use crate::core::lifecycle::trading_engine::Service;
use crate::core::text;
use actix::Recipient;
use actix::{Message, System};
use anyhow::Result;
use futures::future::join_all;
use futures::FutureExt;
use itertools::Itertools;
use log::{error, info, trace};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::sync::oneshot::Sender;
use tokio::time::{sleep, Duration};

#[derive(Message)]
#[rtype(result = "()")]
pub struct GracefulShutdownMsg {
    pub service_finished: Sender<Result<()>>,
}

struct ActorInfo {
    name: String,
    actor: Recipient<GracefulShutdownMsg>,
}

#[derive(Default)]
struct State {
    services: Vec<Arc<dyn Service>>,
    actors: Vec<ActorInfo>,
}

#[derive(Default)]
pub struct ShutdownService {
    state: Mutex<State>,
}

impl ShutdownService {
    pub fn register_service(self: &Arc<Self>, service: Arc<dyn Service>) {
        trace!("Registered in ShutdownService service '{}'", service.name());
        self.state.lock().services.push(service);
    }

    pub fn register_services(self: &Arc<Self>, services: &[Arc<dyn Service>]) {
        for service in services {
            self.register_service(service.clone());
        }
    }

    pub fn register_actor(&self, name: String, actor: Recipient<GracefulShutdownMsg>) {
        trace!("Registered in ShutdownService actor '{}'", name);
        self.state.lock().actors.push(ActorInfo { name, actor });
    }

    pub(crate) async fn graceful_shutdown(&self) -> Vec<String> {
        let mut finish_receivers = Vec::new();

        trace!("Prepare to drop services in ShutdownService started");

        {
            trace!("Running graceful shutdown for actors started");

            let state_guard = self.state.lock();
            for actor_info in &state_guard.actors {
                let (service_finished, receiver) = oneshot::channel::<Result<()>>();
                let _ = actor_info
                    .actor
                    .try_send(GracefulShutdownMsg { service_finished });

                let actor_name = format!("actor {}", actor_info.name);

                trace!("Waiting graceful shutdown finishing for {}", actor_name);
                finish_receivers.push((actor_name, receiver));
            }

            trace!("Running graceful shutdown for actors finished");

            trace!("Running graceful shutdown for services started");
            for service in &state_guard.services {
                let receiver = service.clone().graceful_shutdown();

                if let Some(receiver) = receiver {
                    let service_name = format!("service {}", service.name());

                    trace!("Waiting finishing graceful shutdown for {}", service_name);
                    finish_receivers.push((service_name, receiver));
                } else {
                    trace!(
                        "Service {} not needed waiting graceful shutdown or already finished",
                        service.name()
                    )
                }
            }
            trace!("Running graceful shutdown for services finished");
        }

        // log errors when its came
        let finishing_services_futures = finish_receivers
            .into_iter()
            .map(|(service_name, receiver)| {
                receiver.map(
                    move |finishing_service_send_result| match finishing_service_send_result {
                        Err(err) => {
                            error!(
                                "Can't receive message for finishing graceful shutdown in {} because of error: {:?}",
                                service_name,
                                err
                            );
                        },
                        Ok(finishing_service_result) => match finishing_service_result {
                            Err(err) => {
                                error!(
                                    "{} finished on graceful shutdown with error: {:?}",
                                    service_name,
                                    err
                                );
                            }
                            Ok(_) => {
                                trace!(
                                    "Graceful shutdown for {} completed successfully",
                                    service_name
                                );
                            },
                        },
                    },
                )
            })
            .collect_vec();

        const TIMEOUT: Duration = Duration::from_secs(3);
        tokio::select! {
            _ = join_all(finishing_services_futures) => trace!("All services sent finished marker at given time"),
            _ = sleep(TIMEOUT) => error!("Not all services finished after timeout ({} sec)", TIMEOUT.as_secs()),
        }

        trace!("Prepare to drop services in ShutdownService finished");

        trace!("Stopping actor system");
        System::current().stop();

        trace!("Drop services in ShutdownService started");

        let weak_services;
        {
            let mut state_guard = self.state.lock();
            weak_services = state_guard
                .services
                .drain(..)
                .map(|x| Arc::downgrade(&x))
                .collect_vec();
        }

        trace!("Drop services in ShutdownService finished");

        let not_dropped_services = weak_services
            .iter()
            .filter_map(|weak_service| {
                if weak_service.strong_count() > 0 {
                    weak_service
                        .upgrade()
                        .map(|service| service.name().to_string())
                } else {
                    None
                }
            })
            .collect_vec();

        if not_dropped_services.is_empty() {
            info!("After graceful shutdown all services dropped completely")
        } else {
            error!(
                "After graceful shutdown follow services wasn't dropped:{}{}",
                text::LINE_ENDING,
                not_dropped_services.join(text::LINE_ENDING)
            )
        }

        not_dropped_services
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::logger::init_logger;
    use tokio::sync::oneshot::Receiver;

    #[actix_rt::test]
    pub async fn success() {
        init_logger();

        pub struct TestService;

        impl TestService {
            pub fn new() -> Arc<Self> {
                Arc::new(Self)
            }
        }

        impl Service for TestService {
            fn name(&self) -> &str {
                "SomeTestService"
            }

            fn graceful_shutdown(self: Arc<Self>) -> Option<Receiver<Result<()>>> {
                None
            }
        }

        let shutdown_service = Arc::new(ShutdownService::default());

        let test = TestService::new();
        shutdown_service.clone().register_service(test);

        let not_dropped_services = shutdown_service.graceful_shutdown().await;
        assert_eq!(not_dropped_services.len(), 0);
    }

    #[actix_rt::test]
    pub async fn failed() {
        init_logger();

        const REF_TEST_SERVICE: &str = "RefTestService";
        pub struct RefTestService(Mutex<Option<Arc<RefTestService>>>);

        impl RefTestService {
            pub fn new() -> Arc<Self> {
                Arc::new(Self(Mutex::new(None)))
            }

            pub fn set_ref(&self, service: Arc<RefTestService>) {
                *self.0.lock() = Some(service);
            }
        }

        impl Service for RefTestService {
            fn name(&self) -> &str {
                REF_TEST_SERVICE
            }

            fn graceful_shutdown(self: Arc<Self>) -> Option<Receiver<Result<()>>> {
                None
            }
        }

        let shutdown_service = Arc::new(ShutdownService::default());

        let test = RefTestService::new();
        let clone = test.clone();
        test.set_ref(clone);
        shutdown_service.clone().register_service(test);

        let not_dropped_services = shutdown_service.graceful_shutdown().await;
        assert_eq!(not_dropped_services, vec![REF_TEST_SERVICE.to_string()]);
    }
}
