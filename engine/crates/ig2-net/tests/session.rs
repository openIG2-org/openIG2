//! The session layer on loopback links and a real save (`_local/saves/quick-1.sav`; ADR 0011 §4–§6, ADR 0007 §3–§4):
//! a lobby starts a two-human game that runs into a real-time space battle; an MP save taken mid-battle resumes in a
//! new lobby with the same hashes; a pause holds every peer; a player who drops mid-battle is awaited, marked away
//! once the grace (on the test's own clock) runs out and rejoins with `PlayerBack`; a drop outside a battle is away at
//! once; a peer whose battle state drifts is resynced by the host; a rejoiner comes back mid-battle through the
//! lobby's admission (join code, password, rules, name, timeout); a peer that goes silent on a live link is dropped
//! after the keep-alive, a held session with every peer pumping drops nobody, and the joiner notices a silent host.
//! Host migration (three humans): the host drops mid-battle with bundles in flight to one survivor only, the survivors
//! elect the same successor, it adopts the missing bundles, the other resumes, the old host's empire is awaited then
//! away; and outside a battle with a survivor a few turns behind, the old host's player coming back afterwards.
//! Every turn's hashes agree throughout.

mod common;

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use ig2_net::lobby::{LobbyClient, LobbyHost, LobbyMsg, Refusal, Settings, Status, ADMISSION_TIMEOUT, WRONG_PASSWORD_DELAY};
use ig2_net::session::{self, ClientSession, HostSession, MpSave, SessionEvent, SessionSettings, DEFAULT_GRACE, DEFAULT_KEEP_ALIVE};
use ig2_net::{loopback, BattleOrder, Command, Loopback, Msg, Peer, Transport};
use ig2_sim::game::{Game, GameEvent, GameRules, RealTimeEvent};
use ig2_sim::save::SaveState;

const SAVE: &str = "quick-1.sav";
const RULES: [u8; 32] = [0x5a; 32];

const SESSION: u64 = 0xC0DE;
const PASSWORD: &str = "korona";

fn open() -> Settings {
    Settings { name: "Session test".into(), scenario: "quick".into(), private: false, password: None }
}

/// A lobby for empires 6 (the host) and `guest`'s empire, both ready.
fn lobby(slots: &[(u8, u8)], guest: u8) -> (LobbyHost, LobbyClient) {
    lobby_with(open(), slots, guest)
}

fn lobby_with(settings: Settings, slots: &[(u8, u8)], guest: u8) -> (LobbyHost, LobbyClient) {
    let (password, code) = (settings.password.clone(), settings.private.then_some(SESSION));
    let mut h = LobbyHost::new(settings, SESSION, RULES, slots, "Host");
    let (a, b) = loopback();
    h.admit(Box::new(b));
    let mut c = LobbyClient::join(Box::new(a), "Guest", RULES, password, code);
    let pump = |h: &mut LobbyHost, c: &mut LobbyClient, done: &dyn Fn(&LobbyHost, &LobbyClient) -> bool| {
        let start = Instant::now();
        while !done(h, c) {
            h.pump().unwrap();
            c.pump().unwrap();
            assert!(start.elapsed() < Duration::from_secs(5), "the lobby stalled");
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    pump(&mut h, &mut c, &|h, c| h.state().members.len() == 2 && c.status == Status::InLobby);
    assert!(h.pick(0, Some(6)));
    c.pick(Some(guest));
    pump(&mut h, &mut c, &|h, _| h.state().empire_of(1) == Some(guest));
    c.ready(true);
    pump(&mut h, &mut c, &|h, _| h.state().members[1].ready);
    assert!(h.set_ready(0, true));
    (h, c)
}

/// The joiner's side once the host started.
fn joined<'r>(mut c: LobbyClient, rules: &'r GameRules) -> ClientSession<'r> {
    let start = Instant::now();
    while c.status != Status::Started {
        c.pump().unwrap();
        assert!(start.elapsed() < Duration::from_secs(5), "no start");
    }
    let (start, link) = c.into_game().unwrap();
    ClientSession::new(&start, link, rules).unwrap()
}

/// The game as a load leaves it.
fn game<'r>(state: SaveState, rules: &'r GameRules) -> Game<'r> {
    let mut g = Game::new(state, rules);
    g.saved();
    g
}

struct Net<'r> {
    hs: HostSession<'r>,
    clients: Vec<ClientSession<'r>>,
    /// The test's clock.
    now: Duration,
    events: Vec<SessionEvent>,
}

