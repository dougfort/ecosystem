use anyhow::{anyhow, Result};
use clap::{App, Arg};
use futures::Stream;
use futures_util::StreamExt;
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

pub mod observer {
    tonic::include_proto!("observer");
}
use observer::event_observer_client::EventObserverClient;

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

        let inbound_state_ref = self.state.clone();
        tokio::spawn(async move {
            let mut stream = request.into_inner();
            while let Some(food) = stream.next().await {
                let food = food.unwrap();
                tracing::debug!("server inbound food = {:?}", food);
                {
                    let mut state = inbound_state_ref.lock().unwrap();
                    state.consume_food(&food);
                }
            }
        });

        let (tx, rx) = mpsc::channel(1);

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
                tracing::debug!("server outbound food = {:?}", food);
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

    let matches = App::new("organism")
        .version("0.0.1")
        .arg(
            Arg::with_name("index")
                .long("index")
                .help("index of this organism")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("name")
                .short("n")
                .long("name")
                .help("name of this organism")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("incoming")
                .short("i")
                .long("incoming")
                .help("incoming food flow")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("outgoing")
                .short("o")
                .long("outgoing")
                .help("outgoing food flow")
                .takes_value(true),
        )
        .get_matches();

    let settings = get_configuration()?;

    let server_name = matches.value_of("name").unwrap().to_string();

    let server_index: usize = matches.value_of("index").unwrap().parse()?;
    let server_addr = format!(
        "{}:{}",
        settings.application.addr_host,
        settings.application.addr_base_port + server_index
    );

    // create a channel for events
    let (event_tx, event_rx): (
        tokio::sync::mpsc::Sender<observer::Event>,
        tokio::sync::mpsc::Receiver<observer::Event>,
    ) = mpsc::channel(32);

    let outgoing_food = matches.value_of("outgoing").unwrap();
    let incoming_food = matches.value_of("incoming").unwrap();
    let organism_service = OrganismService {
        state: Arc::new(Mutex::new(State::new(&outgoing_food, &incoming_food))),
    };

    for peer_index in 1..settings.ecosystem.population_size + 1 {
        if peer_index != server_index {
            let state_ref = Arc::clone(&organism_service.state);
            let peer_addr = format!(
                "{}:{}",
                settings.application.addr_host,
                settings.application.addr_base_port + peer_index
            );
            let tx = event_tx.clone();
            let server_name = server_name.clone();
            tokio::spawn(
                async move { connect_to_peer(server_name, state_ref, &peer_addr, tx).await },
            );
        }
    }

    let observer_addr = format!("{}:{}", settings.observer.host, settings.observer.port,);

    tokio::spawn(async move { connect_to_observer(&observer_addr, event_rx).await });

    let svc = OrganismServer::new(organism_service);

    tracing::info!(
        "Starting server {}; outgoing food = {}; incoming food = {}; address = {}",
        server_name,
        outgoing_food,
        incoming_food,
        server_addr
    );
    Server::builder()
        .trace_fn(|_| tracing::info_span!("ecosystem_server"))
        .add_service(svc)
        .serve(server_addr.parse().unwrap())
        .await?;

    Ok(())
}

async fn connect_to_observer(
    observer_addr: &str,
    mut event_rx: tokio::sync::mpsc::Receiver<observer::Event>,
) {
    let mut client = connect_client_to_observer(&observer_addr)
        .await
        .expect("unable to connect to observer");

    let outbound = async_stream::stream! {

        let mut event_id = 1;
        while let Some(message) = event_rx.recv().await {
            event_id += 1;
            let mut event = message;
            event.id = event_id;
            tracing::info!("client outbound event = {:?}", event);
            yield event;
        }
    };

    let response = client
        .events(Request::new(outbound))
        .await
        .expect("events failed");
    let inbound = response.into_inner();

    let stream_id = inbound.stream_id;
    tracing::info!("event stream_id: {:?}", stream_id);
}

async fn connect_client_to_observer(
    addr: &str,
) -> Result<EventObserverClient<tonic::transport::Channel>, anyhow::Error> {
    let mut retries: usize = 10;
    loop {
        let uri = format!("http://{}", addr);
        match EventObserverClient::connect(uri).await {
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

async fn connect_to_peer(
    server_name: String,
    state_ref: Arc<Mutex<State>>,
    peer_addr: &str,
    event_tx: tokio::sync::mpsc::Sender<observer::Event>,
) {
    let mut client = connect_client_to_peer(&peer_addr)
        .await
        .expect("unable to connect");

    let outbound_server_name = server_name.clone();
    let outbound_state_ref = state_ref.clone();
    let mut event_id: i32 = 0;
    let tx = event_tx.clone();
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
            tracing::debug!("client outbound food = {:?}", food);
            event_id += 1;
            tx.send(observer::Event{
                source: outbound_server_name.clone(),
                id: event_id,
                event_type: 1,
                food_kind: food.kind.clone(),
                food_amount: food.amount,
            })
            .await
            .expect("event_tx.send failed");
            yield food;
        }
    };

    let response = client
        .food_flow(Request::new(outbound))
        .await
        .expect("food_flow failed");
    let mut inbound = response.into_inner();

    let inbound_server_name = server_name.clone();
    let inbound_state_ref = state_ref.clone();
    let tx = event_tx.clone();
    while let Some(food) = inbound.message().await.expect("inbound.message() failed") {
        tracing::debug!("client inbound food = {:?}", food);
        event_id += 1;
        tx.send(observer::Event {
            source: inbound_server_name.clone(),
            id: event_id,
            event_type: 2,
            food_kind: food.kind.clone(),
            food_amount: food.amount,
        })
        .await
        .expect("event_tx.send failed");
        {
            let mut state = inbound_state_ref.lock().unwrap();
            state.consume_food(&food);
        }
    }
}

async fn connect_client_to_peer(
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
