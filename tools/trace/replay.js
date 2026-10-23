// Frida agent: run the GOG ig2.exe forward from an original save and write a save at every sim tick boundary, so
// the port can replay the same ticks from the same save (engine/crates/ig2-sim/tests/save_replay.rs).
// It starts scenario/quick.scn (to reach the starmap), then loads save<slot> with the exe's own
// `LoadSave(slot, name, 1)` (exe 0x543c80) from the frame hook, sets the speed and writes save `k = 0` at once.
// From then on, at every entry of `IdoNapValtozott` (exe 0x53e970: called once per strategic tick, right after
// `FRISSIT_Datum` advanced the clock and before anything else of the tick runs) it pins libc rand() with
// `srand(seed + Ido2)` and writes save k = 1, 2, ... with `LoadSave(100 + k, name, 2)`; so save k holds the state
// after tick k-1 (whole) plus tick k's clock step. With each save it reports a set of globals that the save
// doesn't carry ('vol'). Addresses are image VAs, rebased at runtime. Output holds game state: keep it under
// _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  LoadSave: va(0x543c80), IdoNapValtozott: va(0x53e970),
  Ido: va(0xbae04c), Ido2: va(0xbae050), AktKep: va(0xbae0fc), AktKepKell: va(0xbae250), AlapNev: va(0xbb6964),
};

// Globals outside the save stream that the ported systems read (name, VA, size).
const VOLATILE = [
  ['speed', 0xbae25c, 4], ['pause', 0xbae264, 4], ['aktkep', 0xbae0fc, 4], ['mp', 0xbae2c0, 4],
  ['speedlock', 0xbad0d4, 4], ['aioff', 0xb77244, 4], ['alltechs', 0xb77254, 4], ['cheat', 0xb7727c, 4],
  ['debugx', 0xbae587, 1], ['debugspeed', 0xb77240, 4], ['forced_answer', 0xb7724c, 4], ['spy_forced', 0xb77250, 4],
  ['ddebug', 0xa3f0b0, 4], ['tut_active', 0xf6915d, 1], ['tut_paused', 0xf66f2a, 1], ['tut_event', 0xf66f40, 4],
  ['tut_norecruit', 0xf66f2b, 1],
  ['rnd', 0xbaeb64, 4], ['rndossz', 0xbaebb4, 4],
  ['radar2', 0xbb61a8, 0x260], ['radar2sq', 0xbb6408, 0x260], ['radarsz', 0xbb6668, 0x4c], ['radarszsq', 0xbb66b8, 0x4c],
  ['dipnap', 0xab6ffc, 4], ['dipstart', 0xab69b5, 1],
  // Story engine statics (re/rules/campaign.md §2).
  ['st_flags', 0xf2f8ac, 17], ['st_last', 0xf2273c, 4], ['st_oldido', 0xf432b4, 4], ['st_szvegzett', 0xf26560, 4],
  ['st_delayed', 0xf26530, 1], ['st_restart', 0xf261fc, 4], ['st_skip', 0xf23a04, 4], ['st_err', 0xf23a1c, 4],
  ['st_signal', 0xf26564, 8], ['st_addship', 0xf26200, 4], ['st_special', 0xf265a4, 32], ['st_radar', 0xf2657c, 8],
  ['st_dipl', 0xf2656c, 8], ['st_diplvalasz', 0xf42660, 4], ['st_arr', 0xf2654c, 20], ['st_gr', 0xf26538, 20],
  ['st_youwas', 0xf26531, 1], ['st_grwin', 0xf26532, 2], ['st_kem', 0xf26588, 4], ['st_spw', 0xf265c4, 8],
  ['st_locked', 0xf265dc, 4],
  // Colony side: AutoBuildMitEpit, NextColonyMap's cursor, HanyTradePortVan, TalalmanyEroHP.
  ['abmitepit', 0x996530, 0x80], ['mapcursor', 0x996420, 8], ['tradeports', 0xbb23f4, 0x24], ['erohp', 0xba9210, 193 * 4],
  // Unsaved bytes inside saved blocks: the tail of contact slot -1, the cell between DiplomaciaMikorHivtaUtoljara
  // and MiTervMutato.
  ['slotm1tail', 0xa41e94, 4], ['mikorgap', 0xdaf664, 4],
  ['battlespeed', 0xbae260, 4],
  // BirodalomMIzik (the AI plays the empire), BirodalomTamadhat (may attack): set by the scenario, not saved.
  ['mizik', 0x915f58, 19], ['tamadhat', 0x915f44, 19],
  // FRISSIT_Trade (re/rules/trade.md): OldEtaHonap, AktEtaDays, TradeCheat, StoryDesignDataMut and
  // StoryDesignElsoTerv (the records themselves go with the first dump).
  ['etahonap', 0x928db0, 4], ['etadays', 0xf43428, 4], ['tradecheat', 0xf43835, 1], ['stdesignn', 0xf419d0, 4],
  ['stdesignfirst', 0xf419d4, 4],
  // re/rules/info-messages.md: InfoOldHonap; the scrolling line's text and flag (UI state the posts read).
  ['infomonth', 0xb84c78, 4], ['scrolltext', 0xe41230, 0xc8], ['scrollimp', 0xe412f8, 1],
  // The battles (info-messages.md §3.2): the debug switches 0xB77230..0xB7723F (always simulate, never lopsided, ...),
  // MultiplayerBattleStatusz, the story's battle bans, spFlottak/SpWarHanyFlotta/spKozeliBolygo/SpWarKiNyert,
  // spwsflotta and the CTF values.
  ['battledebug', 0xb77230, 0x10], ['mpbattle', 0xbae558, 4], ['st_bans', 0xf42664, 0x10], ['spw_fleets', 0xbaa1c0, 0x30],
  ['spw_planetflag', 0xb9c027, 1], ['spwsflotta', 0xf21260, 0x60], ['ctf', 0xf261f0, 8],
  // The battlefields (re/formats/save.md 4.13): melyikhatter's cursor (the economy prologue's background picker),
  // FRISSIT_Grwar's gvalue counter, the MP space battle flag that skips FRISSIT_Grwar.
  ['bgcursor', 0x996488, 4], ['gvalue', 0x9b2900, 4], ['mpspacewar', 0xbad114, 4],
  // The messages screen's unsaved dialogue bits (info-messages.md §7): ParbeszedVege/ParbeszedVegeJon, KiirtaMar,
  // the rows (first unread, AktUzenet0to3, AktValasz0to3, the replay index), AktValasztasok, the first unread's
  // id, the open block's four choices (part, next block, disabled; ParbeszPlace follows at 0xD8E280).
  ['msg_over', 0xd909ba, 2], ['msg_typed', 0xda6607, 1], ['msg_rows', 0xd90dc0, 0x18], ['msg_nchoices', 0xdabe3c, 4],
  ['msg_unread', 0x91494c, 4], ['msg_ch0', 0xd8e160, 9], ['msg_ch1', 0xd8e1bc, 9], ['msg_ch2', 0xd8e218, 9],
  ['msg_ch3', 0xd8e274, 9],
];
// Story queues outside the save ({ids, dual, n}).
const QUEUES = [['vegzes', 0xf43290], ['talal', 0xf4329c]];

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, slot: 99, name: 'Replay', speed: 3, ticks: 40,
  seed: 4242, stage: false, traders: false, spies: false, spiesMode: 'base', merges: false, wormholes: false, battles: false, battlesMode: 'space', deaths: false, deathsMode: 'base', messages: false, messagesMode: 'base', scroll2: false, tutorialSighting: false, slotBase: 100, preSlot: 98, lopsidedTanks: 40, evenTanks: 16, lostTanks: 3, landShips: 30,
 traderNear: 0.5, storyCountdown: 100, aiEmpire: 3, randlog: false, extraOffers: 5, orders: 0, ordersEvery: 3, colony: 0, research: 0, researchEvery: 2, info: 0, infoEvery: 2, answers: 0, answersEvery: 2 };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 4) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

function hexOf(p, size) {
  const b = new Uint8Array(p.readByteArray(size));
  let s = '';
  for (let i = 0; i < b.length; i++) s += (b[i] < 16 ? '0' : '') + b[i].toString(16);
  return s;
}

const nf = (p, r, a) => new NativeFunction(p, r, a, { abi: 'mscdecl', traps: 'all' });
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const setPause = nf(A.SetIdoPause, 'void', ['int', 'int']);
const loadSave = nf(A.LoadSave, 'void', ['int', 'pointer', 'int']);
const crt = Process.getModuleByName('msvcr120.dll');
const srand = nf(crt.getExportByName('srand'), 'void', ['uint']);

function volatile(full) {
  const v = { mem: VOLATILE.map(([n, a, sz]) => [n, a, hexOf(va(a), sz)]), queues: {} };
  if (full) {
    v.mem.push(['techs', 0xb9c0c8, hexOf(va(0xb9c0c8), 193 * 0x114)]);
    const n = va(0xf419d0).readS32();
    if (n > 0) v.mem.push(['stdesigns', 0xf2f8c0, hexOf(va(0xf2f8c0), n * 0x94)]);
  }
  for (const [n, a] of QUEUES) {
    const q = va(a), ids = q.readPointer(), dual = q.add(4).readPointer(), cnt = q.add(8).readS32(), out = [];
    for (let i = 0; i < cnt; i++) out.push([ids.add(4 * i).readS32(), dual.add(4 * i).readS32()]);
    v.queues[n] = out;
  }
  const p = A.AlapNev.readPointer();
  v.alapnev = p.isNull() ? '' : p.readCString();
  // BirodalomNev[e] (0xBB6730, char *[19]): the empire names messages quote, as hex (the game's code page).
  v.empnames = [];
  for (let e = 0; e < 19; e++) {
    const q = va(0xbb6730).add(4 * e).readPointer();
    let n = 0;
    if (!q.isNull()) while (n < 64 && q.add(n).readU8() !== 0) n++;
    v.empnames.push(n ? hexOf(q, n) : '');
  }
  // KemCsaladtag_Nev[x] (0xBB6968, char *[6]): the family members the kidnap message names (info-messages.md §3.3).
  v.famnames = [];
  for (let x = 0; x < 6; x++) {
    const q = va(0xbb6968).add(4 * x).readPointer();
    let n = 0;
    if (!q.isNull()) while (n < 64 && q.add(n).readU8() !== 0) n++;
    v.famnames.push(n ? hexOf(q, n) : '');
  }
  return v;
}

// --stage: exercise the trade market (re/rules/trade.md) from the loaded save. The edits go into save 'pre'
// (pre.sav); then the human buys twice through the exe's own MULTIGET_Trade (0x6070b0), whose two Multi_Get calls
// (0x513160) are pointed at a stub handing it one staged message (Multi_Send only queues for a later frame);
// save 0 follows. Each purchase is reported as {k: 'buy', e, msg} with the 0x98-byte message.
const T = {
  EN: va(0x912a3c), HanyBolygo: va(0x912ac8), Bolygok: va(0xb85ea8), Tervek: va(0xb9aa18), OsszHanyTerv: va(0xb9c028),
  N: va(0xf439cc), Offers: va(0xb9aaf0), Count: va(0xf66de0), Deliveries: va(0xf439d0), Temp: va(0xf43388),
  EtaDays: va(0xf43428), StoryC4: va(0xf274bc), Slots: va(0xba9840), Epuletek: va(0xba9e90),
  MultiGet: va(0x513160), MultigetTrade: va(0x6070b0), TradeAr: va(0x608830),
  getCalls: [va(0x6070c8), va(0x607262)],
};
const multigetTrade = nf(T.MultigetTrade, 'void', []);
const tradeAr = nf(T.TradeAr, 'int', ['int', 'int']);
const design = (e, d) => T.Tervek.add(4 * e).readPointer().add(d * 0x84);
const designs = (e) => T.OsszHanyTerv.add(4 * e).readS32();
const owner = (p) => T.Bolygok.add(p * 0x188 + 0x20).readU8();

function addDelivery(e, planet, days, units, price, rec) {
  const n = T.Count.add(4 * e).readS32();
  const d = T.Deliveries.add((e * 0x32 + n) * 0x98);
  d.writeByteArray(new Array(0x98).fill(0));
  d.writeS32(planet); d.add(4).writeS32(days); d.add(8).writeS32(-1); d.add(0xc).writeS32(units);
  d.add(0x10).writeS32(price);
  d.add(0x14).writeByteArray(rec.readByteArray(0x84));
  T.Count.add(4 * e).writeS32(n + 1);
}

function findDesign(e, test) {
  for (let d = 1; d <= designs(e); d++) {
    const r = design(e, d);
    if (r.add(0x16).readS16() >= 0 && test(r)) return r;
  }
  return null;
}