impl<'r> Net<'r> {
    /// One round of 10 ms: the host, then each joiner (two turns at most, so they drift within the delay); `act`
    /// sees each peer after it ran.
    fn round(&mut self, act: &mut impl FnMut(&mut Peer<'r>)) {
        self.now += Duration::from_millis(10);
        self.events.extend(self.hs.pump(self.now, 1).unwrap());
        act(&mut self.hs.own);
        for c in &mut self.clients {
            c.pump(self.now, 2).unwrap();
            act(&mut c.peer);
        }
    }

    fn run_to(&mut self, turn: u64, mut act: impl FnMut(&mut Peer<'r>)) {
        for _ in 0..100_000 {
            if self.hs.own.turn >= turn && self.clients.iter().all(|c| c.peer.turn >= turn) {
                return;
            }
            self.round(&mut act);
        }
        panic!("the session stalled at {}", self.hs.own.turn);
    }

    fn idle(&mut self, rounds: usize) {
        for _ in 0..rounds {
            self.round(&mut |p| p.events.clear());
        }
    }

    fn turn(&self) -> u64 {
        self.hs.own.turn
    }

    fn took(&mut self, e: &SessionEvent) -> bool {
        let n = self.events.len();
        self.events.retain(|x| x != e);
        self.events.len() < n
    }
}

/// A session from the lobby (6 hosts, 3 joins) on `state`.
fn start<'r>(rules: &'r GameRules, state: &SaveState) -> Net<'r> {
    start_with(open(), rules, state)
}

fn start_with<'r>(settings: Settings, rules: &'r GameRules, state: &SaveState) -> Net<'r> {
    let (h, c) = lobby_with(settings, &[(6, 0), (3, 0)], 3);
    let hs = HostSession::start(h, game(state.clone(), rules), rules, SessionSettings::default()).ok().expect("the session starts");
    let clients = vec![joined(c, rules)];
    Net { hs, clients, now: Duration::ZERO, events: Vec::new() }
}

fn space_started(p: &Peer) -> Option<usize> {
    p.events.iter().find_map(|e| if let GameEvent::RealTime(RealTimeEvent::SpaceStarted { slot }) = e { Some(*slot) } else { None })
}

/// Each of the peer's units in battle `b` attacks the first enemy unit.
fn attack_all(p: &mut Peer, b: usize) {
    let battle = &p.game.realtime.slots[b];
    let me = p.empire as i32;
    let mine: Vec<usize> = (0..battle.units.len()).filter(|&u| battle.units[u].alive && battle.units[u].owner == me).collect();
    let Some(target) = (0..battle.units.len()).find(|&u| battle.units[u].alive && battle.units[u].owner > 0 && battle.units[u].owner != me) else { return };
    for u in mine {
        p.issue(Command::SpaceOrder { battle: b as u8, order: BattleOrder::Attack { unit: u as u16, target: target as u16 } });
    }
}

/// `SpWar_MultiCsata_vegetert[e]`: `e` has left the battle screen.
fn left_screen(g: &Game, e: u32) -> i32 {
    g.state.mem.i32(0xBAD118 + 4 * e)
}

/// Run until both peers' space battle has run a few ticks, both humans ordering their units to attack; returns its
/// slot.
fn into_battle(net: &mut Net) -> usize {
    let mut slot = None;
    let mut ordered = [false; 9];
    for _ in 0..40 {
        let target = net.turn() + 25;
        net.run_to(target, |p| {
            slot = slot.or(space_started(p));
            p.events.clear();
            if let Some(b) = slot {
                if !ordered[p.empire as usize] && p.game.realtime.slots[b].active {
                    ordered[p.empire as usize] = true;
                    attack_all(p, b);
                }
            }
        });
        if let Some(b) = slot {
            if std::iter::once(&net.hs.own).chain(net.clients.iter().map(|c| &c.peer)).all(|p| p.game.realtime.slots[b].active && p.game.realtime.slots[b].tick > 20) {
                return b;
            }
        }
    }
    panic!("no real-time space battle");
}

/// Every turn both peers ran has the same hash; returns how many.
fn agree(a: &Peer, b: &Peer) -> usize {
    let mut n = 0;
    for (t, h) in &b.hashes {
        if let Some((_, h0)) = a.hashes.iter().find(|(t0, _)| t0 == t) {
            assert_eq!(h, h0, "turn {t}: empire {} vs {}", b.empire, a.empire);
            n += 1;
        }
    }
    n
}

