//! The lobby over loopback links (ADR 0008 §3): admission checks, the SPAKE2 password check and its channel
//! binding, private sessions, slots, ready flags, chat, kick, and the start into lockstep on a real save.

mod common;

use std::time::{Duration, Instant};

use ig2_net::lobby::{LobbyClient, LobbyEvent, LobbyHost, LobbyMsg, Refusal, Settings, SlotKind, Status, LOBBY_VERSION};
use ig2_net::{loopback, Command, Loopback, Msg, Snapshot, Transport};
use ig2_sim::game::Game;

const RULES: [u8; 32] = [0x5a; 32];
const SLOTS: [(u8, u8); 3] = [(1, 0), (2, 3), (6, 5)];

fn settings(private: bool, password: Option<&str>) -> Settings {
    Settings { name: "Test session".into(), scenario: "quick".into(), private, password: password.map(Into::into) }
}

fn host(private: bool, password: Option<&str>) -> LobbyHost {
    LobbyHost::new(settings(private, password), 0xC0DE, RULES, &SLOTS, "Host")
}

/// A joiner on a fresh loopback link.
fn join(h: &mut LobbyHost, name: &str, password: Option<&str>, code: Option<u64>) -> LobbyClient {
    let (a, b) = loopback();
    h.admit(Box::new(b));
    LobbyClient::join(Box::new(a), name, RULES, password.map(Into::into), code)
}

