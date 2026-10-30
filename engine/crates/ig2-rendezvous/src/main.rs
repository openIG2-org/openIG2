//! `ig2-rendezvous`: the rendezvous and relay service for internet multiplayer (ADR 0018, `docs/RENDEZVOUS.md`).
//! Rooms by join code, hole-punch help and a QUIC datagram relay on one UDP port; no game logic, no accounts,
//! nothing written to disk. Prints its counts now and then, never addresses.

use std::time::{Duration, Instant};

use ig2_net::rendezvous::{serve, Limits, Service, DEFAULT_PORT};

const USAGE: &str = "usage: ig2-rendezvous [--listen ADDR:PORT] [--max-rooms N] [--rooms-per-ip N] [--peers-per-room N] [--room-kib-per-sec N] [--control-per-sec N] [--status-secs N]";

fn main() {
    if let Err(e) = run() {
        eprintln!("ig2-rendezvous: {e}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut listen = format!("0.0.0.0:{DEFAULT_PORT}");
    let mut l = Limits::default();
    let mut status = 60u64;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().ok_or_else(|| format!("{a} needs a value\n{USAGE}"));
        let num = |v: String| v.parse::<u64>().map_err(|e| format!("{a}: {e}"));
        match a.as_str() {
            "--listen" => listen = val()?,
            "--max-rooms" => l.max_rooms = num(val()?)? as usize,
            "--rooms-per-ip" => l.rooms_per_ip = num(val()?)? as usize,
            "--peers-per-room" => l.peers_per_room = num(val()?)? as usize,
            "--room-kib-per-sec" => {
                l.room_rate = num(val()?)? as f64 * 1024.0;
                l.room_burst = l.room_rate * 4.0;
            }
            "--control-per-sec" => {
                l.control_rate = num(val()?)? as f64;
                l.control_burst = l.control_rate * 2.0;
            }
            "--status-secs" => status = num(val()?)?,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => return Err(format!("unknown argument {a}\n{USAGE}")),
        }
    }
    let socket = std::net::UdpSocket::bind(&listen).map_err(|e| format!("{listen}: {e}"))?;
    println!("ig2-rendezvous: listening on {} (UDP), {l:?}", socket.local_addr().map_err(|e| e.to_string())?);
    let mut last = Instant::now();
    serve(&socket, &mut Service::new(l), |s| {
        if status > 0 && last.elapsed() >= Duration::from_secs(status) {
            last = Instant::now();
            let c = s.counts;
            println!("ig2-rendezvous: {} rooms, {} packets / {} bytes relayed, {} dropped", s.rooms(), c.relayed_packets, c.relayed_bytes, c.dropped);
        }
        false
    })
    .map_err(|e| e.to_string())
}
