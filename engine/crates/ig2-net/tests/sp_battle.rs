//! Single player's own real-time battle (`re/rules/space-combat.md` §4 step 6, §7.1, §7.11, §7.12;
//! `ig2_sim::game::SpBattles`): with the client's battle screen on, a real-time route starts the battle paused at
//! speed 1, the battle ticks only while the screen is up, the human's orders arrive as messages, the strategic clock
//! runs on beside it (SP has no battle-in-progress flag), and leaving the finished battle's screen ends it.

mod common;

use ig2_net::{BattleOrder, Command};
use ig2_sim::game::{Game, GameEvent, RealTimeEvent};
use ig2_sim::save::va;
use ig2_sim::spacebattle::Order;

#[test]
fn a_single_player_battle_runs_on_the_battle_screen() {
    let Some((rules, state)) = common::setup("quick-1.sav") else { return };
    let mut state = common::space_clash(state);
    // Back to single player: 6 is the human, 3 the AI; speed ×1.
    state.mem.set_u8(va::MULTIPLAYER, 0);
    state.mem.set_u8(va::PLAYER_KI + 3, 0);
    state.mem.set_u8(va::PLAYER_KI + 6, 1);
    state.mem.set_i32(va::EN, 6);
    let mut g = Game::new(state, &rules);
    g.saved();
    g.set_speed(1, false);
    g.sp_battles.live = true;
    let started = |ev: &[GameEvent]| ev.iter().find_map(|e| if let GameEvent::RealTime(RealTimeEvent::SpaceStarted { slot }) = e { Some(*slot) } else { None });
    let mut slot = None;
    for _ in 0..400 {
        g.advance_clock();
        g.run_tick();
        let ev = g.take_events();
        if let Some(s) = started(&ev) {
            slot = Some(s);
            break;
        }
    }
    let b = slot.expect("the attack started a real-time battle");
    assert!(g.paused(), "SpaceWarIndit pauses (SetIdoPause(true))");
    assert_eq!(g.state.mem.i32(va::IDO_SPEED), 1, "SetIdoSpeed(1)");
    assert!(g.realtime.slots[b].active && g.realtime.slots[b].shown);
    // Off the battle screen the battle waits.
    g.set_pause(false, false);
    let tick0 = g.realtime.slots[b].tick;
    g.advance_clock();
    g.run_tick();
    assert_eq!(g.realtime.slots[b].tick, tick0, "FRISSIT_SpaceWar runs on screen 0xB only");
    g.sp_battles.on_screen = true;
    g.sp_battles.fade = 1.0;
    let me = 6;
    let battle = &g.realtime.slots[b];
    let mine: Vec<usize> = (0..battle.units.len()).filter(|&u| battle.units[u].alive && battle.units[u].owner == me).collect();
    let theirs: Vec<usize> = (0..battle.units.len()).filter(|&u| battle.units[u].alive && battle.units[u].owner > 0 && battle.units[u].owner != me).collect();
    assert!(!mine.is_empty() && !theirs.is_empty(), "both sides on the field");
    let o = |order| Command::SpaceOrder { battle: b as u8, order };
    o(BattleOrder::Attack { unit: mine[0] as u16, target: theirs[0] as u16 }).apply(&mut g, me as u8);
    assert_eq!(g.realtime.slots[b].units[mine[0]].target, Some(theirs[0]), "the human's attack order");
    // Unpaused, the attacker closes on its target (`attack_order`, then the battle's movement).
    let gap = |g: &Game| {
        let s = &g.realtime.slots[b];
        let (p, q) = (s.units[mine[0]].pos, s.units[theirs[0]].pos);
        ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt()
    };
    let gap0 = gap(&g);
    g.set_pause(false, false);
    for _ in 0..100 {
        g.advance_clock();
        g.run_tick();
    }
    assert!(g.realtime.slots[b].f2d86, "the first unpaused tick ends the formation phase (+0x2d86)");
    assert!(gap(&g) < gap0 - 5.0, "the attacker moves to its target: {gap0} -> {}", gap(&g));
    // Another empire's unit, or an order from the AI's empire: refused.
    assert!(!g.space_order(me as u8, b, Order::Attack { unit: theirs[0], target: mine[0] }));
    assert!(!g.space_order(3, b, Order::Attack { unit: theirs[0], target: mine[0] }));
    let clock = g.state.mem.i32(va::IDO);
    let mut over = false;
    for _ in 0..20000 {
        g.advance_clock();
        g.run_tick();
        if g.take_events().iter().any(|e| matches!(e, GameEvent::RealTime(RealTimeEvent::SpaceOver { .. }))) {
            over = true;
            break;
        }
    }
    assert!(over, "the battle ended");
    // `Fake_FRISSITES` freezes the strategic time while the SP battle screen is up (time-and-rng.md §1; obs fx8).
    assert_eq!(g.state.mem.i32(va::IDO), clock, "SP freezes the strategic clock on the battle screen");
    assert!(g.realtime.slots[b].active, "the slot stays until the screen is left");
    Command::SpaceBattleLeft.apply(&mut g, me as u8);
    assert!(!g.realtime.slots[b].active, "SpaceWarVege closed the slot");
    assert!(g.take_events().iter().any(|e| matches!(e, GameEvent::RealTime(RealTimeEvent::SpaceEnded { .. }))));
}
