//! Port mapping against fake gateways on 127.0.0.1 (never the real router): NAT-PMP mapping, renewal, removal on
//! drop, result codes, timeouts, answers from the wrong address; UPnP (feature `upnp`) against a fake IGD control URL.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ig2_net::portmap::{Error, Mapper, Mapping, NatPmp, Protocol};

const EXTERNAL: Ipv4Addr = Ipv4Addr::new(203, 0, 113, 7);

/// A fake NAT-PMP gateway: answers opcode 0 with `EXTERNAL`, opcode 1 with internal port + 1000 (a lifetime-0
/// request deletes) and the configured lifetime, or every request with result `code`.
struct FakeGateway {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    stop: Arc<AtomicBool>,
}

impl FakeGateway {
    fn start(lifetime: u32, code: u16) -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.set_read_timeout(Some(Duration::from_millis(10))).unwrap();
        let addr = socket.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (r, s) = (requests.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut buf = [0u8; 64];
            while !s.load(Ordering::Relaxed) {
                let Ok((n, from)) = socket.recv_from(&mut buf) else { continue };
                let req = buf[..n].to_vec();
                r.lock().unwrap().push(req.clone());
                let mut a = vec![0, 128 + req[1]];
                a.extend_from_slice(&code.to_be_bytes());
                a.extend_from_slice(&1234u32.to_be_bytes()); // seconds since epoch
                match req[1] {
                    0 => a.extend_from_slice(&EXTERNAL.octets()),
                    1 => {
                        let internal = u16::from_be_bytes([req[4], req[5]]);
                        let life = u32::from_be_bytes([req[8], req[9], req[10], req[11]]);
                        a.extend_from_slice(&req[4..6]);
                        let ext = if life == 0 { 0 } else { internal + 1000 };
                        a.extend_from_slice(&ext.to_be_bytes());
                        a.extend_from_slice(&(if life == 0 { 0 } else { lifetime }).to_be_bytes());
                    }
                    _ => continue,
                }
                socket.send_to(&a, from).unwrap();
            }
        });
        Self { addr, requests, stop }
    }

    fn requests(&self) -> Vec<Vec<u8>> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for FakeGateway {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn internal() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 40123)
}

fn udp_requests(reqs: &[Vec<u8>]) -> Vec<(u16, u16, u32)> {
    reqs.iter()
        .filter(|r| r[1] == 1)
        .map(|r| (u16::from_be_bytes([r[4], r[5]]), u16::from_be_bytes([r[6], r[7]]), u32::from_be_bytes([r[8], r[9], r[10], r[11]])))
        .collect()
}

#[test]
fn natpmp_maps_renews_and_removes_on_drop() {
    let gw = FakeGateway::start(1, 0);
    let m = Mapping::open(Box::new(NatPmp::new(gw.addr).unwrap()), internal(), Duration::from_secs(7200)).unwrap();
    let g = m.granted();
    assert_eq!(g.protocol, Protocol::NatPmp);
    assert_eq!(g.external, SocketAddrV4::new(EXTERNAL, 41123));
    assert_eq!(g.lifetime, Duration::from_secs(1));
    // Renewed at half the granted lifetime, asking for the port the router gave.
    let start = Instant::now();
    while udp_requests(&gw.requests()).len() < 3 {
        assert!(start.elapsed() < Duration::from_secs(5), "no renewal");
        std::thread::sleep(Duration::from_millis(10));
    }
    let udp = udp_requests(&gw.requests());
    assert_eq!(udp[0], (40123, 40123, 7200));
    assert_eq!(udp[1], (40123, 41123, 7200));
    drop(m);
    let udp = udp_requests(&gw.requests());
    assert_eq!(*udp.last().unwrap(), (40123, 0, 0), "deleted on drop");
}

#[test]
fn natpmp_close_reports_and_errors_surface() {
    let gw = FakeGateway::start(3600, 0);
    let m = Mapping::open(Box::new(NatPmp::new(gw.addr).unwrap()), internal(), Duration::from_secs(60)).unwrap();
    m.close().unwrap();
    assert_eq!(*udp_requests(&gw.requests()).last().unwrap(), (40123, 0, 0));
    // A refusal (result 2: not authorized).
    let refusing = FakeGateway::start(3600, 2);
    let mut c = NatPmp::new(refusing.addr).unwrap();
    assert!(matches!(c.map(internal(), 40123, Duration::from_secs(60)), Err(Error::NatPmp(2))));
    assert!(Mapping::open(Box::new(NatPmp::new(refusing.addr).unwrap()), internal(), Duration::from_secs(60)).is_err());
}