#[test]
fn a_session_runs_from_the_lobby_into_a_battle_and_resumes_from_an_mp_save() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
    let mut net = start(&rules, &state);
    assert_eq!((net.hs.own.empire, net.clients[0].peer.empire), (6, 3));
    assert_eq!(net.hs.humans().map(|(e, n, here)| (e, n.to_string(), here)).collect::<Vec<_>>(), [(6, "Host".into(), true), (3, "Guest".into(), true)]);
    let b = into_battle(&mut net);
    assert!(net.hs.own.game.realtime_running());
    let orders = net.hs.host.log.iter().flat_map(|b| &b.commands).filter(|c| matches!(c.command, Command::SpaceOrder { .. })).count();
    assert!(orders > 0, "both humans' orders went out");
    assert_eq!(agree(&net.hs.own, &net.clients[0].peer) as u64, net.turn().min(net.clients[0].peer.turn), "every turn from 0");
    assert!(net.hs.host.desyncs.is_empty(), "{:?}", net.hs.host.desyncs);

    // An MP save mid-battle: the host's state at the next turn and the session's humans.
    let file = net.hs.save().unwrap();
    let saved = MpSave::read(&file, RULES).unwrap();
    assert_eq!(saved.header.turn, net.turn());
    assert_eq!(saved.session.humans.iter().map(|h| (h.empire, h.name.as_str())).collect::<Vec<_>>(), [(6, "Host"), (3, "Guest")]);
    assert_eq!(saved.session.grace_ms, DEFAULT_GRACE.as_millis() as u64);
    assert!(saved.snapshot.restore(&rules).unwrap().0.realtime.slots[b].active, "mid-battle");
    assert!(matches!(MpSave::read(&file, [1; 32]), Err(session::Error::Rules)));
    assert!(matches!(MpSave::read(&saved.snapshot.to_file(session::rules_id(RULES), false), RULES), Err(session::Error::NotMultiplayer)));

    // The session goes on with no orders; the save resumed in a new lobby runs the same turns.
    let at = saved.header.turn;
    net.run_to(at + 60, |p| p.events.clear());
    let (wrong, _c) = lobby(&[(6, 0), (2, 0)], 2);
    assert!(HostSession::resume(wrong, &saved, &rules).is_err(), "other empires than the saved humans'");
    let (h, c) = lobby(&[(6, 0), (3, 0)], 3);
    let hs = HostSession::resume(h, &saved, &rules).ok().expect("the save resumes");
    assert_eq!(hs.host.next_turn(), at);
    let mut again = Net { hs, clients: vec![joined(c, &rules)], now: Duration::ZERO, events: Vec::new() };
    assert_eq!(again.clients[0].peer.turn, at);
    again.run_to(at + 60, |p| p.events.clear());
    let n = agree(&net.hs.own, &again.hs.own);
    assert!(n >= 60, "{n}");
    assert!(agree(&again.hs.own, &again.clients[0].peer) >= 59);
    assert!(again.hs.host.desyncs.is_empty() && net.hs.host.desyncs.is_empty());
    eprintln!("battle slot {b}, saved at turn {at}, {n} turns identical after the resume");
}

#[test]
fn a_save_resumes_without_a_saved_player_whose_empire_the_computer_plays() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let (mut net, _cuts) = start3(&rules, &state);
    net.run_to(20, |p| p.events.clear());
    let saved = MpSave::read(&net.hs.save().unwrap(), RULES).unwrap();
    assert_eq!(saved.session.humans.iter().map(|h| h.empire).collect::<Vec<_>>(), [6, 3, 2]);
    let ki = |p: &Peer, e: u32| p.game.state.mem.u8(ig2_sim::save::va::PLAYER_KI + e);

    // "Third" (2) doesn't come back: the room of 6 and 3 resumes, 2 goes to the computer on every machine.
    let at = saved.header.turn;
    let (h, c) = lobby(&[(6, 0), (3, 0), (2, 0)], 3);
    let hs = HostSession::resume(h, &saved, &rules).ok().expect("the save resumes without 2");
    let mut again = Net { hs, clients: vec![joined(c, &rules)], now: Duration::ZERO, events: Vec::new() };
    assert_eq!(again.hs.host.next_turn(), at);
    for p in [&again.hs.own, &again.clients[0].peer] {
        assert_eq!((ki(p, 6), ki(p, 3), ki(p, 2)), (1, 1, 2), "empire {}", p.empire);
    }
    again.run_to(at + 120, |p| p.events.clear());
    assert!(agree(&again.hs.own, &again.clients[0].peer) >= 119);
    assert!(again.hs.host.desyncs.is_empty(), "{:?}", again.hs.host.desyncs);

    // Still refused: a member on an empire that wasn't a saved human's.
    let (wrong, _c) = lobby(&[(6, 0), (1, 0)], 1);
    assert!(HostSession::resume(wrong, &saved, &rules).is_err());
}