function stageTrade() {
  const en = T.EN.readS32(), ai = cfg.aiEmpire, out = { en, ai };
  const mine = [], theirs = [];
  for (let p = 1; p <= T.HanyBolygo.readS32(); p++) {
    if (owner(p) === en) mine.push(p);
    else if (owner(p) === ai) theirs.push(p);
  }
  out.mine = mine; out.theirs = theirs;
  // A working trade port (kind 50) on the human's last planet: re-targeting lands there.
  if (mine.length > 1) {
    const p = mine[mine.length - 1], slots = T.Slots.add(4 * p).readS32(), b = T.Epuletek.add(4 * p).readPointer();
    for (let j = slots; j >= 1; j--) {
      const r = b.add(j * 0x20);
      if (r.readU8() !== 0) { r.writeU8(50); r.add(8).writeS16(0); r.add(0x19).writeU8(r.add(0x19).readU8() | 1); out.port = [p, j]; break; }
    }
  }
  T.StoryC4.writeS32(cfg.storyCountdown);
  const plain = (r) => r.add(0x16).readS16() === 1 && r.add(0x34).readU32() === 0 && r.add(0x38).readU8() === 0;
  const ship = findDesign(en, plain), tank = findDesign(en, (r) => r.add(0x16).readS16() === 2);
  const fighter = findDesign(en, (r) => r.add(0x16).readS16() === 1 && r.add(0x36).readU8() !== 0);
  const foreign = findDesign(ai, plain);
  // A few more offers (copies of designs, 2 units each), then every offer a year older: past the 180-day
  // limit, so the daily expiry draws apply.
  for (const r of [ship, tank, foreign, ship, tank].slice(0, cfg.extraOffers)) {
    const n = T.N.readS32();
    if (!r || n >= 40) continue;
    const o = T.Offers.add(n * 0x84);
    o.writeByteArray(r.readByteArray(0x84));
    o.add(0x6c).writeS32(2); o.add(0x74).writeS32(A.Ido.readS32() / 18000 | 0); o.add(0x78).writeS32(5); o.add(0x7c).writeS32(1);
    T.N.writeS32(n + 1);
  }
  for (let i = 0; i < T.N.readS32(); i++) { const r = T.Offers.add(i * 0x84 + 0x74); r.writeS32(r.readS32() - 1); }
  const h = mine[0];
  if (ship) {
    addDelivery(en, h, 1, 2, 1000, ship);
    addDelivery(en, h, 0, 1, 500, ship);
    addDelivery(en, h, -1, 1, 500, ship);
    if (theirs.length) addDelivery(en, theirs[0], 5, 1, 700, ship);
  }
  if (foreign) addDelivery(en, h, 2, 1, 900, foreign);
  if (tank) addDelivery(en, h, 3, 3, 600, tank);
  if (fighter) addDelivery(en, h, 2, 1, 300, fighter);
  if (foreign) addDelivery(ai, h, 4, 2, 1200, foreign);
  out.designs = [!!ship, !!tank, !!fighter, !!foreign];
  return out;
}

// --traders: make the traders' income (re/rules/fleet-movement.md §4.3) non-zero. Every planet of empires 1..=8
// gets its last finished non-hub building turned into a working trade centre (kind 48) and the one before into a
// bank (49); traders flying to a planet are moved `traderNear` units short of it with a speed override (+0x3c) of 2,
// so they arrive in the first ticks (the next economy step may switch the new buildings off). Done before save 0, so the save carries it.
const F = { Flottak: va(0xbb40a4), HanyFlotta: va(0xbb40a8) };
function stageTraders() {
  const out = { centres: [], banks: [], moved: [] };
  const working = (r) => { r.add(8).writeS16(0); r.add(0x19).writeU8(r.add(0x19).readU8() | 1); };
  for (let p = 1; p <= T.HanyBolygo.readS32(); p++) {
    const o = owner(p);
    if (o < 1 || o > 8) continue;
    const slots = T.Slots.add(4 * p).readS32(), b = T.Epuletek.add(4 * p).readPointer();
    const free = [];
    for (let j = slots; j >= 1 && free.length < 2; j--) {
      const r = b.add(j * 0x20), kind = r.readU8();
      if (kind !== 0 && kind !== 1 && r.add(8).readS16() === 0) free.push(r);
    }
    if (free.length) { free[0].writeU8(48); working(free[0]); out.centres.push(p); }
    if (free.length > 1) { free[1].writeU8(49); working(free[1]); out.banks.push(p); }
  }
  const fl = F.Flottak.readPointer(), n = F.HanyFlotta.readS32();
  for (let f = 1; f < n; f++) {
    const r = fl.add(f * 0x80), id = r.add(0x16).readS16();
    if (r.add(0x15).readU8() !== 9 || id < 10000 || id >= 11000) continue;
    if (r.add(0x40).readS16() !== 1 || r.add(0x42).readS16() !== 1 || r.add(0x50).readS16() !== 0) continue;
    const t = r.add(0x44).readFloat() | 0, pl = T.Bolygok.add(t * 0x188);
    const tx = pl.add(0x18).readFloat(), ty = pl.add(0x1c).readFloat();
    const x = r.add(0x30).readFloat(), y = r.add(0x34).readFloat(), d = Math.hypot(x - tx, y - ty);
    if (d <= cfg.traderNear) continue;
    r.add(0x30).writeFloat(tx + (x - tx) * cfg.traderNear / d);
    r.add(0x34).writeFloat(ty + (y - ty) * cfg.traderNear / d);
    r.add(0x3c).writeFloat(2);
    out.moved.push([f, t]);
  }
  return out;
}

// --spies: stage the spy reports and messages (re/rules/info-messages.md §3.3) for the first daily spy passes. New
// spies are appended to the rosters (Kemek 0xBAFCC8 [9][13] x 0x4c, OsszHanyKem 0xBB1F84). --spies-mode picks the
// mix; the force modes also set the SP debug switch 0xB77250 (espionage.md §1.1; saved, so the port reads it):
//   base   for the human a mission that ends that day (empire report on the AI), one whose catch check falls on it
//          against an AI counter-spy, one returning, one ranking up several times, a double agent its own
//          counter-spy exposes, a counter-spy and one at home; for the AI a counter-spy and missions against the
//          human that end that day (money, sabotage, rebellion, assassination, kidnap, mass unrest) plus one whose
//          catch check meets the human's counter-spy.
//   force1 every mission succeeds: the human's reports (done), its spy coming home, and the AI's money (the
//          human gets money), ship (the human gets a ship), sabotage, rebellion, kidnap and mass-unrest missions;
//          the assassination ends a day later so it can't pick the returning spy first.
//   force2 every spy is caught: six of the human's spies meet their catch check against the AI; the AI's roster
//          (at-home spies) covers their slots k, whose loyalty (the captor's roll reads them) alternates 0 / 255,
//          so turned, double-agent, released and executed fates all come up.
//   lost   the AI's spies meet the human's counter-spy in duels tuned to the "identified and lost" outcome, which
//          needs three living empires knowing the spy: the human's kind-4 report and post 104.
//   force3 a recruit every day: the human's roster has room, so it gets an applicant; one spy returns home.
const K = { Kemek: va(0xbafcc8), Ossz: va(0xbb1f84), Forced: va(0xb77250), Money: va(0xbad168), Pools: va(0xbad200),
  FlottaNew: va(0x53d400), HajoNew: va(0x53e800) };
const flottaNew = nf(K.FlottaNew, 'int', ['pointer', 'int', 'int', 'float', 'float', 'float']);
const hajoNew = nf(K.HajoNew, 'void', ['int', 'int']);
function spyPlans(en, ai) {
  // [state, mission, target, left, catch_at, infiltration, counter, combat, xp, master]
  const big = 100000, ctr = [3, 13, 0, big, -1000, 50, 255, 255, 0, 0];
  switch (cfg.spiesMode) {
    case 'force1': return {
      [en]: [[3, 2, ai, 50, -1000, 200, 10, 10, 0, 0], [3, 3, ai, 50, -1000, 200, 10, 10, 0, 0], [2, 0, 0, 50, -1000, 50, 10, 10, 0, 0],
        [1, 0, 0, 0, -1000, 50, 10, 10, 0, 0]],
      [ai]: [ctr, [3, 5, en, 50, -1000, 255, 10, 10, 0, 0], [3, 7, en, 50, -1000, 255, 10, 10, 0, 0], [3, 12, en, 50, -1000, 255, 10, 10, 0, 0],
        [3, 6, en, 50, -1000, 255, 10, 10, 0, 0], [3, 9, en, 50, -1000, 255, 10, 10, 0, 0], [3, 11, en, 50, -1000, 255, 10, 10, 0, 0],
        [3, 10, en, 100, -1000, 255, 10, 255, 0, 0]],
    };
    case 'force2': return {
      [en]: Array.from({ length: 6 }, () => [3, 4, ai, 100, 75, 50, 10, 10, 0, 0]),
      // At-home spies fill the AI's roster past the human's slots: the captor's slot k must be saved (a slot
      // past OsszHanyKem isn't), and Spy_Copy then appends the first two turned spies and fails the third.
      [ai]: [ctr, ...Array.from({ length: 7 }, () => [1, 0, 0, 0, -1000, 50, 10, 10, 0, 0])],
    };
    case 'lost': return {
      // Catch checks against the human's counter-spy (counter 255, so no attacker slips) with both combats 1, so
      // the duel lands in the "identified and lost" band (45 <= c < 52) now and then; the spies are known to three
      // living empires (a third is revived below).
      [en]: [[3, 13, 0, big, -1000, 50, 255, 1, 0, 0]],
      [ai]: Array.from({ length: 10 }, () => [3, 5, en, 100, 75, 0, 10, 1, 0, 0]),
    };
    case 'force3': return { [en]: [[2, 0, 0, 50, -1000, 50, 10, 10, 0, 0]], [ai]: [] };
    default: return {
      [en]: [[3, 2, ai, 50, -1000, 200, 10, 10, 0, 0], [3, 4, ai, 100, 75, 0, 10, 0, 0, 0], [2, 0, 0, 50, -1000, 50, 10, 10, 0, 0],
        [1, 0, 0, 0, -1000, 50, 10, 10, big, 0], [1, 0, 0, 0, -1000, 0, 10, 10, 0, ai], ctr, [1, 0, 0, 0, -1000, 50, 10, 0, 0, 0]],
      [ai]: [ctr, [3, 5, en, 50, -1000, 255, 10, 10, 0, 0], [3, 6, en, 50, -1000, 255, 10, 10, 0, 0],
        [3, 9, en, 50, -1000, 255, 10, 10, 0, 0], [3, 10, en, 50, -1000, 255, 10, 255, 0, 0], [3, 11, en, 50, -1000, 255, 10, 10, 0, 0],
        [3, 12, en, 50, -1000, 255, 10, 10, 0, 0], [3, 5, en, 100, 75, 0, 10, 0, 0, 0]],
    };
  }
}
function stageSpies() {
  const en = T.EN.readS32(), ai = cfg.aiEmpire, out = { mode: cfg.spiesMode };
  const plans = spyPlans(en, ai);
  for (const e of [en, ai]) {
    let n = K.Ossz.add(4 * e).readS32();
    out[e] = [];
    for (const [st, m, t, left, cat, inf, cnt, cmb, xp, master] of plans[e]) {
      if (n >= 12) break;
      n++;
      const r = K.Kemek.add((e * 13 + n) * 0x4c);
      r.writeByteArray(new Array(0x4c).fill(0));
      r.writeUtf8String('Stage ' + n);
      r.add(0x10).writeS16(st); r.add(0x12).writeU8(1); r.add(0x18).writeU8(master);
      r.add(0x1a).writeU16(m); r.add(0x1c).writeU8(t);
      r.add(0x24).writeS32(left); r.add(0x28).writeS32(left); r.add(0x2c).writeS32(cat);
      r.add(0x31).writeByteArray([inf, cnt, cmb, 5]);
      r.add(0x3c).writeS32(xp);
      out[e].push(n);
    }
    K.Ossz.add(4 * e).writeS32(n);
  }
  if (cfg.spiesMode === 'lost') {
    // Revive a dead empire with a race (alive flag only; BirodalomMIzik 0 so the AI leaves it alone) and mark the
    // AI's staged spies known to it, the AI and the human: a8 = 3 (espionage.md §4 step 2).
    const faj = va(0xbae574), alive = va(0xbae6ac), mizik = va(0x915f58);
    let y = 0;
    for (let x = 1; x <= 8 && !y; x++) if (x !== en && x !== ai && alive.add(x).readU8() === 0 && ![0, 5].includes(faj.add(x).readU8())) y = x;
    if (y) {
      alive.add(y).writeU8(1); mizik.add(y).writeU8(0);
      for (const k of out[ai]) for (const x of [en, ai, y]) K.Kemek.add((ai * 13 + k) * 0x4c + 0x41 + x).writeU8(1);
    }
    out.revived = y;
  }
  const forced = { force1: 1, force2: 2, force3: 3 }[cfg.spiesMode] || 0;
  if (forced) K.Forced.writeS32(forced);
  if (cfg.spiesMode === 'force2') {
    // The AI captor's loyalty roll reads its own slot k (espionage.md §4, quirk kept): 0 keeps fates 2/3, 255 makes
    // them 5/6. +0x18 must be clear or the fate is always 5/6.
    out.loyalty = out[en].map((k, i) => {
      const r = K.Kemek.add((ai * 13 + k) * 0x4c);
      r.add(0x34).writeU8(i % 2 ? 255 : 0); r.add(0x18).writeU8(0);
      return [k, r.add(0x34).readU8()];
    });
    // Stream 4 (RandSeed 0xBAF2D0 [21], saved) from --seed: the fates differ per seed.
    if (cfg.seed !== 4242) va(0xbaf2d0).add(16).writeU32(Math.imul(cfg.seed, 2654435761) >>> 0);
    out.stream4 = va(0xbaf2d0).add(16).readU32();
  }
  if (cfg.spiesMode === 'force1') {
    // Money to steal (MoneyMennyi(EN, 0) > 0; pools 1..3 as colony_side.js writes them) and one ship to destroy.
    const pools = K.Pools.add(en * 48);
    for (let j = 1; j < 4; j++) pools.add(8 * j).writeDouble(20000);
    K.Money.add(8 * en).writeDouble(pools.add(8).readDouble() + 0 + pools.add(16).readDouble() + pools.add(24).readDouble());
    let pick = 0;
    for (let d = 1; d <= designs(en); d++) {
      const p = design(en, d);
      if (p.add(0x80).readS32() !== 0 || p.add(0x16).readS16() !== 1) continue;
      const armed = p.add(0x2c).readS32() > 0 || p.add(0x30).readS32() > 0;
      if (!pick || !armed) pick = d;
      if (!armed) break;
    }
    let home = 0;
    for (let p = 1; p <= T.HanyBolygo.readS32() && !home; p++) if (owner(p) === en) home = p;
    if (pick && home) {
      const q = T.Bolygok.add(0x188 * home);
      const f = flottaNew(Memory.allocAnsiString('Stage'), en, 0, q.add(0x18).readFloat(), q.add(0x1c).readFloat(), 0);
      if (f > 0) hajoNew(f, pick);
      out.ship = [home, pick, f];
    }
    out.money = K.Money.add(8 * en).readDouble();
  }
  return out;
}

