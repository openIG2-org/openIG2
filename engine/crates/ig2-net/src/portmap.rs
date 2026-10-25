//! Port mapping on the host's router (ADR 0008 §2): UPnP IGD (feature `upnp`, `igd-next`) and NAT-PMP (RFC 6886,
//! ours). Opt-in: nothing here runs unless the host's player enabled it; a [`Mapping`] renews its lease in the
//! background and removes it when closed or dropped.
//!
//! Only [`open_live`] searches the real network; the mappers themselves take the gateway's address, so tests run
//! them against fake gateways on 127.0.0.1.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// The lease asked for.
pub const LEASE: Duration = Duration::from_secs(2 * 60 * 60);
/// The mapping's description on the router.
pub const DESCRIPTION: &str = "IG2 session";
/// The NAT-PMP server port on the gateway.
pub const NATPMP_PORT: u16 = 5351;
/// A failed renewal is retried after this long.
const RETRY: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("the gateway didn't answer")]
    Timeout,
    #[error("NAT-PMP result code {0}")]
    NatPmp(u16),
    #[error("malformed answer")]
    BadReply,
    #[error("{0}")]
    Upnp(String),
    #[error("no IPv4 address on the LAN")]
    NoLocalAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Upnp,
    NatPmp,
}

/// A mapping the router granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Granted {
    pub protocol: Protocol,
    /// The router's external address and the port it forwards.
    pub external: SocketAddrV4,
    /// The lease granted (UPnP: the one asked for).
    pub lifetime: Duration,
}

/// One router protocol.
pub trait Mapper: Send {
    /// Forward UDP `external_port` to `internal` for `lifetime` (also renews).
    fn map(&mut self, internal: SocketAddrV4, external_port: u16, lifetime: Duration) -> Result<Granted, Error>;
    /// Remove the forwarding.
    fn unmap(&mut self, internal: SocketAddrV4, external_port: u16) -> Result<(), Error>;
}

// ------------------------------------------------------------------------------------------------ NAT-PMP

/// NAT-PMP (RFC 6886) over UDP to the gateway's port 5351.
pub struct NatPmp {
    gateway: SocketAddr,
    socket: UdpSocket,
    /// The first try's timeout; it doubles per retry (RFC 6886 §3.1: 250 ms).
    pub timeout: Duration,
    pub tries: u32,
}

impl NatPmp {
    /// A client for the NAT-PMP server at `gateway` (normally `router:5351`).
    pub fn new(gateway: SocketAddr) -> Result<Self, Error> {
        let bind: SocketAddr = if gateway.is_ipv4() { (Ipv4Addr::UNSPECIFIED, 0).into() } else { "[::]:0".parse().unwrap() };
        Ok(Self { gateway, socket: UdpSocket::bind(bind)?, timeout: Duration::from_millis(250), tries: 4 })
    }

    /// Send `req` and wait for the answer to opcode `op` (from the gateway only), retrying with doubling timeouts.
    fn request(&self, req: &[u8], len: usize) -> Result<Vec<u8>, Error> {
        let mut timeout = self.timeout;
        let mut buf = [0u8; 64];
        for _ in 0..self.tries {
            self.socket.send_to(req, self.gateway)?;
            self.socket.set_read_timeout(Some(timeout))?;
            loop {
                match self.socket.recv_from(&mut buf) {
                    Ok((n, from)) => {
                        if from != self.gateway || n < len || buf[0] != 0 || buf[1] != 128 + req[1] {
                            continue;
                        }
                        let code = u16::from_be_bytes([buf[2], buf[3]]);
                        if code != 0 {
                            return Err(Error::NatPmp(code));
                        }
                        return Ok(buf[..n].to_vec());
                    }
                    Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => break,
                    Err(e) if e.kind() == io::ErrorKind::ConnectionReset => break,
                    Err(e) => return Err(e.into()),
                }
            }
            timeout *= 2;
        }
        Err(Error::Timeout)
    }

