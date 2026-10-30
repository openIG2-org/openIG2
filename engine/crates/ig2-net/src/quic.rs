//! The QUIC transport (ADR 0005 §2, ADR 0007 §6; feature `quic`): `quinn` over one UDP port, one reliable
//! bidirectional stream per peer carrying length-prefixed [`Msg`] frames.
//!
//! The host makes a fresh self-signed certificate per session; joiners pin it (it travels with the LAN
//! announcement or the join details), so no certificate authority is involved. The links are synchronous
//! [`Transport`]s backed by a tokio runtime the endpoint owns.

use std::collections::HashSet;
use std::future::Future;
use std::io;
use std::net::{SocketAddr, SocketAddrV4};
use std::task::{ready, Context, Poll};
use std::time::{Duration, Instant};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;

use tokio::runtime::{Handle, Runtime};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::joincode::PIN_BYTES;
use crate::lockstep::Msg;
use crate::rendezvous::{Packet, Refusal, MAGIC};
use crate::transport::Transport;
use crate::wire;

/// The certificate's server name (the session is identified by the pinned certificate, not a host name).
const SERVER_NAME: &str = "ig2-session";
/// The joiner's first frame: protocol magic and version.
const HELLO: &[u8; 8] = b"IG2N\x01\x00\x00\x00";
/// The largest frame accepted (a snapshot is a few hundred KB compressed, a few MB raw).
const MAX_FRAME: usize = 64 << 20;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("certificate: {0}")]
    Cert(#[from] rcgen::Error),
    #[error("TLS: {0}")]
    Tls(#[from] rustls::Error),
    #[error("QUIC set-up: {0}")]
    Config(String),
    #[error(transparent)]
    Connect(#[from] quinn::ConnectError),
    #[error(transparent)]
    Connection(#[from] quinn::ConnectionError),
    #[error("handshake: {0}")]
    Handshake(String),
}

fn runtime() -> Result<Arc<Runtime>, Error> {
    Ok(Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?))
}

/// One end of a QUIC connection as a [`Transport`].
pub struct QuicLink {
    out: UnboundedSender<Vec<u8>>,
    inc: Receiver<Vec<u8>>,
    conn: quinn::Connection,
    // A joiner's link owns its endpoint and runtime; a host's links live on the host's (they close with it).
    _endpoint: Option<quinn::Endpoint>,
    _rt: Option<Arc<Runtime>>,
}

impl QuicLink {
    /// Run the stream's reader and writer tasks.
    fn start(rt: &Handle, conn: quinn::Connection, mut send: quinn::SendStream, mut recv: quinn::RecvStream) -> Self {
        let (out, mut out_rx) = unbounded_channel::<Vec<u8>>();
        let (inc_tx, inc) = mpsc::channel();
        rt.spawn(async move {
            while let Some(b) = out_rx.recv().await {
                if send.write_all(&(b.len() as u32).to_le_bytes()).await.is_err() || send.write_all(&b).await.is_err() {
                    break;
                }
            }
            let _ = send.finish();
        });
        rt.spawn(async move {
            loop {
                let mut len = [0u8; 4];
                if recv.read_exact(&mut len).await.is_err() {
                    break;
                }
                let n = u32::from_le_bytes(len) as usize;
                if n > MAX_FRAME {
                    break;
                }
                let mut b = vec![0; n];
                if recv.read_exact(&mut b).await.is_err() || inc_tx.send(b).is_err() {
                    break;
                }
            }
        });
        Self { out, inc, conn, _endpoint: None, _rt: None }
    }

    /// The other end's address.
    pub fn remote(&self) -> SocketAddr {
        self.conn.remote_address()
    }

    /// The connection ended (closed by either side, or timed out).
    pub fn is_closed(&self) -> bool {
        self.conn.close_reason().is_some()
    }

    /// Close the connection.
    pub fn close(&self) {
        self.conn.close(0u32.into(), b"bye");
    }
}

impl Transport for QuicLink {
    fn remote(&self) -> Option<std::net::IpAddr> {
        Some(self.remote().ip())
    }

    fn send(&mut self, msg: &Msg) {
        let _ = self.out.send(wire::to_vec(msg).expect("messages encode"));
    }

    fn recv(&mut self) -> Result<Option<Msg>, wire::Error> {
        match self.inc.try_recv() {
            Ok(b) => wire::from_slice(&b).map(Some),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(None),
        }
    }

    /// The TLS session's exported keying material (RFC 5705, label `EXPORTER-ig2-lobby`): the same on both ends
    /// of one connection, different across a man in the middle (ADR 0008 §3.1).
    fn binding(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        self.conn.export_keying_material(&mut out, b"EXPORTER-ig2-lobby", b"").expect("32 bytes are exportable");
        out
    }

    fn closed(&self) -> bool {
        self.is_closed()
    }
}

/// The host's endpoint: it listens on one UDP port and hands out a [`QuicLink`] per joiner.
pub struct QuicHost {
    endpoint: quinn::Endpoint,
    cert: Vec<u8>,
    joined: Receiver<QuicLink>,
    relay: Option<Registration>,
    _rt: Arc<Runtime>,
}

impl QuicHost {
    /// Listen on `addr` with a fresh self-signed certificate.
    pub fn bind(addr: SocketAddr) -> Result<Self, Error> {
        Self::bind_with(addr, None)
    }

    /// Listen on `addr` and use the rendezvous service at `relay` (ADR 0018): [`QuicHost::register`] opens the
    /// room; joiners that can't reach this host directly come through the service.
    pub fn bind_relayed(addr: SocketAddr, relay: SocketAddrV4) -> Result<Self, Error> {
        Self::bind_with(addr, Some(relay))
    }

    fn bind_with(addr: SocketAddr, relay: Option<SocketAddrV4>) -> Result<Self, Error> {
        let rt = runtime()?;
        let ck = rcgen::generate_simple_self_signed(vec![SERVER_NAME.into()])?;
        let cert = ck.cert.der().to_vec();
        let key = rustls::pki_types::PrivateKeyDer::Pkcs8(ck.signing_key.serialize_der().into());
        let mut config = quinn::ServerConfig::with_single_cert(vec![cert.clone().into()], key)?;
        config.transport_config(transport());
        let (endpoint, relay) = {
            let _g = rt.enter();
            match relay {
                None => (quinn::Endpoint::server(config, addr)?, None),
                Some(r) => {
                    let (socket, notes) = RelaySocket::bind(addr, r)?;
                    let ep = quinn::Endpoint::new_with_abstract_socket(quinn::EndpointConfig::default(), Some(config), socket.clone(), Arc::new(quinn::TokioRuntime))?;
                    (ep, Some(Registration { socket, notes, state: std::sync::Mutex::new((Rendezvous::Off, None)) }))
                }
            }
        };
        let (tx, joined) = mpsc::channel();
        let ep = endpoint.clone();
        rt.spawn(async move {
            while let Some(incoming) = ep.accept().await {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let Ok(conn) = incoming.await else { return };
                    let Ok((send, mut recv)) = conn.accept_bi().await else { return };
                    let mut hello = [0u8; 8];
                    if recv.read_exact(&mut hello).await.is_err() || &hello != HELLO {
                        conn.close(1u32.into(), b"bad hello");
                        return;
                    }
                    let _ = tx.send(QuicLink::start(&Handle::current(), conn, send, recv));
                });
            }
        });
        Ok(Self { endpoint, cert, joined, relay, _rt: rt })
    }

    /// The bound address (with the port the OS picked for port 0).
    pub fn local_addr(&self) -> Result<SocketAddr, Error> {
        Ok(self.endpoint.local_addr()?)
    }

    /// The session certificate (DER) joiners pin.
    pub fn cert(&self) -> &[u8] {
        &self.cert
    }

    /// The certificate's SHA-256 (announced on the LAN).
    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint(&self.cert)
    }

    /// The next peer that connected, if any (non-blocking).
    pub fn accept(&self) -> Option<QuicLink> {
        self.joined.try_recv().ok()
    }

    /// Open (and keep refreshing) the room of session `session` at the rendezvous service; nothing without one.
    pub fn register(&self, session: u64) {
        let Some(r) = &self.relay else { return };
        let mut pin = [0; PIN_BYTES];
        pin.copy_from_slice(&self.fingerprint()[..PIN_BYTES]);
        *r.state.lock().unwrap() = (Rendezvous::Asking, Some(Instant::now()));
        let socket = Arc::downgrade(&r.socket);
        self._rt.spawn(async move {
            let mut every = tokio::time::interval(REFRESH);
            // Until the host (its socket) is gone.
            loop {
                every.tick().await;
                let Some(s) = socket.upgrade() else { break };
                s.control(&Packet::Register { session, pin });
            }
        });
    }

    /// The room at the rendezvous service.
    pub fn rendezvous(&self) -> Rendezvous {
        let Some(r) = &self.relay else { return Rendezvous::Off };
        let mut st = r.state.lock().unwrap();
        while let Ok(n) = r.notes.try_recv() {
            match n {
                Packet::Registered { seen } => st.0 = Rendezvous::Registered(seen),
                Packet::Refused(why) => st.0 = Rendezvous::Refused(why),
                _ => {}
            }
        }
        if st.0 == Rendezvous::Asking && st.1.is_some_and(|t| t.elapsed() > NO_ANSWER) {
            st.0 = Rendezvous::NoAnswer;
        }
        st.0
    }
}