#[test]
fn a_dropped_player_is_awaited_then_away_and_rejoins() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
    let mut net = start(&rules, &state);
    let b = into_battle(&mut net);

    // A pause from the joiner holds every peer, its resume lets them go on.
    net.clients[0].pause(true);
    net.idle(10);
    assert!(net.took(&SessionEvent::Paused { empire: 3, on: true }));
    assert_eq!(net.clients[0].status.paused, [3]);
    let at = net.turn();
    net.idle(50);
    assert_eq!(net.turn(), at, "a paused session stands still");
    net.clients[0].pause(false);
    net.run_to(at + 10, |p| p.events.clear());
    assert!(net.took(&SessionEvent::Paused { empire: 3, on: false }));

    // 3 drops mid-battle: the host holds and waits.
    assert!(net.hs.own.game.realtime_running());
    let gone = net.clients.remove(0);
    drop(gone);
    net.idle(5);
    assert!(net.took(&SessionEvent::Dropped { empire: 3, waiting: true }), "{:?}", net.events);
    let held_at = net.turn();
    assert_eq!(held_at, net.hs.host.next_turn());
    net.idle(100);
    assert_eq!(net.turn(), held_at, "a held session stands still");
    let s = net.hs.status(net.now);
    assert!(s.held() && s.waiting.len() == 1 && s.waiting[0].0 == 3 && s.waiting[0].1 > 55_000, "{s:?}");

    // The grace runs out: 3 is away, the battle goes on.
    net.now += DEFAULT_GRACE;
    net.idle(1);
    assert!(net.took(&SessionEvent::Away { empire: 3 }));
    net.run_to(held_at + 10, |p| p.events.clear());
    assert_eq!(left_screen(&net.hs.own.game, 3), 1, "away counts as having left the screen");
    assert!(net.hs.own.game.realtime.slots[b].active, "the battle still runs");

    // 3 comes back on a new link: the host's state now, then PlayerBack.
    let (mut ours, theirs) = loopback();
    net.hs.rejoin(3, Box::new(theirs), net.now).unwrap();
    assert!(matches!(net.hs.rejoin(3, Box::new(loopback().0), net.now), Err(session::Error::Connected(3))));
    let start = ClientSession::wait_start(&mut ours).unwrap().expect("a start");
    assert_eq!((start.empire, start.first_input), (3, net.hs.host.next_turn().max(start.delay)));
    net.clients.push(ClientSession::new(&start, Box::new(ours), &rules).unwrap());
    let back_at = net.turn();
    net.run_to(back_at + 60, |p| p.events.clear());
    assert!(net.took(&SessionEvent::Rejoined { empire: 3, turn: back_at }));
    assert!(net.hs.host.log.iter().filter(|b| b.turn >= back_at).flat_map(|b| &b.commands).any(|c| c.command == Command::PlayerBack && c.issuer == 3));
    if net.hs.own.game.realtime.slots[b].active {
        assert_eq!(left_screen(&net.hs.own.game, 3), 0, "back on the battle's screen");
    }
    assert!(net.clients[0].status.away.is_empty() && !net.clients[0].status.held());
    assert!(net.hs.host.desyncs.is_empty(), "{:?}", net.hs.host.desyncs);
    let n = agree(&net.hs.own, &net.clients[0].peer);
    assert!(n >= 59, "{n}");
    eprintln!("battle slot {b}: held at {held_at}, back at {back_at}, {n} turns agreed");
}

#[test]
fn a_drop_outside_a_battle_is_away_at_once() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let mut net = start(&rules, &state);
    net.run_to(10, |p| p.events.clear());
    assert!(!net.hs.own.game.realtime_running());
    net.clients.clear();
    net.idle(3);
    assert!(net.took(&SessionEvent::Dropped { empire: 3, waiting: false }));
    assert_eq!(net.hs.status(net.now).away, [3]);
    let at = net.turn();
    net.run_to(at + 20, |p| p.events.clear());
    let (mut ours, theirs) = loopback();
    net.hs.rejoin(3, Box::new(theirs), net.now).unwrap();
    let start = ClientSession::wait_start(&mut ours).unwrap().unwrap();
    net.clients.push(ClientSession::new(&start, Box::new(ours), &rules).unwrap());
    let back = net.turn();
    net.run_to(back + 30, |p| p.events.clear());
    assert!(net.hs.humans().all(|(_, _, here)| here));
    assert!(agree(&net.hs.own, &net.clients[0].peer) >= 29);
    assert!(net.hs.host.desyncs.is_empty());
}

#[test]
fn a_desynced_peer_is_resynced_by_the_host() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
    let mut net = start(&rules, &state);
    let b = into_battle(&mut net);
    // The joiner's battle drifts (as a stray `rand()` in its battle tick would).
    let p = &mut net.clients[0].peer;
    let drift = p.turn;
    let u = p.game.realtime.slots[b].units.iter().position(|u| u.alive).unwrap();
    p.game.realtime.slots[b].units[u].alive = false;
    net.run_to(drift + 10, |p| p.events.clear());
    let resynced: Vec<_> = net.events.iter().filter(|e| matches!(e, SessionEvent::Resynced { .. })).cloned().collect();
    let [SessionEvent::Resynced { empire: 3, turn, at }] = resynced[..] else { panic!("{:?}", net.events) };
    assert_eq!(turn, drift);
    assert_eq!(net.clients[0].restarts, 1);
    assert_eq!(net.clients[0].resyncs, 1);
    assert!(net.clients[0].peer.hashes.iter().all(|&(t, _)| t >= at), "a fresh peer from the host's state");
    let reports = net.hs.host.desyncs.len();
    net.run_to(at + 60, |p| p.events.clear());
    assert_eq!(net.hs.host.desyncs.len(), reports, "in step again: {:?}", net.hs.host.desyncs);
    assert_eq!(net.clients[0].restarts, 1);
    let n = agree(&net.hs.own, &net.clients[0].peer);
    assert!(n >= 59, "{n}");
}

/// Run the session (100 ms of the test's clock a round) until `c`'s admission ends; returns how it ended.
fn come_back(net: &mut Net, c: &mut LobbyClient) -> Status {
    for _ in 0..1000 {
        net.now += Duration::from_millis(90);
        net.idle(1);
        if !matches!(c.pump().unwrap(), Status::Joining) {
            return c.status.clone();
        }
    }
    panic!("the admission stalled");
}