#[test]
fn natpmp_times_out_and_ignores_other_senders() {
    // A gateway that never answers; a stranger answers instead (from the wrong address).
    let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut c = NatPmp::new(silent.local_addr().unwrap()).unwrap();
    c.timeout = Duration::from_millis(30);
    c.tries = 2;
    let stranger = UdpSocket::bind("127.0.0.1:0").unwrap();
    let t = std::thread::spawn(move || {
        let mut buf = [0u8; 64];
        silent.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let (_, from) = silent.recv_from(&mut buf).unwrap();
        let mut a = vec![0, 128, 0, 0, 0, 0, 0, 1];
        a.extend_from_slice(&EXTERNAL.octets());
        stranger.send_to(&a, from).unwrap();
        // Stay bound: a closed port would make Windows report the retry as a reset.
        std::thread::sleep(Duration::from_millis(300));
    });
    let start = Instant::now();
    assert!(matches!(c.external_ip(), Err(Error::Timeout)));
    assert!(start.elapsed() >= Duration::from_millis(90));
    t.join().unwrap();
}

#[cfg(feature = "upnp")]
mod upnp {
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    use super::*;
    use ig2_net::portmap::Upnp;

    type Seen = Arc<Mutex<Vec<(String, String)>>>;

    /// A fake IGD control endpoint: records `(SOAPAction, body)` and answers each action.
    fn fake_igd() -> (SocketAddr, Seen) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s = seen.clone();
        std::thread::spawn(move || {
            for stream in l.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let (mut action, mut len) = (String::new(), 0usize);
                loop {
                    let mut line = String::new();
                    r.read_line(&mut line).unwrap();
                    let line = line.trim_end();
                    if line.is_empty() {
                        break;
                    }
                    let lower = line.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                    if lower.starts_with("soapaction:") {
                        action = line[11..].trim().trim_matches('"').to_string();
                    }
                }
                let mut body = vec![0; len];
                r.read_exact(&mut body).unwrap();
                s.lock().unwrap().push((action.clone(), String::from_utf8(body).unwrap()));
                let inner = if action.ends_with("#GetExternalIPAddress") {
                    format!("<u:GetExternalIPAddressResponse xmlns:u=\"urn:schemas-upnp-org:service:WANIPConnection:1\"><NewExternalIPAddress>{EXTERNAL}</NewExternalIPAddress></u:GetExternalIPAddressResponse>")
                } else if action.ends_with("#AddPortMapping") {
                    "<u:AddPortMappingResponse xmlns:u=\"urn:schemas-upnp-org:service:WANIPConnection:1\"/>".into()
                } else {
                    "<u:DeletePortMappingResponse xmlns:u=\"urn:schemas-upnp-org:service:WANIPConnection:1\"/>".into()
                };
                let xml = format!("<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"><s:Body>{inner}</s:Body></s:Envelope>");
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{xml}", xml.len());
                stream.write_all(resp.as_bytes()).unwrap();
            }
        });
        (addr, seen)
    }

    fn schema() -> HashMap<String, Vec<String>> {
        let add = ["NewRemoteHost", "NewExternalPort", "NewProtocol", "NewInternalPort", "NewInternalClient", "NewEnabled", "NewPortMappingDescription", "NewLeaseDuration"];
        HashMap::from([
            ("AddPortMapping".to_string(), add.iter().map(|s| s.to_string()).collect()),
            ("DeletePortMapping".to_string(), ["NewRemoteHost", "NewExternalPort", "NewProtocol"].iter().map(|s| s.to_string()).collect()),
        ])
    }

    #[test]
    fn upnp_maps_and_removes() {
        let (addr, seen) = fake_igd();
        let gateway = igd_next::Gateway { addr, root_url: "/".into(), control_url: "/ctl".into(), control_schema_url: "/scpd".into(), control_schema: schema() };
        let m = Mapping::open(Box::new(Upnp { gateway }), internal(), Duration::from_secs(7200)).unwrap();
        assert_eq!(m.granted().external, SocketAddrV4::new(EXTERNAL, 40123));
        assert_eq!(m.granted().protocol, Protocol::Upnp);
        m.close().unwrap();
        let seen = seen.lock().unwrap();
        let actions: Vec<_> = seen.iter().map(|(a, _)| a.rsplit('#').next().unwrap().to_string()).collect();
        assert_eq!(actions, ["AddPortMapping", "GetExternalIPAddress", "DeletePortMapping"]);
        let add = &seen[0].1;
        for want in ["<NewExternalPort>40123</NewExternalPort>", "<NewProtocol>UDP</NewProtocol>", "<NewInternalClient>192.168.1.20</NewInternalClient>", "<NewLeaseDuration>7200</NewLeaseDuration>", "<NewPortMappingDescription>IG2 session</NewPortMappingDescription>"] {
            assert!(add.contains(want), "{want} in {add}");
        }
        assert!(seen[2].1.contains("<NewExternalPort>40123</NewExternalPort>"));
    }
}
