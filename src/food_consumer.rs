use tonic::Request;

pub mod ecosystem {
    tonic::include_proto!("ecosystem");
}
use ecosystem::food_source_client::FoodSourceClient;
use ecosystem::FoodRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = FoodSourceClient::connect("http://[::1]:10000").await?;
    let request_id = 1;
    let food_request = FoodRequest { id: request_id };

    let mut stream = client
        .get_food(Request::new(food_request))
        .await?
        .into_inner();

    while let Some(food) = stream.message().await? {
        println!("food = {:?}", food);
    }

    Ok(())
}
