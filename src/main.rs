use futures::Stream;
use futures_util::StreamExt;
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
pub struct OrganismService {
    source_name: String,
}

#[tonic::async_trait]
impl Organism for OrganismService {
    type FoodFlowStream = Pin<Box<dyn Stream<Item = Result<Food, Status>> + Send + Sync + 'static>>;

    async fn food_flow(
        &self,
        request: Request<tonic::Streaming<Food>>,
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

        let source = self.source_name.clone();

        tokio::spawn(async move {
            let interval_seconds: u64 = 2;
            let interval_duration = time::Duration::from_secs(interval_seconds);
            let mut interval = time::interval(interval_duration);
            let mut food_id: usize = 0;

            loop {
                interval.tick().await;
                food_id += 1;
                let food = Food {
                    id: food_id as i32,
                    source: source.clone(),
                    kind: 1,
                    amount: 1,
                };
                tracing::info!("food = {:?}", food);
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args: Vec<String> = env::args().collect();
    let organism_service = OrganismService {
        source_name: args[1].clone(),
    };
    let food_source = organism_service.source_name.clone();

    let peer_addr = args[2].clone();
    tokio::spawn(async move {
        let mut retries: usize = 10;
        let mut client = None;
        loop {
            let uri = format!("http://{}", peer_addr);
            match OrganismClient::connect(uri).await {
                Ok(c) => {
                    client = Some(c);
                    break;
                }
                Err(e) => {
                    tracing::error!("{}) unable to connect: {:?}", retries, e);
                    if retries == 0 {
                        break;
                    };
                    time::sleep(time::Duration::from_secs(10)).await;
                    retries -= 1;
                }
            }
        }
        if client.is_none() {
            panic!("unable to connect");
        }
        let mut client = client.unwrap();

        let outbound = async_stream::stream! {
            let interval_seconds: u64 = 2;
            let interval_duration = time::Duration::from_secs(interval_seconds);
            let mut interval = time::interval(interval_duration);
            let mut food_id: usize = 0;

            loop {
                interval.tick().await;
                food_id += 1;
                let food = Food {
                    id: food_id as i32,
                    source: food_source.clone(),
                    kind: 1,
                    amount: 1,
                };
                tracing::info!("food = {:?}", food);
                yield food;
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
    });

    let server_addr = organism_service.source_name.clone().parse().unwrap();
    let svc = OrganismServer::new(organism_service);

    tracing::info!(message = "Starting server.", %server_addr);
    Server::builder()
        .trace_fn(|_| tracing::info_span!("ecosystem_server"))
        .add_service(svc)
        .serve(server_addr)
        .await?;

    Ok(())
}