// One purchase through the exe's handler: TradeSzallitasTemp = {planet, days, offer, units, TradeAr, offer copy}.
let stub = null;
function buy(planet, days, offer, units) {
  const en = T.EN.readS32();
  if (stub === null) {
    stub = Memory.alloc(Process.pageSize);
    const flag = stub.add(0x100);
    Memory.patchCode(stub, 64, (code) => {
      const w = new X86Writer(code, { pc: stub });
      w.putMovRegAddress('eax', flag); w.putMovRegRegPtr('ecx', 'eax');
      w.putTestRegReg('ecx', 'ecx'); w.putJccShortLabel('je', 'none', 'no-hint');
      w.putMovRegPtrU32('eax', 0);
      w.putMovRegRegOffsetPtr('ecx', 'esp', 8); w.putMovRegU32('eax', en); w.putMovRegPtrReg('ecx', 'eax');
      w.putMovRegRegOffsetPtr('ecx', 'esp', 0xc); w.putMovRegU32('eax', 0x98); w.putMovRegPtrReg('ecx', 'eax');
      w.putMovRegU32('eax', 1); w.putRet();
      w.putLabel('none'); w.putXorRegReg('eax', 'eax'); w.putRet();
      w.flush();
    });
  }
  const t = T.Temp;
  t.writeByteArray(new Array(0x98).fill(0));
  t.writeS32(planet); t.add(4).writeS32(days); t.add(8).writeS32(offer); t.add(0xc).writeS32(units);
  t.add(0x10).writeS32(tradeAr(offer, units));
  t.add(0x14).writeByteArray(T.Offers.add(offer * 0x84).readByteArray(0x84));
  const msg = hexOf(t, 0x98);
  const before = T.Count.add(4 * en).readS32();
  const saved = T.getCalls.map((c) => c.readByteArray(5));
  for (const c of T.getCalls) {
    Memory.patchCode(c, 5, (code) => { const w = new X86Writer(code, { pc: c }); w.putCallAddress(stub); w.flush(); });
  }
  stub.add(0x100).writeU32(1);
  multigetTrade();
  T.getCalls.forEach((c, i) => Memory.patchCode(c, 5, (code) => code.writeByteArray(saved[i])));
  emit({ k: 'buy', e: en, msg, accepted: T.Count.add(4 * en).readS32() > before });
}

// --merges: fleet merges and the other ship-carrying arrivals (re/rules/fleet-movement.md §4.3). For the human and
// the AI (`aiEmpire`), next to an own planet P (arrival point P + (0, 2)): two idle fleets, a weak one and a strong
// one with vehicles (the human's named exactly AlapFlottaNev, so it takes the arriving fleet's name); a fleet moving
// to P that arrives at once and merges into the strong one; a follower of that fleet (retargeted by the merge, then
// merging itself); an idle fleet and a fleet following it from close by (merge on reaching an own fleet). Then a
// colony ship and a terraformer next to unowned planets (colonise / terraform: FlottaDelete with ships), a fleet of
// a dead empire (deleted with its ships) and, if there are wormholes, a fleet moving through one. All built with
// the exe's FlottaNew/HajoNew/TankNew/FlottaMozgat; new fleets get speed and override 2 so they move before the
// next speed refresh. Done before save 0, so the save carries it.
const M = {
  TankNew: va(0x541750), FlottaMozgat: va(0x53d270),
  Kolonizalo: va(0xbb4108), Terraformalo: va(0xbb4158), Alive: va(0xbae6ac), Wormhole: va(0xbb60f8),
  WormholeCount: va(0xbb61a0),
};
// flottaNew / hajoNew: the --spies set-up's (same exe functions).
const tankNew = nf(M.TankNew, 'void', ['int', 'int', 'int']);
const flottaMozgat = nf(M.FlottaMozgat, 'void', ['int', 'int', 'int', 'float', 'float']);
// EpuletNewSearchLocation(p, kind, done, dmg%, x, y) (exe 0x53cca0): a building where the AI would put it; the slot.
const epuletNew = nf(va(0x53cca0), 'int', ['int', 'int', 'int', 'int', 'int', 'int']);
// AddWormhole(x1, y1, x2, y2) (exe 0x53aec0): the new wormhole's index (0 when there are already 5).
const addWormhole = nf(va(0x53aec0), 'int', ['int', 'int', 'int', 'int']);
function stageMerges() {
  const out = { fleets: {}, skipped: [] };
  const fl = (f) => F.Flottak.readPointer().add(f * 0x80);
  const np = T.HanyBolygo.readS32();
  const pxy = (p) => [T.Bolygok.add(p * 0x188 + 0x18).readFloat(), T.Bolygok.add(p * 0x188 + 0x1c).readFloat()];
  const kinds = (e) => {
    const k = { ships: [], ground: [], colony: [], terra: [] };
    for (let d = 1; d <= designs(e); d++) {
      const r = design(e, d), cls = r.add(0x16).readS16();
      const strong = r.add(0x2c).readS32() > 0 || r.add(0x30).readS32() > 0;
      if (cls === 1 && r.add(0x34).readU8()) k.colony.push(d);
      else if (cls === 1 && r.add(0x35).readU8()) k.terra.push(d);
      else if (cls === 1 && strong && r.add(0x36).readU8() === 0 && r.add(0x3c).readFloat() > 0) k.ships.push(d);
      else if (cls === 2 && strong) k.ground.push(d);
    }
    return k;
  };
  const make = (tag, e, x, y, ships, ground) => {
    const f = flottaNew(A.AlapNev.readPointer(), e, 0, x, y, 0);
    if (f < 1) { out.skipped.push(tag); return 0; }
    for (const d of ships) hajoNew(f, d);
    for (const [d, n] of ground) tankNew(f, d, n);
    fl(f).add(0x38).writeFloat(2);
    fl(f).add(0x3c).writeFloat(2);
    out.fleets[tag] = f;
    return f;
  };
  const unowned = [];
  for (let p = 1; p <= np; p++) if (owner(p) === 0) unowned.push(p);
  let u = 0;
  for (const e of [T.EN.readS32(), cfg.aiEmpire]) {
    const k = kinds(e);
    let home = 0;
    for (let p = 1; p <= np && !home; p++) if (owner(p) === e) home = p;
    if (!home || !k.ships.length) { out.skipped.push(`empire ${e}: no planet or ships`); continue; }
    const [x, y] = pxy(home), s = k.ships, g = k.ground.length ? [[k.ground[0], 3]] : [];
    make(`${e}:weak`, e, x + 3, y + 2, [s[0]], []);
    const strong = make(`${e}:strong`, e, x - 3, y + 2, [s[0], s[s.length - 1], s[0]], g);
    if (strong && e === T.EN.readS32()) fl(strong).writeAnsiString(A.AlapNev.readPointer().readCString());
    const a = make(`${e}:arrive`, e, x + 0.4, y + 2.3, [s[s.length - 1], s[0]], g);
    if (a) {
      fl(a).writeAnsiString(`Merger ${e}`);
      fl(a).add(0x52).writeU16(fl(a).add(0x52).readU16() | 0x800);
      flottaMozgat(a, 1, 1, home, 0);
      const b = make(`${e}:follower`, e, x + 9, y + 8, [s[0]], []);
      if (b) flottaMozgat(b, 1, 2, a, 0);
    }
    const d = make(`${e}:idle`, e, x + 25, y - 20, [s[0]], []);
    const c = make(`${e}:joiner`, e, x + 26, y - 20.5, [s[s.length - 1]], g);
    if (d && c) flottaMozgat(c, 1, 2, d, 0);
    for (const [tag, list, bit, pool] of [['colonise', k.colony, 0x08, M.Kolonizalo], ['terraform', k.terra, 0x10, M.Terraformalo]]) {
      if (!list.length || u >= unowned.length) { out.skipped.push(`${e}:${tag}`); continue; }
      const p = unowned[u++], [ux, uy] = pxy(p);
      const f = make(`${e}:${tag}`, e, ux + 0.3, uy + 2.2, [list[0]], []);
      if (!f) continue;
      flottaMozgat(f, 1, 1, p, 0);
      fl(f).add(0x52).writeU16(fl(f).add(0x52).readU16() | bit);
      pool.add(4 * e).writeS32(pool.add(4 * e).readS32() - 1);
      out.fleets[`${e}:${tag}:planet`] = p;
    }
  }
  for (let e = 1; e <= 8; e++) {
    if (M.Alive.add(e).readU8() || designs(e) < 1) continue;
    const k = kinds(e);
    if (!k.ships.length) continue;
    make(`${e}:dead`, e, 10, 10, [k.ships[0], k.ships[0]], []);
    break;
  }
  const nw = M.WormholeCount.readS32(), en = T.EN.readS32();
  out.wormholes = nw;
  if (nw > 0) {
    const w = M.Wormhole.add(0x1c), wa = w.add(10).readS16();
    if (wa > 0) {
      const k = kinds(en), x = fl(wa).add(0x30).readFloat(), y = fl(wa).add(0x34).readFloat();
      const f = k.ships.length ? make(`${en}:wormhole`, en, x + 0.5, y, [k.ships[0]], []) : 0;
      if (f) flottaMozgat(f, 1, 2, wa, 0);
    }
  }
  return out;
}

// --wormholes: wormhole trips (re/rules/fleet-movement.md §4.3 "move/attack a wormhole fleet"). quick.scn's two
// "- Wormhole -" fleets (bit 10) belong to no wormhole (OsszHanyWormhole 0), so the exe's own AddWormhole (0x53aec0,
// what stories 902/239 call) opens one from next to the human's home planet (end A) to next to the AI's (end B).
// Then: a human fleet moving to end A (comes out at B), an AI fleet attacking end B (out at A), a human fleet moving
// to end A from afar (arrives some ticks later) and a human fleet moving to one of the scenario's unlisted wormhole
// fleets (no trip; story special 0x14 all the same). Done before save 0.
function stageWormholes() {
  const out = { fleets: {}, skipped: [] };
  const fl = (f) => F.Flottak.readPointer().add(f * 0x80);
  const np = T.HanyBolygo.readS32(), en = T.EN.readS32(), ai = cfg.aiEmpire;
  const pxy = (p) => [T.Bolygok.add(p * 0x188 + 0x18).readFloat(), T.Bolygok.add(p * 0x188 + 0x1c).readFloat()];
  const home = (e) => { for (let p = 1; p <= np; p++) if (owner(p) === e) return p; return 0; };
  const ship = (e) => {
    for (let d = 1; d <= designs(e); d++) {
      const r = design(e, d);
      if (r.add(0x16).readS16() === 1 && (r.add(0x2c).readS32() > 0 || r.add(0x30).readS32() > 0) && r.add(0x36).readU8() === 0
        && r.add(0x34).readU8() === 0 && r.add(0x35).readU8() === 0 && r.add(0x3c).readFloat() > 0) return d;
    }
    return 0;
  };
  const make = (tag, e, x, y) => {
    const d = ship(e), f = d ? flottaNew(A.AlapNev.readPointer(), e, 0, x, y, 0) : 0;
    if (f < 1) { out.skipped.push(tag); return 0; }
    hajoNew(f, d);
    fl(f).add(0x38).writeFloat(2);
    fl(f).add(0x3c).writeFloat(2);
    out.fleets[tag] = f;
    return f;
  };
  const hh = home(en), ah = home(ai);
  if (!hh || !ah) { out.skipped.push('homes'); return out; }
  const [hx, hy] = pxy(hh), [ax, ay] = pxy(ah);
  const listed = new Set();
  for (let w = 1; w <= M.WormholeCount.readS32(); w++) { const r = M.Wormhole.add(0x1c * w); listed.add(r.add(10).readS16()); listed.add(r.add(0x16).readS16()); }
  const dangling = [];
  for (let f = 1; f <= F.HanyFlotta.readS32(); f++) if (fl(f).add(0x15).readU8() !== 0xff && (fl(f).add(0x52).readU16() & 0x400) && !listed.has(f)) dangling.push(f);
  const w = addWormhole(Math.trunc(hx) + 6, Math.trunc(hy) - 6, Math.trunc(ax) - 6, Math.trunc(ay) + 6);
  out.wormhole = w;
  if (w < 1) return out;
  const r = M.Wormhole.add(0x1c * w), fa = r.add(10).readS16(), fb = r.add(0x16).readS16();
  out.ends = [fa, fb];
  const [xa, ya] = [fl(fa).add(0x30).readFloat(), fl(fa).add(0x34).readFloat()], [xb, yb] = [fl(fb).add(0x30).readFloat(), fl(fb).add(0x34).readFloat()];
  const h1 = make(`${en}:through`, en, xa + 0.5, ya);
  if (h1) flottaMozgat(h1, 1, 2, fa, 0);
  const a1 = make(`${ai}:attack`, ai, xb - 0.4, yb + 0.3);
  if (a1) flottaMozgat(a1, 2, 2, fb, 0);
  const h2 = make(`${en}:far`, en, xa + 20, ya + 15);
  if (h2) flottaMozgat(h2, 1, 2, fa, 0);
  out.dangling = dangling;
  if (dangling.length) {
    const d = dangling[0], h3 = make(`${en}:dangling`, en, fl(d).add(0x30).readFloat() + 0.5, fl(d).add(0x34).readFloat());
    if (h3) flottaMozgat(h3, 1, 2, d, 0);
  }
  return out;
}

