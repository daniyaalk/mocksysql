use crate::materialization::StateDiffLog;
use std::env;
use std::net::TcpListener;
use std::sync::Arc;

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

    match listener {
        Err(_) => println!("Error when binding to socket!"),

        Ok(listener) => {
            for client in listener.incoming() {
                match client {
                    Err(_) => println!("Error establishing connection!"),

                    Ok(client_stream) => {
                        let state_difference_map = Arc::clone(&state_difference_map);

                        std::thread::spawn(move || {
                            connection_handler::initiate(client_stream, state_difference_map)
                        });
                    }
                }
            }
        }
    }
}
