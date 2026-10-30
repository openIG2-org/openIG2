//! The MP loopback play-through (`TASKS.md` → MP loopback play-through, `docs/BUILDING.md` → Play-through harness): a
//! generated galaxy of two machines, host and joiner, each a [`World`] on the player's MP path ([`World::new_mp`],
//! [`World::tick`] pumping the session), linked by an in-process loopback. Both humans are played by the sim's AI
//! (set on both machines before the first turn, the same way, so the lockstep stays exact) or stay idle. Every
//! `compare_every` ticks both machines stop at the same turn and their snapshots of the shared state are compared;
//! the lockstep's own per-turn hashes are checked throughout. Every `save_every` months the host's MP save is read
//! back and resumed in a new room, the joiner rejoining it, and the game goes on from there. A desync ends the run
//! with the first turn whose hashes differ and the fields that differ.

use std::time::{Duration, Instant};

use ig2_net::galaxy::GalaxyOptions;
use ig2_net::lobby::{LobbyClient, LobbyHost, Settings, Status};
use ig2_net::session::MpSave;
use ig2_net::{loopback, Command, Snapshot, TURN_TICKS};
use ig2_sim::game::{choice_count, choice_disabled, dialogue_in_progress, GameOver, MessageOrder, RealTimeEvent};
use ig2_sim::save::va;

use super::{check, months, signature, End, Harness, Limits, Report};
use crate::mp_lobby::{launch, Begun, Me, Net, OfflineNet, Session};
use crate::world::World;

/// The rules fingerprint both machines share (any value: the same install on both).
const RULES: [u8; 32] = [0x6d; 32];
const NAMES: [&str; 2] = ["Host", "Guest"];

/// The machine's lockstep turn.
fn turn(w: &World) -> u64 {
    w.net.as_ref().map_or(0, |(n, _)| n.peer().turn)
}

/// The sim ticks the machine ran.
fn ticks(w: &World) -> u64 {
    w.net.as_ref().map_or(0, |(n, _)| n.peer().tick)
}

/// Pump a room until `done`, or fail after a few seconds.
fn settle(h: &mut LobbyHost, c: &mut LobbyClient, done: impl Fn(&LobbyHost, &LobbyClient) -> bool) -> Result<(), String> {
    let t = Instant::now();
    while !done(h, c) {
        h.pump().map_err(|e| e.to_string())?;
        c.pump().map_err(|e| e.to_string())?;
        if t.elapsed() > Duration::from_secs(10) {
            return Err("the room stalled".into());
        }
    }
    Ok(())
}

/// A room of two over loopback, both ready: a new galaxy's (`options`), or one seating the saved humans.
fn room(seed: u64, options: Option<GalaxyOptions>, saved: Option<&MpSave>) -> Result<(LobbyHost, LobbyClient), String> {
    let settings = Settings { name: "Play-through".into(), scenario: "galaxy".into(), private: false, password: None };
    let mut h = match saved {
        None => LobbyHost::new_galaxy(settings, seed, RULES, NAMES[0]),
        Some(s) => LobbyHost::new(settings, seed, RULES, &s.session.humans.iter().map(|h| (h.empire, 0)).collect::<Vec<_>>(), NAMES[0]),
    };
    if let Some(o) = options {
        if !h.set_options(o) {
            return Err(format!("the room refused {o:?}"));
        }
    }
    let (a, b) = loopback();
    h.admit(Box::new(b));
    let mut c = LobbyClient::join(Box::new(a), NAMES[1], RULES, None, None);
    settle(&mut h, &mut c, |h, c| h.state().members.len() == 2 && c.state.as_ref() == Some(h.state()))?;
    if let Some(s) = saved {
        let empire = |name: &str| s.session.humans.iter().find(|h| h.name == name).map(|h| h.empire);
        h.pick(0, empire(NAMES[0]));
        c.pick(empire(NAMES[1]));
        settle(&mut h, &mut c, |h, _| h.state().empire_of(1) == empire(NAMES[1]))?;
    }
    c.ready(true);
    settle(&mut h, &mut c, |h, _| h.state().members[1].ready)?;
    h.set_ready(0, true);
    Ok((h, c))
}

