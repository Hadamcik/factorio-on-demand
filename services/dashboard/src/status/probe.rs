use tokio::{net::TcpStream, time::{timeout, Duration}};
use std::net::SocketAddr;

pub async fn probe_machine_online(addr: &SocketAddr, timeout_ms: u64) -> bool {
    match timeout(Duration::from_millis(timeout_ms), TcpStream::connect(addr)).await {
        Ok(Ok(_)) => true,
        Ok(Err(err)) => matches!(
            err.kind(),
            std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::NotConnected
        ),
        Err(_) => false,
    }
}

pub async fn probe_service_online(addr: &SocketAddr, timeout_ms: u64) -> bool {
    matches!(
        timeout(Duration::from_millis(timeout_ms), TcpStream::connect(addr)).await,
        Ok(Ok(_))
    )
}
