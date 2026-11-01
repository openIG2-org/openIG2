//! Order latency in the lockstep (ADR 0063): how long a local order takes to run on the issuing peer, on loopback and
//! over links with a simulated round trip, as the client paces its turns (a turn every two 50 ms ticks, messages
//! every 10 ms frame, a turn caught up when behind); and the adaptive input delay each peer picks from its own round
//! trip, with peers on different delays staying in step.

#[allow(dead_code)]
mod common;

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use ig2_net::{loopback, wire, Command, Host, Loopback, Msg, Peer, Transport, LOWER_AFTER};
use ig2_sim::game::Game;

/// A loopback end whose messages arrive `oneway` ms after they were sent, on a shared virtual clock.
struct Lag {
    inner: Loopback,
    now: Rc<Cell<u64>>,
    oneway: Rc<Cell<u64>>,
    queue: VecDeque<(u64, Msg)>,
}

impl Transport for Lag {
    fn send(&mut self, msg: &Msg) {
        self.inner.send(msg);
    }

    fn recv(&mut self) -> Result<Option<Msg>, wire::Error> {
        while let Some(m) = self.inner.recv()? {
            self.queue.push_back((self.now.get() + self.oneway.get(), m));
        }
        Ok(match self.queue.front() {
            Some(&(at, _)) if at <= self.now.get() => self.queue.pop_front().map(|(_, m)| m),
            _ => None,
        })
    }

    fn rtt(&self) -> Option<Duration> {
        Some(Duration::from_millis(2 * self.oneway.get()))
    }
}

struct Net {
    now: Rc<Cell<u64>>,
    host: Host,
    /// Each peer, its link, its one-way time and its ticks since its last turn.
    peers: Vec<(Peer<'static>, Lag, Rc<Cell<u64>>, u32)>,
}

impl Net {
    /// A host and a peer per one-way time (ms), all starting at input delay `delay`.
    fn new(oneway: &[u64], delay: u64) -> Option<Self> {
        let (rules, state) = common::setup("quick-1.sav")?;
        let rules = Box::leak(Box::new(rules));
        let now = Rc::new(Cell::new(0));
        let mut host = Host::new(delay);
        let mut peers = Vec::new();
        for (i, &ms) in oneway.iter().enumerate() {
            let (slot, empire) = (i as u8 + 1, [6u8, 3, 4][i]);
            let (a, b) = loopback();
            let lag = |inner, o: &Rc<Cell<u64>>| Lag { inner, now: now.clone(), oneway: o.clone(), queue: VecDeque::new() };
            let o = Rc::new(Cell::new(ms));
            host.add_peer(slot, empire, Box::new(lag(a, &o)));
            let mut g = Game::new(state.clone(), rules);
            g.saved();
            peers.push((Peer::new(slot, empire, g, delay), lag(b, &o), o, 0));
        }
        Some(Self { now, host, peers })
    }

    /// One 10 ms frame: the host relays, each peer adapts and moves its messages, and on every fifth frame (a 50 ms
    /// tick) runs a turn every second tick or when behind (`World::mp_tick`).
    fn frame(&mut self) {
        let t = self.now.get() + 10;
        self.now.set(t);
        self.host.pump().unwrap();
        for (p, link, _, since) in &mut self.peers {
            let rtt = link.rtt().unwrap();
            p.adapt(rtt);
            let tick = t % 50 == 0;
            let due = tick && (*since + 1 >= ig2_net::TURN_TICKS || p.behind());
            let ran = p.poll(link, usize::from(due)).unwrap();
            if tick {
                *since = if ran > 0 { 0 } else { *since + 1 };
            }
        }
    }

    /// The ms from peer `i` issuing an order to the turn that runs it on that peer, the order given `at` ms into a
    /// turn.
    fn latency(&mut self, i: usize, at: u64) -> u64 {
        let turn = self.peers[i].0.turn;
        while self.peers[i].0.turn == turn {
            self.frame();
        }
        for _ in 0..at / 10 {
            self.frame();
        }
        let from = self.now.get();
        self.peers[i].0.issue(Command::StoryChoice { id: 10031 });
        while self.peers[i].0.in_flight().next().is_some() {
            self.frame();
            assert!(self.now.get() - from < 5000, "the order never ran");
        }
        self.now.get() - from
    }

