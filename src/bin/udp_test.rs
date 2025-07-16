use tokio::net::{TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    // SOCKS5 proxy address (Hippolyzer)
    let socks5_addr = "127.0.0.1:9061";
    // The ultimate UDP target you want to reach (the real destination)
    let udp_target_addr = "127.0.0.1:9061";
    let udp_target: std::net::SocketAddr = udp_target_addr.parse().expect("Invalid target address");
    // Use real values or test values as needed
    let sequence_number = 1u32;
    let circuit_code = 0x12345678u32;
    let session_id = uuid::Uuid::new_v4();
    let agent_id = uuid::Uuid::new_v4();

    // --- Build UseCircuitCode packet (Low frequency) ---
    let mut packet = Vec::new();
    packet.push(0x00); // flags: low frequency
    packet.extend_from_slice(&sequence_number.to_be_bytes()); // 4 bytes packet id (big-endian)
    packet.push(0x00); // offset
    packet.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x03]); // message number for UseCircuitCode
    packet.extend_from_slice(&circuit_code.to_be_bytes()); // 4 bytes circuit code (big-endian)
    packet.extend_from_slice(session_id.as_bytes()); // 16 bytes session id
    packet.extend_from_slice(agent_id.as_bytes()); // 16 bytes agent id

    // Print full packet bytes with field labels
    println!("[DEBUG] Packet bytes:");
    println!("  flags:        {:02X}", packet[0]);
    println!("  packet_id:    {:02X?}", &packet[1..5]);
    println!("  offset:       {:02X}", packet[5]);
    println!("  msg_num:      {:02X?}", &packet[6..10]);
    println!("  circuit_code: {:02X?}", &packet[10..14]);
    println!("  session_id:   {:02X?}", &packet[14..30]);
    println!("  agent_id:     {:02X?}", &packet[30..46]);
    println!("  full:         {:02X?}", packet);

    // 1. Connect to SOCKS5 proxy over TCP
    let mut tcp = TcpStream::connect(socks5_addr).await.expect("Failed to connect to SOCKS5 proxy");
    println!("[SOCKS5] Connected to proxy at {}", socks5_addr);

    // 2. Send SOCKS5 handshake (no authentication)
    tcp.write_all(&[0x05, 0x01, 0x00]).await.expect("Failed to send handshake");
    let mut handshake_resp = [0u8; 2];
    tcp.read_exact(&mut handshake_resp).await.expect("Failed to read handshake response");
    assert_eq!(handshake_resp, [0x05, 0x00], "SOCKS5 handshake failed");
    println!("[SOCKS5] Handshake successful");

    // 3. Send UDP ASSOCIATE request
    let udp_associate_req = [
        0x05, 0x03, 0x00, 0x01, // VER, CMD, RSV, ATYP
        0x00, 0x00, 0x00, 0x00, // ADDR (0.0.0.0)
        0x00, 0x00,             // PORT (0)
    ];
    tcp.write_all(&udp_associate_req).await.expect("Failed to send UDP ASSOCIATE");

    // 4. Read UDP ASSOCIATE response
    let mut resp_hdr = [0u8; 4];
    tcp.read_exact(&mut resp_hdr).await.expect("Failed to read UDP ASSOCIATE header");
    assert_eq!(resp_hdr[0], 0x05, "Invalid SOCKS version in response");
    assert_eq!(resp_hdr[1], 0x00, "UDP ASSOCIATE failed");
    let atyp = resp_hdr[3];
    let udp_relay_addr = match atyp {
        0x01 => { // IPv4
            let mut addr_port = [0u8; 6];
            tcp.read_exact(&mut addr_port).await.expect("Failed to read IPv4 address/port");
            let ip = Ipv4Addr::new(addr_port[0], addr_port[1], addr_port[2], addr_port[3]);
            let port = u16::from_be_bytes([addr_port[4], addr_port[5]]);
            SocketAddr::new(IpAddr::V4(ip), port)
        },
        0x03 => { // Domain
            let mut len = [0u8; 1];
            tcp.read_exact(&mut len).await.expect("Failed to read domain length");
            let mut domain = vec![0u8; len[0] as usize];
            tcp.read_exact(&mut domain).await.expect("Failed to read domain");
            let mut port = [0u8; 2];
            tcp.read_exact(&mut port).await.expect("Failed to read port");
            let domain = String::from_utf8_lossy(&domain);
            let port = u16::from_be_bytes(port);
            format!("{}:{}", domain, port).parse().expect("Invalid domain/port")
        },
        0x04 => { // IPv6
            let mut addr_port = [0u8; 18];
            tcp.read_exact(&mut addr_port).await.expect("Failed to read IPv6 address/port");
            let ip = std::net::Ipv6Addr::from(<[u8; 16]>::try_from(&addr_port[0..16]).unwrap());
            let port = u16::from_be_bytes([addr_port[16], addr_port[17]]);
            SocketAddr::new(IpAddr::V6(ip), port)
        },
        _ => panic!("Unknown ATYP in UDP ASSOCIATE response: {}", atyp),
    };
    println!("[SOCKS5] UDP relay address: {}", udp_relay_addr);

    // 5. Bind a local UDP socket
    let udp_socket = UdpSocket::bind("0.0.0.0:0").await.expect("Failed to bind UDP socket");
    println!("[UDP TEST] Bound to {}", udp_socket.local_addr().unwrap());

    // 6. Build the SOCKS5 UDP request header
    let mut udp_packet = Vec::new();
    udp_packet.extend_from_slice(&[0x00, 0x00, 0x00]); // RSV, FRAG
    // ATYP + DST.ADDR + DST.PORT
    match udp_target {
        SocketAddr::V4(addr) => {
            udp_packet.push(0x01); // ATYP = IPv4
            udp_packet.extend_from_slice(&addr.ip().octets());
            udp_packet.extend_from_slice(&addr.port().to_be_bytes());
        },
        SocketAddr::V6(addr) => {
            udp_packet.push(0x04); // ATYP = IPv6
            udp_packet.extend_from_slice(&addr.ip().octets());
            udp_packet.extend_from_slice(&addr.port().to_be_bytes());
        },
    }
    udp_packet.extend_from_slice(&packet);

    // 7. Send the UDP packet to the relay
    let sent = udp_socket.send_to(&udp_packet, udp_relay_addr).await.expect("Failed to send UDP packet");
    println!("[UDP TEST] Sent {} bytes to {} (SOCKS5 relay)", sent, udp_relay_addr);

    // 8. Optionally, receive a response (with SOCKS5 header)
    let mut buf = [0u8; 1500];
    if let Ok((n, src)) = udp_socket.recv_from(&mut buf).await {
        println!("[UDP TEST] Received {} bytes from {}", n, src);
        // Parse SOCKS5 UDP header
        if n > 10 {
            let rsv = &buf[0..2];
            let frag = buf[2];
            let atyp = buf[3];
            println!("[UDP TEST] SOCKS5 header: RSV={:?} FRAG={} ATYP={}", rsv, frag, atyp);
            // You can parse the rest as needed
        }
    }
    // Keep TCP connection alive until done
    // (tcp is dropped here)
} 