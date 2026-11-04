//! The WebRTC transport (ADR 0047 part 4): a browser joins a host over one reliable, ordered data channel, signalled
//! through the rendezvous service (`rendezvous::serve_with_http`: the page POSTs its offer, the host's QUIC socket
//! gets it as `Offer` and answers). Natively (feature `webrtc`) the host answers with `str0m` on a UDP socket of its
//! own ([`answer`]); a native joiner can offer the same way ([`connect`]). In the browser [`connect`] is the page's
//! `RTCPeerConnection`.
//!
//! Messages larger than one data-channel message travel in [`CHUNK`]-byte pieces, each led by a byte that says whether
//! more follow. The password binding (ADR 0008 §3.1) is a hash of both ends' DTLS fingerprints from the offer and
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

/// The HTTP path the page posts its offer to: the join code's session and pin in hex.
pub fn offer_path(session: u64, pin: &[u8]) -> String {
    let pin: String = pin.iter().map(|b| format!("{b:02x}")).collect();
    format!("/offer/{session:016x}/{pin}")
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
#[derive(Default)]
struct Shared {
    open: std::sync::atomic::AtomicBool,
    closed: std::sync::atomic::AtomicBool,
    remote: std::sync::Mutex<Option<std::net::IpAddr>>,
}

/// One end of a data channel as a [`Transport`] (`Send`: the channel itself lives on the driver's thread natively,
/// on the page in the browser).
pub struct Link {
    #[cfg(not(target_arch = "wasm32"))]
    out: std::sync::mpsc::Sender<Vec<u8>>,
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
        let _ = self.out.send(b);
    }

    fn recv(&mut self) -> Result<Option<Msg>, wire::Error> {
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

    use super::{binding, pieces, Error, Joiner, Link, Shared, LABEL};
    use crate::transport::Transport;

    /// How long a link may take to open.
    const OPENING: Duration = Duration::from_secs(15);
    /// The driver's longest sleep (queued sends wait at most this long).
    const TICK: Duration = Duration::from_millis(4);

    fn rtc_err(e: impl std::fmt::Display) -> Error {
        Error::Rtc(e.to_string())
    }

    /// A socket of its own and host candidates for this machine's loopback and LAN addresses.
    fn bind(rtc: &mut Rtc) -> Result<(UdpSocket, u16, Option<Ipv4Addr>), Error> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        let port = socket.local_addr()?.port();
        let lan = crate::portmap::local_ipv4(Ipv4Addr::new(192, 0, 2, 1)).ok().filter(|ip| !ip.is_loopback());
        for ip in [Some(Ipv4Addr::LOCALHOST), lan].into_iter().flatten() {
            rtc.add_local_candidate(Candidate::host(SocketAddr::from((ip, port)), "udp").map_err(rtc_err)?);
        }
        Ok((socket, port, lan))
    }

    fn rtc() -> Rtc {
        Rtc::builder().build(Instant::now())
    }

    fn link(shared: Arc<Shared>, offer: &str, answer: &str) -> (Link, Receiver<Vec<u8>>, Sender<Vec<u8>>) {
        let (out, out_rx) = mpsc::channel();
        let (inc_tx, inc) = mpsc::channel();
        (Link { out, inc, shared, binding: binding(offer, answer) }, out_rx, inc_tx)
    }

    /// Answer a browser's `offer` (its SDP): the answer's SDP for the service, and the link, open once the browser
    /// connected (`Link::is_open`; closed when it never does).
    pub fn answer(offer: &str) -> Result<(String, Link), Error> {
        let mut rtc = rtc();
        let (socket, port, lan) = bind(&mut rtc)?;
        let parsed = SdpOffer::from_sdp_string(offer).map_err(rtc_err)?;
        let answer = rtc.sdp_api().accept_offer(parsed).map_err(rtc_err)?.to_sdp_string();
        let shared = Arc::new(Shared::default());
        let (l, out_rx, inc_tx) = link(shared.clone(), offer, &answer);
        std::thread::spawn(move || drive(rtc, socket, port, lan, None, shared, out_rx, inc_tx));
        Ok((answer, l))
    }

    /// Join the room of a join code's `session` and `pin` through the rendezvous service at `service` (its HTTP
    /// signalling, as a browser does): blocks until the channel is open.
    pub fn connect(service: SocketAddrV4, session: u64, pin: &[u8]) -> Result<Link, Error> {
        let mut rtc = rtc();
        let (socket, port, lan) = bind(&mut rtc)?;
        let mut change = rtc.sdp_api();
        let cid = change.add_channel(LABEL.into());
        let (offer, pending) = change.apply().ok_or_else(|| rtc_err("no offer"))?;
        let offer = offer.to_sdp_string();
        let answer = post(service, &super::offer_path(session, pin), &offer)?;
        rtc.sdp_api().accept_answer(pending, SdpAnswer::from_sdp_string(&answer).map_err(rtc_err)?).map_err(rtc_err)?;
        let shared = Arc::new(Shared::default());
        let (l, out_rx, inc_tx) = link(shared.clone(), &offer, &answer);
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
                        Event::ChannelData(d) => match joiner.push(&d.data) {
                            Ok(Some(m)) => {
                                let _ = inc.send(m);
                            }
                            Ok(None) => {}
                            Err(e) => return Err(rtc_err(e)),
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
pub use web::connect;

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::sync::Arc;

    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{MessageEvent, RtcDataChannel, RtcDataChannelState, RtcDataChannelType, RtcPeerConnection, RtcSdpType, RtcSessionDescriptionInit};

    use super::{binding, pieces, Error, Joiner, Link, Shared, LABEL};

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

    /// Join the room of a join code's `session` and `pin` through the rendezvous service whose HTTP side is `base`
    /// (`http://host:port`): the offer goes there, the host's answer comes back, then the channel opens.
    pub async fn connect(base: &str, session: u64, pin: &[u8]) -> Result<Link, Error> {
        let pc = RtcPeerConnection::new().map_err(js)?;
        let dc = pc.create_data_channel(LABEL);
        dc.set_binary_type(RtcDataChannelType::Arraybuffer);
        let offer = JsFuture::from(pc.create_offer()).await.map_err(js)?;
        let offer: RtcSessionDescriptionInit = offer.unchecked_into();
        JsFuture::from(pc.set_local_description(&offer)).await.map_err(js)?;
        // The whole offer at once (no trickling): wait for the candidates, two seconds at most.
        for _ in 0..40 {
            if pc.ice_gathering_state() == web_sys::RtcIceGatheringState::Complete {
                break;
            }
            sleep(50).await;
        }
        let offer = pc.local_description().ok_or_else(|| Error::Rtc("no offer".into()))?.sdp();
        let init = web_sys::RequestInit::new();
        init.set_method("POST");
        init.set_body(&JsValue::from_str(&offer));
        let url = format!("{base}{}", super::offer_path(session, pin));
        let window = web_sys::window().expect("a window");
        let resp: web_sys::Response = JsFuture::from(window.fetch_with_str_and_init(&url, &init)).await.map_err(|e| Error::Signal(js(e).to_string()))?.unchecked_into();
        match resp.status() {
            200 => {}
            404 => return Err(Error::Signal("no such room".into())),
            code => return Err(Error::Signal(format!("HTTP {code}"))),
        }
        let answer = JsFuture::from(resp.text().map_err(js)?).await.map_err(js)?.as_string().unwrap_or_default();
        let desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
        desc.set_sdp(&answer);
        JsFuture::from(pc.set_remote_description(&desc)).await.map_err(js)?;
        for _ in 0..200 {
            match dc.ready_state() {
                RtcDataChannelState::Open => break,
                RtcDataChannelState::Closing | RtcDataChannelState::Closed => return Err(Error::Timeout),
                _ => sleep(50).await,
            }
        }
        if dc.ready_state() != RtcDataChannelState::Open {
            pc.close();
            return Err(Error::Timeout);
        }
        let shared = Arc::new(Shared::default());
        shared.open.store(true, Ordering::SeqCst);
        let (inc_tx, inc) = mpsc::channel();
        let mut joiner = Joiner::default();
        let s = shared.clone();
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
            let b = js_sys::Uint8Array::new(&e.data()).to_vec();
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
        Ok(Link { inc, shared, binding: binding(&offer, &answer), _page: Handle(id) })
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
    fn the_binding_covers_both_fingerprints() {
        let offer = "v=0\r\na=fingerprint:sha-256 AA:BB\r\n";
        let answer = "v=0\r\na=fingerprint:sha-256 cc:dd\r\n";
        assert_eq!(binding(offer, answer), binding(&offer.replace("AA:BB", "aa:bb"), answer));
        assert_ne!(binding(offer, answer), binding(offer, "a=fingerprint:sha-256 CC:DE\r\n"));
        assert_ne!(binding(offer, answer), binding(answer, offer));
        assert_eq!(offer_path(0x5eed, &[1, 0xab]), "/offer/0000000000005eed/01ab");
    }
}