/// Both machines' worlds from a ready room: the host launches first (its start sends the joiner its `Start`). Before
/// the first turn the harness writes its own set-up into the shared state on both machines alike: the fastest speed
/// and (`autopilot`) the AI on both humans' empires (`Game::autopilot`; a resumed save already holds it).
fn start<'r>(hx: &'r Harness, texts: &crate::new_game::StartTexts, (lobby, mut c): (LobbyHost, LobbyClient), save: Option<MpSave>, seed: u32, autopilot: bool) -> Result<[World<'r>; 2], String> {
    let rules = &hx.rules;
    let host = Begun::Host { lobby: Box::new(lobby), listener: OfflineNet.listen()?, save: save.map(Box::new) };
    let host = texts.with(&hx.install, |data| launch(host, data, rules, seed, Box::new(OfflineNet)))??;
    let t = Instant::now();
    while c.status != Status::Started {
        c.pump().map_err(|e| e.to_string())?;
        if t.elapsed() > Duration::from_secs(10) {
            return Err("the joiner got no start".into());
        }
    }
    let (s, link) = c.into_game().ok_or("the joiner's start")?;
    let joiner = Begun::Joiner { start: Box::new(s), link, me: Me { name: NAMES[1].into(), password: None, rules: RULES } };
    let joiner = texts.with(&hx.install, |data| launch(joiner, data, rules, seed, Box::new(OfflineNet)))??;
    let mut sessions = [host, joiner];
    let humans = sessions.each_ref().map(|s| s.peer().empire);
    for s in &mut sessions {
        let g = &mut s.peer_mut().game;
        g.empire_names = hx.empire_names.clone();
        // As the speed message every machine gets (`ProgramSebesseg` 4: the players choose).
        g.set_speed(3, true);
        for e in humans {
            if autopilot && g.state.mem.u8(va::PLAYER_KI + e as u32) != 2 {
                g.autopilot(e as usize);
            }
        }
    }
    Ok(sessions.map(|s| World::new_mp(s, rules)))
}

/// The messages screen, one step a visit (its orders run `delay` turns later on every machine): a dialogue typed
/// out and its first allowed answer, else the automatic messages and the first unread one opened.
fn visit_inbox(w: &mut World) -> u32 {
    if dialogue_in_progress(&w.game.state).is_some() {
        w.send(MessageOrder::TypedOut.into());
        if let Some(k) = (0..choice_count(&w.game.state)).find(|&k| !choice_disabled(&w.game.state, k)) {
            w.send(MessageOrder::Answer { k: k as i32 }.into());
            return 1;
        }
        return 0;
    }
    w.send(MessageOrder::Enter.into());
    w.send(MessageOrder::Auto.into());
    let b = w.game.state.inbox();
    if let Some(i) = (0..b.count.max(0) as usize).find(|&i| b.messages.get(i * ig2_sim::messages::MESSAGE + 4) == Some(&0)) {
        w.send(MessageOrder::Scroll { pointer: i as i32, row: 0 }.into());
        w.send(MessageOrder::Open { row: 0 }.into());
    }
    0
}

/// The first turn whose hashes the two machines both have and disagree on.
fn first_divergence(a: &[(u64, u64)], b: &[(u64, u64)]) -> Option<u64> {
    b.iter().filter(|(t, h)| a.iter().any(|(t0, h0)| t0 == t && h0 != h)).map(|(t, _)| *t).min()
}

/// The lockstep's desync reports on either machine.
fn desyncs(worlds: &[World; 2]) -> Vec<String> {
    let mut out = Vec::new();
    for w in worlds {
        let Some((net, _)) = &w.net else { continue };
        out.extend(net.peer().desyncs.iter().map(|d| format!("{d:?}")));
        if let Session::Host { session, .. } = net {
            out.extend(session.host.desyncs.iter().map(|d| format!("{d:?}")));
        }
    }
    out
}

/// The compare: both snapshots at the same turn. `Err`: where they differ.
pub(super) fn compare(a: &Snapshot, b: &Snapshot, rules: &ig2_sim::game::GameRules) -> Result<(), String> {
    if a.turn != b.turn {
        return Err(format!("compared at turns {} and {}", a.turn, b.turn));
    }
    if a.hash == b.hash {
        return Ok(());
    }
    let lines = ig2_net::snapshot::diff(a, b, rules, 40).map_err(|e| e.to_string())?;
    Err(format!("turn {} (tick {}): {}", a.turn, a.tick, lines.join("; ")))
}