    /// The router's external address (opcode 0).
    pub fn external_ip(&self) -> Result<Ipv4Addr, Error> {
        let r = self.request(&[0, 0], 12)?;
        Ok(Ipv4Addr::new(r[8], r[9], r[10], r[11]))
    }

    /// Opcode 1 (UDP): returns the mapped external port and the lifetime granted.
    fn udp(&self, internal: u16, external: u16, lifetime: u32) -> Result<(u16, u32), Error> {
        let mut req = [0u8; 12];
        req[1] = 1;
        req[4..6].copy_from_slice(&internal.to_be_bytes());
        req[6..8].copy_from_slice(&external.to_be_bytes());
        req[8..12].copy_from_slice(&lifetime.to_be_bytes());
        let r = self.request(&req, 16)?;
        if u16::from_be_bytes([r[8], r[9]]) != internal {
            return Err(Error::BadReply);
        }
        Ok((u16::from_be_bytes([r[10], r[11]]), u32::from_be_bytes([r[12], r[13], r[14], r[15]])))
    }
}

impl Mapper for NatPmp {
    fn map(&mut self, internal: SocketAddrV4, external_port: u16, lifetime: Duration) -> Result<Granted, Error> {
        let ip = self.external_ip()?;
        let (port, secs) = self.udp(internal.port(), external_port, lifetime.as_secs().min(u32::MAX as u64) as u32)?;
        if secs == 0 {
            return Err(Error::BadReply);
        }
        Ok(Granted { protocol: Protocol::NatPmp, external: SocketAddrV4::new(ip, port), lifetime: Duration::from_secs(secs as u64) })
    }

    fn unmap(&mut self, internal: SocketAddrV4, _external_port: u16) -> Result<(), Error> {
        // RFC 6886 §3.4: lifetime 0 and suggested external port 0 delete the mapping of this internal port.
        self.udp(internal.port(), 0, 0).map(|_| ())
    }
}

// ------------------------------------------------------------------------------------------------ UPnP

/// UPnP IGD (`igd-next`): `AddPortMapping` / `DeletePortMapping` / `GetExternalIPAddress` on a gateway's control URL.
#[cfg(feature = "upnp")]
pub struct Upnp {
    pub gateway: igd_next::Gateway,
}

#[cfg(feature = "upnp")]
impl Upnp {
    /// Search the LAN for an IGD (SSDP multicast). Only [`open_live`] calls this.
    pub fn search(timeout: Duration) -> Result<Self, Error> {
        let opts = igd_next::SearchOptions { timeout: Some(timeout), single_search_timeout: Some(timeout), ..Default::default() };
        let gateway = igd_next::search_gateway(opts).map_err(|e| Error::Upnp(e.to_string()))?;
        Ok(Self { gateway })
    }
}

#[cfg(feature = "upnp")]
impl Mapper for Upnp {
    fn map(&mut self, internal: SocketAddrV4, external_port: u16, lifetime: Duration) -> Result<Granted, Error> {
        let secs = lifetime.as_secs().clamp(1, u32::MAX as u64) as u32;
        self.gateway
            .add_port(igd_next::PortMappingProtocol::UDP, external_port, internal.into(), secs, DESCRIPTION)
            .map_err(|e| Error::Upnp(e.to_string()))?;
        let ip = match self.gateway.get_external_ip().map_err(|e| Error::Upnp(e.to_string()))? {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => return Err(Error::BadReply),
        };
        Ok(Granted { protocol: Protocol::Upnp, external: SocketAddrV4::new(ip, external_port), lifetime: Duration::from_secs(secs as u64) })
    }

    fn unmap(&mut self, _internal: SocketAddrV4, external_port: u16) -> Result<(), Error> {
        self.gateway.remove_port(igd_next::PortMappingProtocol::UDP, external_port).map_err(|e| Error::Upnp(e.to_string()))
    }
}

// ------------------------------------------------------------------------------------------------ the lease

/// A live mapping: renewed at half its lifetime by a background thread, removed by [`Mapping::close`] or on drop.
pub struct Mapping {
    granted: Arc<Mutex<Granted>>,
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<Result<(), Error>>>,
}

