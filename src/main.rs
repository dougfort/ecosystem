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

        tokio::spawn(async move {
            let mut stream = request.into_inner();
            while let Some(food) = stream.next().await {
                let food = food.unwrap();
                tracing::info!("incoming food = {:?}", food);
            }
        });

        let state_ref = Arc::clone(&self.state);
        tokio::spawn(async move {
            let interval_seconds: u64 = 2;
            let interval_duration = time::Duration::from_secs(interval_seconds);
            let mut interval = time::interval(interval_duration);

            loop {
                interval.tick().await;
                let food = {
                    let mut state = state_ref.lock().unwrap();
                    state.create_food()
                };
                if let Some(food) = food {
                    tracing::info!("food = {:?}", food);
                    if let Err(e) = tx.send(Ok(food)).await {
                        tracing::info!("tx.send failed: {}", e);
                        break;
                    }
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args: Vec<String> = env::args().collect();

    let server_name = args[1].clone();
    let organism_service = OrganismService {
        state: Arc::new(Mutex::new(State::new(&server_name))),
    };

    let server_addr = args[2].clone();
    for arg in args.iter().skip(3) {
        let state_ref = Arc::clone(&organism_service.state);
        let peer_addr = arg.clone();
        tokio::spawn(async move { connect_to_peer(state_ref, &peer_addr).await });
    }

    let svc = OrganismServer::new(organism_service);

    tracing::info!("Starting server {} {}", "food", server_addr);
    Server::builder()
        .trace_fn(|_| tracing::info_span!("ecosystem_server"))
        .add_service(svc)
        .serve(server_addr.parse().unwrap())
        .await?;

    Ok(())
}

async fn connect_to_peer(state_ref: Arc<Mutex<State>>, peer_addr: &str) {
    let mut client = connect_client(&peer_addr).await.expect("unable to connect");

    let outbound = async_stream::stream! {
        let interval_seconds: u64 = 2;
        let interval_duration = time::Duration::from_secs(interval_seconds);
        let mut interval = time::interval(interval_duration);
        let mut food_id: usize = 0;

        loop {
            interval.tick().await;
            let food = {
                let mut state = state_ref.lock().unwrap();
                state.create_food()
            };
            tracing::info!("food = {:?}", food);
            if let Some(food) = food {
                yield food;
            }
        }
    };

    let response = client
        .food_flow(Request::new(outbound))
        .await
        .expect("food_flow failed");
    let mut inbound = response.into_inner();

    while let Some(food) = inbound.message().await.expect("nbound.message() failed") {
        println!("food = {:?}", food);
    }
}

async fn connect_client(
    addr: &str,
) -> Result<OrganismClient<tonic::transport::Channel>, tonic::transport::Error> {
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
                    return Err(e);
                };
                time::sleep(time::Duration::from_secs(10)).await;
                retries -= 1;
            }
        }
    }
}
