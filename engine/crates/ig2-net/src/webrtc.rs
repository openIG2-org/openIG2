//! The WebRTC transport (ADR 0047 part 4): a browser joins a host over one reliable, ordered data channel, signalled
//! through the rendezvous service (`rendezvous::serve_with_http`: the page POSTs its offer, the host's QUIC socket
//! gets it as `Offer` and answers). Natively (feature `webrtc`) the host answers with `str0m` on a UDP socket of its
//! own ([`answer`]); a native joiner can offer the same way ([`connect`]). In the browser [`connect`] is the page's
//! `RTCPeerConnection`; a page hosts with [`PageHost`] (it long-polls the service for offers and answers them).
//! Every end asks the relay server's STUN answers for its public address.
//!
//! Messages larger than one data-channel message travel in [`CHUNK`]-byte pieces, each led by a byte that says whether
//! more follow. Between them each end probes the round trip every [`PING_EVERY`] (lead bytes [`PING`] and its echo
//! [`PONG`]) for [`Transport::rtt`], so a room's input delay adapts to the link as it does over QUIC (ADR 0063). The password binding (ADR 0008 §3.1) is a hash of both ends' DTLS fingerprints from the offer and
//! the answer: the same on both ends, different across a service that swapped them.

use crate::lockstep::Msg;
use crate::transport::Transport;
use crate::wire;

/// The data channel's largest piece (every browser takes 16 KiB messages).
pub const CHUNK: usize = 16 << 10;
/// The largest message put back together (as QUIC's frames).
use crate::transport::MAX_MESSAGE;
/// The data channel's label.
pub const LABEL: &str = "ig2";

/// A round-trip probe's lead byte; 8 bytes follow (the sender's clock, its own to read).
pub const PING: u8 = 2;
/// A probe's echo: the probe's bytes come back behind it.
pub const PONG: u8 = 3;
/// How often each end probes the round trip.
pub const PING_EVERY: std::time::Duration = std::time::Duration::from_millis(500);

/// `b` in data-channel pieces: a byte 1 (more follow) or 0 (the last), then up to [`CHUNK`] - 1 bytes.
pub fn pieces(b: &[u8]) -> Vec<Vec<u8>> {
    let n = b.len().div_ceil(CHUNK - 1).max(1);
    (0..n)
        .map(|i| {
            let part = &b[i * (CHUNK - 1)..((i + 1) * (CHUNK - 1)).min(b.len())];
            let mut v = Vec::with_capacity(part.len() + 1);
            v.push(u8::from(i + 1 < n));
            v.extend_from_slice(part);
            v
        })
        .collect()
}

/// Puts [`pieces`] back together.
#[derive(Default)]
pub struct Joiner {
    buf: Vec<u8>,
}

impl Joiner {
    /// The whole message once its last piece came (`Err`: an empty piece or an oversized message, the link is bad).
    pub fn push(&mut self, piece: &[u8]) -> Result<Option<Vec<u8>>, &'static str> {
        let (&more, part) = piece.split_first().ok_or("an empty piece")?;
        if self.buf.len() + part.len() > MAX_MESSAGE {
            return Err("a message too long");
        }
        self.buf.extend_from_slice(part);
        Ok((more == 0).then(|| std::mem::take(&mut self.buf)))
    }
}

/// The DTLS fingerprint lines of an SDP, in order.
fn fingerprints(sdp: &str) -> impl Iterator<Item = &str> {
    sdp.lines().filter_map(|l| l.trim().strip_prefix("a=fingerprint:")).map(str::trim)
}

/// The link's password binding: SHA-256 over the offer's and the answer's DTLS fingerprints.
pub fn binding(offer: &str, answer: &str) -> [u8; 32] {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(b"ig2-webrtc-binding");
    for f in fingerprints(offer).chain([""]).chain(fingerprints(answer)) {
        h.update(f.to_ascii_lowercase().as_bytes());
        h.update(b"\n");
    }
    h.finalize().into()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}

/// The service's HTTP side for a page reached over HTTPS or not: an HTTPS page may only fetch HTTPS, so it signals
/// through a TLS proxy on port 443 of the relay's host (docs/RENDEZVOUS.md); STUN stays on the relay's UDP port.
pub fn signal_base(relay: &str, https: bool) -> String {
    if https {
        format!("https://{}", relay.rsplit_once(':').map_or(relay, |r| r.0))
    } else {
        format!("http://{relay}")
    }
}

/// The HTTP path the page posts its offer to: the join code's session and pin in hex.
pub fn offer_path(session: u64, pin: &[u8]) -> String {
    format!("/offer/{session:016x}/{}", hex(pin))
}

