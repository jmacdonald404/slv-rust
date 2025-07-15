use reqwest::{Client, Proxy};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Set up the HTTP proxy for both HTTP and HTTPS
    let proxy = Proxy::all("http://127.0.0.1:9062")?;
    let client = Client::builder()
        .proxy(proxy)
        .danger_accept_invalid_certs(true) // Remove if you want strict certs
        .build()?;

    // Make a simple GET request to the login URL
    let resp = client
        .get("https://login.agni.lindenlab.com/cgi-bin/login.cgi")
        .send()
        .await?;

    println!("Status: {}", resp.status());
    let body = resp.text().await?;
    println!("Body: {}", &body[..std::cmp::min(500, body.len())]); // Print first 500 chars

    Ok(())
} 