impl Drop for QuicHost {
    fn drop(&mut self) {
        self.endpoint.close(0u32.into(), b"session over");
        if let Some(r) = &self.relay {
            // Let the relayed peers' close notices through before the room goes.
            if tokio::runtime::Handle::try_current().is_err() {
                let _ = self._rt.block_on(async { tokio::time::timeout(Duration::from_millis(300), self.endpoint.wait_idle()).await });
            }
            r.socket.control(&Packet::Unregister);
        }
    }
}

/// SHA-256 of a certificate (DER): what LAN announcements and join details carry (ADR 0008 §1).
pub fn fingerprint(cert: &[u8]) -> [u8; 32] {
    <sha2::Sha256 as sha2::Digest>::digest(cert).into()
}

/// How a joiner authenticates the host's session certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pin {
    /// The whole certificate (DER).
    Cert(Vec<u8>),
    /// Its SHA-256 (from a LAN announcement).
    Fingerprint([u8; 32]),
    /// Its SHA-256's leading bytes (a join code's, `joincode::PIN_BYTES`).
    Prefix(Vec<u8>),
    /// None: a direct join by address. The connection is encrypted but the host unauthenticated until a password
    /// check (ADR 0008 §3.1) binds the channel.
    Any,
}

/// Accepts the server certificate whose fingerprint matches (or any), then checks the handshake signatures as
/// usual, so the server must hold the certificate's key.
#[derive(Debug)]
struct Pinned {
    fingerprint: Option<Vec<u8>>,
    algs: rustls::crypto::WebPkiSupportedAlgorithms,
}

