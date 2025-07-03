use slv_rust::networking::transport::UdpTransport;

#[tokio::main]
async fn main() {
    let transport = UdpTransport::new("127.0.0.1:0").await;
    match transport {
        Ok(_) => println!("UDP transport initialized successfully."),
        Err(e) => eprintln!("Failed to initialize UDP transport: {}", e),
    }
}