// --tutorial-sighting: the tutorial waiting for event 1 ("exploreaplanet") with no script (re/rules/tutorial.md §4):
// TutorialPaused (0xF66F2A) 1, TutorialEvent (0xF66F40) 1, TutorialActive (0xF6915D) 0. FRISSIT_Starmap's next first
// sighting copies GetString(2, 4, 6) over that planet's name and TutorialCheckEvent(1) (0x60bdf0) unpauses (Active 1;
// the saves show it). TutorialFrissit (0x60c020) is stubbed so no script runs, and TutorialActMouseAllapot (0xF66F20,
// 0 in a game no tutorial ever started) is set to -1 as TutorialStart does: ReadNext would read the missing recording.
// So that a sighting happens (quick-1 starts with no fleet that has a radar), the human gets a new fleet (FlottaNew +
// HajoNew of its first warship design, which gets radar level 2 at design +0x6a; the starmap's refresh gives a fleet
// its best ship's radar, faded outside the owner's supply, fleet-movement.md §6) 10 units short of the planet it hasn't
// seen (planet +0xd8 + 2·EN == 0) that lies nearest to one of its planets, on the line from that planet.
function stageTutorial() {
  va(0xf66f2a).writeU8(1);
  va(0xf66f40).writeS32(1);
  va(0xf6915d).writeU8(0);
  const mouse = va(0xf66f20).readS32();
  va(0xf66f20).writeS32(-1);
  const en = T.EN.readS32(), np = T.HanyBolygo.readS32();
  let d = 1;
  while (d <= designs(en) && !(design(en, d).add(0x16).readS16() === 1 && design(en, d).add(0x3c).readFloat() > 0)) d++;
  const pos = (k) => [T.Bolygok.add(k * 0x188 + 0x18).readFloat(), T.Bolygok.add(k * 0x188 + 0x1c).readFloat()];
  let best = null;
  for (let p = 1; p <= np; p++) {
    if (T.Bolygok.add(p * 0x188 + 0xd8 + 2 * en).readS16() !== 0) continue;
    for (let h = 1; h <= np; h++) {
      if (owner(h) !== en) continue;
      const [px, py] = pos(p), [hx, hy] = pos(h), dd = Math.hypot(px - hx, py - hy);
      if (best === null || dd < best.d) best = { p, h, d: dd };
    }
  }
  let made = null;
  if (d <= designs(en) && best !== null) {
    design(en, d).add(0x6a).writeU8(2);
    const [px, py] = pos(best.p), [hx, hy] = pos(best.h), t = Math.max(0, 1 - 10 / best.d);
    const f = flottaNew(A.AlapNev.readPointer(), en, 0, hx + (px - hx) * t, hy + (py - hy) * t, 0);
    if (f > 0) hajoNew(f, d);
    made = { fleet: f, design: d, planet: best.p, from: best.h, dist: best.d };
  }
  Interceptor.replace(va(0x60c020), new NativeCallback(() => {}, 'void', [], 'mscdecl'));
  return { paused: 1, event: 1, made, mouse };
}

// --deaths: empires dying in the first days (re/rules/diplomacy.md §3.2, §12: FRISSIT_Viszony's death check and
// BirodalomMeghal 0x53b630). Three dead empires with a race are revived as AI empires (BirodalomEl 1,
// BirodalomMIzik 1) without planets, their countdowns (BirodalomVisszaSzamlalas 0xBAEB68) set:
//   x  countdown 1, two fleets (ships; ships and vehicles), a fleet of the human's and one of the AI's following
//      the first: dies on the first day (countdown 0); its fleets go with FlottaDelete, the followers stop
//   y  countdown 96, nothing: dies on the first day too (power 1.0 and countdown 95 < 96), after x
//   w  countdown 3, one fleet: dies on the third day
// AI 3 stays alive, so MegnyeresEllenoriz finds an enemy left (no victory screen).
// --deaths-mode campaign instead makes the victory check's campaign branch start story 0x195 (diplomacy.md §12.2):
// CampaignType (0x928278) 1, AI 3 the last enemy, its planets handed to the human (owner byte only), so the next
// FRISSIT_Viszony's BirodalomEro/BirodalomHanyBolygo put the human above twice its own; StoryAdat+0xc8 0.
function stageDeaths() {
  const out = { fleets: {}, skipped: [], mode: cfg.deathsMode };
  const en = T.EN.readS32(), ai = cfg.aiEmpire;
  if (cfg.deathsMode === 'campaign') {
    va(0x928278).writeS32(1);
    out.given = [];
    for (let p = 1; p <= T.HanyBolygo.readS32(); p++) if (owner(p) === ai) { T.Bolygok.add(p * 0x188 + 0x20).writeU8(en); out.given.push(p); }
    va(0xf273f8).add(0xc8).writeS32(0);
    out.human = [va(0xbaea78).add(4 * en).readFloat(), va(0xbaeb18).add(4 * en).readS32()];
    return out;
  }
  const faj = va(0xbae574), alive = va(0xbae6ac), mizik = va(0x915f58), countdown = va(0xbaeb68);
  const fl = (f) => F.Flottak.readPointer().add(f * 0x80);
  const ships = (e) => {
    const k = { ships: [], ground: [] };
    for (let d = 1; d <= designs(e); d++) {
      const r = design(e, d), cls = r.add(0x16).readS16();
      const special = [0x34, 0x35, 0x36].some((o) => r.add(o).readU8() !== 0);
      if (cls === 1 && !special && r.add(0x3c).readFloat() > 0) k.ships.push(d);
      else if (cls === 2) k.ground.push(d);
    }
    return k;
  };
  const make = (tag, e, x, y, s, g) => {
    const f = flottaNew(A.AlapNev.readPointer(), e, 0, x, y, 0);
    if (f < 1) { out.skipped.push(tag); return 0; }
    for (const d of s) hajoNew(f, d);
    for (const [d, n] of g) tankNew(f, d, n);
    fl(f).add(0x38).writeFloat(2);
    fl(f).add(0x3c).writeFloat(2);
    out.fleets[tag] = [f, fl(f).add(0x54).readS32(), fl(f).add(0x60).readS32()];
    return f;
  };
  const pick = [];
  for (let e = 1; e <= 8 && pick.length < 3; e++) {
    if (e === en || e === ai || alive.add(e).readU8() || [0, 5].includes(faj.add(e).readU8())) continue;
    if (pick.length !== 1 && !ships(e).ships.length) continue;
    pick.push(e);
  }
  out.empires = pick;
  if (pick.length < 3) { out.skipped.push('fewer than three dead empires with ships'); return out; }
  const [x, y, w] = pick;
  for (const [e, c] of [[x, 1], [y, 96], [w, 3]]) {
    alive.add(e).writeU8(1); mizik.add(e).writeU8(1); countdown.add(4 * e).writeS32(c);
  }
  const kx = ships(x), kw = ships(w);
  const a = make('x:ships', x, 10, 10, [kx.ships[0], kx.ships[kx.ships.length - 1]], []);
  make('x:mixed', x, 14, 10, [kx.ships[0]], kx.ground.length ? [[kx.ground[0], 4]] : []);
  make('w:ships', w, 10, 16, [kw.ships[0]], []);
  if (a) {
    for (const e of [en, ai]) {
      const k = ships(e);
      if (!k.ships.length) { out.skipped.push(`${e}:follower`); continue; }
      const b = make(`${e}:follower`, e, 30, 30 + 4 * (e === ai), [k.ships[0]], []);
      if (b) flottaMozgat(b, 1, 2, a, 0);
    }
  }
  out.countdowns = [x, y, w].map((e) => countdown.add(4 * e).readS32());
  return out;
}

// --messages: the scrolling-line posts and the inbox expiry no other recording reaches (re/rules/info-messages.md
// §3.2, §4). Before save 0 (so the port starts from the same state):
//   the human's planets (not protected +0x64): the first one loses every building (abandoned, 5); the second gets
//     morale 150 and unrest 24 (UnrestWarning at 25, 9); the third morale 150 and unrest 30 (rebellion, 8)
//   two planets of the AI (`aiEmpire`): morale 150, unrest 30, the AI knows the human (rebels may join the
//     human, 19); every planet of another owner gets the human's spy satellite (+0xc4 + EN = 1: guns expel it, 13)
//   TRADE_THIS[EN] (0xBAE150) 12345: 1 January posts last year's trade income (21, ~tick 211 at x16)
//   --messages-mode diplo also: DEBUGX (0xBAE587) on, the AI's contact age with the human -1 and their relation
//     neutral (3), the diplomacy counter last[ai*9+EN] (0xDAF520) 290: MI_DIPLOMACY sends the human messages (10),
//     a declaration of war among them (DiplomaciaMegtamadas, 6)
//   the inbox (Uzenetek 0xDA6620, 0x88 each): four messages with blocks 0x7771..0x7774: expiry Ido + 16*20 and
//     Ido + 16*60, one kept (+4 = 1) past its expiry, one with expiry 0 (FRISSIT_Messages, Story_MessageElevules)
function stageMessages() {
  const en = T.EN.readS32(), ai = cfg.aiEmpire, np = T.HanyBolygo.readS32();
  const out = { en, ai, skipped: [] };
  const rec = (p) => T.Bolygok.add(p * 0x188);
  const unrest = (p) => va(0xb99668).add(4 * p);
  const free = (p) => rec(p).add(0x64).readU8() === 0 && rec(p).add(0x186).readU8() === 0 && rec(p).add(0x187).readU8() === 0;
  const mine = [], theirs = [];
  for (let p = 1; p <= np; p++) {
    if (owner(p) === en && free(p)) mine.push(p);
    else if (owner(p) === ai) theirs.push(p);
  }
  out.mine = mine;
  if (mine.length > 3) {
    // The home planet stays: take the last three.
    mine.splice(0, mine.length - 3);
  }
  if (mine.length >= 1) {
    const p = mine[0], b = T.Epuletek.add(4 * p).readPointer(), slots = T.Slots.add(4 * p).readS32();
    for (let j = 1; j <= slots; j++) b.add(j * 0x20).writeU8(0);
    va(0xba9518).add(4 * p).writeS32(0);
    out.abandoned = p;
  } else out.skipped.push('abandoned');
  if (mine.length >= 2) { rec(mine[1]).add(0x30).writeS32(150); unrest(mine[1]).writeS32(24); out.warning = mine[1]; } else out.skipped.push('warning');
  if (mine.length >= 3) { rec(mine[2]).add(0x30).writeS32(150); unrest(mine[2]).writeS32(30); out.revolt = mine[2]; } else out.skipped.push('revolt');
  const rebels = theirs.filter(free).slice(0, 2);
  for (const p of rebels) { rec(p).add(0x30).writeS32(150); unrest(p).writeS32(30); }
  va(0xbaebb8).add(ai * 19 + en).writeU8(1);
  out.rebels = rebels;
  out.satellites = [];
  for (let p = 1; p <= np; p++) if (owner(p) !== 0 && owner(p) !== en) { rec(p).add(0xc4 + en).writeU8(1); out.satellites.push(p); }
  va(0xbae150).add(4 * en).writeS32(12345);
  out.mode = cfg.messagesMode;
  if (cfg.messagesMode === 'diplo') stageDiplo(en, ai, out);
  const box = va(0xda6620), n = va(0xdabdd0), ido = A.Ido.readS32();
  const msgs = [[0x7771, 0, ido + 16 * 20], [0x7772, 1, ido + 16 * 5], [0x7773, 0, 0], [0x7774, 0, ido + 16 * 60]];
  msgs.forEach(([block, kept, t], i) => {
    const m = box.add(i * 0x88);
    m.writeS32(block); m.add(4).writeU8(kept); m.add(8).writeS32(t);
  });
  out.inboxBefore = n.readS32();
  n.writeS32(msgs.length);
  out.inbox = msgs;
  return out;
}

// --messages-mode diplo: the AI writes to the human (MI_DIPLOMACY, 10) and declares war (6). Not a replay: the
// message's reason code (MiertValaszoltAztAmitValaszolt exe 0x4c7e80, diplomacy.md §11) isn't ported.
function stageDiplo(en, ai, out) {
  va(0xbae587).writeU8(1);
  va(0xbaed28).add(4 * (ai * 19 + en)).writeS32(-1);
  const rel = (a, b) => va(0xdabf38).add((a * 19 + b) * 0x26 + 0x25);
  out.level = [rel(ai, en).readU8(), rel(en, ai).readU8()];
  rel(ai, en).writeU8(3); rel(en, ai).writeU8(3);
  va(0xdaf520).add(4 * (ai * 9 + en)).writeS32(290);
}