/// `name` knocks on the host's door over a new link.
fn knock(net: &mut Net, name: &str, rules: [u8; 32], password: Option<&str>, code: Option<u64>) -> LobbyClient {
    let (ours, theirs) = loopback();
    net.hs.admit(Box::new(theirs), net.now);
    LobbyClient::rejoin(Box::new(ours), name, rules, password.map(Into::into), code)
}

#[test]
fn a_rejoiner_is_admitted_mid_battle_like_a_joiner() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
    let settings = Settings { private: true, password: Some(PASSWORD.into()), ..open() };
    let mut net = start_with(settings, &rules, &state);
    let b = into_battle(&mut net);

    // 3 drops mid-battle: the session holds for it.
    net.clients.clear();
    net.idle(5);
    assert!(net.took(&SessionEvent::Dropped { empire: 3, waiting: true }), "{:?}", net.events);
    let held_at = net.turn();
    let code = Some(SESSION);

    // The door turns away what the lobby's would, and the names that aren't the away human's.
    let refusals = [
        ("Guest", [1; 32], Some(PASSWORD), code, Refusal::Rules),
        ("Guest", RULES, Some(PASSWORD), None, Refusal::Code),
        ("Guest", RULES, Some("koronaa"), code, Refusal::WrongPassword),
        ("Stranger", RULES, Some(PASSWORD), code, Refusal::Started),
        ("Host", RULES, Some(PASSWORD), code, Refusal::Connected),
    ];
    for (name, fingerprint, password, code, why) in refusals {
        let start = net.now;
        let mut c = knock(&mut net, name, fingerprint, password, code);
        assert_eq!(come_back(&mut net, &mut c), Status::Refused(why), "{name}");
        assert!(net.took(&SessionEvent::Refused(why)), "{why:?}: {:?}", net.events);
        if why == Refusal::WrongPassword {
            assert!(net.now - start >= WRONG_PASSWORD_DELAY, "one guess a second");
        }
    }
    // A connection that says nothing is dropped after the admission timeout.
    let (mut silent, theirs) = loopback();
    net.hs.admit(Box::new(theirs), net.now);
    net.idle(1);
    assert_eq!(net.hs.admitting(), 1);
    net.now += ADMISSION_TIMEOUT - Duration::from_secs(1);
    net.idle(1);
    assert_eq!(net.hs.admitting(), 1, "not before the timeout");
    net.now += Duration::from_secs(1);
    net.idle(1);
    assert!(net.took(&SessionEvent::Refused(Refusal::TimedOut)), "{:?}", net.events);
    assert_eq!(net.hs.admitting(), 0);
    assert_eq!(silent.recv().unwrap(), Some(Msg::Lobby(LobbyMsg::Refused(Refusal::TimedOut))));
    assert_eq!(net.turn(), held_at, "the session held throughout");
    let s = net.hs.status(net.now);
    assert!(s.waiting.len() == 1 && s.waiting[0].0 == 3, "still within the grace: {s:?}");

    // The right name, code and password: welcome, then the host's state now on the same link.
    let mut c = knock(&mut net, "Guest", RULES, Some(PASSWORD), code);
    assert_eq!(come_back(&mut net, &mut c), Status::Admitted);
    assert_eq!(c.member, Some(1));
    let mut link = c.into_link().unwrap();
    let start = ClientSession::wait_start(&mut *link).unwrap().expect("a start after the welcome");
    assert_eq!((start.slot, start.empire, start.first_input), (1, 3, net.hs.host.next_turn().max(start.delay)));
    net.clients.push(ClientSession::new(&start, link, &rules).unwrap());
    assert!(net.took(&SessionEvent::Rejoined { empire: 3, turn: held_at }), "{:?}", net.events);
    assert!(!net.hs.status(net.now).held());

    // Once back, a second claim to the empire is refused.
    let mut again = knock(&mut net, "Guest", RULES, Some(PASSWORD), code);
    assert_eq!(come_back(&mut net, &mut again), Status::Refused(Refusal::Connected));

    net.run_to(held_at + 60, |p| p.events.clear());
    assert!(net.hs.own.game.realtime.slots[b].tick > 0);
    assert!(!net.hs.host.log.iter().flat_map(|b| &b.commands).any(|c| matches!(c.command, Command::PlayerAway | Command::PlayerBack)), "never away");
    assert!(net.hs.host.desyncs.is_empty(), "{:?}", net.hs.host.desyncs);
    let n = agree(&net.hs.own, &net.clients[0].peer);
    assert!(n >= 59, "{n}");
    eprintln!("battle slot {b}: held at {held_at}, admitted back, {n} turns agreed");
}