pub(super) fn play(hx: &Harness, ais: u8, planets: u8, limits: Limits, report: &mut Report) -> End {
    let texts = match crate::new_game::StartTexts::read(&hx.install) {
        Ok(t) => t,
        Err(e) => return End::Error(e),
    };
    let options = GalaxyOptions { planets, ais, battle_status: 0, ..GalaxyOptions::default() };
    let mut worlds = match room(limits.seed as u64 | 1, Some(options), None).and_then(|r| start(hx, &texts, r, None, limits.seed, limits.autopilot)) {
        Ok(w) => w,
        Err(e) => return End::Error(e),
    };
    report.human = worlds[0].net.as_ref().unwrap().0.peer().empire as usize;
    let every = (limits.compare_every / TURN_TICKS as u64).max(1);
    let mut next_compare = turn(&worlds[0]) + every;
    let mut compares = 0u64;
    // The turns the joiner hashed (the host checks each against its own), over every session of the run.
    let hashes = |w: &World| w.net.as_ref().map_or(0, |(n, _)| n.peer().hashes.len());
    let mut hashed = 0;
    let mut ever_alive = worlds[0].game.state.alive();
    let mut progress = (signature(&worlds[0].game), months(&worlds[0].game));
    let mut next_save = months(&worlds[0].game) + limits.save_every;
    // A real-time space battle each machine is on (left when it is over).
    let mut battle = [false; 2];
    let mut round = 0u64;
    let mut clock = (turn(&worlds[0]), Instant::now());
    let end = loop {
        round += 1;
        let tick = ticks(&worlds[0]);
        report.ticks = tick;
        if tick >= limits.ticks {
            break End::Budget;
        }
        // Each machine runs to the compare turn, no further: the host can't outrun the joiner's inputs, so both get there.
        for (k, w) in worlds.iter_mut().enumerate() {
            if turn(w) >= next_compare {
                continue;
            }
            if round % 400 == k as u64 && !battle[k] {
                report.answers += visit_inbox(w);
            }
            w.pump();
            w.tick();
            for r in std::mem::take(&mut w.realtime) {
                match r {
                    RealTimeEvent::SpaceStarted { .. } => {
                        if k == 0 {
                            report.client("real-time space battle");
                        }
                        battle[k] = true;
                    }
                    RealTimeEvent::SpaceOver { .. } => {
                        w.send(Command::SpaceBattleLeft);
                        battle[k] = false;
                    }
                    RealTimeEvent::GroundStarted { .. } if k == 0 => report.client("real-time ground battle"),
                    _ => {}
                }
            }
            for v in std::mem::take(&mut w.videos) {
                if k == 0 {
                    report.client(format!("video {v:?}"));
                }
            }
            while w.pop_message().is_some() {}
            w.local_lines.clear();
            w.chat_lines.clear();
            if w.net_over {
                return End::Error(format!("machine {k}'s session ended at turn {}", turn(w)));
            }
        }
        let d = desyncs(&worlds);
        if !d.is_empty() {
            let at = first_divergence(&worlds[0].net.as_ref().unwrap().0.peer().hashes, &worlds[1].net.as_ref().unwrap().0.peer().hashes);
            report.problem(tick, format!("desync: first differing turn {at:?}; {}", d.join("; ")));
            break End::Error("desync".into());
        }
        if turn(&worlds[0]) < next_compare || turn(&worlds[1]) < next_compare {
            if turn(&worlds[0]) != clock.0 {
                clock = (turn(&worlds[0]), Instant::now());
            } else if clock.1.elapsed() > Duration::from_secs(30) {
                break End::Error(format!("the session stalled at turn {} (joiner at {})", clock.0, turn(&worlds[1])));
            }
            continue;
        }

        // Both at the compare turn.
        next_compare += every;
        compares += 1;
        let [a, b] = &mut worlds;
        let (sa, sb) = (a.mp_snapshot().unwrap(), b.mp_snapshot().unwrap());
        if let Err(e) = compare(&sa, &sb, &hx.rules) {
            let at = first_divergence(&a.net.as_ref().unwrap().0.peer().hashes, &b.net.as_ref().unwrap().0.peer().hashes);
            report.problem(tick, format!("the machines differ at {e} (first differing turn hash: {at:?})"));
            break End::Error("desync".into());
        }
        for (e, alive) in worlds[0].game.state.alive().iter().enumerate() {
            ever_alive[e] |= *alive;
        }
        check(&worlds[0].game, tick, &ever_alive, report);
        if compares % 50 == 0 && std::env::var_os("IG2_PLAYTHROUGH_DEBUG").is_some() {
            let g = &worlds[0].game;
            let (colony, map, money) = (g.state.colony(), g.state.starmap(), g.state.treasuries());
            let of = |e: usize| (colony.planets.iter().filter(|p| p.owner as usize == e).count(), map.fleets.iter().filter(|f| f.owner as usize == e).count(), money[e].total as i64, g.state.mem.u8(va::PLAYER_KI + e as u32), g.state.mem.u8(va::AI_EMPIRE + e as u32));
            eprintln!("{:?} (planets, fleets, money, ki, ai): {:?}", super::date(g), (1..9).filter(|&e| g.state.alive()[e]).map(|e| (e, of(e))).collect::<Vec<_>>());
        }
        for (k, w) in worlds.iter().enumerate() {
            if let Some(o) = w.game.over {
                let e = w.net.as_ref().unwrap().0.peer().empire;
                report.note(tick, format!("machine {k} (empire {e}, alive {}): {o:?}", worlds[0].game.state.alive()[e as usize]));
            }
        }
        // A machine's own end (`Game::over`: its human won), or no human left.
        if let Some(o) = worlds.iter().find_map(|w| w.game.over.filter(|o| *o != GameOver::Lost)) {
            break End::Over(o);
        }
        let alive = worlds[0].game.state.alive();
        if worlds.iter().all(|w| !alive[w.net.as_ref().unwrap().0.peer().empire as usize]) {
            break End::Over(GameOver::Lost);
        }
        let sig = signature(&worlds[0].game);
        if sig != progress.0 {
            progress = (sig, months(&worlds[0].game));
        } else if months(&worlds[0].game) - progress.1 >= limits.stall {
            break End::Stalled;
        }
        if limits.save_every > 0 && months(&worlds[0].game) >= next_save && !battle.iter().any(|b| *b) && !worlds[0].game.realtime_running() {
            next_save = months(&worlds[0].game) + limits.save_every;
            hashed += hashes(&worlds[1]);
            match resume(hx, &texts, &mut worlds, limits, tick, report) {
                Ok(w) => worlds = w,
                Err(e) => return End::Error(e),
            }
        }
    };
    report.note(0, format!("{compares} state compares at the same turn, {} turns hashed alike", hashed + hashes(&worlds[1])));
    let w = &worlds[0];
    report.date = super::date(&w.game);
    let colony = w.game.state.colony();
    let alive = w.game.state.alive();
    report.empires = (1..alive.len()).filter(|&e| alive[e]).map(|e| (e, colony.planets.iter().skip(1).filter(|p| p.owner as usize == e).count())).collect();
    end
}

