use slv_rust::networking::transport::UdpTransport;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let server_addr = "127.0.0.1:12345";
    let client_addr = "127.0.0.1:12346";

    let server = UdpTransport::new(server_addr).await.unwrap();
    let client = UdpTransport::new(client_addr).await.unwrap();

    let msg = b"Hello, world!";
    client.send(msg, &server_addr.parse().unwrap()).await.unwrap();

    let mut buf = [0; 1024];
    let (len, addr) = server.recv(&mut buf).await.unwrap();

    println!("Received {} bytes from {}: {}", len, addr, String::from_utf8_lossy(&buf[..len]));

    server.send(&buf[..len], &addr).await.unwrap();

    let (len, _addr) = client.recv(&mut buf).await.unwrap();
    println!("Received echo: {}", String::from_utf8_lossy(&buf[..len]));
}