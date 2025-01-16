use std::net::TcpListener;
mod connection_handler;
mod connection;
mod state_handler;
mod mysql;
// crate::connection;


fn main() {

    let bind_address: &str = "127.0.0.1:6033";


    let listener = TcpListener::bind(bind_address);

    match listener{
        Err(_) => println!("Error when binding to socket!"),

        Ok(listener) => {
            for client in listener.incoming() {

                match client {
                    Err(_) => println!("Error establishing connection!"),
                
                    Ok(client_stream) => {
                        std::thread::spawn(move || connection_handler::initiate(client_stream));
                    }
                }

            }
        }
    }

    

}