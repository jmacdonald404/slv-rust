use slv_rust::networking::circuit::Circuit;
use slv_rust::networking::protocol::messages::Message;
use std::net::SocketAddr;
use tokio::time::{self, Duration};

#[tokio::main]
async fn main() {
    let server_listen_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let client_listen_addr: &str = "0.0.0.0:0"; // Bind to ephemeral port

    // Server side
    let mut server_circuit = Circuit::new(&server_listen_addr.to_string(), "0.0.0.0:0".parse().unwrap()).await.unwrap();
    let server_handle = tokio::spawn(async move {
        println!("Server listening on {}", server_listen_addr);
        let (received_header, received_message, sender_addr) = server_circuit.recv_message().await.unwrap();
        println!("Server received: {:?}, {:?} from {}", received_header, received_message, sender_addr);
        // Echo back the message to the sender
        server_circuit.send_message(&received_message, &sender_addr).await.unwrap();
        println!("Server echoed message back to {}.", sender_addr);
    });

    // Give the server a moment to start listening
    time::sleep(Duration::from_millis(100)).await;

    // Client side
    let mut client_circuit = Circuit::new(client_listen_addr, server_listen_addr).await.unwrap();

    let message = Message::KeepAlive;

    client_circuit.send_message(&message, &server_listen_addr).await.unwrap();
    println!("Client sent KeepAlive message to {}", server_listen_addr);

    let (received_header, received_message, _sender_addr) = client_circuit.recv_message().await.unwrap();
    match received_message {
        Message::KeepAlive => {
            println!("Client received echo: {:?}, {:?}", received_header, Message::KeepAlive);
        }
        Message::Ack { sequence_id } => {
            println!("Client received ACK for sequence_id: {}", sequence_id);
        }
        _ => {
            println!("Client received unknown message: {:?}, {:?}", received_header, received_message);
        }
    }

    server_handle.await.unwrap();
}