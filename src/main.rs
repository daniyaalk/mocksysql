#[cfg(feature = "replay")]
use crate::connection::{KafkaProducerConfig, ReplayLogEntry};
#[cfg(feature = "replay")]
use crate::materialization::ReplayLog;
use crate::materialization::StateDiffLog;
#[cfg(feature = "replay")]
use crate::util::cache::get_cache_ttl;
#[cfg(feature = "replay")]
use kafka::consumer::Consumer;
#[cfg(feature = "replay")]
use kafka::producer::{Producer, RequiredAcks};
#[cfg(feature = "replay")]
use log::debug;
#[cfg(feature = "replay")]
use log::error;
use std::env;
use std::net::TcpListener;
use std::sync::Arc;
#[cfg(feature = "replay")]
use std::sync::Mutex;
#[cfg(feature = "replay")]
use std::time::Duration;
#[cfg(feature = "replay")]
use ttl_cache::TtlCache;

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

    #[cfg(feature = "replay")]
    let kafka_producer: KafkaProducerConfig = prepare_kafka_producer_config();

    #[cfg(feature = "replay")]
    let replay_map = prepare_and_run_kafka_consumer();

    match listener {
        Err(_) => println!("Error when binding to socket!"),

        Ok(listener) => {
            for client in listener.incoming() {
                match client {
                    Err(_) => println!("Error establishing connection!"),

                    Ok(client_stream) => {
                        #[cfg(feature = "replay")]
                        let kafka_producer = kafka_producer.clone();
                        #[cfg(feature = "replay")]
                        let replay_map = replay_map.clone();
                        let state_difference_map = Arc::clone(&state_difference_map);
                        std::thread::spawn(move || {
                            connection_handler::initiate(
                                client_stream,
                                state_difference_map,
                                #[cfg(feature = "replay")]
                                kafka_producer,
                                #[cfg(feature = "replay")]
                                replay_map,
                            )
                        });
                    }
                }
            }
        }
    }
}

#[cfg(feature = "replay")]
fn prepare_and_run_kafka_consumer() -> ReplayLog {
    if let Ok(value) = env::var("kafka_replay_response_enable") {
        if value == "true" {
            let kafka_host = env::var("KAFKA_HOST").expect("KAFKA_HOST is not set");
            let kafka_topic = env::var("KAFKA_TOPIC").expect("KAFKA_TOPIC is not set");

            let consumer =
                Consumer::from_hosts(kafka_host.split(",").map(|s| s.to_string()).collect())
                    .with_topic(kafka_topic.clone())
                    .with_fetch_max_bytes_per_partition(16 * 1024 * 1024) // 16MB
                    .create()
                    .unwrap();
            let map = Arc::new(Mutex::new(TtlCache::new(usize::MAX)));
            #[cfg(feature = "replay")]
            spawn_kafka_read(consumer, map.clone());
            return Some(map.clone());
        }
    }
    None
}

#[cfg(feature = "replay")]
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

#[cfg(feature = "replay")]
fn spawn_kafka_read(mut consumer: Consumer, replay_map: Arc<Mutex<TtlCache<String, String>>>) {
    std::thread::spawn(move || loop {
        match consumer.poll() {
            Ok(poll) => {
                for ms in poll.iter() {
                    for m in ms.messages() {
                        let message_string = String::from_utf8(m.value.to_vec()).unwrap();

                        match serde_json::from_str::<ReplayLogEntry>(&*message_string) {
                            Ok(entry) => {
                                let mut map = replay_map.lock().unwrap();
                                debug!("{:?}", entry);
                                map.insert(entry.last_command, entry.output, get_cache_ttl());
                            }
                            Err(e) => println!("Error deserializing replay log entry, {}", e),
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error receiving messages, {:?}", e);
            }
        }
    });
}