#[test]
fn a_kicked_player_is_away_at_once_and_turned_away_at_the_door() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
    let mut net = start(&rules, &state);
    let b = into_battle(&mut net);
    assert!(net.hs.kick(6, net.now).is_err(), "the host can't kick itself");
    net.hs.kick(3, net.now).unwrap();
    assert!(net.hs.kick(3, net.now).is_err(), "a second kick: no such player");
    // The joiner hears it: closed for good, no host migration.
    net.idle(3);
    assert!(net.took(&SessionEvent::Kicked { empire: 3 }), "{:?}", net.events);
    let c = &net.clients[0];
    assert!(c.kicked() && c.closed() && c.successors().is_empty());
    net.clients.clear();
    // In the battle it is away without the grace: `PlayerAway`, and the session runs on.
    let s = net.hs.status(net.now);
    assert_eq!(s.away, [3]);
    assert!(!s.held());
    let at = net.turn();
    net.run_to(at + 30, |p| p.events.clear());
    assert!(net.hs.host.log.iter().flat_map(|b| &b.commands).any(|c| c.issuer == 3 && matches!(c.command, Command::PlayerAway)));
    assert!(net.hs.own.game.realtime.slots[b].tick > 0);
    // Its name stays out.
    let mut c = knock(&mut net, "Guest", RULES, None, None);
    assert_eq!(come_back(&mut net, &mut c), Status::Refused(Refusal::Kicked));
    assert!(net.took(&SessionEvent::Refused(Refusal::Kicked)), "{:?}", net.events);
}

#[test]
fn a_silent_peer_is_dropped_after_the_keep_alive_and_comes_back() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
    let mut net = start(&rules, &state);
    let b = into_battle(&mut net);

    // 3's link stays up but nothing comes over it: the host stalls on its inputs, then drops it at the keep-alive.
    let mut silent = net.clients.remove(0);
    let since = net.now;
    while net.now < since + DEFAULT_KEEP_ALIVE - Duration::from_millis(100) {
        net.idle(1);
    }
    assert!(!net.events.iter().any(|e| matches!(e, SessionEvent::Dropped { .. })), "not before the keep-alive: {:?}", net.events);
    assert!(!silent.closed());
    net.idle(20);
    assert!(net.took(&SessionEvent::Dropped { empire: 3, waiting: true }), "{:?}", net.events);
    let held_at = net.turn();
    net.idle(50);
    assert_eq!(net.turn(), held_at, "awaited mid-battle: the session holds");
    assert!(net.hs.status(net.now).waiting.iter().any(|w| w.0 == 3));
    silent.pump(net.now, 0).unwrap();
    assert!(silent.closed(), "the host let the link go");
    drop(silent);

    // It comes back through the door within the grace: the host's state now, the same hashes from there on.
    let mut c = knock(&mut net, "Guest", RULES, None, None);
    assert_eq!(come_back(&mut net, &mut c), Status::Admitted);
    let mut link = c.into_link().unwrap();
    let start = ClientSession::wait_start(&mut *link).unwrap().expect("a start after the welcome");
    net.clients.push(ClientSession::new(&start, link, &rules).unwrap());
    assert!(net.took(&SessionEvent::Rejoined { empire: 3, turn: held_at }), "{:?}", net.events);
    net.run_to(held_at + 60, |p| p.events.clear());
    assert!(net.hs.own.game.realtime.slots[b].tick > 0);
    assert!(!net.events.iter().any(|e| matches!(e, SessionEvent::Dropped { .. })), "{:?}", net.events);
    assert!(net.hs.host.desyncs.is_empty(), "{:?}", net.hs.host.desyncs);
    let n = agree(&net.hs.own, &net.clients[0].peer);
    assert!(n >= 59, "{n}");
}

#[test]
fn a_held_session_with_every_peer_pumping_drops_nobody() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
    let mut net = start(&rules, &state);
    into_battle(&mut net);
    net.clients[0].pause(true);
    net.idle(10);
    assert!(net.took(&SessionEvent::Paused { empire: 3, on: true }));
    let at = net.turn();
    // Three keep-alives long without a bundle: only the heartbeat crosses the links.
    let until = net.now + 3 * DEFAULT_KEEP_ALIVE;
    while net.now < until {
        net.idle(1);
    }
    assert_eq!(net.turn(), at, "still paused");
    assert!(net.events.is_empty(), "{:?}", net.events);
    assert!(net.hs.humans().all(|(_, _, here)| here));
    assert!(!net.clients[0].closed(), "the joiner hears the host");
    net.clients[0].pause(false);
    net.run_to(at + 20, |p| p.events.clear());
    assert!(net.took(&SessionEvent::Paused { empire: 3, on: false }));
    assert!(net.events.is_empty(), "{:?}", net.events);
    assert!(agree(&net.hs.own, &net.clients[0].peer) as u64 >= at + 19);
}

#[test]
fn the_joiner_notices_a_silent_host() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let mut net = start(&rules, &state);
    net.run_to(10, |p| p.events.clear());
    // The host stops pumping (its links stay up): the joiner gives up on it after the keep-alive.
    let since = net.now;
    let c = &mut net.clients[0];
    let mut now = since;
    while now < since + DEFAULT_KEEP_ALIVE - Duration::from_millis(10) {
        now += Duration::from_millis(10);
        c.pump(now, 2).unwrap();
        assert!(!c.closed(), "not before the keep-alive ({:?})", now - since);
    }
    now += Duration::from_millis(10);
    c.pump(now, 2).unwrap();
    assert!(c.closed());
}

