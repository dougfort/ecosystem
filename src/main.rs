use anyhow::{anyhow, Result};
use futures::Stream;
use futures_util::StreamExt;
use std::env;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio::time;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

pub mod ecosystem {
    tonic::include_proto!("ecosystem");
}
use ecosystem::organism_client::OrganismClient;
use ecosystem::organism_server::{Organism, OrganismServer};

pub mod configuration;
use configuration::get_configuration;

pub mod state;
use state::State;

#[derive(Debug)]
pub struct OrganismService {
    state: Arc<Mutex<State>>,
}

#[tonic::async_trait]
impl Organism for OrganismService {
    type FoodFlowStream =
        Pin<Box<dyn Stream<Item = Result<ecosystem::Food, Status>> + Send + Sync + 'static>>;

    async fn food_flow(
        &self,
        request: Request<tonic::Streaming<ecosystem::Food>>,
    ) -> Result<Response<Self::FoodFlowStream>, Status> {
        tracing::info!("food_flow = {:?}", request);

        let (tx, rx) = mpsc::channel(1);

        let inbound_state_ref = self.state.clone();
        tokio::spawn(async move {
            let mut stream = request.into_inner();
            while let Some(food) = stream.next().await {
                let food = food.unwrap();
                tracing::info!("server inbound food = {:?}", food);
                {
                    let mut state = inbound_state_ref.lock().unwrap();
                    state.consume_food(&food);
                }
            }
        });

        let outbound_state_ref = self.state.clone();
        tokio::spawn(async move {
            let interval_seconds: u64 = 2;
            let interval_duration = time::Duration::from_secs(interval_seconds);
            let mut interval = time::interval(interval_duration);

            loop {
                interval.tick().await;
                let food = {
                    let mut state = outbound_state_ref.lock().unwrap();
                    state.produce_food()
                };
                tracing::info!("server outbound food = {:?}", food);
                if let Err(e) = tx.send(Ok(food)).await {
                    tracing::info!("tx.send failed: {}", e);
                    break;
                }
            }

            tracing::info!("exit interval loop")
        });

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

    let args: Vec<String> = env::args().collect();
    let server_index: usize = args[1].parse().expect("invalid argument");

    let settings = get_configuration()?;

    let server_name = format!("organism{}", server_index);
    let server_kind = server_index;
    let server_addr = format!(
        "{}:{}",
        settings.application.addr_host,
        settings.application.addr_base_port + server_index
    );

    let organism_service = OrganismService {
        state: Arc::new(Mutex::new(State::new(&server_name, server_kind))),
    };

    for peer_index in 1..settings.ecosystem.population_size + 1 {
        if peer_index != server_index {
            let state_ref = Arc::clone(&organism_service.state);
            let peer_addr = format!(
                "{}:{}",
                settings.application.addr_host,
                settings.application.addr_base_port + peer_index
            );
            tokio::spawn(async move { connect_to_peer(state_ref, &peer_addr).await });
        }
    }

    let svc = OrganismServer::new(organism_service);

    tracing::info!(
        "Starting server {} food kind = {} address = {}",
        server_name,
        server_kind,
        server_addr
    );
    Server::builder()
        .trace_fn(|_| tracing::info_span!("ecosystem_server"))
        .add_service(svc)
        .serve(server_addr.parse().unwrap())
        .await?;

    Ok(())
}

async fn connect_to_peer(state_ref: Arc<Mutex<State>>, peer_addr: &str) {
    let mut client = connect_client(&peer_addr).await.expect("unable to connect");

    let outbound_state_ref = state_ref.clone();
    let outbound = async_stream::stream! {
        let interval_seconds: u64 = 2;
        let interval_duration = time::Duration::from_secs(interval_seconds);
        let mut interval = time::interval(interval_duration);

        loop {
            interval.tick().await;
            let food = {
                let mut state = outbound_state_ref.lock().unwrap();
                state.produce_food()
            };
            tracing::info!("client outbound food = {:?}", food);
            yield food;
        }
    };

    let response = client
        .food_flow(Request::new(outbound))
        .await
        .expect("food_flow failed");
    let mut inbound = response.into_inner();

    let inbound_state_ref = state_ref.clone();
    while let Some(food) = inbound.message().await.expect("inbound.message() failed") {
        tracing::info!("client inbound food = {:?}", food);
        {
            let mut state = inbound_state_ref.lock().unwrap();
            state.consume_food(&food);
        }
    }
}

async fn connect_client(
    addr: &str,
) -> Result<OrganismClient<tonic::transport::Channel>, anyhow::Error> {
    let mut retries: usize = 10;
    loop {
        let uri = format!("http://{}", addr);
        match OrganismClient::connect(uri).await {
            Ok(c) => {
                return Ok(c);
            }
            Err(e) => {
                tracing::error!("{}) unable to connect: {:?}", retries, e);
                if retries == 0 {
                    return Err(anyhow!("unable to connect {}", e));
                };
                time::sleep(time::Duration::from_secs(10)).await;
                retries -= 1;
            }
        }
    }
}