// --battles: auto-resolved space battles and ground wars in the first ticks (re/rules/info-messages.md §3.2: the
// battle outcomes 11/12, 15-18, 22-25). The human's empire is handed to the AI (PlayerKi[EN] = 0; the AI's
// pass is switched off, every fleet is visible): no battle opens the real-time screen, and SpaceWarIndit/GroundWarIndit
// still post the human's messages. Attackers sit within one step of their targets; the starmap loop stops at the
// first battle, so the next attacker in slot order fights in the next tick. --battles-mode picks the mix:
//   space   duels between the human's and the AI's fleets: lopsided won/lost (15/16), even ones (17/18), the AI
//           attacking the human, and a planet battle against a defended planet (defence fleet within 15)
//   ground  planet attacks with ground vehicles on undefended planets (GroundWarIndit straight away): lopsided
//           ones (22/23), even ones (11/12), a garrisoned planet (TankNew(-p)) the attacker loses (no message).
//   land    a planet battle the attacker wins and lands after (SpaceWarIndit -> GroundWarIndit): the human's big
//           fleet with ground vehicles against an AI planet with its guns on and a weak defence fleet (planet guns
//           destroyed in space: EpuletDelete), then lopsided attacks that lose (24 to the human owner, 25 to the
//           human attacker): a few vehicles against the turrets of a planet whose fortress value
//           (BolygoMennyiErodEro, saved; recomputed by the next economy step) is cleared, so the ratio counts as
//           lopsided.
function stageBattles() {
  const out = { mode: cfg.battlesMode, fleets: {}, skipped: [] };
  const en = T.EN.readS32(), ai = cfg.aiEmpire;
  const fl = (f) => F.Flottak.readPointer().add(f * 0x80);
  const np = T.HanyBolygo.readS32();
  const pxy = (p) => [T.Bolygok.add(p * 0x188 + 0x18).readFloat(), T.Bolygok.add(p * 0x188 + 0x1c).readFloat()];
  const kinds = (e) => {
    const k = { ships: [], ground: [] };
    for (let d = 1; d <= designs(e); d++) {
      const r = design(e, d), cls = r.add(0x16).readS16();
      const strong = r.add(0x2c).readS32() > 0 && r.add(0x30).readS32() > 0;
      const special = [0x34, 0x35, 0x36, 0x37, 0x38, 0x39].some((o) => r.add(o).readU8() !== 0);
      if (cls === 1 && strong && !special && r.add(0x3c).readFloat() > 0) k.ships.push(d);
      else if (cls === 2 && strong) k.ground.push(d);
    }
    return k;
  };
  const make = (tag, e, x, y, ships, ground) => {
    const f = flottaNew(A.AlapNev.readPointer(), e, 0, x, y, 0);
    if (f < 1) { out.skipped.push(tag); return 0; }
    fl(f).writeAnsiString(tag.slice(0, 20));  // the name field is 21 bytes; the owner follows
    for (const d of ships) hajoNew(f, d);
    for (const [d, n] of ground) tankNew(f, d, n);
    fl(f).add(0x38).writeFloat(2);
    fl(f).add(0x3c).writeFloat(2);
    out.fleets[tag] = [f, fl(f).add(0x54).readS32(), fl(f).add(0x60).readS32(), fl(f).add(0x6c).readS32(), fl(f).add(0x5c).readS32(), fl(f).add(0x68).readS32()];
    return f;
  };
  const planets = (e) => { const l = []; for (let p = 1; p <= np; p++) if (owner(p) === e) l.push(p); return l; };
  const K = { [en]: kinds(en), [ai]: kinds(ai) };
  out.kinds = K;
  out.planets = { [en]: planets(en), [ai]: planets(ai) };
  if (!K[en].ships.length || !K[ai].ships.length) { out.skipped.push('no warship designs'); return out; }
  const big = (e, n) => Array(n).fill(K[e].ships[K[e].ships.length - 1]);
  const small = (e) => [K[e].ships[0]];
  // Battles far from every planet (no battle planet within 20): a row of duels at (x0 + 30i, y0).
  let spot = 0;
  const open = () => { const s = [60 + 25 * (spot % 6), 40 + 25 * Math.floor(spot / 6)]; spot++; return s; };
  const duel = (tag, a, sa, b, sb) => {
    const [x, y] = open();
    const t = make(`${tag}:target`, b, x, y, sb, []);
    const f = t ? make(`${tag}:attacker`, a, x + 0.5, y + 0.3, sa, []) : 0;
    if (f) flottaMozgat(f, 2, 2, t, 0);
  };
  if (cfg.battlesMode === 'space') {
    duel('won', en, big(en, 6), ai, small(ai));
    duel('lost', en, small(en), ai, big(ai, 6));
    duel('even', en, big(en, 2), ai, big(ai, 2));
    duel('ai', ai, big(ai, 2), en, big(en, 2));
    duel('ai-lopsided', ai, big(ai, 8), en, small(en));
    // A planet battle: the AI attacks the human's planet with a defence fleet next to it.
    const p = out.planets[en][0];
    if (p) {
      const [x, y] = pxy(p);
      make('planet:defender', en, x - 2, y + 1, big(en, 2), []);
      const f = make('planet:attacker', ai, x + 0.4, y + 2.3, big(ai, 3), []);
      if (f) flottaMozgat(f, 2, 1, p, 0);
    }
  } else {
    if (!K[en].ground.length || !K[ai].ground.length) { out.skipped.push('no ground designs'); return out; }
    const tanks = (e, n) => [[K[e].ground[K[e].ground.length - 1], n]];
    const fortress = (p) => va(0xb99cb8).add(4 * p);
    const attack = (tag, e, p, ships, ground) => {
      const [x, y] = pxy(p);
      const f = make(tag, e, x + 0.4, y + 2.3, ships, ground);
      if (f) { flottaMozgat(f, 2, 1, p, 0); out.fleets[tag].push(p); }
      return f;
    };
    // Undefended planets (`fleet-movement.md` §4.3: no defence ship, no working gun or shield building, no idle own
    // fleet within 15): the attack goes straight to GroundWarIndit. Targets without a nearby fleet are disarmed: the
    // defence-ship count is cleared and the guns/shields switched off (+0x19 bit 0; the next economy step may turn
    // them on again, after the attacks). Garrison and fortress strength stay (they decide "lopsided").
    out.why = {};
    const undefended = (e) => out.planets[e].filter((p) => {
      const [x, y] = pxy(p);
      for (let f = 1; f <= F.HanyFlotta.readS32(); f++) {
        const r = fl(f);
        if (r.add(0x15).readU8() === e && Math.abs(r.add(0x30).readFloat() - x) + Math.abs(r.add(0x34).readFloat() - y) < 16) {
          out.why[p] = `fleet ${f}`;
          return false;
        }
      }
      const q = T.Bolygok.add(p * 0x188);
      const n = T.Slots.add(4 * p).readS32(), b = T.Epuletek.add(4 * p).readPointer();
      out.why[p] = { ships: q.add(0x114).readS32(), garrison: q.add(0x58).readS32(), strength: q.add(0x5c).readS32(),
        fortress: va(0xb99cb8).add(4 * p).readS32(), guns: [] };
      q.add(0x114).writeS32(0);
      for (let s = 1; s <= n; s++) {
        const k = b.add(0x20 * s).readU8();
        if ((k >= 0x1c && k <= 0x1f) || (k >= 0x24 && k <= 0x26)) {
          out.why[p].guns.push(k);
          b.add(0x20 * s + 0x19).writeU8(b.add(0x20 * s + 0x19).readU8() & 0xfe);
        }
      }
      return true;
    }).slice(0, 3);
    if (cfg.battlesMode === 'land') {
      // The planet battle: an AI planet nobody else stands next to, guns as they are, a one-ship AI defence fleet
      // next to it (idle, within 15).
      const target = out.planets[ai].find((p) => {
        const [x, y] = pxy(p);
        for (let f = 1; f <= F.HanyFlotta.readS32(); f++) {
          const r = fl(f);
          if (r.add(0x15).readU8() !== 0xff && Math.abs(r.add(0x30).readFloat() - x) + Math.abs(r.add(0x34).readFloat() - y) < 16) return false;
        }
        return true;
      });
      out.landTarget = target || 0;
      if (target) {
        const [x, y] = pxy(target);
        const q = T.Bolygok.add(target * 0x188);
        out.landTargetState = { ships: q.add(0x114).readS32(), garrison: q.add(0x58).readS32(), fortress: fortress(target).readS32() };
        // Finished space guns (a planet gun and a turreted one): the battle destroys some (EpuletDelete, with the
        // turret's battlefield group).
        out.landGuns = [0x24, 0x1c, 0x24].map((kind) => epuletNew(target, kind, 1, 0, -1, -1));
        make('land:defence', ai, x - 2, y + 1, small(ai), []);
        attack('land:won', en, target, big(en, cfg.landShips), tanks(en, cfg.lopsidedTanks));
      }
      const rest = (e) => out.planets[e].filter((p) => p !== target);
      const mine = undefended(en).filter((p) => p !== target), theirs = undefended(ai).filter((p) => p !== target);
      out.undefended = { [en]: mine, [ai]: theirs, rest: [rest(en).length, rest(ai).length] };
      for (const p of [theirs[0], mine[0]]) if (p) { out.why[p].fortressCleared = fortress(p).readS32(); fortress(p).writeS32(0); }
      if (theirs[0]) attack('land:lopsided-lost', en, theirs[0], small(en), tanks(en, cfg.lostTanks));
      if (mine[0]) attack('land:ai-lopsided-lost', ai, mine[0], small(ai), tanks(ai, cfg.lostTanks));
      va(0xbae560).add(en).writeU8(0);
      va(0xb77244).writeS32(1);
      va(0xf265d4).writeU8(1);
      return out;
    }
    const mine = undefended(en), theirs = undefended(ai);
    out.undefended = { [en]: mine, [ai]: theirs };
    // Lopsided (ground strength > 5 x (garrison + fortress)): the attacker wins (22 to the human attacking, 23 to
    // the human losing the planet).
    if (theirs[0]) attack('ground:won', en, theirs[0], small(en), tanks(en, cfg.lopsidedTanks));
    if (mine[0]) attack('ground:ai', ai, mine[0], small(ai), tanks(ai, cfg.lopsidedTanks));
    // Even wars the attacker should still win (12 to the human attacking, 11 to the owner).
    if (theirs[1]) attack('ground:even-won', en, theirs[1], small(en), tanks(en, cfg.evenTanks));
    if (mine[1]) attack('ground:even-ai', ai, mine[1], small(ai), tanks(ai, cfg.evenTanks));
    // A garrisoned planet: the attacker loses (no message).
    if (theirs[2]) {
      tankNew(-theirs[2], K[ai].ground[K[ai].ground.length - 1], 8);
      attack('ground:even-lost', en, theirs[2], small(en), tanks(en, 3));
    }
  }
  // The human's empire to the AI (no real-time screens); FRISSIT_MI off (the debug switch 0xB77244, dumped as
  // 'aioff'), else the AI's fleet pass redirects the staged fleets, the human's too now; every fleet visible to
  // everyone (Story_Minden_Flotta_Latszik, saved), else an attack on an unseen fleet turns into a move.
  va(0xbae560).add(en).writeU8(0);
  va(0xb77244).writeS32(1);
  va(0xf265d4).writeU8(1);
  return out;
}

