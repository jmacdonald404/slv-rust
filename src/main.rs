use slv_rust::networking::circuit::Circuit;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let server_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let circuit = Circuit::new(server_addr).await;

    match circuit {
        Ok(_) => println!("Circuit created successfully."),
        Err(e) => eprintln!("Failed to create circuit: {}", e),
    }
}
