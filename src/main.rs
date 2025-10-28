use crate::connection::KafkaProducerConfig;
use crate::materialization::StateDiffLog;
use kafka::producer::{Producer, RequiredAcks};
use log::error;
use std::env;
use std::fs::{File, OpenOptions};
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

    let mut kafka_producer: KafkaProducerConfig = None;
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

            kafka_producer = Some((kafka_topic, Arc::new(Mutex::new(producer))));
        }
    }

    match listener {
        Err(_) => println!("Error when binding to socket!"),

        Ok(listener) => {
            for client in listener.incoming() {
                match client {
                    Err(_) => println!("Error establishing connection!"),

                    Ok(client_stream) => {
                        let kafka_producer = kafka_producer.clone();
                        let state_difference_map = Arc::clone(&state_difference_map);
                        std::thread::spawn(move || {
                            connection_handler::initiate(
                                client_stream,
                                state_difference_map,
                                kafka_producer,
                            )
                        });
                    }
                }
            }
        }
    }
}
