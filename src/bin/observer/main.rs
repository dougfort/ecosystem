use anyhow::Result;
use futures::Stream;
use std::pin::Pin;
use tokio::sync::mpsc;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use configuration::get_configuration;

pub mod observer {
    tonic::include_proto!("observer");
}
use observer::event_observer_server::{EventObserver, EventObserverServer};

#[derive(Debug)]
pub struct EventObserverService {}

#[tonic::async_trait]
impl EventObserver for EventObserverService {
    type EventsStream =
        Pin<Box<dyn Stream<Item = Result<observer::Event, Status>> + Send + Sync + 'static>>;

    async fn events(
        &self,
        request: Request<observer::Join>,
    ) -> Result<Response<Self::EventsStream>, Status> {
        tracing::info!("request = {:?}", request);

        let (_tx, rx) = mpsc::channel(1);

        Ok(Response::new(Box::pin(
            tokio_stream::wrappers::ReceiverStream::new(rx),
        )))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let settings = get_configuration()?;
    let server_addr = format!("{}:{}", settings.observer.host, settings.observer.port);

    let observer_service = EventObserverService {};
    let svc = EventObserverServer::new(observer_service);

    tracing::info!("Starting server: {:?}", server_addr);
    Server::builder()
        .trace_fn(|_| tracing::info_span!("observer_server"))
        .add_service(svc)
        .serve(server_addr.parse().unwrap())
        .await?;

    Ok(())
}
