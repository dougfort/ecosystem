use crate::ecosystem::Food;
use anyhow::{anyhow, Result};
use clap::{App, Arg};
use futures_util::StreamExt;
use tokio::sync::{mpsc, oneshot};
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

#[derive(Debug)]
enum StateCommand {
    Produce {
        responder_tx: oneshot::Sender<Option<Food>>,
    },
    Consume {
        food: Food,
    },
}

#[derive(Debug)]
pub struct OrganismService {
    state_tx: mpsc::Sender<StateCommand>,
}

#[tonic::async_trait]
impl Organism for OrganismService {
    async fn food_flow(
        &self,
        request: Request<tonic::Streaming<ecosystem::Food>>,
    ) -> Result<Response<ecosystem::StreamId>, Status> {
        tracing::info!("food_flow = {:?}", request);
        let state_tx = self.state_tx.clone();

        tokio::spawn(async move {
            let mut stream = request.into_inner();
            while let Some(food_next) = stream.next().await {
                match food_next {
                    Ok(food) => {
                        tracing::debug!("server inbound food = {:?}", food);
                        state_tx.send(StateCommand::Consume { food }).await.unwrap();
                    }
                    Err(error) => {
                        tracing::error!("inbound stream error = {:?}", error);
                        break;
                    }
                }
            }
        });

        Ok(Response::new(ecosystem::StreamId { id: 3 }))
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
        mpsc::Sender<observer::Event>,
        mpsc::Receiver<observer::Event>,
    ) = mpsc::channel(32);

    let (state_tx, mut state_rx): (mpsc::Sender<StateCommand>, mpsc::Receiver<StateCommand>) =
        mpsc::channel(32);

    let outgoing_food = matches.value_of("outgoing").unwrap();
    let incoming_food = matches.value_of("incoming").unwrap();

    let organism_service = OrganismService { state_tx };

    let food_kind = outgoing_food.to_string();
    let state_handle = tokio::spawn(async move {
        while let Some(cmd) = state_rx.recv().await {
            match cmd {
                StateCommand::Produce { responder_tx } => {
                    let kind = food_kind.clone();
                    let amount = 1;
                    let food = Food { kind, amount };
                    tracing::info!("StateCommand::Produce {:?}", food);
                    responder_tx.send(Some(food)).unwrap();
                }
                StateCommand::Consume { food } => {
                    tracing::info!("StateCommand::Consume {:?}", food);
                }
            }
        }
    });

    for peer_index in 1..settings.ecosystem.population_size + 1 {
        tracing::info!("peer_index = {}, server_index = {}", peer_index, server_index);
        if peer_index != server_index {
            let peer_addr = format!(
                "{}:{}",
                settings.application.addr_host,
                settings.application.addr_base_port + peer_index
            );
            let event_tx = event_tx.clone();
            let state_tx = organism_service.state_tx.clone();
            let server_name = server_name.clone();
            tracing::info!("connect_to_peer {} {}", peer_index, peer_addr);
            tokio::spawn(async move {
                connect_to_peer(server_name, state_tx, &peer_addr, event_tx).await
            });
        }
    }

    let observer_addr = format!("{}:{}", settings.observer.host, settings.observer.port,);

    connect_to_observer(&observer_addr, event_rx).await?;

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

    state_handle.await.expect("state_handle.await");

    Ok(())
}

async fn connect_to_observer(
    observer_addr: &str,
    mut event_rx: tokio::sync::mpsc::Receiver<observer::Event>,
) -> Result<()> {
    let mut client = connect_client_to_observer(observer_addr).await?;

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

    let response = client.events(Request::new(outbound)).await?;
    let inbound = response.into_inner();

    let stream_id = inbound.stream_id;
    tracing::info!("event stream_id: {:?}", stream_id);

    Ok(())
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
                tracing::warn!(
                    "client unable to connect to observer, retries left = {}: {}",
                    retries,
                    e
                );
                if retries == 0 {
                    return Err(anyhow!("client unable to connect to observer {}", e));
                };
                time::sleep(time::Duration::from_secs(10)).await;
                retries -= 1;
            }
        }
    }
}

async fn connect_to_peer(
    server_name: String,
    state_tx: mpsc::Sender<StateCommand>,
    peer_addr: &str,
    event_tx: mpsc::Sender<observer::Event>,
) {
    let mut client = connect_client_to_peer(peer_addr)
        .await
        .expect("unable to connect");

    let outbound_server_name = server_name.clone();
    let mut event_id: i32 = 0;
    let tx = event_tx.clone();
    let outbound = async_stream::stream! {
        let interval_seconds: u64 = 2;
        let interval_duration = time::Duration::from_secs(interval_seconds);
        let mut interval = time::interval(interval_duration);

        loop {
            interval.tick().await;
            let food = {
                let (responder_tx, responder_rx) = oneshot::channel();
                state_tx.send(StateCommand::Produce{responder_tx}).await.unwrap();
                match responder_rx.await {
                    Ok(food) => food,
                    Err(_) => None,
                }
            };
            if let Some(f) = food {
                tracing::debug!("client outbound food = {:?}", f);
                event_id += 1;
                tx.send(observer::Event{
                    source: outbound_server_name.clone(),
                    id: event_id,
                    event_type: 1,
                    food_kind: f.kind.clone(),
                    food_amount: f.amount,
                })
                .await
                .expect("event_tx.send failed");
                yield f;
            }
        }
    };

    let response = client
        .food_flow(Request::new(outbound))
        .await
        .expect("food_flow failed");
    let inbound = response.into_inner();

    let stream_id = inbound.id;
    tracing::debug!("client stream_id {}", stream_id);
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
                tracing::warn!(
                    "unable to connect client to peer, retries left = {} {}",
                    retries,
                    e
                );
                if retries == 0 {
                    return Err(anyhow!("unable to connect client to peer {}", e));
                };
                time::sleep(time::Duration::from_secs(10)).await;
                retries -= 1;
            }
        }
    }
}