/// Pump the host and the clients until `done` holds (up to 5 s); returns the host's events.
fn pump(h: &mut LobbyHost, clients: &mut [&mut LobbyClient], mut done: impl FnMut(&LobbyHost, &[&mut LobbyClient]) -> bool) -> Vec<LobbyEvent> {
    let start = Instant::now();
    let mut events = Vec::new();
    loop {
        events.extend(h.pump().unwrap());
        for c in clients.iter_mut() {
            c.pump().unwrap();
        }
        if done(h, clients) {
            return events;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "timed out; events {events:?}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn status(c: &LobbyClient) -> Status {
    c.status.clone()
}

#[test]
fn open_session_slots_ready_chat_and_start() {
    let mut h = host(false, None);
    assert!(h.announcement(40000, [1; 32]).is_some());
    let mut a = join(&mut h, "Alice", None, None);
    let mut b = join(&mut h, "Bob", None, None);
    pump(&mut h, &mut [&mut a, &mut b], |h, _| h.state().members.len() == 3);
    pump(&mut h, &mut [&mut a, &mut b], |_, c| c.iter().all(|c| c.state.as_ref().is_some_and(|s| s.members.len() == 3)));
    assert_eq!((a.member, b.member), (Some(1), Some(2)));
    assert_eq!(status(&a), Status::InLobby);

    // Picks: a taken slot is refused, a pick moves the member, picking clears every ready flag.
    assert!(h.pick(0, Some(6)));
    assert!(h.set_ready(0, true));
    a.pick(Some(6));
    b.pick(Some(2));
    pump(&mut h, &mut [&mut a, &mut b], |h, _| h.state().empire_of(2) == Some(2));
    assert_eq!(h.state().empire_of(0), Some(6));
    assert_eq!(h.state().empire_of(1), None);
    assert!(!h.state().members[0].ready, "a pick clears the ready flags");
    a.ready(true); // no empire: refused
    a.pick(Some(1));
    pump(&mut h, &mut [&mut a, &mut b], |h, _| h.state().empire_of(1) == Some(1));
    assert!(!h.state().members.iter().any(|m| m.ready));
    a.ready(true);
    b.ready(true);
    pump(&mut h, &mut [&mut a, &mut b], |h, _| h.state().members[1..].iter().all(|m| m.ready));
    assert!(!h.can_start(), "the host isn't ready");
    let h = h.start(vec![1, 2, 3], 2).err().unwrap();
    let mut h = *h;
    h.set_ready(0, true);
    assert!(h.can_start());

    // Chat reaches everyone.
    a.say("hello");
    let ev = pump(&mut h, &mut [&mut a, &mut b], |_, c| c.iter().all(|c| !c.chat.is_empty()));
    assert!(ev.contains(&LobbyEvent::Said(1, "hello".into())));
    assert_eq!(b.chat, vec![(1, "hello".to_string())]);
    pump(&mut h, &mut [&mut a, &mut b], |_, c| c.iter().all(|c| c.state.as_ref().is_some_and(|s| s.members.iter().all(|m| m.ready))));

    // Start: everyone gets its slot, empire and the same start state.
    let started = h.start(vec![9, 8, 7], 2).ok().unwrap();
    assert_eq!((started.own.slot, started.own.empire, started.own.first_input), (0, 6, 2));
    pump_clients(&mut [&mut a, &mut b], |c| c.iter().all(|c| c.status == Status::Started));
    let (sa, _) = a.into_game().unwrap();
    let (sb, _) = b.into_game().unwrap();
    assert_eq!((sa.slot, sa.empire, sb.slot, sb.empire), (1, 1, 2, 2));
    assert_eq!(sa.humans, vec![(0, 6), (1, 1), (2, 2)]);
    assert_eq!(sa.snapshot, vec![9, 8, 7]);
    assert_eq!(sb.first_input, 2);
}

fn pump_clients(clients: &mut [&mut LobbyClient], mut done: impl FnMut(&[&mut LobbyClient]) -> bool) {
    let start = Instant::now();
    while !done(clients) {
        for c in clients.iter_mut() {
            c.pump().unwrap();
        }
        assert!(start.elapsed() < Duration::from_secs(5), "timed out");
    }
}

#[test]
fn admission_refusals() {
    let mut h = host(false, None);
    // Wrong version and wrong rules (raw hellos).
    let (mut raw, b) = loopback();
    h.admit(Box::new(b));
    raw.send(&Msg::Lobby(LobbyMsg::Hello { version: LOBBY_VERSION + 1, rules: RULES, name: "x".into(), code: None }));
    let mut c = join(&mut h, "Eve", None, None);
    let (mut raw2, b2) = loopback();
    h.admit(Box::new(b2));
    raw2.send(&Msg::Lobby(LobbyMsg::Hello { version: LOBBY_VERSION, rules: [0; 32], name: "y".into(), code: None }));
    let ev = pump(&mut h, &mut [&mut c], |h, _| h.state().members.len() == 2);
    assert!(ev.contains(&LobbyEvent::Refused(Refusal::Version)) && ev.contains(&LobbyEvent::Refused(Refusal::Rules)));
    assert_eq!(recv(&mut raw), Msg::Lobby(LobbyMsg::Refused(Refusal::Version)));
    assert_eq!(recv(&mut raw2), Msg::Lobby(LobbyMsg::Refused(Refusal::Rules)));
    // A lockstep message before the hello is a protocol error.
    let (mut raw3, b3) = loopback();
    h.admit(Box::new(b3));
    raw3.send(&Msg::Hash { turn: 0, hash: 0 });
    h.pump().unwrap();
    assert_eq!(recv(&mut raw3), Msg::Lobby(LobbyMsg::Refused(Refusal::Protocol)));

    // Full: three slots, three members; a closed slot lowers the capacity.
    let mut d = join(&mut h, "Dan", None, None);
    pump(&mut h, &mut [&mut c, &mut d], |h, _| h.state().members.len() == 3);
    let mut e = join(&mut h, "Late", None, None);
    pump(&mut h, &mut [&mut e], |_, c| c[0].status != Status::Joining);
    assert_eq!(status(&e), Status::Refused(Refusal::Full));

    // Kick frees the member's slot; leaving too.
    d.pick(Some(2));
    pump(&mut h, &mut [&mut c, &mut d], |h, _| h.state().empire_of(2) == Some(2));
    h.kick(2);
    pump(&mut h, &mut [&mut c, &mut d], |_, c| c[1].status == Status::Refused(Refusal::Kicked));
    assert_eq!(h.state().slots[1].kind, SlotKind::Open);
    c.pick(Some(1));
    pump(&mut h, &mut [&mut c], |h, _| h.state().empire_of(1) == Some(1));
    c.leave();
    let ev = pump(&mut h, &mut [], |h, _| h.state().members.len() == 1);
    assert!(ev.contains(&LobbyEvent::Left(1)));
    assert_eq!(h.state().slots[0].kind, SlotKind::Open);
    assert!(h.set_slot(1, SlotKind::Closed) && !h.set_slot(6, SlotKind::Human(3)));
    let mut f = join(&mut h, "Fay", None, None);
    pump(&mut h, &mut [&mut f], |_, c| c[0].status != Status::Joining);
    assert_eq!(status(&f), Status::InLobby);
    let mut g = join(&mut h, "Gus", None, None);
    pump(&mut h, &mut [&mut g], |_, c| c[0].status != Status::Joining);
    assert_eq!(status(&g), Status::Refused(Refusal::Full));
    // A slot given to the AI doesn't count for humans either.
    assert!(h.set_slot(1, SlotKind::Open) && h.set_slot(2, SlotKind::Ai));
    assert_eq!(h.announcement(1, [0; 32]).unwrap().slots, 2);
}

fn recv(l: &mut Loopback) -> Msg {
    let start = Instant::now();
    loop {
        if let Some(m) = l.recv().unwrap() {
            return m;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}

#[test]
fn private_sessions_need_the_join_code() {
    let mut h = host(true, None);
    assert!(h.announcement(40000, [1; 32]).is_none());
    let mut none = join(&mut h, "NoCode", None, None);
    let mut wrong = join(&mut h, "Wrong", None, Some(1));
    let code = h.session();
    let mut right = join(&mut h, "Right", None, Some(code));
    pump(&mut h, &mut [&mut none, &mut wrong, &mut right], |_, c| c.iter().all(|c| c.status != Status::Joining));
    assert_eq!(status(&none), Status::Refused(Refusal::Code));
    assert_eq!(status(&wrong), Status::Refused(Refusal::Code));
    assert_eq!(status(&right), Status::InLobby);
}

/// A loopback end reporting its own channel binding: two ends with different bindings are what a man in the middle
/// relaying two TLS sessions looks like.
struct Bound(Loopback, [u8; 32]);

impl Transport for Bound {
    fn send(&mut self, msg: &Msg) {
        self.0.send(msg)
    }
    fn recv(&mut self) -> Result<Option<Msg>, ig2_net::wire::Error> {
        self.0.recv()
    }
    fn binding(&self) -> [u8; 32] {
        self.1
    }
}

#[test]
fn passwords_are_checked_without_sending_them() {
    let mut h = LobbyHost::new(settings(false, Some("hunter2")), 0xC0DE, RULES, &[(1, 0), (2, 1), (3, 2), (4, 3), (5, 4), (6, 5)], "Host");
    assert!(h.announcement(1, [0; 32]).unwrap().password);
    let mut good = join(&mut h, "Good", Some("hunter2"), None);
    let mut none = join(&mut h, "None", None, None);
    pump(&mut h, &mut [&mut good, &mut none], |_, c| c.iter().all(|c| c.status != Status::Joining));
    assert_eq!(status(&good), Status::InLobby);
    assert_eq!(status(&none), Status::Refused(Refusal::PasswordNeeded));

    // The password never appears on the link: record every message a joiner sends.
    let (a, mut tap) = loopback();
    let mut spy = LobbyClient::join(Box::new(a), "Spy", RULES, Some("hunter2".into()), None);
    let (mut relay_host, b) = loopback();
    h.admit(Box::new(b));
    let mut sent = Vec::new();
    let start = Instant::now();
    while spy.status == Status::Joining {
        while let Some(m) = tap.recv().unwrap() {
            sent.push(ig2_net::wire::to_vec(&m).unwrap());
            relay_host.send(&m);
        }
        h.pump().unwrap();
        while let Some(m) = relay_host.recv().unwrap() {
            tap.send(&m);
        }
        spy.pump().unwrap();
        assert!(start.elapsed() < Duration::from_secs(5));
    }
    assert_eq!(spy.status, Status::InLobby);
    assert!(sent.iter().all(|m| !m.windows(7).any(|w| w == b"hunter2")));

    // A wrong password is refused after the delay.
    let mut bad = join(&mut h, "Bad", Some("hunter3"), None);
    let t = Instant::now();
    let ev = pump(&mut h, &mut [&mut bad], |_, c| c[0].status != Status::Joining);
    assert_eq!(status(&bad), Status::Refused(Refusal::WrongPassword));
    assert!(t.elapsed() >= Duration::from_millis(900));
    assert!(ev.contains(&LobbyEvent::Refused(Refusal::WrongPassword)));

    // The right password through a man in the middle (different bindings) fails: the host refuses the joiner's
    // confirmation.
    let (a, b) = loopback();
    h.admit(Box::new(Bound(b, [1; 32])));
    let mut mitm = LobbyClient::join(Box::new(Bound(a, [2; 32])), "Mitm", RULES, Some("hunter2".into()), None);
    pump(&mut h, &mut [&mut mitm], |_, c| c[0].status != Status::Joining);
    assert_eq!(status(&mitm), Status::Refused(Refusal::WrongPassword));
}

#[test]
fn a_fake_host_is_caught_by_the_joiner() {
    // Someone without the password answers the joiner's hello: its confirmation can't check out.
    let (a, mut fake) = loopback();
    let mut c = LobbyClient::join(Box::new(a), "Victim", RULES, Some("hunter2".into()), None);
    assert!(matches!(recv(&mut fake), Msg::Lobby(LobbyMsg::Hello { .. })));
    let (_, out) = spake2::Spake2::<spake2::Ed25519Group>::start_b(&spake2::Password::new(b"guess"), &spake2::Identity::new(b"ig2-joiner"), &spake2::Identity::new(b"ig2-host"));
    fake.send(&Msg::Lobby(LobbyMsg::Pake(out)));
    c.pump().unwrap();
    fake.send(&Msg::Lobby(LobbyMsg::Confirm([0; 32])));
    c.pump().unwrap();
    assert_eq!(status(&c), Status::Refused(Refusal::HostNotAuthentic));
    // Skipping the confirmation and welcoming directly is caught too.
    let (a, mut fake) = loopback();
    let mut c = LobbyClient::join(Box::new(a), "Victim", RULES, Some("hunter2".into()), None);
    recv(&mut fake);
    fake.send(&Msg::Lobby(LobbyMsg::Welcome { member: 1 }));
    c.pump().unwrap();
    assert_eq!(status(&c), Status::Refused(Refusal::HostNotAuthentic));
}

#[test]
fn start_runs_lockstep_on_a_real_save() {
    let Some((rules, state)) = common::setup("quick-1.sav") else { return };
    let mut h = host(false, Some("pw"));
    let mut a = join(&mut h, "Alice", Some("pw"), None);
    pump(&mut h, &mut [&mut a], |h, _| h.state().members.len() == 2);
    // The quick scenario's human is empire 6; the joiner takes the AI empire 3.
    assert!(h.set_slot(2, SlotKind::Open));
    h.pick(0, Some(6));
    pump(&mut h, &mut [&mut a], |_, c| c[0].state.is_some());
    a.pick(Some(2));
    pump(&mut h, &mut [&mut a], |h, _| h.state().empire_of(1) == Some(2));
    a.ready(true);
    h.set_ready(0, true);
    pump(&mut h, &mut [&mut a], |h, _| h.can_start());

    let mut g = Game::new(state, &rules);
    g.saved();
    g.canonical_view(6);
    let snap = Snapshot::capture(&g, &[], 0, 0).to_file(0x5a5a, true);
    let mut started = h.start(snap, 2).ok().unwrap();
    pump_clients(&mut [&mut a], |c| c[0].status == Status::Started);
    let (sa, mut la) = a.into_game().unwrap();
    let mut own = started.own.peer(&rules).unwrap();
    let mut pa = sa.peer(&rules).unwrap();
    assert_eq!((own.empire, pa.empire), (6, 2));
    pa.issue(Command::StoryChoice { id: 1 });
    for _ in 0..200 {
        started.host.pump().unwrap();
        own.poll(&mut started.link, 1).unwrap();
        pa.poll(la.as_mut(), 1).unwrap();
        if own.turn >= 10 && pa.turn >= 10 {
            break;
        }
    }
    assert!(own.turn >= 10 && pa.turn >= 10);
    let n = own.hashes.len().min(pa.hashes.len());
    assert_eq!(own.hashes[..n], pa.hashes[..n]);
    started.host.pump().unwrap();
    assert!(started.host.desyncs.is_empty());
}