impl rustls::client::danger::ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        match self.fingerprint {
            Some(ref f) if !fingerprint(end_entity).starts_with(f) => Err(rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer)),
            _ => Ok(rustls::client::danger::ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.algs.supported_schemes()
    }
}

/// Join the host at `addr` whose session certificate is `cert` (DER); blocks until connected.
pub fn connect(addr: SocketAddr, cert: &[u8]) -> Result<QuicLink, Error> {
    connect_pinned(addr, &Pin::Cert(cert.to_vec()))
}

/// Both ends' transport settings: keep-alives (a relayed peer's room stays open on them, ADR 0018).
fn transport() -> Arc<quinn::TransportConfig> {
    let mut transport = quinn::TransportConfig::default();
    transport.keep_alive_interval(Some(Duration::from_secs(2)));
    Arc::new(transport)
}

/// A joiner's settings: TLS 1.3 authenticating the host by `pin`.
fn client_config(pin: &Pin) -> Result<quinn::ClientConfig, Error> {
    let tls = match pin {
        Pin::Cert(cert) => {
            let mut roots = rustls::RootCertStore::empty();
            roots.add(cert.clone().into())?;
            rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_protocol_versions(&[&rustls::version::TLS13])?
                .with_root_certificates(roots)
                .with_no_client_auth()
        }
        Pin::Fingerprint(_) | Pin::Prefix(_) | Pin::Any => {
            let provider = rustls::crypto::ring::default_provider();
            let algs = provider.signature_verification_algorithms;
            let fingerprint = match pin {
                Pin::Fingerprint(f) => Some(f.to_vec()),
                Pin::Prefix(f) => Some(f.clone()),
                _ => None,
            };
            rustls::ClientConfig::builder_with_provider(Arc::new(provider))
                .with_protocol_versions(&[&rustls::version::TLS13])?
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(Pinned { fingerprint, algs }))
                .with_no_client_auth()
        }
    };
    let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(tls).map_err(|e| Error::Config(e.to_string()))?;
    let mut config = quinn::ClientConfig::new(Arc::new(crypto));
    config.transport_config(transport());
    Ok(config)
}

/// Join the host at `addr`, authenticating it by `pin`; blocks until connected.
pub fn connect_pinned(addr: SocketAddr, pin: &Pin) -> Result<QuicLink, Error> {
    let rt = runtime()?;
    let config = client_config(pin)?;
    let local: SocketAddr = if addr.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse().unwrap();
    let endpoint = {
        let _g = rt.enter();
        quinn::Endpoint::client(local)?
    };
    let (conn, send, recv) = rt.block_on(handshake(&endpoint, config, addr))?;
    let mut link = QuicLink::start(rt.handle(), conn, send, recv);
    link._endpoint = Some(endpoint);
    link._rt = Some(rt);
    Ok(link)
}