// --orders N: every --orders-every ticks (from tick 1), N random starmap orders go through the exe's own
// MULTIGET_Starmap (0x59d930, re/screens/starmap.md §10) at IdoNapValtozott, before that tick's save: its
// Multi_GetNextTipus calls (0x513340) are pointed at a stub returning the staged type once to a call whose range
// holds it, its Multi_Get calls (0x513160) at a stub written for the order that stores each value into the
// out-parameters ([esp + 8 + 8i]; the list ends with a 0 in a marker's place; 0x1a's block is 16 dwords). Each is
// reported as {k: 'sorder', before: k, type, vals} (vals: the issuer, then the dwords). Set-up before save 0:
// "always simulate" battles (0xB77230, so attacks don't stop the clock), stations on the human's planets, two
// fleets of the human's ships and vehicles beside its first planet.
const SO = {
  Multiget: va(0x59d930), Hajok: va(0xbb4100), HanyHajo: va(0xbb4104), Tankok: va(0xbb41a8), HanyTank: va(0xbb4154),
  next: [0x59d973, 0x59da1c, 0x59da31, 0x59eb13, 0x59eb4a, 0x59f449, 0x59f466, 0x59f581].map(va),
  get: [0x59d99f, 0x59d9fc, 0x59da6c, 0x59dbf6, 0x59dd52, 0x59de16, 0x59df00, 0x59e022, 0x59e1f9, 0x59e283, 0x59e301,
    0x59e650, 0x59eb84, 0x59ec31, 0x59ece6, 0x59ed36, 0x59f1a9, 0x59f1fc, 0x59f23b, 0x59f34a, 0x59f381, 0x59f3c4, 0x59f3fd,
    0x59f497].map(va),
};
const multigetStarmap = nf(SO.Multiget, 'void', []);
const soTankNew = nf(va(0x541750), 'void', ['int', 'int', 'int']);
let soSeed = 0;
// mulberry32 on --seed: the staged orders are reproducible per seed.
function srnd(n) {
  soSeed = (soSeed + 0x6d2b79f5) | 0;
  let t = soSeed;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return n > 0 ? (((t ^ (t >>> 14)) >>> 0) % n) : 0;
}
const spick = (a) => a[srnd(a.length)];
const fbits = (x) => { const b = new ArrayBuffer(4); new DataView(b).setFloat32(0, x, true); return new DataView(b).getInt32(0, true); };
function soPatch(calls, target) {
  const saved = calls.map((c) => c.readByteArray(5));
  for (const c of calls) Memory.patchCode(c, 5, (code) => { const w = new X86Writer(code, { pc: c }); w.putCallAddress(target); w.flush(); });
  return () => calls.forEach((c, i) => Memory.patchCode(c, 5, (code) => code.writeByteArray(saved[i])));
}
let soNext = null, soGet = null;
function soStubs(outs) {
  if (soNext === null) {
    soNext = Memory.alloc(Process.pageSize);
    soGet = Memory.alloc(Process.pageSize);
    const type = soNext.add(0x200);
    Memory.patchCode(soNext, 0x80, (code) => {
      const w = new X86Writer(code, { pc: soNext });
      w.putMovRegAddress('ecx', type); w.putMovRegRegPtr('eax', 'ecx');
      w.putCmpRegI32('eax', 0); w.putJccShortLabel('je', 'none', 'no-hint');
      w.putMovRegRegOffsetPtr('edx', 'esp', 4); w.putCmpRegReg('eax', 'edx'); w.putJccShortLabel('jl', 'none', 'no-hint');
      w.putMovRegRegOffsetPtr('edx', 'esp', 8); w.putCmpRegReg('eax', 'edx'); w.putJccShortLabel('jg', 'none', 'no-hint');
      w.putMovRegPtrU32('ecx', 0); w.putRet();
      w.putLabel('none'); w.putXorRegReg('eax', 'eax'); w.putRet();
      w.flush();
    });
  }
  Memory.patchCode(soGet, 0x800, (code) => {
    const g = new X86Writer(code, { pc: soGet });
    outs.forEach((o, i) => {
      g.putMovRegRegOffsetPtr('ecx', 'esp', 8 + 8 * i);
      // {u8: v}: a one-byte field (mov byte ptr [ecx], v), so the stub doesn't write over its neighbours.
      if (!Array.isArray(o)) g.putBytes([0xc6, 0x01, o.u8 & 0xff]);
      else o.forEach((v, j) => g.putMovRegOffsetPtrU32('ecx', 4 * j, v >>> 0));
      if (i + 1 < outs.length) {
        g.putMovRegRegOffsetPtr('ecx', 'esp', 0xc + 8 * i);
        g.putCmpRegI32('ecx', 0); g.putJccNearLabel('je', 'done', 'no-hint');
      }
    });
    g.putLabel('done'); g.putMovRegU32('eax', 1); g.putRet();
    g.flush();
  });
}
function soRun(type, outs) {
  soStubs(outs);
  soNext.add(0x200).writeS32(type);
  const undoNext = soPatch(SO.next, soNext), undoGet = soPatch(SO.get, soGet);
  try { multigetStarmap(); } finally { undoNext(); undoGet(); }
}
function soState() {
  const en = T.EN.readS32(), np = T.HanyBolygo.readS32();
  const fl = F.Flottak.readPointer(), n = F.HanyFlotta.readS32();
  const planets = [], all = [], mine = [];
  for (let p = 1; p <= np; p++) if (owner(p) === en) planets.push(p);
  for (let f = 1; f <= n; f++) {
    const o = fl.add(f * 0x80 + 0x15).readU8();
    if (o === 0xff) continue;
    all.push(f);
    if (o === en) mine.push(f);
  }
  const des = { 1: [], 2: [], st: [] };
  for (let d = 1; d <= designs(en); d++) {
    const r = design(en, d), c = r.add(0x16).readS16();
    if (c === 1 && r.add(0x37).readU8()) des.st.push(d);
    else if (c === 1 || c === 2) des[c].push(d);
  }
  return { en, np, planets, all, mine, des };
}
// What holder h (fleet > 0, planet -p) carries: [[design, ship index or 0], ...].
function soHeld(h) {
  const out = [], hs = SO.Hajok.readPointer();
  for (let i = 1; i <= SO.HanyHajo.readS32(); i++) {
    const r = hs.add(6 * i);
    if (r.add(2).readS16() !== 0x7fff && r.readS16() === h) out.push([r.add(2).readS16(), i]);
  }
  for (let i = 1; i <= SO.HanyTank.readS32(); i++) {
    const r = SO.Tankok.add(8 * i);
    if (r.add(2).readS16() !== 0x7fff && r.readS16() === h && r.add(4).readS32() > 0) out.push([r.add(2).readS16(), 0]);
  }
  return out;
}
// A whole production block (+0x78..+0xb8) of the human's own designs.
function soBlock(st, p) {
  const b = new Uint8Array(T.Bolygok.add(p * 0x188 + 0x78).readByteArray(0x40));
  const dv = new DataView(b.buffer);
  for (let line = 0; line < 2; line++) {
    const list = line === 0 ? st.des[1].concat(st.des.st) : st.des[2];
    const m = list.length ? srnd(5) : 0;
    const ui = [0, 1, 2, 3];
    for (let k = 3; k > 0; k--) { const j = srnd(k + 1); const t = ui[k]; ui[k] = ui[j]; ui[j] = t; }
    for (let k = 0; k < 4; k++) {
      dv.setInt16(8 * line + 2 * k, k < m ? spick(list) : 0, true);
      b[0x10 + 4 * line + k] = 1 + srnd(5); b[0x18 + 4 * line + k] = srnd(2);
      b[0x20 + 4 * line + k] = ui[k]; b[0x35 + 4 * line + k] = srnd(2);
    }
    const head = dv.getInt16(8 * line, true);
    dv.setInt32(0x28 + 4 * line, head > 0 ? design(st.en, head).add(0x20).readS32() : 0, true);
    b[0x30 + line] = 0;
  }
  const words = [];
  for (let i = 0; i < 16; i++) words.push(dv.getInt32(4 * i, true));
  return words;
}
function soMake(st) {
  const en = st.en, r = srnd(100);
  const P = st.planets.length ? st.planets : [1];
  const anyPlanet = () => 1 + srnd(st.np);
  const myPlanet = () => (srnd(8) ? spick(P) : anyPlanet());
  const fleet = () => (st.mine.length && srnd(4) ? spick(st.mine) : (st.all.length ? spick(st.all) : 1));
  if (r < 24) {
    const kind = spick([0, 0, 0, 1, 1, 2]), ord = spick([1, 1, 1, 2, 0, 13, 14]);
    let x = 0, y = 0;
    if (kind === 0) { x = srnd(600) - 300 + srnd(100) / 100; y = srnd(600) - 300 + srnd(100) / 100; }
    if (kind === 1) x = anyPlanet();
    if (kind === 2) x = st.all.length ? spick(st.all) : 1;
    return [0x15, [[en], [fleet()], [ord], [kind], [fbits(x)], [fbits(y)]]];
  }
  if (r < 29) return [0x14, [[en], [fleet()]]];
  if (r < 34) return [0x16, [[en], [anyPlanet()], [1 + srnd(2)]]];
  if (r < 39) {
    const p = myPlanet();
    return [0x17, [[en], [p], [srnd(T.Bolygok.add(p * 0x188 + 0x114).readS32() + 2)]]];
  }
  if (r < 43) return [0x18, [[en], [myPlanet()], [srnd(3)]]];
  if (r < 52) return [0x19, [[en], [myPlanet()], [srnd(11)], [srnd(12)]]];
  if (r < 56) { const p = myPlanet(); return [0x1a, [[en], [p], soBlock(st, p)]]; }
  if (r < 70) {
    const holders = st.mine.concat(P.map((p) => -p));
    if (!holders.length) return null;
    const from = spick(holders), held = soHeld(from);
    let to = spick(holders);
    if (to === from) to = spick(holders);
    // Ships only go between fleets (the transfer screen; HajoAtpakol onto a planet writes outside Flottak).
    const ok = held.filter(([, i]) => to > 0 || i === 0);
    const [d, i] = ok.length ? spick(ok) : [spick(st.des[2].concat(to > 0 ? st.des[1] : []).concat([1])), 0];
    return [0x1c, [[en], [from], [to], [d], [srnd(2) ? 0 : i], [1 + srnd(6)]]];
  }
  if (r < 87) {
    const op = 1 + srnd(5), line = srnd(2);
    const list = line === 0 ? st.des[1].concat(st.des.st) : st.des[2];
    const value = op === 4 ? (list.length && srnd(3) ? spick(list) : 0) : srnd(4);
    return [0x1d, [[en], [op], [myPlanet()], [line], [0], [srnd(4)], [value]]];
  }
  if (r < 91) return [0x400, [[en], [myPlanet()], [srnd(3) ? spick(P) : 0]]];
  if (r < 94) return [0x40f, [[en], [myPlanet()]]];
  if (r < 97) return [0x406, [[en], [1 + srnd(3)]]];
  if (r < 99) return [0x407, [[en], [0]]];
  return [0x403, [[en], [srnd(5)]]];
}
// libc rand() is pinned for the batch (garrisons landing draw their dropships' offsets): reported as 'libc'.
function stageOrders() {
  const libc = (cfg.seed * 7919 + 17 + A.Ido2.readS32()) >>> 0;
  srand(libc);
  for (let i = 0; i < cfg.orders; i++) {
    const o = soMake(soState());
    if (!o) continue;
    soRun(o[0], o[1]);
    emit({ k: 'sorder', before: k, libc, type: o[0], vals: [].concat(...o[1]) });
  }
}
// --research N: every --research-every ticks (from tick 1), N random research-screen messages go through the exe's
// own MULTIGET_Research (0x51ee70, re/screens/research.md §6) at IdoNapValtozott, before that tick's save, with the
// starmap orders' stubs: 0x2a start (mostly a researchable tech of the human while it researches nothing, as the
// button does; now and then any tech, or with a project running), 0x29 stop (mostly the running project), 0x2b the
// urgent toggle, rarely 0x31 (develop every tech some empire knows). Mostly the human's, now and then an AI's.
// Each is reported as {k: 'rorder', before: k, libc, type, vals} (vals: the issuer, then the dwords).
const RO = {
  Multiget: va(0x51ee70), Akt: va(0xb9a9b8), Techs: va(0xb9c0c8),
  next: [0x51ee7a, 0x51f02d].map(va),
  get: [0x51eea6, 0x51ef01, 0x51ef3b, 0x51ef74].map(va),
};
const multigetResearch = nf(RO.Multiget, 'void', []);
function roRun(type, outs) {
  soStubs(outs);
  soNext.add(0x200).writeS32(type);
  const undoNext = soPatch(RO.next, soNext), undoGet = soPatch(RO.get, soGet);
  try { multigetResearch(); } finally { undoNext(); undoGet(); }
}
const roStatus = (t, e) => RO.Techs.add(t * 0x114 + 0x45 + e).readU8();
function roMake() {
  const en = T.EN.readS32();
  const e = srnd(6) ? en : 1 + srnd(8);
  const akt = RO.Akt.add(4 * e).readS32();
  const r = srnd(100);
  const any = () => 1 + srnd(192);
  if (r < 45) {
    const ok = [];
    for (let t = 1; t < 193; t++) if (roStatus(t, e) === 3) ok.push(t);
    const t = ok.length && srnd(5) ? spick(ok) : any();
    if (akt !== 0 && srnd(3)) return [0x29, [[e], [akt]]];
    return [0x2a, [[e], [t]]];
  }
  if (r < 75) return [0x29, [[e], [akt > 0 && srnd(4) ? akt : any()]]];
  if (r < 98) return [0x2b, [[e]]];
  return [0x31, [[e]]];
}
function stageResearch() {
  const libc = (cfg.seed * 7919 + 29 + A.Ido2.readS32()) >>> 0;
  srand(libc);
  for (let i = 0; i < cfg.research; i++) {
    const o = roMake();
    roRun(o[0], o[1]);
    emit({ k: 'rorder', before: k, libc, type: o[0], vals: [].concat(...o[1]) });
  }
}
// --info N: every --info-every ticks (from tick 1), N random info-screen messages go through the exe's own
// MULTIGET_Info (0x4ece60, re/screens/info.md §7) at IdoNapValtozott, before that tick's save, with the starmap
// orders' stubs: 0x5b the money triangle's three shares (f64; mostly a point of the triangle as Info_Main measures
// it, with its 0.03 cut-off, now and then any values) and 0x5c the spending limit (f64; mostly the bar's rounded
// values). Mostly the human's, now and then an AI's. Each is reported as {k: 'iorder', before: k, libc, type, vals}
// (vals: the issuer, then each f64 as its low and high dwords).
const IO = {
  Multiget: va(0x4ece60),
  next: [0x4ece6a, 0x4ecf45].map(va),
  get: [0x4eceac, 0x4ecf1f].map(va),
};
const multigetInfo = nf(IO.Multiget, 'void', []);
function ioRun(type, outs) {
  soStubs(outs);
  soNext.add(0x200).writeS32(type);
  const undoNext = soPatch(IO.next, soNext), undoGet = soPatch(IO.get, soGet);
  try { multigetInfo(); } finally { undoNext(); undoGet(); }
}
const dwords = (x) => { const b = new ArrayBuffer(8); const dv = new DataView(b); dv.setFloat64(0, x, true); return [dv.getInt32(0, true), dv.getInt32(4, true)]; };
function ioMake() {
  const en = T.EN.readS32();
  const e = srnd(5) ? en : 1 + srnd(8);
  if (srnd(100) < 60) {
    let sh;
    if (srnd(6)) {
      // A point (x, y) of the triangle (319, 50 + h) / (271.9, 50) / (366.1, 50), h = sqrt(2700).
      const h = Math.sqrt(2700), y = 51 + srnd(51), half = (50 + h - y) / h * 47.1;
      const x = Math.floor(319 - half + srnd(1000) / 1000 * 2 * half);
      const d = (ax, ay, bx, by) => Math.abs((bx - ax) * (y - ay) - (by - ay) * (x - ax)) / Math.hypot(ay - by, bx - ax);
      sh = [d(271.9, 50, 366.1, 50) / h, d(319, 50 + h, 366.1, 50) / h / 1.25, d(319, 50 + h, 271.9, 50) / h / 1.25].map((v) => (v < 0.03 ? 0 : v));
      if (!sh.some((v) => v > 0)) sh[0] = 0.5;
    } else {
      sh = [srnd(1000) / 997, srnd(3) ? srnd(1000) / 991 : 0, srnd(1000) / 983];
      if (!sh.some((v) => v > 0)) sh[2] = 0.25;
    }
    return [0x5b, [[e]].concat(sh.map(dwords))];
  }
  let lim;
  if (srnd(5)) {
    const x = srnd(101), v = Math.floor((Math.exp(x * 2000 * 3 / 200000) - 1) * 10500);
    lim = Math.trunc(v / 100) * 100;
    if (lim > 15000) lim = Math.trunc(lim / 1000) * 1000;
  } else lim = srnd(3) ? srnd(200000) + 0.5 : 0;
  return [0x5c, [[e], dwords(lim)]];
}
function stageInfo() {
  const libc = (cfg.seed * 7919 + 41 + A.Ido2.readS32()) >>> 0;
  srand(libc);
  for (let i = 0; i < cfg.info; i++) {
    const o = ioMake();
    ioRun(o[0], o[1]);
    emit({ k: 'iorder', before: k, libc, type: o[0], vals: [].concat(...o[1]) });
  }
}
// --answers N: every --answers-every ticks (from tick 1), N random inbox steps at IdoNapValtozott, before that tick's
// save (re/rules/info-messages.md §6-§7): the stories' message calls through the exe's own functions (NewMessage
// 0x508fe0 with SetMessageFajOverride/SetParamStr/SetParamInt, KiszedUzenet 0x506200, KiszedUtolsoUzenet 0x506140,
// LetiltReszlet 0x5063d0) and the messages screen's steps as the screen makes them: a list row clicked
// (Messages_Main's code, then KiertekelUjuzenet 0x505d60), an answer (AktValasz0to3 + VoltUjValasz, then
// ValaszKivalasztva 0x509d10), FRISSIT_Messages2's automatic message (0x5058b0, 0x504a20, 0x505d60), the arrows. The
// typing end lies inside MessagesGfx2D_2 (exe 0x507b43..0x507c1b) and is transcribed here. Set-up before save 0:
// the kinds loaded (LoadMessagesFile 0x5063f0), no dialogue running, five messages. Each step is reported as
// {k: 'morder', before: k, op, vals}: op 'new' [id, days], 'race' [r], 'param' [slot, text], 'paramint' [slot, v],
// 'remove' [id], 'withdraw' [id], 'disable' [part], or 'order' [type, ...values] (ig2_sim MessageOrder's wire form).
const MS = {
  newMessageRaw: nf(va(0x508fe0), 'void', ['int', 'int']),
  // An automatic kind sets AktKepKell 10 (the screen would open and pause the game): kept as it was.
  newMessage: (id, days) => { const kk = va(0xbae250).readS32(); MS.newMessageRaw(id, days); va(0xbae250).writeS32(kk); },
  race: nf(va(0x5094c0), 'void', ['int']),
  paramStr: nf(va(0x509570), 'void', ['int', 'pointer']),
  paramInt: nf(va(0x509500), 'void', ['int', 'int']),
  remove: nf(va(0x506200), 'void', ['int']),
  withdraw: nf(va(0x506140), 'void', ['int', 'int']),
  disable: nf(va(0x5063d0), 'void', ['int']),
  load: nf(va(0x5063f0), 'void', []),
  open: nf(va(0x505d60), 'void', []),
  answer: nf(va(0x509d10), 'void', []),
  findAuto: nf(va(0x5058b0), 'void', []),
  placeAuto: nf(va(0x504a20), 'void', []),
  g: (a) => va(a).readS32(), s: (a, v) => va(a).writeS32(v), b: (a) => va(a).readU8(), sb: (a, v) => va(a).writeU8(v),
};
// UzenetFajtak (0xD90DF0, 0x110 each: +0 id, +4 conversation, +5 automatic), HanyUzenetFajta 0xDABE38.
function msKinds() {
  const n = MS.g(0xdabe38), out = { conv: [], auto: [], plain: [], flags: {} };
  for (let i = n - 1; i >= 0; i--) {
    const r = va(0xd90df0 + 0x110 * i);
    out.flags[r.readS32()] = [r.add(4).readU8(), r.add(5).readU8()];
  }
  for (const id in out.flags) {
    const f = out.flags[id];
    (f[1] ? out.auto : f[0] ? out.conv : out.plain).push(id | 0);
  }
  return out;
}
let msK = null;
function msPick(list) { return list.length ? list[srnd(list.length)] : 1; }
function msNew() {
  const r = srnd(10), id = r < 6 ? msPick(msK.conv) : r < 8 ? msPick(msK.auto) : msPick(msK.plain);
  const days = srnd(3) ? 0 : 1 + srnd(40);
  MS.newMessage(id, days);
  emit({ k: 'morder', before: k, op: 'new', vals: [id, days] });
  if (!srnd(3)) { const x = srnd(16); MS.race(x); emit({ k: 'morder', before: k, op: 'race', vals: [x] }); }
  if (!srnd(3)) {
    const slot = srnd(6), t = ['Orion', 'X', 'a long parameter text'][srnd(3)];
    MS.paramStr(slot, Memory.allocAnsiString(t));
    emit({ k: 'morder', before: k, op: 'param', vals: [slot, t] });
  }
  if (!srnd(4)) {
    const slot = srnd(6), v = srnd(100000) - 500;
    MS.paramInt(slot, v);
    emit({ k: 'morder', before: k, op: 'paramint', vals: [slot, v] });
  }
}
function msOrder(type, vals) { emit({ k: 'morder', before: k, op: 'order', vals: [type].concat(vals) }); }
// MessagesGfx2D_2's typing end (exe 0x507b43..0x507c1b), transcribed: Szovegvegen; a plain kind clears
// FolyamatbanU/UH; a dialogue that ends here (ParbeszedVegeJon) is over: the entry read; FolyamatbanPA.
function msTypedOut() {
  if (MS.b(0xda6607) === 1) return;
  MS.sb(0xda6607, 1); MS.s(0xdabde0, 1);
  const cur = MS.g(0xdabdc0), id = MS.g(0xda6620 + 0x88 * cur), f = msK.flags[id] || [0, 0];
  if (!f[0]) { MS.s(0xdabda0, -1); MS.s(0xdabda4, -1); }
  if (MS.b(0xd909bb)) {
    MS.sb(0xd909ba, 1); MS.s(0xdabda0, -1); MS.s(0xdabda4, -1); MS.s(0xdabdb0, -1);
    va(0xda6624 + 0x88 * cur).writeU8(1); MS.s(0xdabdb8, -1);
  }
  MS.sb(0xda65e7, MS.b(0xd909ba));
  msOrder(5, []);
}
function msStep() {
  const r = srnd(100), n = MS.g(0xdabdd0);
  if (r < 14 || n === 0) return msNew();
  if (r < 18) {
    const id = MS.g(0xda6620 + 0x88 * srnd(n));
    if (srnd(2)) { MS.remove(id); emit({ k: 'morder', before: k, op: 'remove', vals: [id] }); }
    else { MS.withdraw(id, 0); emit({ k: 'morder', before: k, op: 'withdraw', vals: [id] }); }
    return;
  }
  if (r < 21) { const p = 1 + srnd(9); MS.disable(p); emit({ k: 'morder', before: k, op: 'disable', vals: [p] }); return; }
  if (r < 26) {
    // The arrows (Messages_Main): one newer / older; the selected row follows the message.
    let p = MS.g(0xdabdd4), row = MS.g(0xd90dc8);
    if (srnd(2)) { if (p > 0) { p--; if (row !== -1) row = row < 3 ? row + 1 : -1; } }
    else if (p + 4 < n) { p++; if (row !== -1) row = row < 1 ? -1 : row - 1; }
    MS.s(0xdabdd4, p); MS.s(0xd90dc8, row);
    return msOrder(1, [p, row]);
  }
  const running = MS.g(0xdabda0) !== -1 || !MS.b(0xd909ba);
  if (running) {
    if (MS.b(0xda6607) !== 1) return msTypedOut();
    const c = MS.g(0xdabe3c);
    if (c > 0) {
      const kk = srnd(c);
      MS.s(0xd90dcc, kk); MS.s(0xdabdb4, 1); MS.answer();
      return msOrder(4, [kk]);
    }
    return;
  }
  if (MS.g(0xdabdd8) === -1 && srnd(3)) {
    // A list row (Messages_Main's code after its checks: no dialogue, ParbeszedVege, no automatic message).
    const row = srnd(4);
    MS.s(0xdabdb8, -1);
    if (n > row) { MS.s(0xd90dc8, row); MS.s(0xdabdb8, 1); }
    if (n < MS.g(0xd90dc8)) MS.s(0xd90dc8, -1);
    MS.placeAuto();
    if (MS.g(0xdabdb8) === 1) MS.open();
    return msOrder(2, [row]);
  }
  // FRISSIT_Messages2's automatic message.
  if (MS.g(0xdabdb8) === -1 || (MS.g(0xdabde0) === 1 && MS.b(0xd909ba))) {
    MS.findAuto();
    if (MS.g(0xdabdd8) !== -1) { MS.s(0xdabdb8, 1); MS.placeAuto(); MS.open(); }
  }
  msOrder(3, []);
}
function stageAnswersSetup() {
  if (MS.g(0xdabe38) === 0) MS.load();
  msK = msKinds();
  soSeed = (cfg.seed | 0) ^ 0x3e55;
  MS.s(0xdabda0, -1); MS.s(0xdabda4, -1); MS.sb(0xd909ba, 1); MS.sb(0xd909bb, 0); MS.sb(0xda6607, 1);
  MS.s(0xdabdd8, -1); MS.s(0xdabdb8, -1); MS.s(0xdabdc0, -1); MS.s(0xdabdd4, 0); MS.s(0xd90dc8, -1);
  for (let i = 0; i < 5; i++) MS.newMessage(i < 3 ? msPick(msK.conv) : i < 4 ? msPick(msK.auto) : msPick(msK.plain), 0);
  return { kinds: [msK.conv.length, msK.auto.length, msK.plain.length], inbox: MS.g(0xdabdd0) };
}
function stageAnswers() {
  for (let i = 0; i < cfg.answers; i++) msStep();
}
// --design N: every --design-every ticks (from tick 3), N random design messages go through the exe's own
// MULTIGET_Design (0x4b4d60, re/rules/design-orders.md) at IdoNapValtozott, before that tick's save, with the
// starmap orders' stubs (0x33's flag is a byte field) and their set-up (the human's stations and two fleets): 0x33
// stores an issuer's design back (mode 3: in place or, with flag 1, as a new upgrade copy; else as a new design) with
// its price and build time now and then changed; 0x34 sells (mode 2, now and then another mode) a design's units of
// one holder (0 = all of them, a fleet, or a planet −p); 0x35 refits them to the design's upgrade target. Mostly the
// human's, now and then an AI's. Each is reported as {k: 'dorder', before: k, libc, type, vals} (vals: the issuer,
// then the dwords; 0x33's record as 33 dwords and its flag as one).
const DO = {
  Multiget: va(0x4b4d60),
  next: [0x4b4d7a, 0x4b5740].map(va),
  get: [0x4b4dd4, 0x4b5328, 0x4b565e].map(va),
};
const multigetDesign = nf(DO.Multiget, 'void', []);
function doRun(type, outs) {
  soStubs(outs);
  soNext.add(0x200).writeS32(type);
  const undoNext = soPatch(DO.next, soNext), undoGet = soPatch(DO.get, soGet);
  try { multigetDesign(); } finally { undoNext(); undoGet(); }
}
// The designs of e that hold units, by holder: {d: [holders]} over ships, vehicles and (as 0) stations.
function doHeld(e) {
  const out = {}, add = (d, h) => { (out[d] = out[d] || []).push(h); };
  const hs = SO.Hajok.readPointer(), fl = F.Flottak.readPointer();
  for (let i = 1; i <= SO.HanyHajo.readS32(); i++) {
    const r = hs.add(6 * i), d = r.add(2).readS16(), f = r.readS16();
    if (d !== 0x7fff && f > 0 && fl.add(f * 0x80 + 0x15).readU8() === e) add(d, f);
  }
  for (let i = 1; i <= SO.HanyTank.readS32(); i++) {
    const r = SO.Tankok.add(8 * i), d = r.add(2).readS16(), h = r.readS16();
    if (d === 0x7fff) continue;
    const o = h > 0 ? fl.add(h * 0x80 + 0x15).readU8() : owner(-h);
    if (o === e) add(d, h);
  }
  return out;
}
function doMake() {
  const en = T.EN.readS32();
  const e = srnd(6) ? en : 1 + srnd(8);
  const n = designs(e);
  if (n < 1) return null;
  const held = doHeld(e), withUnits = Object.keys(held).map(Number).filter((d) => d >= 1 && d <= n);
  const live = [];
  for (let d = 1; d <= n; d++) if (design(e, d).add(0x16).readS16() >= 0) live.push(d);
  const upg = live.filter((d) => design(e, d).add(0x1c).readS32() > 0);
  const pick = () => (withUnits.length && srnd(4) ? spick(withUnits) : (live.length ? spick(live) : 1 + srnd(n)));
  const r = srnd(100);
  // The upgrade targets of designs with units: an in-place edit there refits those units (TervAzOsszesHajotAtalalitja).
  // Edited in place at about the old design's price, they are affordable.
  const pairs = withUnits.map((x) => [x, design(e, x).add(0x1c).readS32()]).filter(([, t]) => t >= 1 && t <= n);
  if (r < 15 && pairs.length) {
    const [x, d] = spick(pairs), rec = new Uint8Array(design(e, d).readByteArray(0x84)), dv = new DataView(rec.buffer);
    dv.setInt32(0x24, Math.floor(design(e, x).add(0x24).readS32() * (30 + srnd(90)) / 100), true);
    const words = [];
    for (let i = 0; i < 33; i++) words.push(dv.getInt32(4 * i, true));
    return [0x33, [[e], [3], [d], words, { u8: 0 }]];
  }
  if (r < 40) {
    const d = pick(), rec = new Uint8Array(design(e, d).readByteArray(0x84)), dv = new DataView(rec.buffer);
    if (srnd(3)) dv.setInt32(0x24, Math.max(0, Math.floor(dv.getInt32(0x24, true) * (50 + srnd(250)) / 100) + srnd(3) * srnd(5000)), true);
    if (srnd(3) === 0) dv.setInt32(0x20, Math.max(1, Math.floor(dv.getInt32(0x20, true) * (50 + srnd(150)) / 100)), true);
    const words = [];
    for (let i = 0; i < 33; i++) words.push(dv.getInt32(4 * i, true));
    const mode = srnd(5) ? 3 : spick([0, 1, 2]);
    return [0x33, [[e], [mode], [d], words, { u8: srnd(3) === 0 ? 1 : 0 }]];
  }
  if (r < 75) {
    const d = pick(), hs = held[d] || [];
    const P = soState().planets;
    const h = srnd(4) === 0 || !hs.length ? (srnd(3) || !P.length ? 0 : -spick(P)) : spick(hs);
    return [0x34, [[e], [d], [srnd(6) ? 2 : spick([0, 1, 3])], [h]]];
  }
  const d = upg.length && srnd(4) ? spick(upg) : pick(), hs = held[d] || [];
  return [0x35, [[e], [d], [hs.length && srnd(5) ? spick(hs) : 0]]];
}
// --design at tick 2 (the replay test has no source for fresh battlefield units' bytes at tick 1): the staged fleets' vehicles go to the human's first planet as garrisons (planet holders −p)
// through the exe's own MULTIGET_Starmap 0x1c, reported as starmap orders ({k: 'sorder'}).
function doGarrisons() {
  const st = soState();
  if (!st.planets.length) return;
  const libc = (cfg.seed * 7919 + 17 + A.Ido2.readS32()) >>> 0;
  srand(libc);
  for (const f of st.mine) {
    for (const [d, i] of soHeld(f)) {
      if (i !== 0) continue;
      const o = [0x1c, [[st.en], [f], [-st.planets[0]], [d], [0], [2]]];
      soRun(o[0], o[1]);
      emit({ k: 'sorder', before: k, libc, type: o[0], vals: [].concat(...o[1]) });
    }
  }
}
function stageDesign() {
  const libc = (cfg.seed * 7919 + 59 + A.Ido2.readS32()) >>> 0;
  srand(libc);
  for (let i = 0; i < cfg.design; i++) {
    const o = doMake();
    if (!o) continue;
    doRun(o[0], o[1]);
    emit({ k: 'dorder', before: k, libc, type: o[0], vals: [].concat(...o[1].map((x) => (Array.isArray(x) ? x : [x.u8]))) });
  }
}
function stageOrdersSetup() {
  soSeed = cfg.seed | 0;
  va(0xb77230).writeU8(1);
  const st = soState(), out = { en: st.en, planets: st.planets.length, fleets: st.mine.length, designs: [st.des[1].length, st.des[2].length, st.des.st.length] };
  if (!st.planets.length) return out;
  for (const p of st.planets.slice(0, 3)) {
    if (!st.des.st.length) break;
    const q = T.Bolygok.add(p * 0x188), n = 2 + srnd(4);
    q.add(0x114).writeS32(n);
    for (let k = 1; k <= n; k++) q.add(0xfe + 2 * k).writeS16(spick(st.des.st));
  }
  const home = st.planets[0], q = T.Bolygok.add(home * 0x188);
  out.made = [];
  for (let j = 0; j < 2 && st.des[1].length; j++) {
    const f = flottaNew(Memory.allocAnsiString('Stage'), st.en, 0, q.add(0x18).readFloat() + j, q.add(0x1c).readFloat(), 0);
    if (f <= 0) continue;
    for (let s = 0; s < 3; s++) hajoNew(f, spick(st.des[1]));
    if (st.des[2].length) soTankNew(f, spick(st.des[2]), 2);
    out.made.push(f);
  }
  return out;
}

