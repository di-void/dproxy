use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpSocket, TcpStream};

pub async fn start(
    local_addr: SocketAddr,
    upstream_addr: SocketAddr,
    // TODO: narrow error type
) -> io::Result<()> {
    let socket = TcpSocket::new_v4()?;
    socket.set_reuseaddr(true)?;
    // bind
    socket.bind(local_addr)?;

    // listen
    let listener = socket.listen(100)?; // max of 100 unaccepted tcp connections
    println!("Proxy is listening on: {:?}", listener.local_addr()?);

    // connection loop
    loop {
        match listener.accept().await {
            Ok(conn) => {
                let (mut client, addr) = conn;
                println!("Accepted new client connection: {:?}", addr);

                tokio::spawn(async move {
                    let mut upstream = match TcpStream::connect(&upstream_addr).await {
                        Ok(upstream) => upstream,
                        Err(e) => {
                            eprintln!("Couldn't connect to upstream addr {upstream_addr:?}: {e}");
                            return;
                        }
                    };
                    match tokio::io::copy_bidirectional(&mut client, &mut upstream).await {
                        Ok((a, b)) => {
                            println!("wrote {a} bytes from {addr:?} to {upstream_addr:?}");
                            println!("wrote {b} bytes from {upstream_addr:?} to {addr:?}");
                        }
                        Err(e) => eprintln!(
                            "Error while copying between {addr:?} and {upstream_addr:?}: {e}"
                        ),
                    }
                });
            }
            Err(e) => {
                // EMFILE/ENFILE etc: accept fails instantly until an fd frees up, so back off
                // instead of spinning the CPU and flooding the log.
                eprintln!("Couldn't get client: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}