async fn handshake(endpoint: &quinn::Endpoint, config: quinn::ClientConfig, addr: SocketAddr) -> Result<(quinn::Connection, quinn::SendStream, quinn::RecvStream), Error> {
    let conn = endpoint.connect_with(config, addr, SERVER_NAME)?.await?;
    let (mut send, recv) = conn.open_bi().await?;
    send.write_all(HELLO).await.map_err(|e| Error::Handshake(e.to_string()))?;
    Ok((conn, send, recv))
}

// ------------------------------------------------------------------------------------------------ rendezvous

/// How often a host refreshes its room (the service drops it after 30 s without).
const REFRESH: Duration = Duration::from_secs(5);
/// A room not confirmed this long after the first Register: the service doesn't answer.
const NO_ANSWER: Duration = Duration::from_secs(6);
/// The joiner's direct try at the host's public address (the hole punch), then the relayed one.
const DIRECT: Duration = Duration::from_secs(4);
const RELAYED: Duration = Duration::from_secs(10);

/// A host's room at the rendezvous service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rendezvous {
    /// No service, or not registered yet.
    Off,
    Asking,
    /// The room is open; the service sees this host at this public address.
    Registered(SocketAddrV4),
    Refused(Refusal),
    NoAnswer,
}

struct Registration {
    socket: Arc<RelaySocket>,
    notes: Receiver<Packet<'static>>,
    /// The status and when the first Register went out.
    state: std::sync::Mutex<(Rendezvous, Option<Instant>)>,
}

/// A UDP socket shared by QUIC and the rendezvous service (ADR 0018 §2). Datagrams to a peer marked relayed go to
/// the service wrapped in `To`; the service's `From` datagrams come out as the peer's own, so QUIC sees the peer's
/// public address either way. The service's other packets go to `notes`; a host answers `Incoming` with punches.
/// Plain `tokio` sends and receives, one datagram at a time (no GSO/GRO: the header changes the sizes).
#[derive(Debug)]
pub struct RelaySocket {
    io: tokio::net::UdpSocket,
    relay: SocketAddr,
    relayed: std::sync::Mutex<HashSet<SocketAddr>>,
    notes: mpsc::Sender<Packet<'static>>,
}

impl RelaySocket {
    /// Bind `addr` (inside the runtime) for the service at `relay`.
    fn bind(addr: SocketAddr, relay: SocketAddrV4) -> io::Result<(Arc<Self>, Receiver<Packet<'static>>)> {
        let std = std::net::UdpSocket::bind(addr)?;
        std.set_nonblocking(true)?;
        let (notes, rx) = mpsc::channel();
        Ok((Arc::new(Self { io: tokio::net::UdpSocket::from_std(std)?, relay: relay.into(), relayed: Default::default(), notes }), rx))
    }

    /// A packet to the service (lost if the socket is busy: every control packet is repeated or answered).
    fn control(&self, p: &Packet) {
        let _ = self.io.try_send_to(&p.encode(), self.relay);
    }

    /// From now on, reach `peer` through the service.
    fn relay_to(&self, peer: SocketAddr) {
        self.relayed.lock().unwrap().insert(peer);
    }
}

/// An answer or a notice from the service, without borrowed data.
fn owned(p: Packet) -> Option<Packet<'static>> {
    Some(match p {
        Packet::Registered { seen } => Packet::Registered { seen },
        Packet::Found { host } => Packet::Found { host },
        Packet::Incoming { joiner } => Packet::Incoming { joiner },
        Packet::Refused(r) => Packet::Refused(r),
        _ => return None,
    })
}

impl quinn::AsyncUdpSocket for RelaySocket {
    fn create_io_poller(self: Arc<Self>) -> std::pin::Pin<Box<dyn quinn::UdpPoller>> {
        Box::pin(Writable { socket: self, wait: None })
    }

    fn try_send(&self, t: &quinn::udp::Transmit) -> io::Result<()> {
        let relayed = match t.destination {
            SocketAddr::V4(peer) if self.relayed.lock().unwrap().contains(&t.destination) => Some(Packet::To { peer, data: t.contents }.encode()),
            _ => None,
        };
        let r = match &relayed {
            Some(b) => self.io.try_send_to(b, self.relay),
            None => self.io.try_send_to(t.contents, t.destination),
        };
        match r {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Err(e),
            // Like quinn's own socket: a failed send is a lost datagram, not a dead endpoint.
            _ => Ok(()),
        }
    }