// --colony N: every --orders-every ticks (from tick 1), N random colony orders go through the exe's own
// MULTIGET_Colony (0x48a270, re/screens/colony.md §9) at IdoNapValtozott, like --orders: its 2 Multi_GetNextTipus
// calls and 16 Multi_Get calls are pointed at the stubs. Each is reported as {k: 'corder', before: k, libc, type,
// vals}. The orders are shaped like the screen's: build kinds seen on the map at a random cell of the issuer's
// planet, demolish/switch/priority/repair of its buildings, now and then a foreign planet, an empty or missing slot,
// the shown planet (0x26), the self-destruct switch (0x27/300). Set-up before save 0: a third of the human's
// buildings get random damage (for the repairs).
const CO = {
  Multiget: va(0x48a270), Epuletek: va(0xba9e90), Slots: va(0xba9840), Leiro: va(0x8eba38),
  next: [0x48a29a, 0x48a2b1].map(va),
  get: [0x48a2fa, 0x48a501, 0x48a591, 0x48a602, 0x48a6ea, 0x48a787, 0x48a86b, 0x48a90b, 0x48a943, 0x48a984, 0x48a9da,
    0x48aa34, 0x48ab31, 0x48aba9, 0x48ac0f, 0x48aec7].map(va),
};
const multigetColony = nf(CO.Multiget, 'void', []);
const coSlots = (p) => CO.Slots.add(4 * p).readS32();
const coRec = (p, k) => CO.Epuletek.add(4 * p).readPointer().add(0x20 * k);
function coRun(type, outs) {
  soStubs(outs);
  soNext.add(0x200).writeS32(type);
  const undoNext = soPatch(CO.next, soNext), undoGet = soPatch(CO.get, soGet);
  try { multigetColony(); } finally { undoNext(); undoGet(); }
}
function coKinds() {
  const kinds = new Set();
  for (let p = 1; p <= T.HanyBolygo.readS32(); p++) {
    for (let k = 1; k <= coSlots(p); k++) { const t = coRec(p, k).readU8(); if (t) kinds.add(t); }
  }
  return [...kinds];
}
let coKindList = [];
function coMake(st) {
  const en = st.en, r = srnd(100);
  const P = st.planets.length ? st.planets : [1];
  const planet = () => (srnd(10) ? spick(P) : 1 + srnd(st.np));
  const slotOf = (p) => 1 + srnd(coSlots(p));
  const live = (p) => { const n = coSlots(p), out = []; for (let k = 1; k <= n; k++) if (coRec(p, k).readU8()) out.push(k); return out; };
  const building = (p) => { const l = live(p); return l.length && srnd(12) ? spick(l) : slotOf(p); };
  if (r < 34) {
    const p = planet(), kind = coKindList.length ? spick(coKindList) : 1;
    const x = srnd(8) ? 20 + srnd(160) : -1, y = x < 0 ? -1 : 20 + srnd(160);
    return [0x1f, [[en], [p], [kind], [x], [y]]];
  }
  // The screen only names used slots (1..=HanyEpuletLefoglalva; empty ones now and then): past them the exe reads
  // the block's unused records (a demolish there counts HanyEpulet down).
  if (r >= 34 && r < 92) {
    const p = planet();
    if (coSlots(p) <= 0) return null;
    return [r < 46 ? 0x20 : r < 62 ? 0x21 : r < 74 ? 0x132 : 0x133, [[en], [p], [building(p)]]];
  }
  if (r < 96) return [0x26, [[en], [planet()]]];
  if (r < 98) return [0x27, [[en], [spick(P)]]];
  return [300, [[en], [spick(P)]]];
}
function stageColony() {
  const libc = (cfg.seed * 7919 + 29 + A.Ido2.readS32()) >>> 0;
  srand(libc);
  for (let i = 0; i < cfg.colony; i++) {
    const o = coMake(soState());
    if (!o) continue;
    coRun(o[0], o[1]);
    emit({ k: 'corder', before: k, libc, type: o[0], vals: [].concat(...o[1]) });
  }
}
function stageColonySetup() {
  soSeed = (cfg.seed | 0) ^ 0x5eed;
  coKindList = coKinds();
  const st = soState(), out = { en: st.en, planets: st.planets.length, kinds: coKindList.length, damaged: 0 };
  for (const p of st.planets) {
    for (let k = 1; k <= coSlots(p); k++) {
      const r = coRec(p, k), t = r.readU8();
      if (!t || srnd(3)) continue;
      const hp = CO.Leiro.add(0x24 * t + 8).readS32();
      r.add(4).writeS32(1 + srnd(Math.max(1, hp - 1)));
      out.damaged++;
    }
  }
  return out;
}

