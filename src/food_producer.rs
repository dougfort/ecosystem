use futures::Stream;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio::time;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

pub mod ecosystem {
    tonic::include_proto!("ecosystem");
}
use ecosystem::food_source_server::{FoodSource, FoodSourceServer};
use ecosystem::{Food, FoodRequest};

#[derive(Debug)]
pub struct FoodSourceService;

#[tonic::async_trait]
impl FoodSource for FoodSourceService {
    type GetFoodStream = Pin<Box<dyn Stream<Item = Result<Food, Status>> + Send + Sync + 'static>>;

    async fn get_food(
        &self,
        request: Request<FoodRequest>,
    ) -> Result<Response<Self::GetFoodStream>, Status> {
        println!("GetFood = {:?}", request);

        let (tx, rx) = mpsc::channel(1);

        tokio::spawn(async move {
            let interval_seconds: u64 = 2;
            let interval_duration = time::Duration::from_secs(interval_seconds);
            let mut interval = time::interval(interval_duration);
            let mut food_id: usize = 0;

            loop {
                interval.tick().await;
                food_id += 1;
                println!("Food Id = {}", food_id);
                if let Err(e) = tx.send(Ok(Food { id: food_id as i32 })).await {
                    println!("tx.send failed: {}", e);
                    break;
                }
            }
        });

        Ok(Response::new(Box::pin(
            tokio_stream::wrappers::ReceiverStream::new(rx),
        )))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:10000".parse().unwrap();
    let food_service = FoodSourceService {};
    let svc = FoodSourceServer::new(food_service);

    Server::builder().add_service(svc).serve(addr).await?;

    Ok(())
}