    fn poll_recv(&self, cx: &mut Context, bufs: &mut [io::IoSliceMut<'_>], meta: &mut [quinn::udp::RecvMeta]) -> Poll<io::Result<usize>> {
        loop {
            let mut rb = tokio::io::ReadBuf::new(&mut bufs[0]);
            let from = match ready!(self.io.poll_recv_from(cx, &mut rb)) {
                Ok(a) => a,
                // Windows reports an earlier send's ICMP "unreachable" here.
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                Err(e) => return Poll::Ready(Err(e)),
            };
            let n = rb.filled().len();
            let b = &bufs[0][..n];
            let (addr, start) = if from == self.relay {
                match Packet::decode(b) {
                    Some(Packet::From { peer, data }) => (SocketAddr::V4(peer), n - data.len()),
                    Some(p) => {
                        if let Packet::Incoming { joiner } = p {
                            // Open this host's NAT towards the joiner; its direct try may then get in.
                            for _ in 0..3 {
                                let _ = self.io.try_send_to(&Packet::Punch.encode(), joiner.into());
                            }
                        }
                        if let Some(p) = owned(p) {
                            let _ = self.notes.send(p);
                        }
                        continue;
                    }
                    None => continue,
                }
            } else if b.starts_with(MAGIC) {
                // A punch.
                continue;
            } else {
                (from, 0)
            };
            if start > 0 {
                self.relay_to(addr);
                bufs[0].copy_within(start..n, 0);
            }
            meta[0] = quinn::udp::RecvMeta { addr, len: n - start, stride: n - start, ecn: None, dst_ip: None };
            return Poll::Ready(Ok(1));
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.local_addr()
    }
}

/// Waits for the socket to be writable (one waiter each, as quinn asks).
struct Writable {
    socket: Arc<RelaySocket>,
    wait: Option<std::pin::Pin<Box<dyn Future<Output = io::Result<()>> + Send + Sync>>>,
}

impl std::fmt::Debug for Writable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Writable")
    }
}

impl quinn::UdpPoller for Writable {
    fn poll_writable(mut self: std::pin::Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        let this = &mut *self;
        let wait = this.wait.get_or_insert_with(|| {
            let s = this.socket.clone();
            Box::pin(async move { s.io.writable().await })
        });
        let r = wait.as_mut().poll(cx);
        if r.is_ready() {
            this.wait = None;
        }
        r
    }
}

/// How a joiner may reach a host through the service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// The hole punch first, then the relay (ADR 0018 §4).
    PunchThenRelay,
    /// The relay only (tests; a NAT nobody punches through).
    RelayOnly,
}

/// Join the host of session `session` (a join code's session and pin) through the rendezvous service at `relay`;
/// blocks until connected (about 17 s at most).
pub fn connect_via(relay: SocketAddrV4, session: u64, pin: [u8; PIN_BYTES], via: Via) -> Result<QuicLink, Error> {
    let rt = runtime()?;
    let (socket, notes, endpoint) = {
        let _g = rt.enter();
        let (socket, notes) = RelaySocket::bind("0.0.0.0:0".parse().unwrap(), relay)?;
        let endpoint = quinn::Endpoint::new_with_abstract_socket(quinn::EndpointConfig::default(), None, socket.clone(), Arc::new(quinn::TokioRuntime))?;
        (socket, notes, endpoint)
    };
    let mut host = None;
    'ask: for _ in 0..3 {
        socket.control(&Packet::Lookup { session, pin });
        let until = Instant::now() + Duration::from_secs(1);
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            match notes.recv_timeout(left) {
                Ok(Packet::Found { host: h }) => {
                    host = Some(SocketAddr::V4(h));
                    break 'ask;
                }
                Ok(Packet::Refused(Refusal::Full)) => return Err(Error::Handshake("the relay is full".into())),
                Ok(Packet::Refused(_)) => return Err(Error::Handshake("the relay knows no such game".into())),
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }
    let host = host.ok_or_else(|| Error::Handshake("the relay doesn't answer".into()))?;
    let config = client_config(&Pin::Prefix(pin.to_vec()))?;
    let timed = |t, config| rt.block_on(async { tokio::time::timeout(t, handshake(&endpoint, config, host)).await.unwrap_or_else(|_| Err(Error::Handshake("timed out".into()))) });
    let up = match via {
        Via::PunchThenRelay => timed(DIRECT, config.clone()).ok(),
        Via::RelayOnly => None,
    };
    let (conn, send, recv) = match up {
        Some(c) => c,
        None => {
            socket.relay_to(host);
            timed(RELAYED, config)?
        }
    };
    let mut link = QuicLink::start(rt.handle(), conn, send, recv);
    link._endpoint = Some(endpoint);
    link._rt = Some(rt);
    Ok(link)
}
