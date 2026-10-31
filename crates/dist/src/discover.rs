//! Finding a coordinator on the local network by its pairing code: a coordinator with a code says every two
//! seconds, by UDP broadcast (and to this computer), that it listens on its port, with a beacon made from the
//! code that does not give the code away; a worker with the same code listens for that beacon. Networks that
//! do not carry broadcasts (some offices' Wi-Fi) need the coordinator's address typed instead.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

use crate::secure::Code;
use crate::{Error, Result};

/// The port the beacons go to.
pub const PORT: u16 = 7701;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Beacon {
    terraflux: String,
    port: u16,
    name: String,
    beacon: String,
}

/// A coordinator found: its address (`host:port`), its computer's name and its version.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub addr: String,
    pub name: String,
    pub version: String,
}

/// This computer's name.
pub fn hostname() -> String {
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "a computer".into())
}

/// Say, every two seconds until the task is stopped, that a coordinator of this code listens on `port` (the
/// beacons go to the port `to`). A coordinator listening on this computer only (`on` a loopback address) says it
/// to this computer only: no socket open to the networks (none for the firewall to ask about).
pub fn announce(code: &Code, port: u16, to: u16, on: IpAddr) -> Result<tokio::task::JoinHandle<()>> {
    let local = on.is_loopback();
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_broadcast(!local)?;
    s.set_nonblocking(true)?;
    let from = if local { Ipv4Addr::LOCALHOST } else { Ipv4Addr::UNSPECIFIED };
    s.bind(&SocketAddr::from((from, 0)).into())?;
    let socket = UdpSocket::from_std(s.into())?;
    let b = Beacon { terraflux: crate::coordinator::VERSION.into(), port, name: hostname(), beacon: code.beacon() };
    let bytes = serde_json::to_vec(&b)?;
    Ok(tokio::spawn(async move {
        let targets = [SocketAddrV4::new(Ipv4Addr::BROADCAST, to), SocketAddrV4::new(Ipv4Addr::LOCALHOST, to)];
        loop {
            for t in targets.iter().copied().filter(|t| !local || t.ip().is_loopback()) {
                let _ = socket.send_to(&bytes, t).await;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }))
}

/// Where a worker hears the beacons: every network of this computer.
pub fn everywhere() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::UNSPECIFIED, PORT))
}

/// A socket on the beacons' port, shared with other listeners of this computer.
fn listener(at: SocketAddr) -> Result<UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    #[cfg(all(unix, not(target_os = "solaris")))]
    s.set_reuse_port(true)?;
    s.set_nonblocking(true)?;
    s.bind(&at.into()).map_err(|e| Error::Other(format!("listening for coordinators on {at}: {e}")))?;
    Ok(UdpSocket::from_std(s.into())?)
}

/// The coordinators of this code heard within `wait` (at `on`: [`everywhere`]), the first heard first.
pub async fn find(code: &Code, wait: Duration, on: SocketAddr) -> Result<Vec<Found>> {
    let socket = listener(on)?;
    let want = code.beacon();
    let deadline = Instant::now() + wait;
    let mut found: Vec<Found> = Vec::new();
    let mut buf = [0u8; 2048];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let Ok(Ok((n, from))) = tokio::time::timeout(left, socket.recv_from(&mut buf)).await else { break };
        let Ok(b) = serde_json::from_slice::<Beacon>(&buf[..n]) else { continue };
        if b.beacon != want {
            continue;
        }
        let addr = format!("{}:{}", from.ip(), b.port);
        if !found.iter().any(|f| f.addr == addr || f.name == b.name) {
            found.push(Found { addr, name: b.name, version: b.terraflux });
            // one is enough to start; the others heard in a moment
            if found.len() == 1 {
                let more = Instant::now() + Duration::from_millis(300);
                let rest = deadline.min(more);
                let mut buf2 = [0u8; 2048];
                while let Ok(Ok((n, from))) =
                    tokio::time::timeout(rest.saturating_duration_since(Instant::now()), socket.recv_from(&mut buf2))
                        .await
                {
                    if let Ok(b) = serde_json::from_slice::<Beacon>(&buf2[..n])
                        && b.beacon == want
                    {
                        let addr = format!("{}:{}", from.ip(), b.port);
                        if !found.iter().any(|f| f.addr == addr || f.name == b.name) {
                            found.push(Found { addr, name: b.name, version: b.terraflux });
                        }
                    }
                }
                break;
            }
        }
    }
    Ok(found)
}
