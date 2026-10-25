//! The QUIC transport (ADR 0005 §2, ADR 0007 §6; feature `quic`): `quinn` over one UDP port, one reliable
//! bidirectional stream per peer carrying length-prefixed [`Msg`] frames.
//!
//! The host makes a fresh self-signed certificate per session; joiners pin it (it travels with the LAN
//! announcement or the join details), so no certificate authority is involved. The links are synchronous
//! [`Transport`]s backed by a tokio runtime the endpoint owns.

use std::net::SocketAddr;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;

use tokio::runtime::{Handle, Runtime};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::lockstep::Msg;
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
    _rt: Arc<Runtime>,
}

impl QuicHost {
    /// Listen on `addr` with a fresh self-signed certificate.
    pub fn bind(addr: SocketAddr) -> Result<Self, Error> {
        let rt = runtime()?;
        let ck = rcgen::generate_simple_self_signed(vec![SERVER_NAME.into()])?;
        let cert = ck.cert.der().to_vec();
        let key = rustls::pki_types::PrivateKeyDer::Pkcs8(ck.signing_key.serialize_der().into());
        let mut config = quinn::ServerConfig::with_single_cert(vec![cert.clone().into()], key)?;
        let mut transport = quinn::TransportConfig::default();
        transport.keep_alive_interval(Some(std::time::Duration::from_secs(2)));
        config.transport_config(Arc::new(transport));
        let endpoint = {
            let _g = rt.enter();
            quinn::Endpoint::server(config, addr)?
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
        Ok(Self { endpoint, cert, joined, _rt: rt })
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
}

impl Drop for QuicHost {
    fn drop(&mut self) {
        self.endpoint.close(0u32.into(), b"session over");
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

/// Join the host at `addr`, authenticating it by `pin`; blocks until connected.
pub fn connect_pinned(addr: SocketAddr, pin: &Pin) -> Result<QuicLink, Error> {
    let rt = runtime()?;
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
    let mut transport = quinn::TransportConfig::default();
    transport.keep_alive_interval(Some(std::time::Duration::from_secs(2)));
    config.transport_config(Arc::new(transport));
    let local: SocketAddr = if addr.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse().unwrap();
    let endpoint = {
        let _g = rt.enter();
        quinn::Endpoint::client(local)?
    };
    let (conn, send, recv) = rt.block_on(async {
        let conn = endpoint.connect_with(config, addr, SERVER_NAME)?.await?;
        let (mut send, recv) = conn.open_bi().await?;
        send.write_all(HELLO).await.map_err(|e| Error::Handshake(e.to_string()))?;
        Ok::<_, Error>((conn, send, recv))
    })?;
    let mut link = QuicLink::start(rt.handle(), conn, send, recv);
    link._endpoint = Some(endpoint);
    link._rt = Some(rt);
    Ok(link)
}
