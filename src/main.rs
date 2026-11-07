use dproxy::listener;
use std::net::Ipv4Addr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let local_socket = (Ipv4Addr::LOCALHOST, 8000);
    let upstream_socket = (Ipv4Addr::LOCALHOST, 9000);

    listener::start(local_socket.into(), upstream_socket.into()).await?;
    Ok(())
}
