use futures::Stream;
use std::env;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio::time;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

pub mod ecosystem {
    tonic::include_proto!("ecosystem");
}
use ecosystem::organism_client::OrganismClient;
use ecosystem::organism_server::{Organism, OrganismServer};
use ecosystem::Food;

#[derive(Debug)]
pub struct OrganismService;

#[tonic::async_trait]
impl Organism for OrganismService {
    type FoodFlowStream = Pin<Box<dyn Stream<Item = Result<Food, Status>> + Send + Sync + 'static>>;

    async fn food_flow(
        &self,
        _request: Request<tonic::Streaming<Food>>,
    ) -> Result<Response<Self::FoodFlowStream>, Status> {
        unimplemented!()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // "[::1]:10000"
    let args: Vec<String> = env::args().collect();
    let our_addr = &args[1].parse().unwrap();
    // let peer_addr = &args[2].parse().unwrap();

    let organism_service = OrganismService;
    let svc = OrganismServer::new(organism_service);

    tracing::info!(message = "Starting server.", %our_addr);
    Server::builder()
        .trace_fn(|_| tracing::info_span!("ecosystem_server"))
        .add_service(svc)
        .serve(*our_addr)
        .await?;

    Ok(())
}