let k = 0, frames = 0, phase = 0, loadedAt = 0, homeKep = -1, busy = false;

// --randlog: the callers of libc rand() during the tick (run-length [return address, count]; [-1, 1] marks
// FRISSIT_Trade's entry), reported with the next save as 'rands'.
let rands = [];
function mark(a) {
  if (!cfg.randlog || phase !== 2 || busy) return;
  const last = rands[rands.length - 1];
  if (last && last[0] === a) last[1]++; else rands.push([a, 1]);
}
Interceptor.attach(crt.getExportByName('rand'), {
  onEnter() { if (cfg.randlog) mark(this.returnAddress.sub(mod.base).add(IMAGE_BASE).toInt32()); },
});
Interceptor.attach(va(0x605ed0), { onEnter() { mark(-1); } });

// --messages also logs FRISSIT_Scroll2 (exe 0x556210, the 100 Hz UI step of the scrolling line, info-messages.md
// §2.3) whenever a call changed the line: [start, speed, limit] (f32 bits) and the text length before and after,
// and the queue length before and after ('scroll2', batched by 200).
let scrollRows = [];
function scrollState() {
  const t = va(0xe41230).readCString();
  return [va(0xe412fc).readU32(), va(0xe41300).readU32(), va(0xe41304).readU32(), t ? t.length : 0, va(0xe413d4).readS32()];
}
// --scroll2 instead switches to the starmap (AktKepKell 0xBAE250 = 2: FRISSIT_Scroll2 runs only below screen 0xE),
// writes no per-tick saves and posts test texts through ScrollUzenet (exe 0x556b50) at set times after the load:
// lengths 4..60 (cut at 53), important ones that cut the line, an interrupt.
const scrollUzenet = nf(va(0x556b50), 'void', ['pointer', 'int', 'int', 'int']);
const SCROLL_POSTS = [
  [1000, 'Abcd', 0x14, 0, 0], [1000, 'A message of twenty-one', 2, 0, 0], [1000, 'x'.repeat(37), 3, 0, 0],
  [1000, 'y'.repeat(60), 4, 0, 0], [25000, 'Important one', 5, 1, 0], [26000, 'z'.repeat(52), 6, 1, 0],
  [26000, 'Cut in', 7, 0, 1], [60000, 'Last', 8, 0, 0],
];
let scrollT0 = 0, scrollNext = 0;
function scrollPosts() {
  const t = Date.now() - scrollT0;
  while (scrollNext < SCROLL_POSTS.length && SCROLL_POSTS[scrollNext][0] <= t) {
    const [, text, id, imp, intr] = SCROLL_POSTS[scrollNext++];
    scrollUzenet(Memory.allocAnsiString(text), id, imp, intr);
    emit({ k: 'scroll2post', text, id, imp, intr, kep: A.AktKep.readS32() });
  }
}
Interceptor.attach(va(0x556210), {
  onEnter() { if ((cfg.messages || cfg.scroll2) && phase === 2) this.pre = scrollState(); },
  onLeave() {
    if (!this.pre) return;
    const post = scrollState();
    if (post.some((v, i) => v !== this.pre[i])) scrollRows.push([...this.pre, ...post]);
    if (scrollRows.length >= 200) { emit({ k: 'scroll2', rows: scrollRows }); scrollRows = []; }
  },
});

function save(where) {
  busy = true;
  const slot = cfg.slotBase + k, ido2 = A.Ido2.readS32();
  const seed = (cfg.seed + ido2) >>> 0;
  const vol = volatile(k === 0);
  loadSave(slot, Memory.allocAnsiString(cfg.name + ' ' + k), 2);
  const ev = { k: 'save', idx: k, slot, where, ido: A.Ido.readS32(), ido2, seed, vol };
  if (cfg.randlog) { ev.rands = rands; rands = []; }
  emit(ev);
  flush();
  busy = false;
  k++;
}

Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    if (busy) return;
    frames++;
    if (cfg.scroll2 && phase === 2) scrollPosts();
    if (phase === 0 && frames === cfg.startAfterFrames) {
      betolt(Memory.allocAnsiString(cfg.scenario));
      setSpeed(cfg.speed, 1);
      loadedAt = frames; phase = 1;
      emit({ k: 'scenario' });
      return;
    }
    if (phase === 1 && frames >= loadedAt + 300) {
      homeKep = A.AktKep.readS32();
      busy = true;
      loadSave(cfg.slot, Memory.allocAnsiString(cfg.name), 1);
      setSpeed(cfg.speed, 1);
      setPause(0, 0);
      if (cfg.traders) emit({ k: 'traders', stage: stageTraders() });
      if (cfg.spies) emit({ k: 'spies', stage: stageSpies() });
      if (cfg.merges) emit({ k: 'merges', stage: stageMerges() });
      if (cfg.wormholes) emit({ k: 'wormholes', stage: stageWormholes() });
      if (cfg.battles) emit({ k: 'battles', stage: stageBattles() });
      if (cfg.deaths) emit({ k: 'deaths', stage: stageDeaths() });
      if (cfg.messages) emit({ k: 'messages', stage: stageMessages() });
      if (cfg.scroll2) { A.AktKepKell.writeS32(2); scrollT0 = Date.now(); }
      if (cfg.tutorialSighting) emit({ k: 'tutorial', stage: stageTutorial() });
      if (cfg.orders || cfg.design) emit({ k: 'orders', stage: stageOrdersSetup() });
      if (cfg.colony) emit({ k: 'colony', stage: stageColonySetup() });
      if (cfg.research) { soSeed = (cfg.seed | 0) ^ 0x5eed; va(0xb77230).writeU8(1); }
      if (cfg.info) { soSeed = (cfg.seed | 0) ^ 0x1f0; va(0xb77230).writeU8(1); }
      if (cfg.design) soSeed = (cfg.seed | 0) ^ 0xd35;
      if (cfg.answers) emit({ k: 'messages', stage: stageAnswersSetup() });
      if (cfg.stage) {
        const st = stageTrade();
        loadSave(cfg.preSlot, Memory.allocAnsiString(cfg.name + ' pre'), 2);
        emit({ k: 'pre', slot: cfg.preSlot, stage: st });
        const h = st.mine[0];
        if (T.N.readS32() > 0) {
          buy(h, T.EtaDays.readS32(), 0, 1);
          buy(h, 2, 0, T.Offers.add(0x6c).readS32());
          buy(h, 2, 0, 1);
          if (st.theirs.length) buy(st.theirs[0], 2, T.N.readS32() - 1, 1);
        }
      }
      busy = false;
      emit({ k: 'loaded', ido: A.Ido.readS32(), ido2: A.Ido2.readS32(), kep: A.AktKep.readS32(), home: homeKep });
      save('load');
      phase = 2;
    }
  },
});

Interceptor.attach(A.IdoNapValtozott, {
  onEnter() {
    if (busy || phase !== 2) return;
    if (cfg.orders && k >= 1 && k % cfg.ordersEvery === 0) stageOrders();
    if (cfg.colony && k >= 1 && k % cfg.ordersEvery === 0) stageColony();
    if (cfg.research && k >= 1 && k % cfg.researchEvery === 0) stageResearch();
    if (cfg.info && k >= 1 && k % cfg.infoEvery === 0) stageInfo();
    if (cfg.answers && k >= 1 && k % cfg.answersEvery === 0) stageAnswers();
    if (cfg.design && k === 2) doGarrisons();
    if (cfg.design && k >= 3 && k % cfg.designEvery === 0) stageDesign();
    if (cfg.scroll2) {
      k++;
      if (k > cfg.ticks) { emit({ k: 'scroll2', rows: scrollRows }); scrollRows = []; phase = 3; emit({ k: 'done' }); flush(); }
      return;
    }
    save('tick');
    srand((cfg.seed + A.Ido2.readS32()) >>> 0);
    if (k > cfg.ticks) { phase = 3; emit({ k: 'done' }); flush(); }
  },
});

send({ type: 'ready', base: mod.base.toString() });