/// The HTTP path a page host polls (ADR 0047 part 4): its session, pin and room key in hex.
pub fn host_path(session: u64, pin: &[u8], key: &[u8]) -> String {
    format!("/host/{session:016x}/{}/{}", hex(pin), hex(key))
}

/// The HTTP path a page host posts its answer to offer `id` to.
pub fn answer_path(session: u64, key: &[u8], id: u32) -> String {
    format!("/answer/{session:016x}/{}/{id}", hex(key))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("WebRTC: {0}")]
    Rtc(String),
    #[error("the relay server: {0}")]
    Signal(String),
    #[error("no connection came up")]
    Timeout,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// What a link's driver shares with its [`Link`].
struct Shared {
    open: std::sync::atomic::AtomicBool,
    closed: std::sync::atomic::AtomicBool,
    remote: std::sync::Mutex<Option<std::net::IpAddr>>,
    /// The probes' clock.
    epoch: web_time::Instant,
    /// The last probe went out at (µs on `epoch`; `u64::MAX`: none yet).
    pinged: std::sync::atomic::AtomicU64,
    /// The last probe's round trip in µs (0: none came back yet).
    rtt: std::sync::atomic::AtomicU64,
}

impl Default for Shared {
    fn default() -> Self {
        Self { open: Default::default(), closed: Default::default(), remote: Default::default(), epoch: web_time::Instant::now(), pinged: u64::MAX.into(), rtt: 0.into() }
    }
}

/// A received piece: a message's, a probe (its echo to send back) or an echo (its round trip noted).
#[derive(Debug, PartialEq, Eq)]
enum Piece {
    Message,
    Echo(Vec<u8>),
    Probe,
}

impl Shared {
    fn now(&self) -> u64 {
        self.epoch.elapsed().as_micros() as u64
    }

    /// A probe when one is due (the channel open).
    fn ping_due(&self) -> Option<Vec<u8>> {
        use std::sync::atomic::Ordering::SeqCst;
        let (now, last) = (self.now(), self.pinged.load(SeqCst));
        if !self.open.load(SeqCst) || (last != u64::MAX && now.saturating_sub(last) < PING_EVERY.as_micros() as u64) {
            return None;
        }
        self.pinged.store(now, SeqCst);
        Some([&[PING][..], &now.to_le_bytes()].concat())
    }

    fn take(&self, piece: &[u8]) -> Piece {
        match piece.split_first() {
            Some((&PING, b)) => Piece::Echo([&[PONG][..], b].concat()),
            Some((&PONG, b)) => {
                if let Ok(b) = b.try_into() {
                    self.rtt.store(self.now().saturating_sub(u64::from_le_bytes(b)).max(1), std::sync::atomic::Ordering::SeqCst);
                }
                Piece::Probe
            }
            _ => Piece::Message,
        }
    }
}

/// One end of a data channel as a [`Transport`] (`Send`: the channel itself lives on the driver's thread natively,
/// on the page in the browser).
pub struct Link {
    #[cfg(not(target_arch = "wasm32"))]
    out: std::sync::mpsc::Sender<Vec<u8>>,
    /// The driver's socket and port: an empty datagram to itself wakes it for a message to send at once.
    #[cfg(not(target_arch = "wasm32"))]
    wake: Option<(std::net::UdpSocket, u16)>,
    inc: std::sync::mpsc::Receiver<Vec<u8>>,
    shared: std::sync::Arc<Shared>,
    binding: [u8; 32],
    #[cfg(target_arch = "wasm32")]
    _page: web::Handle,
}