impl Mapping {
    /// Map `internal` (the same external port asked for) with `mapper` and keep it alive.
    pub fn open(mut mapper: Box<dyn Mapper>, internal: SocketAddrV4, lifetime: Duration) -> Result<Self, Error> {
        let first = mapper.map(internal, internal.port(), lifetime)?;
        let granted = Arc::new(Mutex::new(first));
        let (stop, stopped) = mpsc::channel::<()>();
        let g = granted.clone();
        let thread = std::thread::spawn(move || {
            let mut wait = first.lifetime / 2;
            loop {
                match stopped.recv_timeout(wait) {
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                }
                let port = g.lock().unwrap().external.port();
                match mapper.map(internal, port, lifetime) {
                    Ok(now) => {
                        *g.lock().unwrap() = now;
                        wait = now.lifetime / 2;
                    }
                    Err(_) => wait = RETRY.min(first.lifetime / 2),
                }
            }
            let port = g.lock().unwrap().external.port();
            mapper.unmap(internal, port)
        });
        Ok(Self { granted, stop: Some(stop), thread: Some(thread) })
    }

    /// What the router granted (last renewal).
    pub fn granted(&self) -> Granted {
        *self.granted.lock().unwrap()
    }

    /// Remove the mapping now; returns whether the router confirmed.
    pub fn close(mut self) -> Result<(), Error> {
        self.shut()
    }

    fn shut(&mut self) -> Result<(), Error> {
        drop(self.stop.take());
        match self.thread.take() {
            Some(t) => t.join().unwrap_or(Err(Error::BadReply)),
            None => Ok(()),
        }
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        let _ = self.shut();
    }
}

// ------------------------------------------------------------------------------------------------ live

/// This machine's IPv4 address on the route to `towards` (a UDP `connect` only consults the route table; nothing
/// is sent).
pub fn local_ipv4(towards: Ipv4Addr) -> Result<Ipv4Addr, Error> {
    let s = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    s.connect((towards, 9))?;
    match s.local_addr()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() => Ok(ip),
        _ => Err(Error::NoLocalAddress),
    }
}

/// Map the session's UDP `port` on the real router: UPnP first (feature `upnp`), then NAT-PMP at `gateway` (or the
/// conventional `x.y.z.1` of this machine's LAN address). **Touches the LAN's router**: call only when the host's
/// player enabled port mapping. Returns every protocol's error if none worked.
pub fn open_live(port: u16, gateway: Option<Ipv4Addr>) -> Result<Mapping, Vec<(Protocol, Error)>> {
    let mut errors = Vec::new();
    #[cfg(feature = "upnp")]
    match Upnp::search(Duration::from_secs(3)) {
        Ok(upnp) => {
            let gw = match upnp.gateway.addr.ip() {
                IpAddr::V4(ip) => Some(ip),
                IpAddr::V6(_) => None,
            };
            match gw.ok_or(Error::NoLocalAddress).and_then(local_ipv4) {
                Ok(local) => match Mapping::open(Box::new(upnp), SocketAddrV4::new(local, port), LEASE) {
                    Ok(m) => return Ok(m),
                    Err(e) => errors.push((Protocol::Upnp, e)),
                },
                Err(e) => errors.push((Protocol::Upnp, e)),
            }
        }
        Err(e) => errors.push((Protocol::Upnp, e)),
    }
    let natpmp = (|| {
        // The route towards a TEST-NET address gives the LAN address without sending anything.
        let local = local_ipv4(Ipv4Addr::new(192, 0, 2, 1))?;
        let gw = gateway.unwrap_or_else(|| {
            let o = local.octets();
            Ipv4Addr::new(o[0], o[1], o[2], 1)
        });
        let client = NatPmp::new(SocketAddr::from((gw, NATPMP_PORT)))?;
        Mapping::open(Box::new(client), SocketAddrV4::new(local, port), LEASE)
    })();
    match natpmp {
        Ok(m) => Ok(m),
        Err(e) => {
            errors.push((Protocol::NatPmp, e));
            Err(errors)
        }
    }
}