/// The host's MP save, read back and checked, resumed in a new room the joiner rejoins: the game goes on there.
fn resume<'r>(hx: &'r Harness, texts: &crate::new_game::StartTexts, worlds: &mut [World<'r>; 2], limits: Limits, tick: u64, report: &mut Report) -> Result<[World<'r>; 2], String> {
    let file = worlds[0].mp_save().ok_or("no MP save")??;
    let save = MpSave::read(&file, RULES).map_err(|e| format!("the MP save doesn't read back: {e}"))?;
    let (g, pending) = save.snapshot.restore(&hx.rules).map_err(|e| e.to_string())?;
    if ig2_net::state_hash(&g, &pending) != save.snapshot.hash {
        report.problem(tick, "the MP save doesn't restore its own state");
    }
    let at = save.header.turn;
    let again = start(hx, texts, room(limits.seed as u64 ^ at, None, Some(&save))?, Some(save), limits.seed, limits.autopilot)?;
    if turn(&again[0]) != at || turn(&again[1]) != at {
        report.problem(tick, format!("the resumed game starts at turns {} and {}, not the saved {at}", turn(&again[0]), turn(&again[1])));
    }
    report.saves += 1;
    Ok(again)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_diverge_at_their_first_differing_turn() {
        let a = [(1, 10), (2, 20), (3, 30), (4, 40)];
        assert_eq!(first_divergence(&a, &[(2, 20), (3, 31), (4, 41)]), Some(3));
        assert_eq!(first_divergence(&a, &[(2, 20), (5, 99)]), None, "turns only one machine ran don't count");
    }

    /// The compare path: a short 2-machine game agrees; the same snapshot with one byte of the shared state changed
    /// is reported with its turn, part and address.
    #[test]
    fn the_compare_finds_a_changed_byte() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let hx = Harness::new(install).unwrap();
        let texts = crate::new_game::StartTexts::read(&hx.install).unwrap();
        let options = GalaxyOptions { planets: 20, ais: 1, ..GalaxyOptions::default() };
        let mut worlds = start(&hx, &texts, room(7, Some(options), None).unwrap(), None, 7, true).unwrap();
        while worlds.iter().any(|w| turn(w) < 40) {
            for w in worlds.iter_mut().filter(|w| turn(w) < 40) {
                w.pump();
                w.tick();
            }
        }
        let [a, b] = &mut worlds;
        let (sa, sb) = (a.mp_snapshot().unwrap(), b.mp_snapshot().unwrap());
        assert_eq!(compare(&sa, &sb, &hx.rules), Ok(()));
        assert!(desyncs(&worlds).is_empty());
        let (mut g, pending) = sb.restore(&hx.rules).unwrap();
        g.state.mem.set_i32(va::EV, g.state.mem.i32(va::EV) + 1);
        let bad = Snapshot::capture(&g, &pending, sb.turn, sb.tick);
        let e = compare(&sa, &bad, &hx.rules).unwrap_err();
        assert!(e.starts_with("turn 40") && e.contains("\"mem\"") && e.contains(&format!("{:#x}", va::EV)), "{e}");
    }
}
