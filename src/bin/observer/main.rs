use anyhow::Result;
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
    async fn events(
        &self,
        request: Request<tonic::Streaming<observer::Event>>,
    ) -> Result<Response<observer::StreamId>, Status> {
        tracing::info!("request = {:?}", request);

        let mut inbound = request.into_inner();
        while let Some(event) = inbound.message().await.expect("inbound.message() failed") {
            tracing::info!("client inbound event = {:?}", event);
        }

        Ok(Response::new(observer::StreamId { stream_id: 42 }))
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