impl Link {
    /// The channel is open (a host's link is handed to the lobby once it is).
    pub fn is_open(&self) -> bool {
        self.shared.open.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Transport for Link {
    fn send(&mut self, msg: &Msg) {
        let b = wire::to_vec(msg).expect("messages encode");
        #[cfg(target_arch = "wasm32")]
        self._page.send(&b);
        #[cfg(not(target_arch = "wasm32"))]
        if self.out.send(b).is_ok() {
            if let Some((s, port)) = &self.wake {
                let _ = s.send_to(&[], (std::net::Ipv4Addr::LOCALHOST, *port));
            }
        }
    }

    fn recv(&mut self) -> Result<Option<Msg>, wire::Error> {
        // The page has no driver of its own: the game's polls send the probes.
        #[cfg(target_arch = "wasm32")]
        if let Some(p) = self.shared.ping_due() {
            self._page.send_piece(&p);
        }
        match self.inc.try_recv() {
            Ok(b) => wire::from_slice(&b).map(Some),
            Err(_) => Ok(None),
        }
    }

    fn binding(&self) -> [u8; 32] {
        self.binding
    }

    fn closed(&self) -> bool {
        self.shared.closed.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn remote(&self) -> Option<std::net::IpAddr> {
        *self.shared.remote.lock().unwrap()
    }

    fn rtt(&self) -> Option<std::time::Duration> {
        let us = self.shared.rtt.load(std::sync::atomic::Ordering::SeqCst);
        (us > 0).then(|| std::time::Duration::from_micros(us))
    }
}

#[cfg(all(feature = "webrtc", not(target_arch = "wasm32")))]
pub use native::{answer, connect};

#[cfg(all(feature = "webrtc", not(target_arch = "wasm32")))]
mod native {
    use std::collections::VecDeque;
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use str0m::change::{SdpAnswer, SdpOffer};
    use str0m::channel::ChannelId;
    use str0m::net::{Protocol, Receive};
    use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};

    use super::{binding, pieces, Error, Joiner, Link, Piece, Shared, LABEL};
    use crate::transport::Transport;

    /// How long a link may take to open.
    const OPENING: Duration = Duration::from_secs(15);
    /// The driver's longest sleep (queued sends wait at most this long).
    const TICK: Duration = Duration::from_millis(4);
    /// The socket's receive buffer.
    const RECV_BUFFER: usize = 4 << 20;

    fn rtc_err(e: impl std::fmt::Display) -> Error {
        Error::Rtc(e.to_string())
    }

    /// A socket of its own and host candidates for this machine's loopback and LAN addresses, and with `stun` (the
    /// relay server) the public address it sees (ADR 0047 part 4).
    fn bind(rtc: &mut Rtc, stun: Option<SocketAddrV4>) -> Result<(UdpSocket, u16, Option<Ipv4Addr>), Error> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        // A big message's burst overflows Windows' 64 KiB default, and a lost packet waits out SCTP's 1 s retransmit.
        let _ = socket2::SockRef::from(&socket).set_recv_buffer_size(RECV_BUFFER);
        let port = socket.local_addr()?.port();
        let lan = crate::portmap::local_ipv4(Ipv4Addr::new(192, 0, 2, 1)).ok().filter(|ip| !ip.is_loopback());
        for ip in [Some(Ipv4Addr::LOCALHOST), lan].into_iter().flatten() {
            rtc.add_local_candidate(Candidate::host(SocketAddr::from((ip, port)), "udp").map_err(rtc_err)?);
        }
        if let (Some(lan), Some(public)) = (lan, stun.and_then(|s| reflexive(&socket, s))) {
            if *public.ip() != lan && !public.ip().is_loopback() {
                rtc.add_local_candidate(Candidate::server_reflexive(public.into(), SocketAddr::from((lan, port)), "udp").map_err(rtc_err)?);
            }
        }
        Ok((socket, port, lan))
    }

    /// The address the STUN server at `stun` sees `socket` at (`None`: no answer within 300 ms).
    fn reflexive(socket: &UdpSocket, stun: SocketAddrV4) -> Option<SocketAddrV4> {
        let mut id = [0u8; 12];
        id[..8].copy_from_slice(&crate::discovery::random_u64().to_le_bytes());
        id[8..].copy_from_slice(&crate::discovery::random_u64().to_le_bytes()[..4]);
        socket.send_to(&crate::stun::request(id), stun).ok()?;
        let deadline = Instant::now() + Duration::from_millis(300);
        let mut buf = [0u8; 256];
        loop {
            socket.set_read_timeout(Some(deadline.checked_duration_since(Instant::now()).filter(|d| !d.is_zero())?)).ok()?;
            let (n, from) = socket.recv_from(&mut buf).ok()?;
            if let Some(a) = crate::stun::mapped(&buf[..n], id).filter(|_| from == SocketAddr::from(stun)) {
                return Some(a);
            }
        }
    }

    fn rtc() -> Rtc {
        Rtc::builder().build(Instant::now())
    }

    fn link(shared: Arc<Shared>, offer: &str, answer: &str, socket: &UdpSocket, port: u16) -> (Link, Receiver<Vec<u8>>, Sender<Vec<u8>>) {
        let (out, out_rx) = mpsc::channel();
        let (inc_tx, inc) = mpsc::channel();
        let wake = socket.try_clone().ok().map(|s| (s, port));
        (Link { out, wake, inc, shared, binding: binding(offer, answer) }, out_rx, inc_tx)
    }

    /// Answer a browser's `offer` (its SDP): the answer's SDP for the service, and the link, open once the browser
    /// connected (`Link::is_open`; closed when it never does). `stun`: the relay server, for a public candidate.
    pub fn answer(offer: &str, stun: Option<SocketAddrV4>) -> Result<(String, Link), Error> {
        let mut rtc = rtc();
        let (socket, port, lan) = bind(&mut rtc, stun)?;
        let parsed = SdpOffer::from_sdp_string(offer).map_err(rtc_err)?;
        let answer = rtc.sdp_api().accept_offer(parsed).map_err(rtc_err)?.to_sdp_string();
        let shared = Arc::new(Shared::default());
        let (l, out_rx, inc_tx) = link(shared.clone(), offer, &answer, &socket, port);
        std::thread::spawn(move || drive(rtc, socket, port, lan, None, shared, out_rx, inc_tx));
        Ok((answer, l))
    }

    /// Join the room of a join code's `session` and `pin` through the rendezvous service at `service` (its HTTP
    /// signalling, as a browser does): blocks until the channel is open.
    pub fn connect(service: SocketAddrV4, session: u64, pin: &[u8]) -> Result<Link, Error> {
        let mut rtc = rtc();
        let (socket, port, lan) = bind(&mut rtc, Some(service))?;
        let mut change = rtc.sdp_api();
        let cid = change.add_channel(LABEL.into());
        let (offer, pending) = change.apply().ok_or_else(|| rtc_err("no offer"))?;
        let offer = offer.to_sdp_string();
        let answer = post(service, &super::offer_path(session, pin), &offer)?;
        rtc.sdp_api().accept_answer(pending, SdpAnswer::from_sdp_string(&answer).map_err(rtc_err)?).map_err(rtc_err)?;
        let shared = Arc::new(Shared::default());
        let (l, out_rx, inc_tx) = link(shared.clone(), &offer, &answer, &socket, port);
        std::thread::spawn(move || drive(rtc, socket, port, lan, Some(cid), shared, out_rx, inc_tx));
        let start = Instant::now();
        while !l.is_open() {
            if l.closed() || start.elapsed() > OPENING {
                return Err(Error::Timeout);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(l)
    }

    /// The service's HTTP answer to `body` at `path` (a minimal HTTP/1.1 client: one request, `Connection: close`).
    fn post(service: SocketAddrV4, path: &str, body: &str) -> Result<String, Error> {
        use std::io::{Read, Write};
        let mut s = std::net::TcpStream::connect_timeout(&service.into(), Duration::from_secs(5))?;
        s.set_read_timeout(Some(Duration::from_secs(12)))?;
        write!(s, "POST {path} HTTP/1.1\r\nHost: {service}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())?;
        let mut r = Vec::new();
        s.read_to_end(&mut r)?;
        let r = String::from_utf8_lossy(&r);
        let (head, body) = r.split_once("\r\n\r\n").ok_or_else(|| Error::Signal("a malformed answer".into()))?;
        match head.split(' ').nth(1) {
            Some("200") => Ok(body.to_string()),
            Some("404") => Err(Error::Signal("no such room".into())),
            Some(code) => Err(Error::Signal(format!("HTTP {code}"))),
            None => Err(Error::Signal("a malformed answer".into())),
        }
    }

    /// The link's sans-IO loop on its own thread, until the channel closes, the connection drops or the [`Link`] is
    /// gone. `cid`: the channel this end opened (an offerer's); an answerer learns it from `ChannelOpen`.
    #[allow(clippy::too_many_arguments)]
    fn drive(mut rtc: Rtc, socket: UdpSocket, port: u16, lan: Option<Ipv4Addr>, mut cid: Option<ChannelId>, shared: Arc<Shared>, out: Receiver<Vec<u8>>, inc: Sender<Vec<u8>>) {
        let started = Instant::now();
        let mut queued: VecDeque<Vec<u8>> = VecDeque::new();
        let mut joiner = Joiner::default();
        let mut buf = vec![0u8; 2000];
        let mut open = false;
        let result: Result<(), Error> = (|| loop {
            // Outgoing messages, in pieces, as far as the channel's buffer takes them.
            loop {
                match out.try_recv() {
                    Ok(m) => queued.extend(pieces(&m)),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // Tell the other end (SCTP shutdown, DTLS close_notify) before the socket goes.
                        rtc.close().map_err(rtc_err)?;
                        for _ in 0..64 {
                            match rtc.poll_output() {
                                Ok(Output::Transmit(t)) => {
                                    let _ = socket.send_to(&t.contents, t.destination);
                                }
                                Ok(Output::Event(_)) => {}
                                Ok(Output::Timeout(_)) | Err(_) => break,
                            }
                        }
                        return Ok(());
                    }
                }
            }
            // A probe goes ahead of the queue (the other end takes it between a message's pieces).
            if let Some(p) = shared.ping_due() {
                queued.push_front(p);
            }
            if let Some(mut ch) = cid.filter(|_| open).and_then(|c| rtc.channel(c)) {
                while let Some(p) = queued.front() {
                    if !ch.write(true, p).map_err(rtc_err)? {
                        break;
                    }
                    queued.pop_front();
                }
            }
            let deadline = match rtc.poll_output().map_err(rtc_err)? {
                Output::Timeout(t) => t,
                Output::Transmit(t) => {
                    let _ = socket.send_to(&t.contents, t.destination);
                    continue;
                }
                Output::Event(e) => {
                    match e {
                        Event::ChannelOpen(c, _) => {
                            cid = Some(c);
                            open = true;
                            shared.open.store(true, Ordering::SeqCst);
                        }
                        Event::ChannelData(d) => match shared.take(&d.data) {
                            Piece::Echo(p) => queued.push_front(p),
                            Piece::Probe => {}
                            Piece::Message => match joiner.push(&d.data) {
                                Ok(Some(m)) => {
                                    let _ = inc.send(m);
                                }
                                Ok(None) => {}
                                Err(e) => return Err(rtc_err(e)),
                            },
                        },
                        Event::ChannelClose(_) | Event::Closed | Event::IceConnectionStateChange(IceConnectionState::Disconnected) => return Ok(()),
                        _ => {}
                    }
                    continue;
                }
            };
            if !open && started.elapsed() > OPENING {
                return Err(Error::Timeout);
            }
            let now = Instant::now();
            let wait = deadline.saturating_duration_since(now).min(TICK);
            if wait.is_zero() {
                rtc.handle_input(Input::Timeout(now)).map_err(rtc_err)?;
                continue;
            }
            socket.set_read_timeout(Some(wait))?;
            match socket.recv_from(&mut buf) {
                // A wake-up from the link (a message to send).
                Ok((0, _)) => {}
                Ok((n, source)) => {
                    // The address it came to: loopback from this machine's loopback, else the LAN candidate.
                    let ip = if source.ip().is_loopback() { Ipv4Addr::LOCALHOST } else { lan.unwrap_or(Ipv4Addr::LOCALHOST) };
                    let Ok(contents) = buf[..n].try_into() else { continue };
                    let input = Input::Receive(Instant::now(), Receive { proto: Protocol::Udp, source, destination: SocketAddr::from((ip, port)), contents });
                    if rtc.accepts(&input) {
                        rtc.handle_input(input).map_err(rtc_err)?;
                        shared.remote.lock().unwrap().get_or_insert(source.ip());
                    }
                }
                // A timeout, or Windows' ICMP "unreachable" from an earlier send.
                Err(_) => rtc.handle_input(Input::Timeout(Instant::now())).map_err(rtc_err)?,
            }
        })();
        if let Err(e) = result {
            eprintln!("ig2-net: WebRTC link: {e}");
        }
        shared.closed.store(true, Ordering::SeqCst);
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::{connect, PageHost};

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::net::SocketAddrV4;
    use std::rc::Rc;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::sync::Arc;

    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{MessageEvent, RtcConfiguration, RtcDataChannel, RtcDataChannelEvent, RtcDataChannelState, RtcDataChannelType, RtcIceServer, RtcPeerConnection, RtcSdpType, RtcSessionDescriptionInit};

    use super::{binding, pieces, Error, Joiner, Link, Piece, Shared, LABEL};
    use crate::joincode::PIN_BYTES;
    use crate::rendezvous::{Refusal, Rendezvous, KEY_BYTES};

    /// The page's side of a link: its connection, channel and handlers, by link id (they are not `Send`; the
    /// [`Link`] keeps the id).
    struct Page {
        pc: RtcPeerConnection,
        dc: RtcDataChannel,
        _on_message: Closure<dyn FnMut(MessageEvent)>,
        _on_close: Closure<dyn FnMut(JsValue)>,
    }

    thread_local! {
        static PAGES: RefCell<HashMap<u32, Page>> = RefCell::new(HashMap::new());
        static NEXT: RefCell<u32> = const { RefCell::new(0) };
    }

    /// Closes the page's connection when the [`Link`] goes.
    pub struct Handle(u32);

    impl Drop for Handle {
        fn drop(&mut self) {
            if let Some(p) = PAGES.with(|m| m.borrow_mut().remove(&self.0)) {
                p.dc.close();
                p.pc.close();
            }
        }
    }

    impl Handle {
        pub fn send(&self, b: &[u8]) {
            PAGES.with(|m| {
                if let Some(p) = m.borrow().get(&self.0) {
                    for piece in pieces(b) {
                        let _ = p.dc.send_with_u8_array(&piece);
                    }
                }
            });
        }

        /// One piece as it is (a probe).
        pub fn send_piece(&self, b: &[u8]) {
            PAGES.with(|m| {
                if let Some(p) = m.borrow().get(&self.0) {
                    let _ = p.dc.send_with_u8_array(b);
                }
            });
        }
    }

    fn js(e: JsValue) -> Error {
        Error::Rtc(e.as_string().or_else(|| js_sys::JSON::stringify(&e).ok().and_then(|s| s.as_string())).unwrap_or_else(|| "a browser error".into()))
    }

    async fn sleep(ms: i32) {
        let p = js_sys::Promise::new(&mut |res, _| {
            let _ = web_sys::window().expect("a window").set_timeout_with_callback_and_timeout_and_arguments_0(&res, ms);
        });
        let _ = JsFuture::from(p).await;
    }

    /// A connection that learns its public address from the relay server's STUN answers (its UDP port is `relay`;
    /// ADR 0047 part 4).
    fn peer(relay: &str) -> Result<RtcPeerConnection, Error> {
        let server = RtcIceServer::new();
        server.set_urls_str(&format!("stun:{relay}"));
        let cfg = RtcConfiguration::new();
        cfg.set_ice_servers(&js_sys::Array::of1(&server));
        RtcPeerConnection::new_with_configuration(&cfg).map_err(js)
    }

    /// The local description once its candidates are in (no trickling): when gathering completes or the relay
    /// server's STUN answer (the public candidate) is in, two seconds at most. Gathering can wait out a STUN request
    /// on every other interface (IPv6, a VPN) first.
    async fn gathered(pc: &RtcPeerConnection) -> Result<String, Error> {
        for _ in 0..100 {
            if pc.ice_gathering_state() == web_sys::RtcIceGatheringState::Complete || pc.local_description().is_some_and(|d| d.sdp().contains(" typ srflx")) {
                break;
            }
            sleep(20).await;
        }
        Ok(pc.local_description().ok_or_else(|| Error::Rtc("no description".into()))?.sdp())
    }

    /// POST `body` to `url`: the reply's status and bytes.
    async fn post(url: &str, body: &[u8]) -> Result<(u16, Vec<u8>), Error> {
        let init = web_sys::RequestInit::new();
        init.set_method("POST");
        init.set_body(&js_sys::Uint8Array::from(body));
        let window = web_sys::window().expect("a window");
        let resp: web_sys::Response = JsFuture::from(window.fetch_with_str_and_init(url, &init)).await.map_err(|e| Error::Signal(js(e).to_string()))?.unchecked_into();
        let buf = JsFuture::from(resp.array_buffer().map_err(js)?).await.map_err(js)?;
        Ok((resp.status(), js_sys::Uint8Array::new(&buf).to_vec()))
    }

    /// The page's end of channel `dc` as a [`Link`], its handlers set from now on (open once the channel is).
    fn wrap(pc: RtcPeerConnection, dc: RtcDataChannel, binding: [u8; 32]) -> Link {
        dc.set_binary_type(RtcDataChannelType::Arraybuffer);
        let shared = Arc::new(Shared::default());
        shared.open.store(dc.ready_state() == RtcDataChannelState::Open, Ordering::SeqCst);
        let (inc_tx, inc) = mpsc::channel();
        let mut joiner = Joiner::default();
        let (s, echo) = (shared.clone(), dc.clone());
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
            let b = js_sys::Uint8Array::new(&e.data()).to_vec();
            match s.take(&b) {
                Piece::Echo(p) => {
                    let _ = echo.send_with_u8_array(&p);
                    return;
                }
                Piece::Probe => return,
                Piece::Message => {}
            }
            match joiner.push(&b) {
                Ok(Some(m)) => {
                    let _ = inc_tx.send(m);
                }
                Ok(None) => {}
                Err(_) => s.closed.store(true, Ordering::SeqCst),
            }
        });
        let s = shared.clone();
        let on_close = Closure::<dyn FnMut(JsValue)>::new(move |_| s.closed.store(true, Ordering::SeqCst));
        dc.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        dc.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        let id = NEXT.with(|n| {
            let mut n = n.borrow_mut();
            *n += 1;
            *n
        });
        PAGES.with(|m| m.borrow_mut().insert(id, Page { pc, dc, _on_message: on_message, _on_close: on_close }));
        Link { inc, shared, binding, _page: Handle(id) }
    }

    /// Wait (`tries` × 50 ms at most) for `dc` to open: false when it closed or never did.
    async fn opened(dc: impl Fn() -> Option<RtcDataChannelState>, tries: u32) -> bool {
        for _ in 0..tries {
            match dc() {
                Some(RtcDataChannelState::Open) => return true,
                Some(RtcDataChannelState::Closing | RtcDataChannelState::Closed) => return false,
                _ => sleep(50).await,
            }
        }
        false
    }

    /// The relay's HTTP side as this page may reach it ([`super::signal_base`]).
    fn base(relay: &str) -> String {
        let https = web_sys::window().and_then(|w| w.location().protocol().ok()).is_some_and(|p| p == "https:");
        super::signal_base(relay, https)
    }

    /// Join the room of a join code's `session` and `pin` through the rendezvous service `relay` (`host:port`): the
    /// offer goes to its HTTP side, the host's answer comes back, then the channel opens.
    pub async fn connect(relay: &str, session: u64, pin: &[u8]) -> Result<Link, Error> {
        let (pc, base) = (peer(relay)?, base(relay));
        let dc = pc.create_data_channel(LABEL);
        let offer = JsFuture::from(pc.create_offer()).await.map_err(js)?;
        JsFuture::from(pc.set_local_description(&offer.unchecked_into())).await.map_err(js)?;
        let offer = gathered(&pc).await?;
        let (status, answer) = match post(&format!("{base}{}", super::offer_path(session, pin)), offer.as_bytes()).await {
            Ok(r) => r,
            Err(e) => {
                pc.close();
                return Err(e);
            }
        };
        let answer = String::from_utf8_lossy(&answer).into_owned();
        let link = wrap(pc.clone(), dc.clone(), binding(&offer, &answer));
        match status {
            200 => {}
            404 => return Err(Error::Signal("no such room".into())),
            code => return Err(Error::Signal(format!("HTTP {code}"))),
        }
        let desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
        desc.set_sdp(&answer);
        JsFuture::from(pc.set_remote_description(&desc)).await.map_err(js)?;
        if !opened(|| Some(dc.ready_state()), 200).await {
            return Err(Error::Timeout);
        }
        link.shared.open.store(true, Ordering::SeqCst);
        Ok(link)
    }

    struct HostState {
        rendezvous: Rendezvous,
        /// The encoded listing each poll carries (empty: unlisted).
        listing: Vec<u8>,
        /// Joiners' links, handed over once open.
        links: Vec<Link>,
        gone: bool,
    }

    /// A page's room at the relay server (ADR 0047 part 4): it long-polls the service's HTTP side for joiners'
    /// offers, answers each with a connection of its own and hands the channels over once open. Dropped: the polls
    /// stop and the service lets the room expire.
    pub struct PageHost(Rc<RefCell<HostState>>);

    impl PageHost {
        /// Open the room of `session` (the join code's `pin`; `key` only this page knows) at the service `relay`
        /// (`host:port`).
        pub fn open(relay: &str, session: u64, pin: [u8; PIN_BYTES], key: [u8; KEY_BYTES]) -> Self {
            let st = Rc::new(RefCell::new(HostState { rendezvous: Rendezvous::Asking, listing: Vec::new(), links: Vec::new(), gone: false }));
            wasm_bindgen_futures::spawn_local(poll(relay.to_owned(), session, pin, key, st.clone()));
            Self(st)
        }

        pub fn set_listing(&self, listing: Vec<u8>) {
            self.0.borrow_mut().listing = listing;
        }

        /// `Registered` with the page's address as the service sees it (port 0: no port reaches a page).
        pub fn rendezvous(&self) -> Rendezvous {
            self.0.borrow().rendezvous
        }

        /// A joiner's open link.
        pub fn accept(&self) -> Option<Link> {
            let mut s = self.0.borrow_mut();
            s.links.retain(|l| !super::Transport::closed(l));
            let i = s.links.iter().position(Link::is_open)?;
            Some(s.links.swap_remove(i))
        }
    }

    impl Drop for PageHost {
        fn drop(&mut self) {
            self.0.borrow_mut().gone = true;
        }
    }

    async fn poll(relay: String, session: u64, pin: [u8; PIN_BYTES], key: [u8; KEY_BYTES], st: Rc<RefCell<HostState>>) {
        let url = format!("{}{}", base(&relay), super::host_path(session, &pin, &key));
        while !st.borrow().gone {
            let listing = st.borrow().listing.clone();
            let r = post(&url, &listing).await;
            let state = match r.as_ref().map(|(code, b)| (*code, crate::rendezvous::poll_parts(b))) {
                Ok((200, Some((seen, offer)))) => {
                    if let Some((id, sdp)) = offer {
                        let (relay, st) = (relay.clone(), st.clone());
                        wasm_bindgen_futures::spawn_local(async move {
                            if let Err(e) = answer(&relay, session, key, id, &sdp, &st).await {
                                web_sys::console::log_1(&format!("ig2-net: a joiner's offer: {e}").into());
                            }
                        });
                    }
                    st.borrow_mut().rendezvous = Rendezvous::Registered(SocketAddrV4::new(seen, 0));
                    continue;
                }
                Ok((409, _)) => Rendezvous::Refused(Refusal::Taken),
                Ok((429, _)) => Rendezvous::Refused(Refusal::Full),
                // An older service (no page hosts), or none.
                _ => Rendezvous::NoAnswer,
            };
            st.borrow_mut().rendezvous = state;
            sleep(3000).await;
        }
    }

    /// Answer a joiner's `offer` (its id `id`) with a connection of its own; its channel goes to `st` once open.
    async fn answer(relay: &str, session: u64, key: [u8; KEY_BYTES], id: u32, offer: &str, st: &Rc<RefCell<HostState>>) -> Result<(), Error> {
        let (pc, base) = (peer(relay)?, base(relay));
        let desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
        desc.set_sdp(offer);
        JsFuture::from(pc.set_remote_description(&desc)).await.map_err(js)?;
        let answer = JsFuture::from(pc.create_answer()).await.map_err(js)?;
        JsFuture::from(pc.set_local_description(&answer.unchecked_into())).await.map_err(js)?;
        let answer = gathered(&pc).await?;
        // The channel comes after the answer went out: its handlers are set as it comes, before its first message.
        let got: Rc<RefCell<Option<(Link, RtcDataChannel)>>> = Rc::default();
        let (pc2, got2, b) = (pc.clone(), got.clone(), binding(offer, &answer));
        let on_channel = Closure::<dyn FnMut(RtcDataChannelEvent)>::new(move |e: RtcDataChannelEvent| {
            let dc = e.channel();
            *got2.borrow_mut() = Some((wrap(pc2.clone(), dc.clone(), b), dc));
        });
        pc.set_ondatachannel(Some(on_channel.as_ref().unchecked_ref()));
        let sent = post(&format!("{base}{}", super::answer_path(session, &key, id)), answer.as_bytes()).await;
        let open = matches!(sent, Ok((200, _))) && opened(|| got.borrow().as_ref().map(|(_, dc)| dc.ready_state()), 300).await;
        pc.set_ondatachannel(None);
        drop(on_channel);
        let Some((link, _)) = got.borrow_mut().take().filter(|_| open) else {
            pc.close();
            return Err(match sent {
                Ok((200, _)) => Error::Timeout,
                Ok((code, _)) => Error::Signal(format!("HTTP {code}")),
                Err(e) => e,
            });
        };
        link.shared.open.store(true, Ordering::SeqCst);
        if !st.borrow().gone {
            st.borrow_mut().links.push(link);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_join_back() {
        for n in [0, 1, CHUNK - 2, CHUNK - 1, CHUNK, 3 * CHUNK + 5] {
            let b: Vec<u8> = (0..n).map(|i| i as u8).collect();
            let p = pieces(&b);
            assert_eq!(p.len(), n.div_ceil(CHUNK - 1).max(1));
            assert!(p.iter().all(|x| x.len() <= CHUNK));
            let mut j = Joiner::default();
            let got: Vec<_> = p.iter().filter_map(|x| j.push(x).unwrap()).collect();
            assert_eq!(got, vec![b]);
        }
        assert!(Joiner::default().push(&[]).is_err());
    }

    #[test]
    fn a_probe_comes_back_as_its_echo_and_gives_the_round_trip() {
        let (a, b) = (Shared::default(), Shared::default());
        assert_eq!(a.ping_due(), None, "not before the channel is open");
        a.open.store(true, std::sync::atomic::Ordering::SeqCst);
        let ping = a.ping_due().unwrap();
        assert_eq!(a.ping_due(), None, "one per PING_EVERY");
        let Piece::Echo(pong) = b.take(&ping) else { panic!() };
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert_eq!(a.take(&pong), Piece::Probe);
        let rtt = a.rtt.load(std::sync::atomic::Ordering::SeqCst);
        assert!((5_000..1_000_000).contains(&rtt), "{rtt}");
        assert_eq!(b.take(&pieces(b"x")[0]), Piece::Message);
    }

    #[test]
    fn the_binding_covers_both_fingerprints() {
        let offer = "v=0\r\na=fingerprint:sha-256 AA:BB\r\n";
        let answer = "v=0\r\na=fingerprint:sha-256 cc:dd\r\n";
        assert_eq!(binding(offer, answer), binding(&offer.replace("AA:BB", "aa:bb"), answer));
        assert_ne!(binding(offer, answer), binding(offer, "a=fingerprint:sha-256 CC:DE\r\n"));
        assert_ne!(binding(offer, answer), binding(answer, offer));
        assert_eq!(offer_path(0x5eed, &[1, 0xab]), "/offer/0000000000005eed/01ab");
        assert_eq!(signal_base("relay.example:30000", false), "http://relay.example:30000");
        assert_eq!(signal_base("relay.example:30000", true), "https://relay.example");
    }
}