    fn run(&mut self, ms: u64) {
        for _ in 0..ms / 10 {
            self.frame();
        }
    }

    fn assert_in_step(&self) {
        assert!(self.host.desyncs.is_empty(), "{:?}", self.host.desyncs);
        let h0 = &self.peers[0].0.hashes;
        for (p, ..) in &self.peers[1..] {
            let common = p.hashes.iter().filter(|x| h0.contains(x)).count();
            assert!(common >= 20, "{common} turns in common");
            assert!(p.hashes.iter().all(|(t, x)| h0.iter().find(|(t0, _)| t0 == t).is_none_or(|(_, x0)| x0 == x)));
        }
    }
}

/// The worst and mean latency of orders given at 0, 20, …, 80 ms into a turn.
fn measure(net: &mut Net, i: usize) -> (u64, u64) {
    let v: Vec<u64> = (0..5).map(|k| net.latency(i, k * 20)).collect();
    (*v.iter().max().unwrap(), v.iter().sum::<u64>() / v.len() as u64)
}

#[test]
fn order_latency_on_loopback_and_over_a_round_trip() {
    let Some(mut net) = Net::new(&[0, 0], 0) else { return };
    net.run(1000);
    let (worst, mean) = measure(&mut net, 0);
    eprintln!("loopback (delay {}): worst {worst} ms, mean {mean} ms", net.peers[0].0.delay());
    // The order goes out with the next turn's input and runs a turn later: one to two turns.
    assert!(worst <= 200, "loopback worst {worst} ms");
    net.assert_in_step();

    for (oneway, delay) in [(25, 0), (50, 1), (100, 2)] {
        let Some(mut net) = Net::new(&[oneway, 0], 0) else { return };
        net.run(3000);
        assert_eq!(net.peers[0].0.delay(), delay, "round trip {} ms", 2 * oneway);
        let (worst, mean) = measure(&mut net, 0);
        eprintln!("round trip {} ms (delay {delay}): worst {worst} ms, mean {mean} ms", 2 * oneway);
        // The delay's turns, the turn it goes out with and the bundle's way back.
        assert!(worst <= (delay + 2) * 100 + oneway + 50, "round trip {} ms: worst {worst} ms", 2 * oneway);
        net.assert_in_step();
    }
}

#[test]
fn the_input_delay_follows_each_peers_round_trip() {
    // A room that started at delay 5: on a LAN the peers come down to 0, a turn per LOWER_AFTER turns.
    let Some(mut net) = Net::new(&[1, 1], 5) else { return };
    let turns = 5 * LOWER_AFTER + 10;
    net.run(turns * 100 + 1000);
    assert!(net.peers.iter().all(|p| p.0.delay() == 0), "{:?}", net.peers.iter().map(|p| p.0.delay()).collect::<Vec<_>>());
    // Peer 1's link slows to a 300 ms round trip: its delay rises within a second while peer 2's stays at 0; then
    // it recovers and the delay comes down again. Every peer runs the same turns throughout.
    net.peers[0].2.set(150);
    net.run(1000);
    assert!(net.peers[0].0.delay() >= 2 && net.peers[1].0.delay() == 0, "{} {}", net.peers[0].0.delay(), net.peers[1].0.delay());
    // The jump's jitter raises it past what the round trip alone needs; once steady it settles there.
    net.run(2 * LOWER_AFTER * 100 + 3000);
    assert_eq!(net.peers[0].0.delay(), 3);
    net.peers[0].2.set(1);
    let from = net.peers[0].0.turn;
    let mut seen = Vec::new();
    while net.peers[0].0.delay() > 0 {
        net.run(100);
        seen.push(net.peers[0].0.delay());
        assert!(net.peers[0].0.turn - from < 6 * LOWER_AFTER, "{seen:?}");
    }
    eprintln!("back to delay 0 after {} turns", net.peers[0].0.turn - from);
    for (i, k) in [(0, 1), (1, 2)] {
        for _ in 0..k {
            net.latency(i, 30);
        }
    }
    net.run(1000);
    net.assert_in_step();
    assert_eq!(net.host.log.iter().map(|b| b.commands.len()).sum::<usize>(), 3, "no order lost");
}
