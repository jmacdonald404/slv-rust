use rand::Rng;
use std::net::UdpSocket;

pub fn pick_random_udp_port() -> u16 {
    let mut rng = rand::rng();
    for _ in 0..20 {
        let port = rng.random_range(10000..60000);
        if UdpSocket::bind(("0.0.0.0", port)).is_ok() {
            return port;
        }
    }
    0 // fallback, should not happen
} 