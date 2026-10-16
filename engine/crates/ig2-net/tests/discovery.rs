//! LAN discovery on 127.0.0.1 (no broadcast leaves the machine): answers, private sessions, malformed and stray
//! packets, the LAN filter.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use ig2_net::discovery::{answer_packet, is_lan, query_packet, Announcement, Announcer, Browser, MAX_ANSWER, QUERY_LEN};

fn localhost() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

fn announcement(session: u64, name: &str) -> Announcement {
    Announcement {
        session,
        name: name.into(),
        port: 40000 + session as u16,
        fingerprint: [session as u8; 32],
        password: session % 2 == 0,
        version: 1,
        rules: [7; 32],
        scenario: "quick".into(),
        humans: 1,
        slots: 4,
        started: false,
    }
}

/// Pump the announcers and the browser until `done` holds (up to 5 s).
fn pump(browser: &mut Browser, announcers: &mut [&mut Announcer], mut done: impl FnMut(&Browser) -> bool) {
    let start = Instant::now();
    browser.query().unwrap();
    loop {
        for a in announcers.iter_mut() {
            a.poll().unwrap();
        }
        browser.poll().unwrap();
        if done(browser) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn hosts_answer_and_private_ones_stay_silent() {
    let mut open = Announcer::bind(localhost()).unwrap();
    let mut locked = Announcer::bind(localhost()).unwrap();
    let mut private = Announcer::bind(localhost()).unwrap();
    open.set(Some(announcement(1, "Open session")));
    locked.set(Some(announcement(2, "Locked")));
    private.set(None);
    let targets = vec![open.local_addr().unwrap(), locked.local_addr().unwrap(), private.local_addr().unwrap()];
    let mut b = Browser::with_targets(localhost(), targets).unwrap();
    pump(&mut b, &mut [&mut open, &mut locked, &mut private], |b| b.sessions().count() == 2);
    // Give a late answer from the private host the chance to show up.
    std::thread::sleep(Duration::from_millis(50));
    private.poll().unwrap();
    b.poll().unwrap();
    let mut got: Vec<_> = b.sessions().cloned().collect();
    got.sort_by_key(|f| f.announcement.session);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].announcement, announcement(1, "Open session"));
    assert_eq!(got[0].addr, SocketAddr::new("127.0.0.1".parse().unwrap(), 40001));
    assert_eq!(got[1].announcement, announcement(2, "Locked"));
    assert!(got[1].announcement.password);
}

#[test]
fn names_are_truncated_and_answers_never_outgrow_the_query() {
    let mut a = announcement(3, &"x".repeat(500));
    a.scenario = "é".repeat(100);
    let p = answer_packet(9, &a).unwrap();
    assert!(p.len() <= MAX_ANSWER && p.len() < QUERY_LEN);
    let mut host = Announcer::bind(localhost()).unwrap();
    host.set(Some(a));
    let mut b = Browser::with_targets(localhost(), vec![host.local_addr().unwrap()]).unwrap();
    pump(&mut b, &mut [&mut host], |b| b.sessions().count() == 1);
    let f = b.sessions().next().unwrap();
    assert_eq!(f.announcement.name, "x".repeat(32));
    assert_eq!(f.announcement.scenario, "é".repeat(32));
}

#[test]
fn malformed_queries_and_stray_answers_are_ignored() {
    let mut host = Announcer::bind(localhost()).unwrap();
    host.set(Some(announcement(5, "Host")));
    let raw = UdpSocket::bind(localhost()).unwrap();
    raw.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let to = host.local_addr().unwrap();
    let good = query_packet(77);
    // Too short (no padding), too long, padding not zero, wrong version.
    raw.send_to(&good[..14], to).unwrap();
    raw.send_to(&[good.as_slice(), &[0]].concat(), to).unwrap();
    let mut dirty = good;
    dirty[200] = 1;
    raw.send_to(&dirty, to).unwrap();
    let mut old = good;
    old[4] = 0;
    raw.send_to(&old, to).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(host.poll().unwrap(), 0);
    // A good one is answered with the echoed nonce.
    raw.send_to(&good, to).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(host.poll().unwrap(), 1);
    let mut buf = [0u8; 512];
    let (n, _) = raw.recv_from(&mut buf).unwrap();
    assert_eq!(&buf[6..14], &77u64.to_le_bytes());
    assert_eq!(buf[..n], answer_packet(77, &announcement(5, "Host")).unwrap()[..]);

    // A browser drops an answer to a nonce it never sent.
    let probe = UdpSocket::bind(localhost()).unwrap();
    probe.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let mut b = Browser::with_targets(localhost(), vec![probe.local_addr().unwrap()]).unwrap();
    b.query().unwrap();
    let (_, from) = probe.recv_from(&mut buf).unwrap();
    probe.send_to(&answer_packet(1234, &announcement(6, "Stray")).unwrap(), from).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    b.poll().unwrap();
    assert_eq!(b.sessions().count(), 0);
}

#[test]
fn only_lan_addresses_count() {
    for lan in ["127.0.0.1", "10.1.2.3", "172.16.0.9", "172.31.255.1", "192.168.1.20", "169.254.3.4", "::1", "fe80::1", "fd12:3456::1", "::ffff:192.168.0.2"] {
        assert!(is_lan(lan.parse::<IpAddr>().unwrap()), "{lan}");
    }
    for wan in ["8.8.8.8", "172.32.0.1", "100.64.0.1", "2001:db8::1", "2a00:1450::1", "::ffff:8.8.8.8", "255.255.255.255"] {
        assert!(!is_lan(wan.parse::<IpAddr>().unwrap()), "{wan}");
    }
}
