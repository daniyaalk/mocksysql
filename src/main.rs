use crate::connection::{KafkaProducerConfig, ReplayLogEntry};
use crate::materialization::{ReplayLog, StateDiffLog};
use dashmap::DashMap;
use kafka::consumer::Consumer;
use kafka::producer::{Producer, RequiredAcks};
use log::debug;
use std::env;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod connection;
mod connection_handler;
mod mysql;
mod state_handler;

mod materialization;

#[cfg(feature = "tls")]
mod tls;
mod util;

fn main() {
    let bind_address = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:6033".to_owned());

    let listener = TcpListener::bind(bind_address);

    let state_difference_map = StateDiffLog::default();

    env_logger::init();

    let kafka_producer: KafkaProducerConfig = prepare_kafka_producer_config();

    let replay_map = prepare_and_run_kafka_consumer();

    match listener {
        Err(_) => println!("Error when binding to socket!"),

        Ok(listener) => {
            for client in listener.incoming() {
                match client {
                    Err(_) => println!("Error establishing connection!"),

                    Ok(client_stream) => {
                        let kafka_producer = kafka_producer.clone();
                        let replay_map = replay_map.clone();
                        let state_difference_map = Arc::clone(&state_difference_map);
                        std::thread::spawn(move || {
                            connection_handler::initiate(
                                client_stream,
                                state_difference_map,
                                kafka_producer,
                                replay_map,
                            )
                        });
                    }
                }
            }
        }
    }
}

fn prepare_and_run_kafka_consumer() -> ReplayLog {
    if let Ok(value) = env::var("kafka_replay_response_enable") {
        if value == "true" {
            let kafka_host = env::var("KAFKA_HOST").expect("KAFKA_HOST is not set");
            let kafka_topic = env::var("KAFKA_TOPIC").expect("KAFKA_TOPIC is not set");

            let consumer =
                Consumer::from_hosts(kafka_host.split(",").map(|s| s.to_string()).collect())
                    .with_topic(kafka_topic.clone())
                    .create()
                    .unwrap();

            let map = Arc::new(Mutex::new(DashMap::new()));
            spawn_kafka_read(consumer, map.clone());
            return Some(map.clone());
        }
    }
    None
}

fn prepare_kafka_producer_config() -> KafkaProducerConfig {
    if let Ok(value) = env::var("kafka_replay_log_enable") {
        if value == "true" {
            let kafka_host = env::var("KAFKA_HOST").expect("KAFKA_HOST is not set");
            let kafka_topic = env::var("KAFKA_TOPIC").expect("KAFKA_TOPIC is not set");

            let producer =
                Producer::from_hosts(kafka_host.split(",").map(|s| s.to_string()).collect())
                    .with_ack_timeout(Duration::from_secs(1))
                    .with_required_acks(RequiredAcks::One)
                    .create()
                    .unwrap();

            return Some((kafka_topic, Arc::new(Mutex::new(producer))));
        }
    }
    None
}

fn spawn_kafka_read(mut consumer: Consumer, replay_map: Arc<Mutex<DashMap<String, String>>>) {
    std::thread::spawn(move || loop {
        for ms in consumer.poll().unwrap().iter() {
            for m in ms.messages() {
                let message_string = String::from_utf8(m.value.to_vec()).unwrap();

                match serde_json::from_str::<ReplayLogEntry>(&*message_string) {
                    Ok(entry) => {
                        let map = replay_map.lock().unwrap();
                        debug!("{:?}", entry);
                        map.insert(entry.last_command, entry.output);
                    }
                    Err(e) => println!("Error deserializing replay log entry, {}", e),
                }
            }
        }
    });
}