// ------------------------------------------------------------------------------------------------ host migration

/// A joiner's link whose receiving side the test can cut: from then on nothing reaches the joiner (what the host sends
/// is lost, as on a link that died mid-send) and it reports closed; what it sends still goes out.
struct Cut {
    link: Loopback,
    cut: Rc<Cell<bool>>,
}

impl Transport for Cut {
    fn send(&mut self, msg: &Msg) {
        self.link.send(msg);
    }

    fn recv(&mut self) -> Result<Option<Msg>, ig2_net::wire::Error> {
        if self.cut.get() {
            return Ok(None);
        }
        self.link.recv()
    }

    fn closed(&self) -> bool {
        self.cut.get() || self.link.closed()
    }
}

/// A three-human session from the lobby: 6 hosts (slot 0), "Guest" plays 3 (slot 1), "Third" 2 (slot 2). Returns the
/// cut switch of each joiner's link.
fn start3<'r>(rules: &'r GameRules, state: &SaveState) -> (Net<'r>, Vec<Rc<Cell<bool>>>) {
    let mut h = LobbyHost::new(open(), SESSION, RULES, &[(6, 0), (3, 0), (2, 0)], "Host");
    let mut cuts = Vec::new();
    let mut cs: Vec<LobbyClient> = ["Guest", "Third"]
        .iter()
        .map(|name| {
            let (a, b) = loopback();
            h.admit(Box::new(b));
            let cut = Rc::new(Cell::new(false));
            cuts.push(cut.clone());
            LobbyClient::join(Box::new(Cut { link: a, cut }), name, RULES, None, None)
        })
        .collect();
    let pump = |h: &mut LobbyHost, cs: &mut [LobbyClient], done: &dyn Fn(&LobbyHost) -> bool| {
        let start = Instant::now();
        while !done(h) {
            h.pump().unwrap();
            for c in cs.iter_mut() {
                c.pump().unwrap();
            }
            assert!(start.elapsed() < Duration::from_secs(5), "the lobby stalled");
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    pump(&mut h, &mut cs, &|h| h.state().members.len() == 3);
    assert!(h.pick(0, Some(6)));
    cs[0].pick(Some(3));
    cs[1].pick(Some(2));
    pump(&mut h, &mut cs, &|h| h.state().empire_of(1) == Some(3) && h.state().empire_of(2) == Some(2));
    // A pick clears every ready flag: ready once both picked.
    cs[0].ready(true);
    cs[1].ready(true);
    pump(&mut h, &mut cs, &|h| h.state().members.iter().skip(1).all(|m| m.ready));
    assert!(h.set_ready(0, true));
    let hs = HostSession::start(h, game(state.clone(), rules), rules, SessionSettings::default()).ok().expect("the session starts");
    let clients = cs.into_iter().map(|c| joined(c, rules)).collect();
    (Net { hs, clients, now: Duration::ZERO, events: Vec::new() }, cuts)
}

/// The host's process ends: each survivor notices, they elect the same successor, it takes over from its own state
/// and the others reconnect to it over new links. Returns the old host's hashes.
fn lose_host(net: &mut Net<'_>) -> Vec<(u64, u64)> {
    let old = net.hs.own.hashes.clone();
    let elected: Vec<Vec<u8>> = net.clients.iter().map(|c| c.successors()).collect();
    assert!(elected.windows(2).all(|w| w[0] == w[1]), "{elected:?}");
    assert_eq!(elected[0], [1, 2]);
    let i = net.clients.iter().position(|c| c.peer.slot == elected[0][0]).unwrap();
    let successor = net.clients.remove(i);
    let hs = HostSession::migrate(successor, RULES, None, net.now);
    // The old host's process ends here.
    drop(std::mem::replace(&mut net.hs, hs));
    for c in &mut net.clients {
        c.pump(net.now, 0).unwrap();
        assert!(c.closed(), "{} sees the host gone", c.peer.empire);
        let name = c.info().names.iter().find(|n| n.0 == c.peer.slot).unwrap().1.clone();
        let (ours, theirs) = loopback();
        net.hs.admit(Box::new(theirs), net.now);
        c.migrate(elected[0][0], Box::new(ours), &name, None, RULES);
        assert!(!c.closed() && c.migrating());
    }
    old
}

/// Pump until the migration is over.
fn settle(net: &mut Net<'_>) {
    for _ in 0..1000 {
        if !net.hs.migrating() && net.clients.iter().all(|c| !c.migrating()) {
            return;
        }
        net.idle(1);
    }
    panic!("the migration stalled");
}

#[test]
fn the_host_drops_mid_battle_and_a_survivor_takes_over() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
    let (mut net, cuts) = start3(&rules, &state);
    let b = into_battle(&mut net);
    assert!(net.hs.own.game.realtime_running());

    // In flight: the successor-to-be (slot 1) stops hearing the host, which still sends the bundles its inputs allow
    // to the other survivor; then the host's process ends.
    cuts[0].set(true);
    let before = net.clients[0].peer.frontier();
    for _ in 0..50 {
        if net.hs.host.next_turn() > before {
            break;
        }
        net.now += Duration::from_millis(10);
        net.hs.pump(net.now, 1).unwrap();
        net.clients[1].pump(net.now, 2).unwrap();
    }
    let sent = net.hs.host.next_turn();
    assert!(sent > before, "the other survivor holds bundles the successor lacks ({before} vs {sent})");
    while net.clients[1].peer.frontier() < sent {
        net.clients[1].pump(net.now, 0).unwrap();
    }
    assert!(net.clients[0].closed(), "the successor sees the host gone");
    let old = lose_host(&mut net);

    // The successor adopts the missing bundles, the other survivor resumes from its own state, and 6 (the old host's
    // empire, a participant) is awaited for the grace.
    settle(&mut net);
    assert!(net.took(&SessionEvent::Resumed { empire: 2, turn: sent }), "{:?}", net.events);
    assert!(net.took(&SessionEvent::Dropped { empire: 6, waiting: true }), "{:?}", net.events);
    assert!(net.hs.host.next_turn() >= sent);
    assert_eq!(net.clients[0].restarts, 0, "no snapshot: the survivor kept its state");
    let held_at = net.turn();
    net.idle(100);
    assert_eq!(net.turn(), held_at, "the old host's player is awaited");
    assert_eq!(net.clients[0].status.waiting.iter().map(|w| w.0).collect::<Vec<_>>(), [6]);
    // The grace passes a second at a time (every peer pumping, so nobody falls silent).
    let until = net.now + DEFAULT_GRACE;
    while net.now < until {
        net.now += Duration::from_millis(990);
        net.idle(1);
    }
    assert!(net.took(&SessionEvent::Away { empire: 6 }));
    net.run_to(held_at + 70, |p| p.events.clear());
    if net.hs.own.game.realtime.slots[b].active {
        assert_eq!(left_screen(&net.hs.own.game, 6), 1, "away counts as having left the screen");
    }
    assert_eq!(net.clients[0].status.away, [6]);
    assert!(net.hs.host.desyncs.is_empty(), "{:?}", net.hs.host.desyncs);
    let n = agree(&net.hs.own, &net.clients[0].peer);
    assert!(n as u64 >= held_at + 60, "{n}");
    let o = net.hs.own.hashes.iter().filter(|(t, h)| old.iter().any(|x| x == &(*t, *h))).count();
    assert_eq!(o, old.len(), "the old host's turns agree with the successor's");

    // An MP save after the migration has the session's three humans.
    let save = MpSave::read(&net.hs.save().unwrap(), RULES).unwrap();
    let mut humans: Vec<(u8, String)> = save.session.humans.iter().map(|h| (h.empire, h.name.clone())).collect();
    humans.sort();
    assert_eq!(humans, [(2, "Third".into()), (3, "Guest".into()), (6, "Host".into())]);
    eprintln!("battle slot {b}: successor at {before}, adopted to {sent}, held at {held_at}, {n} turns agreed");
}

#[test]
fn the_host_drops_outside_a_battle_with_a_survivor_behind() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let (mut net, _cuts) = start3(&rules, &state);
    net.run_to(20, |p| p.events.clear());
    // Slot 2 lags: it sends its inputs but runs nothing for a few rounds.
    for _ in 0..5 {
        net.now += Duration::from_millis(10);
        net.hs.pump(net.now, 1).unwrap();
        net.clients[0].pump(net.now, 2).unwrap();
        net.clients[1].pump(net.now, 0).unwrap();
    }
    assert!(net.clients[1].peer.turn < net.clients[0].peer.turn, "a survivor behind");
    assert!(!net.hs.own.game.realtime_running());
    lose_host(&mut net);
    settle(&mut net);
    assert!(net.events.iter().any(|e| matches!(e, SessionEvent::Resumed { empire: 2, .. })), "{:?}", net.events);
    assert!(net.took(&SessionEvent::Dropped { empire: 6, waiting: false }), "{:?}", net.events);
    assert_eq!(net.hs.status(net.now).away, [6]);
    let at = net.turn();
    net.run_to(at + 40, |p| p.events.clear());
    assert!(net.hs.host.desyncs.is_empty(), "{:?}", net.hs.host.desyncs);
    assert!(agree(&net.hs.own, &net.clients[0].peer) as u64 >= at + 39);

    // The old host's player comes back through the successor's door (its state, the rejoin path).
    let mut c = knock(&mut net, "Host", RULES, None, None);
    assert_eq!(come_back(&mut net, &mut c), Status::Admitted);
    let mut link = c.into_link().unwrap();
    let start = ClientSession::wait_start(&mut *link).unwrap().expect("a start");
    assert_eq!((start.empire, start.session.host), (6, 1));
    net.clients.push(ClientSession::new(&start, link, &rules).unwrap());
    let back = net.turn();
    net.run_to(back + 30, |p| p.events.clear());
    assert!(net.hs.humans().all(|(_, _, here)| here));
    assert!(agree(&net.hs.own, &net.clients[1].peer) >= 29);
    assert!(net.hs.host.desyncs.is_empty());
}
