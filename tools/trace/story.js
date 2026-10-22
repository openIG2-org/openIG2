// Frida agent: the story engine on the GOG ig2.exe (spec re/rules/campaign.md).
// Records:
//  'tick'   one per FRISSIT_Story (exe 0x5c1ff0) the game runs: the story state and the world inputs the
//           scripts read, before and after (the counters' Story_Check(1) and the daily one nest inside);
//  'check'  one per outermost Story_Check (0x5c5610) the game enters from anywhere else (a Story_* hook);
//  'staged' a call the tracer makes itself (--fuzz): Story_Check(1) after writing event flags and payloads,
//           Story_Special, Story_FleetArrivedSave + Story_FleetArrivedCheck, Story_SpaceBattleFinished with a
//           fake participant list, st_AddGameInfo/st_RemoveGameInfo, WIN, StartStory + Story_Check(1);
//  'init'   (--campaign) InitStory (0x5c2940) inside the campaign start: before, after LoadScenario ('mid')
//           and after.
// Scenario mode loads a skirmish .scn with Betolt_Scenario; between records the fuzzer (--fuzz) queues the
// scenario stories 360..375 with their variables, planet and fleet ids and counters.
// Campaign mode (--campaign N) starts campaign N the way the race menu does (CampaignGameStart 0x4f5e90,
// InitNewGame → InitStory, StartStory(400)) and patches the unported scripts (st_Begin/Rnd/BRnd/Fable/Line/
// Elem/Multi_include) to `ret`, so only the engine, the scenario script and the inline stories of
// Story_Check run. Its staged calls go through a native trampoline the frame hook's return jumps into, so the
// game's code runs outside the agent and the hooks below see it. Every record then also lists the calls the
// stories made to other systems ('acts': FlottaNew, FlottaMozgat, HajoNew, TankNew, st_IsInRR, st_GetTerv,
// AddWormhole, …, with their arguments, depth and return value) and the inputs st_fAddShip read ('probes').
// MEGNYERES (0x53f6b0: the game is won) and MEGHALAS (0x53b630: an empire dies) are patched to count their
// calls and return; st_ShowAnim and SaveRegistrySetting only log.
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  Frissit: va(0x5c1ff0), Check: va(0x5c5610), Win: va(0x5c9770), Megnyeres: va(0x53f6b0), Meghalas: va(0x53b630),
  StartStory: va(0x5c4840), AddGameInfo: va(0x5c9d00), RemoveGameInfo: va(0x5f0d50), BAddAzon: va(0x6031f0),
  VSet: va(0x605780), SzStart: va(0x605570), Special: va(0x5c9550), ArrivedSave: va(0x5c8cb0),
  ArrivedCheck: va(0x5c8c90), SpwFinished: va(0x5c9180), DiplSpecial: va(0x5c8c60),
  CampaignGameStart: va(0x4f5e90), InitStory: va(0x5c2940), LoadScenario: va(0x545b40),
  ReadRegistry: va(0x51b3b0), SaveRegistry: va(0x51b860), GetParam: va(0x53e4c0), ShowAnim: va(0x602bb0),
  // Story state.
  sBolygok: va(0xf23a20), sSzamlalok: va(0xf24388), sLogikaiak: va(0xf24cf0), sValtozok: va(0xf251a8),
  nB: va(0xf23a08), nSz: va(0xf23a0c), nL: va(0xf23a10), nV: va(0xf23a14), Err: va(0xf23a1c), SkipFirst: va(0xf23a04),
  Flags: va(0xf2f8ac), Last: va(0xf2273c), SzVegzett: va(0xf26560), OldIdo: va(0xf432b4), Delayed: va(0xf26530),
  Restart: va(0xf261fc), Locked: va(0xf265dc), Adat: va(0xf273f8), Vodor: va(0xf42678), History: va(0xf43278),
  StartQ: va(0xf43284), Vegzes: va(0xf43290), Talal: va(0xf4329c), Olvas: va(0xf432a8),
  OzonN: va(0xf265d8), Ozon: va(0xf265e0), Signal: va(0xf26564), LastAddship: va(0xf26200),
  Special0: va(0xf265a4), RadarB: va(0xf2657c), RadarF: va(0xf26580), Dipl: va(0xf2656c), DiplValasz: va(0xf42660),
  Arr: va(0xf2654c), Gr: va(0xf26538), GrWin: va(0xf26532), YouWas: va(0xf26531), Spw: va(0xf265c4), KemTipus: va(0xf26588),
  Msg: va(0xf26574), Disable: va(0xf273f0), FnB: va(0xf27524), FnR: va(0xf273f4), FnQ: va(0xf27520), FnX: va(0xf27528),
  FnY: va(0xf2752c), LineStory: va(0xb77288), Scenario: va(0xb7728c),
  ValuesN: va(0xf26204), Values: va(0xf26208), FleetDataN: va(0xf2f870), FleetData: va(0xf27530),
  // World.
  Ido: va(0xbae04c), Speed: va(0xbae25c), EN: va(0x912a3c), Nehezseg: va(0x92827c), MP: va(0xbae2c0),
  Campaign: va(0x928278), El: va(0xbae6ac), HanyB: va(0xbaeb18), Money: va(0xbad168), Techs: va(0xb9c0c8),
  Viszony: va(0xdabf38), RandSeed: va(0xbaf2d0), Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8),
  Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8), AktKep: va(0xbae0fc), AktKepKell: va(0xbae250),
  Ismeri: va(0xbaebb8), PlayerFaj: va(0xbae574), AktPlanet: va(0xb9933c), AktFleet: va(0xb99664),
  Hajok: va(0xbb4100), HajoCount: va(0xbb4104), Tervek: va(0xb9aa18), TervCount: va(0xb9c028), RR2: va(0xbb61a8),
  MiTervMutato: va(0xdaf668), DebugSeed: va(0xb7725c), Ero: va(0xbaea78), Nevek: va(0xbb6730), Near: va(0xf2652c),
  // st_Multi_include (--keep multi): CTF_value, the players' names, the info string table.
  Ctf: va(0xf261ec), PlayerNev: va(0xbae588), InfoStr: va(0xbaa1f0), InfoStrN: va(0xbad0d0),
  // st_BRnd_include (--keep brnd): spFlottak, the wormholes, the global morale, st_bGet3BestProd's static,
  // Story_Minden_Flotta_Latszik, StoryBuildingData.
  SpFlottak: va(0xbaa1c6), Wormholes: va(0xbb60f8), WormholeN: va(0xbb61a0), GMoral: va(0xbaeac4), GMoralSpeed: va(0xbaeb14),
  Best3: va(0xf432b8), AllVis: va(0xf265d4), BuildingData: va(0xf22740), BuildingDataN: va(0xf23a00),
  // st_Begin_include (--keep begin): Tankok, the trade offers, the spies, KemEsemeny, CounterIntelligence_kem,
  // SpWarKiNyert, ResearchAktFejlesztett, the buildings.
  Tanks: va(0xbb41a8), TankN: va(0xbb4154), TradeN: va(0xf439cc), TradeT: va(0xb9aaf0), Kemek: va(0xbafcc8), KemN: va(0xbb1f84),
  KemNews: va(0xf2f874), CiKem: va(0xf265d0), SpWin: va(0xbaa1ec), Research: va(0xb9a9b8), HanyEpulet: va(0xba9840),
  Epuletek: va(0xba9e90),
  // st_Rnd_include (--keep rnd): the finished battle's list count and winner, st_Story, RndHosok.
  SpwN: va(0xf265c8), KiNyert: va(0xbaa1ec), StoryAt: va(0x602f80), RndHosok: va(0x9288b0),
  // st_Rnd_include part 2: KemkedesAzonosito, KemkedesValasz, Colony_foldrenges, StoryKuldetes, KemKuldetesek.
  KemAzon: va(0xf26584), KemValasz: va(0xf2659c), Quake: va(0x9abb70), StoryKuld: va(0x9289ac), KemKuld: va(0x912f48),
  // st_Rnd_include part 2c: KemkedesTulaj, KemkedesAktKem (st_KemEsely), last_hajo_kigyujt_flotta, 138's recruit
  // names, AlapFlottaNev, the ground battle's planet.
  KemTulaj: va(0xf26598), KemAktKem: va(0xf265a0), LastCollect: va(0xf265cc), Recruits: va(0x85e0bc), AlapNev: va(0xbb6964),
  // st_Fable_include (--keep fable) and st_Rnd part 2b: spFlottak and SpWarHanyFlotta (st_AddFleetToSpWar),
  // wormholes, spies, labs, the battle's winner and pacts, KemEsemeny.
  SpFl0: va(0xbaa1c0), SpFlottakN: va(0xbaa1e8), WormN: va(0xbb61a0), Worm: va(0xbb60f8),
  RankXp: va(0x9137a8), Labor: va(0x912728), ResAkt: va(0xb9a9b8),
  Pact: va(0xf26150), KemEv: va(0xf2f874), S: va(0x542ce0), OpenBink: va(0x7325b0), FlottaNew: va(0x53d400),
  // The race menu's state CampaignGameStart reads.
  FajStart: va(0xbbb17c), FajSzetnyilas: va(0xbbb16c), FajRace: va(0xbbb178), FajNehezseg: va(0xbbb180),
};
// The scripts the port doesn't have yet (Story_Check calls them; campaign mode patches them to `ret`).
// st_Elem_include's blocks the port doesn't have yet (none since part 2g): the `push id` before each block's
// st_Story(id, 1) call. Part 2b took out 808 (0x5d8716) and 854 (0x5e253a),
// part 2c 810 (0x5d95a1), 857 (0x5e3f60), 858 (0x5e4379) and 860 (0x5e4964), part 2d 801 (0x5d4bb7), 804 (0x5d6de5),
// 806 (0x5d7387), 807 (0x5d7c83) and 809 (0x5d8ff7), part 2e 814 (0x5d9d93), 815 (0x5da289), 821 (0x5db408), 832
// (0x5dde11) and 834 (0x5de47e), part 2f 836 (0x5def0a), 839 (0x5df708), 840 (0x5dfc10) and 843 (0x5e079f), part 2g
// 844 (0x5e0d66), 856 (0x5e30e2), 862 (0x5e5212) and 863 (0x5e5b4f).
const ELEM_UNPORTED = [];
const UNPORTED = { begin: 0x5cec20, rnd: 0x5f0e20, brnd: 0x5c9f20, fable: 0x5e6ba0, line: 0x5eb6f0, elem: 0x5d4b70, multi: 0x5ee4d0 };

const cfg = { scenario: 'scenario/intruders.scn', startAfterFrames: 1200, count: 2000, fuzz: false, seed: 4242,
  every: 40, speed: 3, campaign: 0, difficulty: 0, registry: null, storyline: null, storymap: null, keep: [] };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 8) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

let fz = 1;
const rnd = (n) => { fz ^= fz << 13; fz >>>= 0; fz ^= fz >>> 17; fz ^= fz << 5; fz >>>= 0; return n > 0 ? fz % n : 0; };
const pick = (arr) => arr[rnd(arr.length)];
const hex = (p, n) => { const b = new Uint8Array(p.readByteArray(n)); let s = ''; for (const x of b) s += (x < 16 ? '0' : '') + x.toString(16); return s; };

const nf = (p, r, a) => new NativeFunction(p, r, a, { abi: 'mscdecl' });
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const setPause = nf(A.SetIdoPause, 'void', ['int', 'int']);
const storyCheck = nf(A.Check, 'void', ['int']);
const startStory = nf(A.StartStory, 'int', ['int', 'int']);
const vSet = nf(A.VSet, 'void', ['int', 'int']);
const szStart = nf(A.SzStart, 'void', ['int', 'int']);
const bAddAzon = nf(A.BAddAzon, 'void', ['int', 'int']);
const storySpecial = nf(A.Special, 'void', ['int', 'int', 'int', 'int', 'int', 'int', 'int', 'int']);
const arrivedSave = nf(A.ArrivedSave, 'void', ['int']);
const arrivedCheck = nf(A.ArrivedCheck, 'void', []);
const spwFinished = nf(A.SpwFinished, 'void', ['pointer', 'int', 'int']);
const addGameInfo = nf(A.AddGameInfo, 'void', ['int', 'pointer', 'pointer', 'pointer', 'int', 'int']);
const removeGameInfo = nf(A.RemoveGameInfo, 'void', ['int']);
const win = nf(A.Win, 'int', ['int', 'int']);

// MEGNYERES / MEGHALAS: count the calls, remember the last argument, return.
const counters = Memory.alloc(16);
function patchCounter(addr, slot) {
  Memory.patchCode(addr, 16, (code) => {
    const w = new X86Writer(code, { pc: addr });
    w.putMovRegRegOffsetPtr('eax', 'esp', 4);
    w.putMovNearPtrReg(counters.add(slot * 8 + 4), 'eax');
    const c = counters.add(slot * 8).toUInt32();
    w.putBytes([0xff, 0x05, c & 0xff, (c >>> 8) & 0xff, (c >>> 16) & 0xff, c >>> 24]); // inc dword [count]
    w.putRet();
    w.flush();
  });
}
patchCounter(A.Megnyeres, 0);
patchCounter(A.Meghalas, 1);
const ends = () => [counters.readS32(), counters.add(4).readS32(), counters.add(8).readS32(), counters.add(12).readS32()];

const nPlanets = () => A.HanyBolygo.readS32();
const nFleets = () => A.FlottaCount.readS32();
const fleet = (f) => A.Flottak.readPointer().add(0x80 * f);
const f32bits = (p) => p.readU32();

function pairs(base, n, flagByte) {
  const out = [];
  for (let i = 1; i <= n; i++) {
    const p = base.add(8 * i);
    out.push([p.readS32(), flagByte ? p.add(4).readU8() : p.add(4).readS32()]);
  }
  return out;
}
function vodor(p) {
  const ids = p.readPointer(), dual = p.add(4).readPointer(), n = p.add(8).readS32(), out = [];
  for (let i = 0; i < n; i++) out.push([ids.add(4 * i).readS32(), dual.add(4 * i).readS32()]);
  return out;
}
function bucketHas(id) {
  const q = vodor(A.Vodor.add(12 * (id & 0xff)));
  return q.some((e) => e[0] === id);
}
// st_Rnd part 2 and st_Elem part 2a read and write the story missions' spy news.
function spyKept() { return cfg.keep.includes('rnd') || cfg.keep.includes('elem'); }
function story() {
  const buckets = {};
  for (let k = 0; k < 256; k++) {
    const q = vodor(A.Vodor.add(12 * k));
    if (q.length) buckets[k] = q;
  }
  const on = A.OzonN.readS32(), ozon = [];
  for (let i = 0; i < on; i++) { const p = A.Ozon.add(12 * i); ozon.push([p.readS32(), p.add(4).readS32(), p.add(8).readS32()]); }
  const ints = (p, n) => { const o = []; for (let i = 0; i < n; i++) o.push(p.add(4 * i).readS32()); return o; };
  return {
    b: pairs(A.sBolygok, A.nB.readS32(), true), sz: pairs(A.sSzamlalok, A.nSz.readS32(), false),
    l: pairs(A.sLogikaiak, A.nL.readS32(), true), v: pairs(A.sValtozok, A.nV.readS32(), false),
    nb: A.nB.readS32(), nsz: A.nSz.readS32(), nl: A.nL.readS32(), nv: A.nV.readS32(),
    err: A.Err.readS32(), flags: hex(A.Flags, 17), last: A.Last.readS32(), szv: A.SzVegzett.readS32(),
    oldido: A.OldIdo.readS32(), delayed: A.Delayed.readU8(), skip: A.SkipFirst.readS32(), restart: A.Restart.readS32(),
    adat: hex(A.Adat, 0x128), buckets, hist: vodor(A.History), start: vodor(A.StartQ), vegzes: vodor(A.Vegzes),
    talal: vodor(A.Talal), olvas: vodor(A.Olvas), ozon, signal: ints(A.Signal, 2), addship: A.LastAddship.readS32(),
    special: ints(A.Special0, 8), radar: [A.RadarB.readS32(), A.RadarF.readS32()],
    dipl: [A.Dipl.readS32(), A.Dipl.add(4).readS32(), A.DiplValasz.readS32()], arr: ints(A.Arr, 5),
    gr: ints(A.Gr, 5).concat([A.GrWin.readU8(), A.GrWin.add(1).readU8()]), youwas: A.YouWas.readU8(),
    spw: [A.Spw.readS32(), A.Spw.add(4).readS32()], kem: A.KemTipus.readS32(),
    msg: ints(A.Msg, 2), disable: A.Disable.readS32(),
    fn: [A.FnB.readS32(), A.FnR.readS32(), A.FnQ.readS32(), A.FnX.readS32(), A.FnY.readS32()],
    line: A.LineStory.readS32(), scen: A.Scenario.readS32(), near: A.Near.readS32(),
    best3: ints(A.Best3, 3), allvis: A.AllVis.readU8(),
    // KemEsemeny (news, spy, died, success, mission, target, tech, 5 techs), CounterIntelligence_kem, SpWarKiNyert.
    kn: [A.KemNews.readS32(), A.KemNews.add(4).readS32(), A.KemNews.add(8).readS32(), A.KemNews.add(0xc).readU8(),
      A.KemNews.add(0x14).readS32(), A.KemNews.add(0x18).readS32(), A.KemNews.add(0x1c).readS32()].concat(ints(A.KemNews.add(0x24), 5)),
    ci: A.CiKem.readS32(), spwin: A.SpWin.readS32(),
    // st_Rnd: the finished space battle's participants (owner +0x18, +0x20, +0x24) and SpWarKiNyert.
    battle: cfg.keep.includes('rnd') || cfg.keep.includes('elem') ? spwList() : undefined, kinyert: cfg.keep.includes('rnd') ? A.KiNyert.readS32() : undefined,
    // st_Rnd part 2: KemkedesAzonosito, KemkedesValasz.
    kaz: spyKept() ? A.KemAzon.readS32() : undefined, kval: spyKept() ? A.KemValasz.readS32() : undefined,
    // st_Rnd part 2c: KemkedesTulaj, KemkedesAktKem, last_hajo_kigyujt_flotta.
    ktul: spyKept() ? A.KemTulaj.readS32() : undefined, kakt: spyKept() ? A.KemAktKem.readS32() : undefined,
    // st_Elem part 2a: st_bGet3Nearest's static.
    n3: cfg.keep.includes('elem') ? ints(va(0xf432c4), 3) : undefined,
    // Part 2g: st_bGet4Brutal's static.
    n4: cfg.keep.includes('elem') ? ints(va(0xf432d0), 4) : undefined,
    lcol: cfg.keep.includes('rnd') ? A.LastCollect.readS32() : undefined,
    ...(cfg.keep.includes('fable') ? fableStory() : {}),
  };
}
function spwList() {
  const p = A.Spw.readPointer(), n = A.SpwN.readS32(), out = [];
  if (p.isNull() || n < 0 || n > 40) return out;
  for (let i = 0; i < n; i++) { const q = p.add(0x30 * i); out.push([q.add(0x18).readS32(), q.add(0x20).readS32(), q.add(0x24).readS32()]); }
  return out;
}
// st_Fable_include reads the finished battle's participants, its winner, and the spy report's payload; it writes
// StoryUrharcViszony[9].
function fableStory() {
  const out = { kinyert: A.KiNyert.readS32(), pact: [], kemev: [0, 4, 8].map((o) => A.KemEv.add(o).readS32()), spwl: [] };
  for (let e = 0; e < 19; e++) out.pact.push(A.Pact.add(4 * e).readS32());
  const lp = A.Spw.readPointer(), n = A.Spw.add(4).readS32();
  // Three entries at least: the scripts read the second (and third) even after a shorter list.
  if (!lp.isNull()) for (let i = 0; i < Math.min(Math.max(n, 3), 12); i++) {
    try { const q = lp.add(0x30 * i); out.spwl.push([q.add(0x18).readS32(), q.add(0x20).readS32(), q.add(0x24).readS32()]); } catch (e) { break; }
  }
  return out;
}
function varOf(id) {
  const n = A.nV.readS32();
  for (let i = 1; i <= n; i++) if (A.sValtozok.add(8 * i).readS32() === id) return A.sValtozok.add(8 * i + 4).readS32();
  return 0;
}
function world() {
  const en = A.EN.readS32(), np = nPlanets(), nfl = nFleets(), planets = [], fleets = [], multi = cfg.keep.includes('multi');
  // st_Begin_include, st_Rnd_include and st_Elem_include dump what st_BRnd's traces have and more.
  const begin = cfg.keep.includes('begin'), rndk = cfg.keep.includes('rnd'), elem = cfg.keep.includes('elem');
  const brnd = cfg.keep.includes('brnd') || begin || rndk || elem;
  const fable = cfg.keep.includes('fable');
  for (let p = 0; p <= np; p++) { // slot 0 too: a failed search returns 0 and the scripts read it
    const b = A.Bolygok.add(0x188 * p);
    const row = [b.add(0x20).readU8(), b.add(0x28).readS32(), b.add(0xd8 + 2 * en).readS16()];
    if (cfg.campaign) {
      row.push(f32bits(b.add(0x18)), f32bits(b.add(0x1c)), b.add(0x6e).readU8(), b.add(0xb8).readS16(), b.add(0x21).readU8());
      // st_Line: +0xbc (st_bSetBarat), the info slots 1..5 (+0x16a, BolygoInfoIs), the views +0xd8 (st_bUnhide).
      const info = []; for (let k = 0; k < 5; k++) info.push(b.add(0x16a + 2 * k).readS16());
      row.push(b.add(0xbc).readU8(), info, hex(b.add(0xd8), 38));
      // st_BRnd: production +0x70/+0x74, morale +0x174.
      if (brnd) row.push(b.add(0x70).readS32(), b.add(0x74).readS32(), b.add(0x174).readS32());
      // st_Begin and st_Rnd: the building slots (HanyEpulet), the radar notice +0xbb.
      if (begin || rndk) row.push(A.HanyEpulet.add(4 * p).readS32(), b.add(0xbb).readU8());
      // st_Rnd: morale +0x30, surface +0x22, richness +0x24.
      if (rndk) row.push(b.add(0x30).readS32(), b.add(0x22).readU8(), b.add(0x24).readS32());
      // st_Elem: +0xbb (st_bSetRadarMsg), +0x186/+0x187 (st_bSetExtra).
      if (elem) row.push(b.add(0xbb).readU8(), b.add(0x186).readU8(), b.add(0x187).readU8());
    }
    planets.push(row);
  }
  const nfx = cfg.keep.includes('fable') ? Math.max(nfl, Math.min(A.HajoCount.readS32(), 2000)) : nfl;
  for (let f = 0; f <= nfx; f++) {
    const q = fleet(f);
    try { q.add(0x7f).readU8(); } catch (e) { break; }
    const row = [q.add(0x15).readU8(), q.add(0x16).readS16()];
    if (cfg.campaign) row.push(f32bits(q.add(0x30)), f32bits(q.add(0x34)), q.add(0x42).readS16(), q.add(0x5c).readS32(), f32bits(q.add(0x3c)));
    // st_BRnd: +0x40 ships, +0x44 the order's target, +0x18, +0x52.
    if (cfg.campaign && (brnd || fable)) row.push(q.add(0x40).readS16(), f32bits(q.add(0x44)), q.add(0x18).readU8(), q.add(0x52).readU16());
    // st_Begin: +0x60 (its strength); st_Rnd: also +0x54 (its ship count).
    if (cfg.campaign && (begin || rndk)) row.push(q.add(0x60).readS32());
    if (cfg.campaign && rndk) row.push(q.add(0x54).readS32(), q.add(0x78).readS32());
    fleets.push(row);
  }
  const hanyb = [], visz = [];
  for (let e = 0; e < 19; e++) { hanyb.push(A.HanyB.add(4 * e).readS32()); visz.push(A.Viszony.add((en * 19 + e) * 0x26 + 0x25).readU8()); }
  const tech = varOf(0xe94);
  const out = {
    ido: A.Ido.readS32(), speed: A.Speed.readS32(), en, neh: A.Nehezseg.readS32(), mp: A.MP.readU8(),
    camp: A.Campaign.readS32(), el: hex(A.El, 19), hanyb, money: A.Money.add(8 * en).readDouble(), visz,
    tech: tech >= 0 && tech < 200 ? [tech, hex(A.Techs.add(0x114 * tech + 0x45), 19)] : [tech, ''],
    seed5: A.RandSeed.add(20).readU32(), planets, fleets,
  };
  if (cfg.keep.includes('fable')) out.nfl = nfl; // the fleets past it are stale records st_Fable 311 reads
  if (cfg.campaign) {
    out.ismeri = hex(A.Ismeri.add(19 * Math.max(0, Math.min(en, 18))), 19);
    out.faj = hex(A.PlayerFaj, 19);
    out.akt = [A.AktPlanet.readS32(), A.AktFleet.readS32()];
    const rr = [], rr0 = []; for (let k = 0; k < 8; k++) { rr.push(f32bits(A.RR2.add(0x20 * en + 4 * k))); rr0.push(f32bits(A.RR2.add(4 * k))); }
    out.rr2 = rr;
    out.rr0 = rr0; // owner 0's row: st_fNewEx reads it when the human has no planet left (planet slot 0)
    const mi = []; for (let e = 0; e < 9; e++) mi.push(A.MiTervMutato.add(50 * e + 26).readS16());
    out.mi13 = mi;
    const ero = []; for (let e = 0; e < 19; e++) ero.push(f32bits(A.Ero.add(4 * e)));
    out.ero = ero;
    if (multi) {
      // st_Multi_include: stream 2 (1003's planet search), CTF_value, fleets' +0x52, planets' names.
      out.seed2 = A.RandSeed.add(8).readU32();
      out.ctf = [0, 1, 2].map((i) => A.Ctf.add(4 * i).readS32());
      out.f52 = []; for (let f = 0; f <= nfl; f++) out.f52.push(fleet(f).add(0x52).readU16());
      out.names = []; for (let p = 0; p <= np; p++) { const b = new Uint8Array(A.Bolygok.add(0x188 * p).readByteArray(8)); let t = ''; for (const x of b) { if (!x) break; t += String.fromCharCode(x); } out.names.push(t); }
    }
    if (cfg.keep.includes('fable')) Object.assign(out, fableWorld(en));
    // Ships and the human's designs only while story 404 (the refit trader) or the multiplayer modes run.
    if (bucketHas(404) || multi || brnd || cfg.keep.includes('fable')) {
      const ships = [], hn = A.HajoCount.readS32(), hp = A.Hajok.readPointer();
      for (let k = 0; k <= hn; k++) ships.push([hp.add(6 * k).readS16(), hp.add(6 * k + 2).readS16()]);
      const designs = [], tn = A.TervCount.add(4 * en).readS32(), tp = A.Tervek.add(4 * en).readPointer();
      // st_BRnd also: +0x20 cost, +0x2c attack, +0x80 story key.
      for (let d = 0; d <= tn; d++) designs.push(brnd ? [tp.add(0x84 * d + 0x1c).readS32(), tp.add(0x84 * d + 0x24).readS32(), tp.add(0x84 * d + 0x20).readS32(), tp.add(0x84 * d + 0x2c).readS32(), tp.add(0x84 * d + 0x80).readS32()] : [tp.add(0x84 * d + 0x1c).readS32(), tp.add(0x84 * d + 0x24).readS32()]);
      // st_Begin also: +0x16 (2 = ground), +0x44 (size), +0x34 (colony ship), +0x6c (3 = for sale).
      if (begin) for (let d = 0; d <= tn; d++) designs[d].push(tp.add(0x84 * d + 0x16).readS16(), tp.add(0x84 * d + 0x44).readS32(), tp.add(0x84 * d + 0x34).readU8(), tp.add(0x84 * d + 0x6c).readS32());
      // st_Elem: +0x34 (story 842's design test); part 2d +0x48 (807's hull classes), +0x36 (its vehicles), +0x28
      // (801's design A).
      if (elem) for (let d = 0; d <= tn; d++) designs[d].push(tp.add(0x84 * d + 0x34).readU8(), tp.add(0x84 * d + 0x48).readS32(), tp.add(0x84 * d + 0x36).readU8(), tp.add(0x84 * d + 0x28).readU8(), tp.add(0x84 * d + 0x44).readS32(), tp.add(0x84 * d + 0x16).readS16());
      out.ships = ships;
      out.designs = designs;
    }
    if (brnd) {
      // The human's tech statuses and every tech's prerequisites (st_GetRandomTalalmany), spFlottak, the
      // wormholes' shown ends, the global morale.
      const t = []; for (let k = 0; k < 193; k++) { const r = A.Techs.add(0x114 * k); t.push(r.add(0x45 + en).readU8(), r.add(0x2d).readU8(), r.add(0x2e).readU8(), r.add(0x2f).readU8()); }
      out.techs = t.map((x) => (x < 16 ? '0' : '') + x.toString(16)).join('');
      out.spf = [A.SpFlottak.readS16(), A.SpFlottak.add(6).readS16()];
      const wh = []; for (let k = 1; k <= A.WormholeN.readS32(); k++) { const q = A.Wormholes.add(0x1c * k); wh.push([q.add(8).readU8(), q.add(0x14).readU8(), q.add(0xa).readS16(), q.add(0x16).readS16()]); }
      out.wh = wh;
      out.gmoral = [A.GMoral.readS32(), A.GMoralSpeed.readS32()];
      if (rndk) { const c = []; for (let k = 0; k < 193; k++) c.push(A.Techs.add(0x114 * k + 0x34).readS32()); out.tcost = c; out.whn = A.WormholeN.readS32(); }
    }
    if (begin) {
      // Every tech's priority +0xb8, +0x34, +0x38 and the human's time +0x58 + 4·EN; the research; Tankok; the
      // trade offers.
      const tr = []; for (let k = 0; k < 193; k++) { const r = A.Techs.add(0x114 * k); tr.push([r.add(0xb8).readU8(), r.add(0x34).readS32(), r.add(0x38).readS32(), r.add(0x58 + 4 * en).readS32()]); }
      out.tres = tr;
      out.res = A.Research.add(4 * en).readS32();
      const tanks = [], tkn = A.TankN.readS32(); for (let k = 0; k <= tkn; k++) { const t = A.Tanks.add(8 * k); tanks.push([t.readS16(), t.add(2).readS16(), t.add(4).readS32()]); }
      out.tanks = tanks;
      const trade = [], trn = A.TradeN.readS32(); for (let k = 0; k < trn; k++) { const t = A.TradeT.add(0x84 * k); trade.push([t.add(0x16).readS16(), t.add(0x80).readS32()]); }
      out.trade = trade;
    }
    if (begin || rndk || elem) {
      // The human's trade agreements and the relations (value 2) seen from the others; the spies (st_Rnd: also
      // +0x3c experience, +0x40 rank); the buildings of the human's planets (st_Begin: and of planets 101 and 191;
      // st_Rnd: also +0x18, +0x19).
      const ta = []; for (let e = 0; e < 19; e++) ta.push(A.Viszony.add((en * 19 + e) * 0x26 + 0x24).readU8());
      out.tra = ta;
      const rel = []; for (let e = 0; e < 19; e++) rel.push(A.Viszony.add((e * 19 + en) * 0x26 + 4).readS16());
      out.rel = rel;
      const spies = [];
      for (let e = 0; e < 9; e++) {
        const k = [];
        for (let i = 0; i < 13; i++) {
          const r = kemRec(e, i), row = [r.add(0x10).readS16(), r.add(0x12).readU8(), r.add(0x18).readU8(), r.add(0x1a).readS16(), r.add(0x32).readU8(), r.add(0x33).readU8()];
          if (rndk || elem) row.push(r.add(0x3c).readS32(), r.add(0x40).readU8(), r.add(0x31).readU8(), r.add(0x16).readS16(), latin1(r, 15));
          k.push(row);
        }
        spies.push([A.KemN.add(4 * e).readS32(), k]);
      }
      out.spies = spies;
      const bld = {}, wanted = begin ? new Set([planetOfId(101), planetOfId(191)]) : new Set();
      for (let p = 1; p <= np; p++) {
        // Story 10 searches every planet when its planet id is missing (st_eSearch3(0, 1)).
        if (A.Bolygok.add(0x188 * p + 0x20).readU8() !== en && !wanted.has(p) && !(begin && bucketHas(10) && !planetOfId(101))) continue;
        const n = A.HanyEpulet.add(4 * p).readS32(), base = A.Epuletek.add(4 * p).readPointer(), list = [];
        for (let k = 0; k <= n; k++) { const r = base.add(0x20 * k), row = [r.readU8(), r.add(4).readS32(), r.add(8).readS16()]; if (rndk) row.push(r.add(0x18).readU8(), r.add(0x19).readU8()); list.push(row); }
        bld[p] = list;
      }
      if (!elem) out.bld = bld;
    }
    if (elem && !rndk) {
      // st_Elem part 2e: the fleet slots past the count up to the ship count (821's escort loop reads them; the dump
      // stops at the allocation's end), the record before the planets (834's fleets start on planet -1), planet
      // 8140's battlefield WF+0x334 (814's self-destruct countdown), spFlottak.
      const nsh = A.HajoCount.readS32(), fext = [];
      try { for (let f = nfl + 1; f <= nsh; f++) { const q = fleet(f); fext.push([q.add(0x15).readU8(), f32bits(q.add(0x30)), f32bits(q.add(0x34))]); } } catch (e) {}
      out.fext = fext;
      const m1 = A.Bolygok.sub(0x188);
      out.bm1 = [f32bits(m1.add(0x18)), f32bits(m1.add(0x1c))];
      const p8140 = planetOfId(8140);
      out.wf334 = p8140 ? [p8140, wf(p8140).add(0x334).readS32()] : [0, 0];
      // spFlottak and its count (821's escort joins the battle).
      const spw = []; for (let k = 0; k <= 4; k++) spw.push([A.SpFl0.add(6 * k).readS16(), A.SpFl0.add(6 * k + 2).readU8()]);
      out.spw = [A.SpFlottakN.readS32(), spw];
      // Part 2f: the levels of empires 0..8 towards each other (836's alliances, 9 x 9 bytes); while 840 runs (or ran
      // in this record) the human's designs 40.. ([d, +0x34, +0x20, +0x24]) and planets building a ship ([p, +0x78,
      // that design's +0x34, +0xa0]): the colony discount.
      let vl = ''; for (let a = 0; a < 9; a++) for (let b = 0; b < 9; b++) vl += ('0' + A.Viszony.add((a * 19 + b) * 0x26 + 0x25).readU8().toString(16)).slice(-2);
      out.vlev = vl;
      if (bucketHas(840) || (cur && cur.pre && cur.pre.w && cur.pre.w.cdis !== undefined)) {
        const tn = A.TervCount.add(4 * en).readS32(), tp = A.Tervek.add(4 * en).readPointer(), ds = [], ps = [];
        if (!tp.isNull()) {
          for (let d = 40; d <= tn && d < 400; d++) { const r = tp.add(0x84 * d); ds.push([d, r.add(0x34).readU8(), r.add(0x20).readS32(), r.add(0x24).readS32()]); }
          for (let p = 1; p <= np; p++) {
            const b = A.Bolygok.add(0x188 * p), d = b.add(0x78).readS16();
            if (b.add(0x20).readU8() === en && d > 0) ps.push([p, d, d < 400 ? tp.add(0x84 * d + 0x34).readU8() : 0, b.add(0xa0).readS32()]);
          }
        }
        out.cdis = [ds, ps];
      }
    }
    if (rndk) {
      // st_Rnd part 2b: the fleet slots past the count up to the ship count (owner, x, y; 101's escort loop reads
      // them), spFlottak and its count, Tankok, the research, every empire's designs' +0x2c and +0x16 (151).
      const nsh = A.HajoCount.readS32(), fext = [];
      // Slots past the allocation fault: the dump stops there (the replay reads owner 0 beyond it).
      try { for (let f = nfl + 1; f <= nsh; f++) { const q = fleet(f); fext.push([q.add(0x15).readU8(), f32bits(q.add(0x30)), f32bits(q.add(0x34))]); } } catch (e) {}
      out.fext = fext;
      const spw = []; for (let k = 0; k <= 4; k++) spw.push([A.SpFl0.add(6 * k).readS16(), A.SpFl0.add(6 * k + 2).readU8()]);
      out.spw = [A.SpFlottakN.readS32(), spw];
      // Every empire's radar row (st_fNewEx places another empire's fleets outside the human's radar of theirs).
      const rrall = []; for (let e = 0; e < 19; e++) { const row = []; for (let k = 0; k < 8; k++) row.push(f32bits(A.RR2.add(0x20 * e + 4 * k))); rrall.push(row); }
      out.rrall = rrall;
      if (!begin) {
        const tanks = [], tkn = A.TankN.readS32(); for (let k = 0; k <= tkn; k++) { const t = A.Tanks.add(8 * k); tanks.push([t.readS16(), t.add(2).readS16(), t.add(4).readS32()]); }
        out.tanks = tanks;
        out.res = A.Research.add(4 * en).readS32();
      }
      // Part 2c's own traces (--inline with a hero) always: a story's start reads them before it runs.
      const heroes = RND2C_HEROES.some(bucketHas) || (cfg.inline || []).some((x) => RND2C_HEROES.includes(x));
      if (bucketHas(151) || heroes || (cur && cur.pre && cur.pre.w && cur.pre.w.des2 !== undefined)) {
        const des2 = [];
        for (let e = 0; e < 19; e++) {
          const tn = A.TervCount.add(4 * e).readS32(), tp = A.Tervek.add(4 * e).readPointer(), d = [];
          if (!tp.isNull() && tn > 0 && tn < 200) for (let k = 0; k <= tn; k++) d.push([tp.add(0x84 * k + 0x2c).readS32(), tp.add(0x84 * k + 0x16).readS16(), tp.add(0x84 * k + 0x80).readS32()]);
          des2.push(d);
        }
        out.des2 = des2;
      }
      // st_Rnd part 2c: the story missions' switches (KemKuldetesek[14..25].+0); while a hero runs the fleets' names
      // and the human's designs' +0x18, +0x19, name and their tech's name (180); 138's planet's name.
      const kmon = []; for (let k = 0; k < 12; k++) kmon.push(A.KemKuld.add(0x20 * (14 + k)).readU8());
      out.kmon = kmon;
      // Stream 4 (138's experience draw), KemRangXPHatar, the trade prices (192), what 174's slip writes (planet slot
      // 1741's +0x174 and +0x178: exe 0xC2CA04, 0xC2CA08).
      out.seed4 = A.RandSeed.add(16).readU32();
      out.rankxp2 = [0, 1, 2, 3, 4, 5].map((r) => A.RankXp.add(4 * r).readS32());
      if (!begin) {
        const tn2 = A.TradeN.readS32(); out.trade = []; for (let k = 0; k < tn2 && k < 64; k++) out.trade.push(A.TradeT.add(0x84 * k + 0x24).readS32());
      }
      out.stray = [va(0xc2ca04).readS32(), va(0xc2ca08).readS32()];
      if (heroes || (cur && cur.pre && cur.pre.w && cur.pre.w.fnames !== undefined)) {
        const fn = []; for (let f = 0; f <= nfl; f++) fn.push(latin1(fleet(f), 0x15));
        out.fnames = fn;
        const tn = A.TervCount.add(4 * en).readS32(), tp = A.Tervek.add(4 * en).readPointer(), dn = [];
        for (let d = 0; d <= tn && d < 200; d++) {
          const r = tp.add(0x84 * d), t = r.add(0x48).readS32();
          dn.push([r.add(0x18).readU8(), r.add(0x19).readU8(), latin1(r, 0x15), t >= 0 && t < 193 ? latin1(A.Techs.add(0x114 * t), 0x14) : '']);
        }
        out.dnames = dn;
      }
      const p1381 = planetOfId(0x565);
      if (p1381) out.pname = [p1381, latin1(A.Bolygok.add(0x188 * p1381), 0x18)];
    }
    if (rndk) {
      // st_Rnd part 2: the others' levels towards the human, the earthquake's planet; while 127 runs every empire's
      // tech statuses (+0x45, 19 bytes) and the human's seen matrix row (+0xc0 + 9·EN, 9 bytes).
      const v2 = []; for (let e = 0; e < 19; e++) v2.push(A.Viszony.add((e * 19 + en) * 0x26 + 0x25).readU8());
      out.visz2 = v2;
      out.quake = A.Quake.readS32();
      if (bucketHas(127) || (cur && cur.pre && cur.pre.w && cur.pre.w.tstat !== undefined)) {
        let ts = '', tseen = '';
        for (let k = 0; k < 193; k++) { const r = A.Techs.add(0x114 * k); ts += hex(r.add(0x45), 19); tseen += hex(r.add(0xc0 + 9 * en), 9); }
        out.tstat = ts; out.tseen = tseen;
      }
    }
    if (begin) {
      // Story 7's planet when it's no longer the human's: its owner's designs (+0x16, +0x44) for the units on it.
      const p73 = planetOfId(73), o73 = p73 ? A.Bolygok.add(0x188 * p73 + 0x20).readU8() : 0;
      if (o73 && o73 !== en && o73 < 19) {
        const tn = A.TervCount.add(4 * o73).readS32(), tp = A.Tervek.add(4 * o73).readPointer(), d = [];
        for (let k = 0; k <= tn; k++) d.push([tp.add(0x84 * k + 0x16).readS16(), tp.add(0x84 * k + 0x44).readS32()]);
        out.odes = [o73, d];
      }
    }
    if (elem) {
      // st_Elem_include: techs 6 and 161..171 (+0x34 price, +0x38 time, the human's time left +0x58+4EN), the
      // trade goods' prices, the building slots (kind, +8) of the human's and empire 11's planets, Viszony[EN][5] / [5][EN] levels and
      // [5][EN].+4.
      out.tt = [6, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171].map((t) => { const r = A.Techs.add(0x114 * t); return [t, r.add(0x34).readS32(), r.add(0x38).readS32(), r.add(0x58 + 4 * en).readS32()]; });
      const tn2 = A.TradeN.readS32(); out.trade = []; for (let k = 0; k < tn2 && k < 64; k++) out.trade.push(A.TradeT.add(0x84 * k + 0x24).readS32());
      out.bld = [];
      for (let p = 1; p <= np; p++) {
        const o = A.Bolygok.add(0x188 * p + 0x20).readU8();
        // the human's (835), empire 11's (817 razes them), 8080's and 8540's (part 2b), empire 10's (806 hands its
        // planet over on strike, part 2d)
        // and 8140's (814's doomsday building, part 2e)
        if (o !== en && o !== 11 && o !== 10 && p !== planetOfId(8080) && p !== planetOfId(8540) && p !== planetOfId(8140)) continue;
        const n = A.HanyEpulet.add(4 * p).readS32(), e = A.Epuletek.add(4 * p).readPointer(), sl = [];
        // Part 2a: also the damage +4 (802's earthquake); part 2b the flags +0x19 (854's working police).
        for (let k = 1; k <= n && k <= 2000; k++) sl.push([e.add(0x20 * k).readU8(), e.add(0x20 * k + 8).readS16(), e.add(0x20 * k + 4).readS32(), e.add(0x20 * k + 0x19).readU8()]);
        out.bld.push([p, sl]);
      }
      // Part 2a: every planet's buildings (HanyEpulet 0xBA9518: 802's pick, 824's average), the story missions' switches, the others' levels
      // towards the human, the earthquake's planet, the battlefields' self-destruct timers (WF+0x334) and the
      // ground battle's planet's units (alive WF+0x188, owner +0x194; 850's madness).
      out.hanyo = []; for (let p = 0; p <= np; p++) out.hanyo.push(va(0xba9518).add(4 * p).readS32());
      const kmon = []; for (let k = 0; k < 12; k++) kmon.push(A.KemKuld.add(0x20 * (14 + k)).readU8());
      out.kmon = kmon;
      const v2 = []; for (let e = 0; e < 19; e++) v2.push(A.Viszony.add((e * 19 + en) * 0x26 + 0x25).readU8());
      out.visz2 = v2;
      out.quake = A.Quake.readS32();
      out.wfsd = []; for (let p = 0; p <= np; p++) out.wfsd.push(wf(p).add(0x334).readS32());
      const gp = A.Gr.add(12).readS32();
      if (gp > 0 && gp <= np) out.wfu = [gp, wfUnits(gp)];
      const v = (a, b) => A.Viszony.add((a * 19 + b) * 0x26);
      out.vrel = [v(en, 5).add(0x25).readU8(), v(5, en).add(0x25).readU8(), v(5, en).add(4).readS16()];
      // Part 2b: every planet's garrison (+0x58, 854's riots), the units of the battlefields 808/854 read (the ground
      // battle's planet, 8080's, 8540's, v(8549)'s and the human's planets) and their status caches
      // WF+0x8cc, the screen request, the held planet and the pause (AktKepKell, AktPlanetKell, 0xF261F8, IdoPause).
      out.gar = []; for (let p = 0; p <= np; p++) out.gar.push(A.Bolygok.add(0x188 * p + 0x58).readS32());
      const wp = new Set([gp, planetOfId(8080), planetOfId(8540), varOf(8549)]);
      for (let p = 1; p <= np; p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === en) wp.add(p);
      out.wfus = []; for (const p of wp) if (p > 0 && p <= np) out.wfus.push([p, wfRecs(p), wf(p).add(0x8cc).readS32()]);
      out.kep = [A.AktKepKell.readS32(), va(0xbae254).readS32(), va(0xf261f8).readS32(), va(0xbae264).readU8()];
      // Tankok (808's start deletes the garrison) unless st_Rnd/st_Begin dumped it.
      if (!out.tanks) { const tanks = [], tkn = A.TankN.readS32(); for (let k = 0; k <= tkn; k++) { const t = A.Tanks.add(8 * k); tanks.push([t.readS16(), t.add(2).readS16(), t.add(4).readS32()]); } out.tanks = tanks; }
      // Part 2d: stream 0 (806's first pick), the date (807's game info), the fleets' names (801 renames them).
      out.seed0 = A.RandSeed.readU32();
      out.date = [va(0xbae040).readS32(), va(0xbae044).readS32(), va(0xbae048).readS32()];
      // Part 2g: the fleets' vehicles (+0x58, 856's toll).
      out.fveh = []; for (let f = 0; f <= nfl; f++) out.fveh.push(fleet(f).add(0x58).readS32());
      if (!out.fnames) { const fn = []; for (let f = 0; f <= nfl; f++) fn.push(latin1(fleet(f), 0x15)); out.fnames = fn; }
    }
  }
  return out;
}
// The battlefield of planet p (WF, exe 0x9C3244 + p·0x8EC) and its units `[alive, owner]` (count +0x180, alive bytes
// +0x188, units +0x330, owner +0x194).
function wf(p) { return va(0x9c3244).add(0x8ec * p); }
function wfUnits(p) {
  const w = wf(p), n = w.add(0x180).readS32(), out = [];
  if (n <= 0 || n > 2000) return out;
  // A battlefield without a war may hold stale pointers: an unreadable one ends the list (the replay sees it short).
  try {
    const alive = w.add(0x188).readPointer(), units = w.add(0x330).readPointer();
    for (let k = 0; k < n; k++) out.push([alive.add(k).readU8(), units.add(4 * k).readPointer().add(0x194).readU32()]);
  } catch (e) { out.push(null); }
  return out;
}
// Part 2b: the units of planet p's battlefield as 854 reads and writes them: [alive, owner +0x194, flags +0x188, +0x18c,
// vehicles +0x190, design +0x15c, +0x198, +0x285, origin fleets +0x19c[0..vehicles] (at most 16)].
function wfRecs(p) {
  const w = wf(p), n = w.add(0x180).readS32(), out = [];
  if (n <= 0 || n > 2000) return out;
  try {
    const alive = w.add(0x188).readPointer(), units = w.add(0x330).readPointer();
    for (let k = 0; k < n; k++) {
      const u = units.add(4 * k).readPointer(), c = u.add(0x190).readS32(), o = [];
      if (c > 0 && c <= 16) { const q = u.add(0x19c).readPointer(); for (let i = 0; i < c; i++) o.push(q.add(4 * i).readS32()); }
      out.push([alive.add(k).readU8(), u.add(0x194).readU32(), u.add(0x188).readU32(), u.add(0x18c).readU8(), c, u.add(0x15c).readS32(), u.add(0x198).readS32(), u.add(0x285).readU8(), o]);
    }
  } catch (e) { out.push(null); }
  return out;
}
// The battlefields and indices holding unit `u` ([[p, k]…]): the planets the episodes act on first (the ground
// battle's, 8080's, 8540's, v(8549)'s, the human's); only when none holds it every other planet's (a stale
// battlefield of a planet without a war may hold the pointer too). Scanning every planet for every call stalled the
// game for minutes.
function unitAt(u) {
  const np = nPlanets(), en = A.EN.readS32(), first = [A.Gr.add(12).readS32(), planetOfId(8080), planetOfId(8540), varOf(8549)];
  for (let p = 1; p <= np; p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === en) first.push(p);
  const scan = (ps) => {
    const all = [], seen = new Set();
    for (const p of ps) {
      if (p < 1 || p > np || seen.has(p)) continue;
      seen.add(p);
      const n = wf(p).add(0x180).readS32();
      if (n <= 0 || n > 2000) continue;
      try { const units = wf(p).add(0x330).readPointer(); for (let i = 0; i < n; i++) if (units.add(4 * i).readPointer().equals(u)) all.push([p, i]); } catch (e) {}
    }
    return all;
  };
  const all = scan(first);
  if (all.length) return all;
  const rest = []; for (let p = 1; p <= np; p++) if (!first.includes(p)) rest.push(p);
  return scan(rest);
}
// A unit call's arguments: [first planet, first index, …extra, every [p, k] holding it].
function unitArgs(u, extra) { const all = unitAt(u), f = all.length ? all[0] : [A.Gr.add(12).readS32(), -1]; return f.concat(extra, [all]); }
// A NUL-terminated name of at most n bytes, one char per byte.
function latin1(q, n) { const b = new Uint8Array(q.readByteArray(n)); let t = ''; for (const x of b) { if (!x) break; t += String.fromCharCode(x); } return t; }
function fableWorld(en) {
  const spf = [A.SpFlottakN.readS32()];
  for (let i = 1; i <= 4; i++) spf.push([A.SpFl0.add(6 * i).readS16(), A.SpFl0.add(6 * i + 2).readU8()]);
  const worm = [], wn = A.WormN.readS32();
  for (let k = 0; k <= Math.min(wn, 64); k++) { const r = A.Worm.add(0x1c * k); worm.push([r.add(8).readU8(), r.add(0xa).readS16(), r.add(0x14).readU8(), r.add(0x16).readS16()]); }
  const kn = A.KemN.add(4 * en).readS32(), kem = [];
  for (let k = 0; k <= Math.min(Math.max(kn, 0), 12); k++) { const r = A.Kemek.add(0x3dc * en + 0x4c * k); kem.push([r.add(0x10).readS16(), r.add(0x16).readS16(), latin1(r, 15)]); }
  const labs = []; for (let k = 0; k < 4; k++) labs.push(A.Labor.add(16 * en + 4 * k).readS32());
  return { spfl: spf, wormn: wn, worm, kemn: kn, kem, kemrow: bucketHas(310) ? hex(A.Kemek.add(0x3dc * en), 0x3dc) : '',
    rankxp: [1, 2].map((r) => A.RankXp.add(4 * r).readS32()), labs, research: A.ResAkt.add(4 * en).readS32() };
}
function storyData() {
  const values = [], n = A.ValuesN.readS32();
  for (let i = 0; i < n; i++) values.push([A.Values.add(8 * i).readS32(), A.Values.add(8 * i + 4).readS32()]);
  const fleets = [], m = A.FleetDataN.readS32();
  for (let i = 0; i < m; i++) {
    const r = A.FleetData.add(0x54 * i), counts = [];
    for (let k = 0; k < 36; k++) counts.push(r.add(0xc + 2 * k).readS16());
    fleets.push([r.readS32(), r.add(4).readS32(), r.add(8).readS32(), counts]);
  }
  const buildings = [], nb = A.BuildingDataN.readS32();
  for (let i = 0; i < nb; i++) { const r = A.BuildingData.add(0x10 * i); buildings.push([r.readS32(), r.add(4).readS32(), r.add(8).readS32(), r.add(12).readS32()]); }
  return { values, fleets, buildings };
}
const snap = () => ({ s: story(), w: world(), ends: ends() });

// ---------------------------------------------------------------- the stories' calls to other systems
// Each logged call: [name, args, depth (0 = called by story code), return value]. Floats travel as their bits.
let cur = null, actDepth = 0, staging = false;
function act(name, args) {
  if (!cur) return null;
  const e = [name, args, actDepth - cur.d0, null];
  cur.acts.push(e);
  // --debug: a staged ground battle's end streams its calls (the campaign sometimes reset itself inside one).
  if (cfg.debug && cur.call === 'flag' && cur.args && cur.args[0] === 4) send({ type: 'batch', events: [{ k: 'dbgact', seq: cur.seq, e: [name, args, e[2]] }] });
  return e;
}
const argI = (ctx, i) => ctx.esp.add(4 + 4 * i).readS32();
const argU = (ctx, i) => ctx.esp.add(4 + 4 * i).readU32();
function logged(addr, name, read, ret) {
  Interceptor.attach(addr, {
    onEnter() { this.e = act(name, read(this.context)); actDepth++; },
    onLeave(r) { actDepth--; if (this.e && ret) this.e[3] = ret(r); },
  });
}
function setupActs() {
  const ints = (n) => (c) => { const o = []; for (let i = 0; i < n; i++) o.push(argI(c, i)); return o; };
  logged(va(0x53d270), 'move', (c) => [argI(c, 0), argI(c, 1), (argI(c, 2) << 16) >> 16, argU(c, 3), argU(c, 4)]);
  logged(va(0x53d040), 'delete', ints(1));
  logged(va(0x53e800), 'ship', ints(2));
  logged(va(0x541750), 'tank', ints(3));
  logged(va(0x53d400), 'fleetnew', (c) => [argI(c, 1), argI(c, 2), argU(c, 3), argU(c, 4), argU(c, 5)], (r) => r.toInt32());
  logged(va(0x5eb300), 'inrr', ints(3), (r) => r.toInt32() & 0xff);
  logged(va(0x5eacf0), 'getterv', ints(3), (r) => r.toInt32());
  logged(va(0x53aec0), 'wormhole', ints(4));
  logged(va(0x5d47b0), 'destroy', ints(1));
  logged(va(0x59af50), 'colonise', ints(7));
  logged(va(0x53b720), 'conquer', ints(2));
  logged(va(0x53fb10), 'money', (c) => [argI(c, 0), argI(c, 1), c.esp.add(12).readDouble()]);
  logged(va(0x541920), 'autodesign', ints(3));
  logged(va(0x5423b0), 'tervszamol', ints(4));
  logged(va(0x5c95b0), 'volcano', ints(1));
  logged(va(0x4bf4b0), 'diplspecial', ints(2));
  logged(va(0x6057f0), 'tutorial', ints(0));
  logged(va(0x53dd90), 'recalc', ints(0));
  logged(va(0x508fe0), 'message', ints(2));
  logged(va(0x5094c0), 'msgrace', ints(1));
  logged(va(0x53cca0), 'building', ints(6));
  logged(va(0x59b860), 'starnames', ints(0));
  // st_Line_include's calls: message parameters (an empire's name or a planet's), message removal, disabled
  // answers, planet info removal, empires met, planets shown.
  logged(va(0x509570), 'param', (c) => {
    const p = c.esp.add(8).readPointer(), bo = p.sub(A.Bolygok).toInt32();
    if (bo >= 0 && bo % 0x188 === 0 && bo / 0x188 <= nPlanets()) return [argI(c, 0), 'planet', bo / 0x188];
    for (let e = 0; e < 19; e++) if (A.Nevek.add(4 * e).readPointer().equals(p)) return [argI(c, 0), 'empire', e];
    const to = p.sub(A.Techs).toInt32();
    if (to >= 0 && to % 0x114 === 0 && to / 0x114 < 193) return [argI(c, 0), 'tech', to / 0x114];
    const fo = p.sub(A.Flottak.readPointer()).toInt32();
    if (fo >= 0 && fo % 0x80 === 0 && fo / 0x80 <= nFleets()) return [argI(c, 0), 'fleet', fo / 0x80];
    const lp = A.Spw.readPointer(), so = lp.isNull() ? -1 : p.sub(lp).toInt32();
    if (so >= 0 && so % 0x30 === 0 && so / 0x30 < 12) return [argI(c, 0), 'participant', so / 0x30];
    const ko = p.sub(A.Kemek).toInt32();
    if (ko >= 0 && ko % 0x4c === 0 && ko / 0x4c < 9 * 13) return [argI(c, 0), 'spy', ko / 0x4c];
    const po = p.sub(A.PlayerNev).toInt32();
    if (po >= 0 && po % 11 === 0 && po / 11 < 19) return [argI(c, 0), 'player', po / 11];
    return [argI(c, 0), 'other', p.readAnsiString()];
  });
  logged(va(0x506200), 'rmmessage', ints(1));
  logged(va(0x5063d0), 'disablepart', ints(1));
  logged(va(0x53c0b0), 'inforemove', ints(3));
  logged(va(0x4c1d60), 'discover', ints(2));
  logged(va(0x604040), 'unhide', ints(1));
  const rndk = cfg.keep.includes('rnd'), elemKept = cfg.keep.includes('elem');
  if (cfg.keep.includes('multi') || cfg.keep.includes('brnd') || cfg.keep.includes('begin') || rndk || elemKept || cfg.keep.includes('fable')) setupMultiActs(ints);
  if (cfg.keep.includes('begin')) {
    // st_Begin_include's calls: units deleted, spy reports, the epidemic's info.
    logged(va(0x5414c0), 'tankdelete', ints(3));
    // KemJelentesAdd runs a nested Story_Check(1) (Story_KemEsemeny) whose calls are the stories' own: logged flat.
    // Its ticker line (ScrollUzenetId) is not a story's call.
    Interceptor.attach(va(0x58d000), { onEnter() { const c = this.context, o = []; for (let i = 0; i < 9; i++) o.push(argI(c, i)); act('spyreport', o); } });
    Interceptor.attach(va(0x556dc0), { onEnter() { actDepth++; }, onLeave() { actDepth--; } });
    logged(va(0x542930), 'virus', ints(1));
  }
  // st_Rnd_include: Virus_CheckAdd (st_Begin's block logs it too); part 2: st_SetSemlegesViszony, the story spy
  // missions offered and withdrawn.
  if (rndk && !cfg.keep.includes('begin')) logged(va(0x542930), 'virus', ints(1));
  if (elemKept) {
    // st_Elem_include's calls: buildings deleted (story 817), the peace a new alliance makes (864).
    logged(va(0x53c730), 'bdelete', ints(2));
    logged(va(0x53b280), 'peace', ints(2));
    // Part 2a: the story spy missions, SetSemlegesViszony (861), WF_Tank::SetMadness (850; thiscall: the unit's
    // index on the ground battle's planet).
    if (!rndk) {
      logged(va(0x6028a0), 'neutral', ints(2));
      logged(va(0x5e6ac0), 'kemenable', ints(1));
      logged(va(0x5d4ab0), 'kemdisable', ints(1));
    }
    // Part 2b: the unit found on any battlefield (854's riot planet isn't the ground battle's).
    logged(va(0x49c140), 'madness', (c) => (cur ? unitArgs(c.ecx, [argI(c, 0) & 0xff]) : null));
    // Part 2b: WF_Tank::NoTarget and SetSide (thiscall), the core TankDelete (854's riot), TervCopy2 (its source as
    // [empire, design]), SetIdoPause; TankDelete with its battlefield units (808's start).
    logged(va(0x48c0b0), 'notarget', (c) => (cur ? unitArgs(c.ecx, []) : null));
    logged(va(0x49c4e0), 'setside', (c) => (cur ? unitArgs(c.ecx, [argI(c, 0)]) : null));
    logged(va(0x541510), 'tankdel0', ints(3));
    logged(va(0x541ca0), 'tervcopy2', (c) => {
      const src = c.esp.add(8).readPointer();
      for (let e = 0; e < 19; e++) {
        const tp = A.Tervek.add(4 * e).readPointer(), o = src.sub(tp).toInt32();
        if (!tp.isNull() && o >= 0 && o % 0x84 === 0 && o / 0x84 <= A.TervCount.add(4 * e).readS32()) return [argI(c, 0), e, o / 0x84, argI(c, 2)];
      }
      return [argI(c, 0), -1, -1, argI(c, 2)];
    });
    logged(A.SetIdoPause, 'pause', ints(2));
    if (!rndk && !cfg.keep.includes('begin')) logged(va(0x5414c0), 'tankdelete', ints(3));
    // Part 2c: st_SetNonagressionViszony (858), Diplomacy_RemoveSpecial (858).
    logged(va(0x602860), 'nonaggr', ints(2));
    logged(va(0x4c0010), 'diplremove', ints(2));
    // Part 2d: FlottaOsszevon (804's OsszevonasCheck).
    logged(va(0x53d860), 'osszevon', ints(2));
    // Part 2e: SearchBolygohozKozeliFlotta with every argument (834's siege).
    if (!rndk) logged(va(0x540f50), 'nearfleet', (c) => [argI(c, 0), argU(c, 1), argI(c, 2) & 0xff, argI(c, 3) & 0xff, argI(c, 4) & 0xff, argI(c, 5) & 0xff, argI(c, 6)], (r) => r.toInt32());
  }
  if (rndk) {
    logged(va(0x6028a0), 'neutral', ints(2));
    logged(va(0x5e6ac0), 'kemenable', ints(1));
    logged(va(0x5d4ab0), 'kemdisable', ints(1));
    logged(va(0x540f50), 'nearfleet', (c) => [argI(c, 0), argU(c, 1)], (r) => r.toInt32());
    logged(va(0x53c730), 'bdelete', ints(2));
    if (!cfg.keep.includes('begin')) logged(va(0x5414c0), 'tankdelete', ints(3));
    // Part 2c: TervCopy (180), Spy_New and KemJelentesAdd (138).
    logged(va(0x541cd0), 'tervcopy', ints(3));
    if (!cfg.keep.includes('fable') && !cfg.keep.includes('begin')) {
      // Spy_New draws the new spy on stream 4: its state after the call goes with the slot.
      logged(va(0x58f830), 'spynew', ints(1), (r) => [r.toInt32(), A.RandSeed.add(16).readU32()]);
      logged(va(0x58d000), 'kemjelentes', ints(14));
    }
  }
  if (cfg.keep.includes('brnd') || cfg.keep.includes('begin') || rndk || elemKept) {
    // st_BRnd_include's calls: designs deleted, number parameters, techs given, the ticker.
    logged(va(0x541d50), 'tervdelete', ints(4));
    logged(va(0x509500), 'paramint', ints(2));
    logged(va(0x602920), 'techstate', ints(3));
    logged(va(0x556b50), 'scroll', (c) => { const q = c.esp.add(4).readPointer(); return [q.isNull() ? '' : q.readAnsiString(), argI(c, 1)]; });
  }
  if (cfg.keep.includes('fable')) setupFableActs(ints);
  // st_fAddShip's inputs (a probe, not a call of its own: the port does its work).
  Interceptor.attach(va(0x604410), {
    onEnter() {
      if (!cur) return;
      const c = this.context, id = argI(c, 0), ty = argI(c, 1), n = argI(c, 2);
      let f = id < 1 ? -id : 0;
      if (id > 0) for (let k = 1; k <= nFleets(); k++) if (fleet(k).add(0x16).readS16() === id && fleet(k).add(0x15).readU8() !== 0xff) { f = k; break; }
      let owner = -1, ground = -1;
      if (f > 0 && f <= nFleets() && fleet(f).add(0x15).readU8() <= 18) {
        owner = fleet(f).add(0x15).readU8();
        const d = A.Tervek.add(4 * owner).readPointer().add(0x84 * ty);
        ground = d.add(0x16).readS16() === 2 || d.add(0x36).readU8() !== 0 ? 1 : 0;
      }
      cur.probes.push(['addship', id, ty, n, f, owner, ground]);
    },
  });
  // The registry and config reads InitStory makes (logged; --registry/--storyline/--storymap override them).
  const keep = [];
  Interceptor.attach(A.GetParam, {
    onEnter(args) { this.key = args[0].readAnsiString(); },
    onLeave(r) {
      if (this.key !== 'storyline' && this.key !== 'storymap') return;
      const o = cfg[this.key];
      if (o !== null && o !== undefined) { const s = Memory.allocAnsiString(String(o)); keep.push(s); r.replace(s); }
      if (cur) cur.probes.push(['config', this.key, r.readAnsiString()]);
    },
  });
  Interceptor.attach(A.ReadRegistry, {
    onEnter(args) { this.key = args[0].readAnsiString(); this.out = args[1]; },
    onLeave(r) {
      if (!cur || cur.k !== 'init') return;
      if (cfg.registry !== null && cfg.registry !== undefined) {
        if (cfg.registry === 'none') r.replace(ptr(0));
        else { this.out.writeS32(cfg.registry); r.replace(ptr(1)); }
      }
      cur.probes.push(['registry', this.key, r.toInt32() & 0xff, this.out.readS32()]);
    },
  });
  Interceptor.replace(A.SaveRegistry, new NativeCallback((key, v) => { act('saveregistry', [key.readAnsiString(), v]); }, 'void', ['pointer', 'int'], 'mscdecl'));
  Interceptor.replace(A.ShowAnim, new NativeCallback((s) => { act('anim', [s.isNull() ? '' : s.readAnsiString()]); }, 'void', ['pointer'], 'mscdecl'));
  // InitStory: the 'init' record, with a snapshot after LoadScenario.
  Interceptor.attach(A.InitStory, {
    onEnter(args) { cur = { k: 'init', seq: seq++, arg: args[0].toInt32(), pre: snap(), acts: [], probes: [], d0: actDepth }; },
    onLeave() {
      cur.post = snap(); emit(cur); cur = null; initAt = frames;
      // Record from here on (the first daily check starts story 400); the speed is set a little later.
      const infos = []; for (let i = 0; i < A.InfoStrN.readS32(); i++) infos.push(A.InfoStr.add(12 * i).readS16());
      const rk = cfg.keep.includes('rnd'), ek = cfg.keep.includes('elem');
      // st_Elem part 2d: 801's texts (the transports' fleet name, the designs' names) and AlapFlottaNev too.
      const texts = (rk ? RND2C_TEXTS : []).concat(ek ? ELEM2D_TEXTS.concat(ELEM2E_TEXTS, ELEM2F_TEXTS, ELEM2G_TEXTS) : []);
      emit({ k: 'setup', init: snap(), data: storyData(), campaign: cfg.campaign, keep: cfg.keep, infos, agent: cfg.keep.includes('fable') ? sText(5, 0xc14) : null,
        stext5: rk || ek ? texts.map((id) => { const o = Memory.alloc(16); sCall(o, 5, id); const q = o.readPointer(); return [id, q.isNull() ? '' : latin1(q, 64)]; }) : null,
        recruits: rk ? [0, 1, 2, 3, 4, 5, 6].map((k) => latin1(A.Recruits.add(8 * k), 8)) : null,
        alapnev: rk || ek ? A.AlapNev.readPointer().readCString() : null, elem2a: ek, elem2b: ek, elem2c: ek, elem2d: ek, elem2e: ek, elem2f: ek, elem2g: ek });
      phase = 1;
    },
  });
  Interceptor.attach(A.LoadScenario, {
    onEnter(args) { if (cur && cur.k === 'init') act('loadscenario', [args[0].readAnsiString(), args[1].toInt32()]); },
    onLeave() { if (cur && cur.k === 'init') cur.mid = snap(); },
  });
}

// st_Multi_include's calls (besides st_Line's): texts, the lockstep message, ships deleted, designs made (with
// the record copied in, read at TervUjraSzamol), planet info lines.
function setupMultiActs(ints) {
  const str = (c, i) => { const q = c.esp.add(4 + 4 * i).readPointer(); return q.isNull() ? '' : q.readCString(); };
  logged(va(0x591360), 'text', (c) => [str(c, 0)]);
  // Type 0x49 (a spy order, st_Elem part 2a): also the second value (the mission).
  logged(va(0x513840), 'multisend', (c) => { const o = [argI(c, 0), argI(c, 1), argI(c, 2), c.esp.add(16).readPointer().readS32()]; if (o[0] === 0x49) o.push(c.esp.add(24).readPointer().readS32()); return o; });
  logged(va(0x53e5d0), 'shipdelete', ints(1));
  logged(va(0x5422f0), 'tervnew', ints(2), (r) => r.toInt32());
  logged(va(0x53beb0), 'infoadd', ints(3));
  Interceptor.attach(va(0x5423b0), {
    onEnter() {
      if (!cur) return;
      const e = argI(this.context, 0), d = argI(this.context, 1);
      if (e >= 0 && e < 19 && d >= 0 && d <= A.TervCount.add(4 * e).readS32()) cur.probes.push(['design', e, d, hex(A.Tervek.add(4 * e).readPointer().add(0x84 * d), 0x84)]);
    },
  });
}

// st_Fable_include's calls: a message taken back, the scrolling text, the story spy and its report, the video
// (inlined st_ShowAnim: OpenBink is replaced, no video plays).
function setupFableActs(ints) {
  logged(va(0x506140), 'withdraw', ints(2));
  logged(va(0x556b50), 'scroll', (c) => { const q = c.esp.add(4).readPointer(); return [q.isNull() ? '' : q.readCString(), argI(c, 1), argI(c, 2), argI(c, 3)]; });
  logged(va(0x58f830), 'spynew', ints(1), (r) => r.toInt32());
  logged(va(0x58d000), 'kemjelentes', ints(14));
  Interceptor.replace(A.OpenBink, new NativeCallback((path) => { act('anim', [path.isNull() ? '' : path.readCString()]); return 0; }, 'int', ['pointer', 'int', 'int', 'int', 'int'], 'mscdecl'));
}

let seq = 0, phase = 0, inTick = false, depth = 0;
Interceptor.attach(A.Frissit, {
  onEnter() { if (phase === 1 && !staging) { inTick = true; cur = { k: 'tick', seq: seq++, pre: snap(), acts: [], probes: [], d0: actDepth }; } },
  onLeave() {
    if (cur && cur.k === 'tick') { cur.post = snap(); emit(cur); cur = null; }
    inTick = false;
    if (seq >= cfg.count && phase === 1) { emit({ k: 'done' }); phase = 2; }
  },
});
Interceptor.attach(A.Check, {
  onEnter(args) {
    depth++;
    if (phase === 1 && !inTick && !staging && depth === 1 && !cur) {
      cur = { k: 'check', seq: seq++, arg: args[0].toInt32() & 0xff, from: '0x' + (this.returnAddress.sub(mod.base).toInt32() + IMAGE_BASE).toString(16), pre: snap(), acts: [], probes: [], d0: actDepth };
    }
  },
  onLeave() {
    depth--;
    if (depth === 0 && cur && cur.k === 'check') { cur.post = snap(); emit(cur); cur = null; }
  },
});

// ---------------------------------------------------------------- the native job runner (campaign mode)
// IG2_gameUpdate's `CALL Fake_FRISSITES` (exe 0x4e0523) is pointed at a stub that calls the frame and then,
// when the agent has queued calls, a job it wrote: the calls made natively (so the Interceptor hooks fire
// inside them), with callbacks into the agent before and after for the snapshots.
const code = Memory.alloc(Process.pageSize * 4);
Memory.protect(code, Process.pageSize * 4, 'rwx');
const stub = code, job = code.add(256), jobPtr = Memory.alloc(8);
jobPtr.writePointer(ptr(0));
function installStub() {
  const w = new X86Writer(stub, { pc: stub });
  w.putCallAddress(A.Fake_FRISSITES);
  w.putPushReg('eax');
  w.putMovRegNearPtr('eax', jobPtr);
  w.putTestRegReg('eax', 'eax');
  w.putJccShortLabel('je', 'done', 'no-hint');
  w.putXorRegReg('ecx', 'ecx');
  w.putMovNearPtrReg(jobPtr, 'ecx');
  w.putCallReg('eax');
  w.putLabel('done');
  w.putPopReg('eax');
  w.putRet();
  w.flush();
  const site = va(0x4e0523);
  Memory.patchCode(site, 5, (c) => { const p = new X86Writer(c, { pc: site }); p.putCallAddress(stub); p.flush(); });
}
let queue = [], armed = [];
// A staged record's `prep` runs right before its first call (after the frame's own ticks), so the 'pre'
// snapshot holds what it wrote (event flags, payloads); `after` runs after the 'post' snapshot.
const preCb = new NativeCallback((i) => {
  const c = armed[i];
  if (cfg.debug) emit({ k: 'dbg', what: 'pre', i, rec: !!c.rec, frames });
  if (c.rec) { staging = true; if (c.prep) c.prep(); cur = c.rec; cur.d0 = actDepth; cur.pre = snap(); }
}, 'void', ['int'], 'mscdecl');
const postCb = new NativeCallback((i) => {
  const c = armed[i];
  if (c.done && cur) { cur.post = snap(); emit(cur); cur = null; staging = false; if (c.after) c.after(); }
}, 'void', ['int'], 'mscdecl');
function call(fn, args, rec, done, prep, after) { queue.push({ fn, args, rec, done, prep, after }); }
function stage(name, calls, extra, prep, after) {
  const rec = Object.assign({ k: 'staged', seq: seq++, call: name, acts: [], probes: [] }, extra || {});
  calls.forEach((c, i) => call(c[0], c[1], i === 0 ? rec : null, i === calls.length - 1, i === 0 ? prep : null, i === calls.length - 1 ? after : null));
}
function armTrampoline() {
  if (!queue.length || !jobPtr.readPointer().isNull()) return;
  armed = queue;
  queue = [];
  const w = new X86Writer(job, { pc: job });
  w.putPushfx();
  w.putPushax();
  armed.forEach((c, i) => {
    w.putPushU32(i); w.putCallAddress(preCb); w.putAddRegImm('esp', 4);
    for (let k = c.args.length - 1; k >= 0; k--) w.putPushU32(c.args[k] >>> 0);
    w.putCallAddress(c.fn);
    if (c.args.length) w.putAddRegImm('esp', 4 * c.args.length);
    w.putPushU32(i); w.putCallAddress(postCb); w.putAddRegImm('esp', 4);
  });
  w.putPopax();
  w.putPopfx();
  w.putRet();
  w.flush();
  jobPtr.writePointer(job);
  if (cfg.debug) { emit({ k: 'dbg', what: 'arm', n: armed.length, frames, jobs: armed.map((c) => (c.rec ? c.rec.call + ' ' + JSON.stringify(c.rec.args) : '')) }); flush(); }
}

// ---------------------------------------------------------------- staged calls, scenario mode (--fuzz)
// Variables, planet and fleet ids the scenario stories read (st_scen.cpp, stories 0x168..0x177).
const STORIES = [0x168, 0x169, 0x16a, 0x16b, 0x16c, 0x16d, 0x16e, 0x16f, 0x170, 0x171, 0x172, 0x173, 0x174, 0x175, 0x176, 0x177];
const fake = Memory.alloc(0x30 * 4);
function liveFleets() { const o = []; for (let f = 1; f <= nFleets(); f++) if (fleet(f).add(0x15).readU8() !== 0xff) o.push(f); return o; }
function empireOrZero() { return rnd(4) === 0 ? 0 : rnd(19); }
function prepare(id) {
  const np = nPlanets(), en = A.EN.readS32();
  const randPlanet = () => 1 + rnd(np);
  switch (id) {
    case 0x168: case 0x16b: case 0x174: {
      const fid = id === 0x168 ? 0xe11 : id === 0x16b ? 0xe2f : 0xe89, lf = liveFleets(), dead = [];
      for (let f = 1; f <= nFleets(); f++) if (fleet(f).add(0x15).readU8() === 0xff) dead.push(f);
      // A dead slot with the id too: st_fSearch skips free slots.
      if (dead.length && rnd(2)) fleet(pick(dead)).add(0x16).writeS16(fid);
      if (lf.length && rnd(3)) fleet(pick(lf)).add(0x16).writeS16(fid);
      break;
    }
    case 0x169: case 0x176: { const n = 1 + rnd(7); for (let k = 1; k <= n; k++) vSet((id === 0x169 ? 0xe1a : 0xe9c) + k, rnd(3) ? empireOrZero() : 0); break; }
    case 0x16a: case 0x16c: case 0x16d: bAddAzon(randPlanet(), { 0x16a: 0xe25, 0x16c: 0xe39, 0x16d: 0xe43 }[id]); break;
    case 0x16e: vSet(0xe4d, 1 + rnd(4)); break;
    case 0x16f: { const m = A.Money.add(8 * en).readDouble(); vSet(0xe57, rnd(2) ? Math.floor(m) - 1 + rnd(3) : rnd(Math.max(1, Math.floor(m * 2) + 2))); break; }
    case 0x170: vSet(0xe61, A.HanyB.add(4 * en).readS32() - 1 + rnd(3)); break;
    case 0x171: {
      const own = []; for (let p = 1; p <= np; p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === en) own.push(p);
      const p = own.length && rnd(3) ? pick(own) : randPlanet(), val = A.Bolygok.add(0x188 * p + 0x28).readS32();
      vSet(0xe6b, rnd(2) ? val - 1 + rnd(3) : rnd(30000)); bAddAzon(p, 0xe6b); break;
    }
    case 0x172: vSet(0xe75, rnd(19)); break;
    case 0x173: vSet(0xe7f, pick([1, 2, 3, 29, 31, 40, 359, 361, 400])); break;
    case 0x175: vSet(0xe93, rnd(19)); vSet(0xe94, 1 + rnd(192)); break;
    case 0x177: vSet(0xea7, rnd(2) ? A.HanyB.add(4 * en).readS32() - 1 + rnd(3) : pick([0, 2, 3, 4, 5, 6, rnd(19)])); break;
  }
}
function stagedJs(name, f) {
  const r = { k: 'staged', seq: seq++, call: name, pre: snap(), acts: [], probes: [], d0: actDepth };
  f(r);
  r.post = snap();
  emit(r);
}
function fuzzStep() {
  const c = rnd(14), en = A.EN.readS32();
  if (c < 5) {
    const id = pick(STORIES);
    prepare(id);
    if (rnd(4) === 0) {
      stagedJs('start', (r) => { r.args = [id]; startStory(id, 0); storyCheck(1); });
    } else {
      startStory(id, 0); // runs at the next Story_Check(1)
    }
  } else if (c === 5) {
    const id = rnd(2) ? pick([0xe4d, 0xe7f]) : 9000 + rnd(12), d = rnd(4);
    szStart(id, d);
  } else if (c === 6) {
    const args = [pick([0x11, 0x12, 0xc, 0xf, 3, 7]), rnd(30), rnd(5), rnd(3), rnd(9), rnd(9), rnd(9), rnd(9)];
    stagedJs('special', (r) => { r.args = args; storySpecial(...args); });
  } else if (c === 7) {
    const lf = liveFleets();
    if (!lf.length) return;
    const f = pick(lf), q = fleet(f), kind = rnd(3), tgt = kind === 2 ? pick(lf) : 1 + rnd(nPlanets());
    const save = [q.add(0x42).readS16(), q.add(0x44).readFloat()];
    stagedJs('arrive', (r) => {
      r.args = [f, kind, tgt];
      q.add(0x42).writeS16(kind); q.add(0x44).writeFloat(tgt);
      arrivedSave(f); arrivedCheck();
      q.add(0x42).writeS16(save[0]); q.add(0x44).writeFloat(save[1]);
    });
  } else if (c === 8) {
    const n = 2 + rnd(2), list = [];
    for (let i = 0; i < n; i++) {
      const owner = rnd(3) ? (i === 0 ? en : rnd(19)) : rnd(19), id = pick([0, 0xe89, 0xe11, rnd(5000)]);
      const p = fake.add(0x30 * i); p.add(0x18).writeS32(owner); p.add(0x20).writeS32(id); p.add(0x24).writeS32(id); list.push([owner, id]);
    }
    stagedJs('spwfinished', (r) => { r.args = list; spwFinished(fake, n, 1); });
  } else if (c === 9) {
    // Raw trigger flags + Story_Check(1): space battle start (1), counter (5), daily (0x10), signal (0xc), ...
    const flag = pick([1, 5, 0x10, 0xc, 0, 3, 0xe]);
    stagedJs('flag', (r) => {
      const szv = flag === 5 ? pick([0xe4d, 0xe7f, 9000 + rnd(12)]) : 0;
      r.args = [flag, szv];
      A.Flags.add(flag).writeU8(1);
      if (flag === 5) A.SzVegzett.writeS32(szv);
      storyCheck(1);
    });
  } else if (c === 10) {
    const id = pick([0xe7f, 0xe80, 0xe81, 0xe4d, 3600 + rnd(200)]), owner = pick([-1, -1, 0, rnd(400)]);
    stagedJs('gameinfo', (r) => { r.args = [id, owner]; addGameInfo(id, Memory.allocAnsiString('x'), ptr(0), ptr(0), 1, owner); });
  } else if (c === 11) {
    const on = A.OzonN.readS32(), id = on && rnd(2) ? A.Ozon.add(12 * rnd(on) + 4).readS32() : 3600 + rnd(200);
    stagedJs('rmgameinfo', (r) => { r.args = [id]; removeGameInfo(id); });
  } else if (c === 12) {
    const ids = [];
    for (const k in story().buckets) for (const [id] of story().buckets[k]) ids.push(id);
    const id = ids.length && rnd(3) ? pick(ids) : pick(STORIES), hist = rnd(2);
    stagedJs('win', (r) => { r.args = [id, hist]; win(id, hist); });
  } else {
    // Unrelated variables, logic flags and planet ids (the engine's tables).
    if (rnd(2)) vSet(8000 + rnd(40), rnd(100) - 20);
    else bAddAzon(rnd(4) ? 1 + rnd(nPlanets()) : 0, 8000 + rnd(40));
  }
}

// ---------------------------------------------------------------- staged calls, campaign mode (--fuzz)
// The inline stories of Story_Check other than the driver (the scripts start them in the game).
const INLINE = [402, 403, 404, 405, 406, 902, 903, 904, 905, 906, 908, 909, 910, 911, 913, 917, 918];
function campaignFuzz() {
  const c = rnd(19), en = A.EN.readS32(), np = nPlanets();
  if (c < 4) {
    // Start an inline story now (StartStory + Story_Check(1)), with what it reads set up.
    const id = pick(cfg.inline && cfg.inline.length ? cfg.inline : INLINE);
    if (bucketHas(id)) return;
    const preps = [];
    if (id === 405) { const e = rnd(3) ? 1 + rnd(8) : 0, v = rnd(4) ? 0 : 1; preps.push(() => { A.Adat.add(0xcc).writeS32(e); vSet(4059, v); }); }
    if (id === 911 || id === 917 || id === 902 || id === 904) {
      const own = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en);
      if (own.length && rnd(4)) { const f = pick(own); preps.push(() => A.AktFleet.writeS32(f)); }
    }
    if (id === 909) {
      const own = []; for (let p = 1; p <= np; p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === en) own.push(p);
      if (!own.length) return;
      const p = pick(own);
      preps.push(() => A.AktPlanet.writeS32(p));
    }
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, () => preps.forEach((f) => f()));
  } else if (c < 6) {
    // Special 0x12 (story 402's bomb) on another empire's planet, 0x11 (902's wormhole) for a live fleet.
    if (rnd(2)) {
      const other = []; for (let p = 1; p <= np; p++) { const o = A.Bolygok.add(0x188 * p + 0x20).readU8(); if (o !== 0 && o !== en) other.push(p); }
      const p = other.length && rnd(4) ? pick(other) : 0;
      const args = [0x12, rnd(5), 0, 0, 0, 0, 0, 0];
      stage('special', [[A.Special, args]], { args }, () => { if (p) A.AktPlanet.writeS32(p); });
    } else {
      const lf = liveFleets();
      if (!lf.length) return;
      const args = [0x11, pick(lf), 0, 0, 0, 0, 0, 0];
      stage('special', [[A.Special, args]], { args });
    }
  } else if (c < 9) {
    // A story counter runs out at the next tick.
    const id = pick([4001, 4002, 4003, 4003, 4011, 4011, 4011, 4035, 4038, 4039, 4039, 4051, 4061]);
    if (id === 4011 && rnd(2)) vSet(4011, rnd(6));
    if (id === 4002 && rnd(2)) vSet(4001, rnd(4));
    szStart(id, rnd(2));
  } else if (c === 9) {
    // Story 404's trader arrives at a fleet (Story_FleetArrivedSave + Story_FleetArrivedCheck).
    const lf = liveFleets();
    let f = 0; for (const k of lf) if (fleet(k).add(0x16).readS16() === 4041) f = k;
    if (!f || !lf.length) return;
    const q = fleet(f), tgt = rnd(3) ? pick(lf.filter((k) => fleet(k).add(0x15).readU8() === en).concat(lf)) : pick(lf);
    let save = null;
    stage('arrive', [[A.ArrivedSave, [f]], [A.ArrivedCheck, []]], { args: [f, 2, tgt] },
      () => { save = [q.add(0x42).readS16(), q.add(0x44).readFloat()]; q.add(0x42).writeS16(2); q.add(0x44).writeFloat(tgt); },
      () => { q.add(0x42).writeS16(save[0]); q.add(0x44).writeFloat(save[1]); });
  } else if (c === 10) {
    // Story 405's message answered (flag 6, block 0x23f0, part 2) or another part.
    const part = rnd(3) ? 2 : 1;
    stage('flag', [[A.Check, [1]]], { args: [6, part] }, () => { A.Flags.add(6).writeU8(1); A.Msg.writeS32(0x23f0); A.Msg.add(4).writeS32(part); });
  } else if (c === 11) {
    // A raw daily check.
    stage('flag', [[A.Check, [1]]], { args: [0x10, 0] }, () => { A.Flags.add(0x10).writeU8(1); });
  } else if (c === 12) {
    // A story ends (signals, e.g. 401's finish starts story 400's timers).
    const ids = [];
    for (const k in story().buckets) for (const [id] of story().buckets[k]) ids.push(id);
    if (!ids.length) return;
    const id = pick(ids), hist = rnd(2);
    stage('win', [[A.Win, [id, hist]]], { args: [id, hist] });
  } else if (c === 13) {
    // Story 403's base and escorts: put fleets with ids 4030..4034 of owner 10 near planet 4031.
    const lf = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === 10);
    if (lf.length) fleet(pick(lf)).add(0x16).writeS16(4030 + rnd(5));
  } else if (c === 14) {
    // Unrelated variables and planet ids.
    if (rnd(2)) vSet(8000 + rnd(40), rnd(100) - 20);
    else bAddAzon(rnd(4) ? 1 + rnd(np) : 0, 8000 + rnd(40));
  } else if (c === 15) {
    // An empire the human knows or not (story 401's episode 9 needs one).
    const e = 1 + rnd(8);
    if (e !== en) A.Ismeri.add(19 * en + e).writeU8(rnd(2));
  } else if (c === 16) {
    // Story 404's refits: some of the human's designs get a design to refit to (+0x1c).
    const tn = A.TervCount.add(4 * en).readS32(), tp = A.Tervek.add(4 * en).readPointer();
    if (tn > 2) for (let k = 0; k < 3; k++) tp.add(0x84 * (1 + rnd(tn)) + 0x1c).writeS32(rnd(4) ? 1 + rnd(tn) : 0);
  } else {
    // Story 403's chase: one of the human's fleets moved next to planet 4031 or one of the escorts.
    let p = 0; const nb = A.nB.readS32();
    for (let i = 1; i <= nb; i++) if (A.sBolygok.add(8 * i).readS32() === 4031) p = A.sBolygok.add(8 * i + 4).readU8();
    const own = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en);
    if (!p || !own.length) return;
    const b = A.Bolygok.add(0x188 * p), q = fleet(pick(own));
    q.add(0x30).writeFloat(b.add(0x18).readFloat() + rnd(81) - 40);
    q.add(0x34).writeFloat(b.add(0x1c).readFloat() + rnd(81) - 40);
  }
}

// ---------------------------------------------------------------- staged calls, st_Line_include (--keep line)
// The storylines and their timers, answers, signals and the planets/empires they read.
const LINE = { 501: [0x1393, 0x1394, 0x1395, 0x1396, 0x1397, 0x1398, 0x1399], 502: [0x139d, 0x139e, 0x139f],
  530: [0x14b5, 0x14b6, 0x14b7, 0x14b8, 0x14b9, 0x14ba, 0x14bb, 0x14bc, 0x14bd, 0x14be],
  550: [0x157d, 0x157f, 0x1581, 0x1582, 0x1583, 0x1584, 0x1586, 0x158f, 0x14be],
  560: [0x15e9, 0x15e2, 0x15e3, 0x15e4, 0x15e5, 0x15e6], 680: [0x1a91, 0x1a92, 0x1a93, 0x1a95, 0x1a96, 0x1a97],
  710: [0x1bbd, 0x1bbe, 0x1bbf, 0x1bc0, 0x1bc1, 0x1bc2, 0x1bc3, 0x1bc4, 0x1bc5],
  720: [0x1c21, 0x1c22, 0x1c23, 0x1c24, 0x1c25, 0x1c26, 0x1c48] };
const LINE_SIGNALS = [801, 813, 816, 832, 835, 837, 856, 857, 858, 861, 862, 863, 864];
const LINE_ANSWERS = [[0x189c, 1], [0x189c, 2], [0x189c, 3], [0x1f67, 3], [0x1f67, 4], [0x1f67, 5], [0x1f67, 6],
  [0x19c9, 2], [0x15a4, 4], [0x15a5, 4], [0x15a4, 3]];
// Planet ids the stories read (StoryInit8160/804, 0x1fcc, 0x1f4a/b): set when the campaign map lacks them.
const LINE_PLANETS = [0x1fe0, 0x1f68, 0x1f69, 0x1f6a, 0x1fcc, 0x1f4a, 0x1f4b];
let lineMarks = 0;
function planetOfId(id) {
  const nb = A.nB.readS32();
  for (let i = 1; i <= nb; i++) if (A.sBolygok.add(8 * i).readS32() === id) return A.sBolygok.add(8 * i + 4).readU8();
  return 0;
}
function lineVars() {
  // The variables the storylines branch on.
  const e = 1 + rnd(8);
  vSet(0x1f41, pick([1, 2, 2, 3])); vSet(0x21a3, rnd(2)); vSet(0x1f54, rnd(2)); vSet(0x1ff5, rnd(3));
  if (rnd(3) === 0) { vSet(0x1457, rnd(2)); vSet(0x1458, e); }
}
function lineFuzz() {
  const c = rnd(10), en = A.EN.readS32(), np = nPlanets();
  for (const id of LINE_PLANETS) {
    if (planetOfId(id)) continue;
    const other = []; for (let p = 1; p <= np; p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() !== en) other.push(p);
    if (other.length) bAddAzon(pick(other), id);
  }
  const running = Object.keys(LINE).map(Number).filter(bucketHas);
  // Transient world changes inside a staged record: an empire 1 or 5 dead (the scripts' aborts).
  const transient = () => {
    if (rnd(5)) return null;
    const e = pick([1, 5]), was = A.El.add(e).readU8();
    return [() => A.El.add(e).writeU8(0), () => A.El.add(e).writeU8(was)];
  };
  if (c < 1 || !running.length) {
    const ids = Object.keys(LINE).map(Number).filter((id) => !bucketHas(id));
    if (!ids.length) return;
    const id = pick(ids), skip = id === 550 && rnd(4) === 0;
    lineVars();
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, () => { if (skip) A.SkipFirst.writeS32(8); });
  } else if (c < 3) {
    // A timer of a running storyline runs out at the next tick.
    lineVars();
    szStart(pick(LINE[pick(running)]), rnd(2));
  } else if (c < 6) {
    // The same as a staged counter flag (with an empire perhaps dead, or unknown). Before st_GetBirodalom808hoz
    // (counters 5301, 5501) the relations with the human, whom it knows and some planets' owners are shuffled
    // for the record (restored afterwards).
    const near = running.filter((x) => x === 530 || x === 550).map((x) => (x === 530 ? 0x14b5 : 0x157d));
    const id = near.length && rnd(3) === 0 ? pick(near) : pick(LINE[pick(running)]), t = transient(), sets = [];
    if ((id === 0x14b5 || id === 0x157d) && rnd(3)) {
      const byte = (q, v) => { let was = 0; sets.push([() => { was = q.readU8(); q.writeU8(v); }, () => q.writeU8(was)]); };
      for (let e = 1; e < 9; e++) {
        if (e === en) continue;
        if (rnd(2)) byte(A.Viszony.add((en * 19 + e) * 0x26 + 0x25), 1 + rnd(5));
        if (rnd(3) === 0) A.Ismeri.add(19 * en + e).writeU8(rnd(2));
      }
      for (let k = rnd(4); k > 0; k--) {
        const p = 1 + rnd(np), q = A.Bolygok.add(0x188 * p + 0x20);
        if (q.readU8() !== en) byte(q, pick([1, 2, 3, 4, 5, 6, 7, 8, 9, 0]));
      }
    }
    if (rnd(3) === 0) { const e = pick([1, 5]); if (e !== en) A.Ismeri.add(19 * en + e).writeU8(rnd(2)); }
    lineVars();
    stage('flag', [[A.Check, [1]]], { args: [5, id] },
      () => { if (t) t[0](); sets.forEach((x) => x[0]()); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(id); },
      () => { sets.slice().reverse().forEach((x) => x[1]()); if (t) t[1](); });
  } else if (c < 8) {
    // An answer (flag 6), a promotion (7) or the disable query (8) of a storyline message; the human's
    // planets marked for message 0x1f67 first.
    // Answers to the running storylines' messages mostly.
    const mine = LINE_ANSWERS.filter(([m]) => (bucketHas(530) && (m === 0x189c || m === 0x1f67)) || (bucketHas(550) && m !== 0x189c && m !== 0x1f67));
    const kind = rnd(4) ? 6 : pick([7, 8, 8]), [b, part] = kind === 6 ? pick(mine.length && rnd(4) ? mine : LINE_ANSWERS) : [pick([0x189c, 0x1f67, 0x1f67]), 0];
    if (b === 0x1f67 && lineMarks < 12 && rnd(2)) {
      const own = []; for (let p = 1; p <= np; p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === en) own.push(p);
      // The human's only planet just loses 9200 of +0x28 (0x1f42 with enough left); with more planets one may
      // be given up (0x1f41, or 0x1f42 below 9500).
      if (own.length) {
        const sole = own.length < 2, p = pick(own), q = A.Bolygok.add(0x188 * p);
        for (let k = 0; k < 5; k++) if (q.add(0x16a + 2 * k).readS16() === 0) { q.add(0x16a + 2 * k).writeS16(sole ? 0x1f42 : pick([0x1f41, 0x1f42, 0x1f42])); break; }
        if (sole) q.add(0x28).writeS32(9600 + rnd(30000)); else if (rnd(2)) q.add(0x28).writeS32(9000 + rnd(1000));
        lineMarks++;
      }
    }
    stage('flag', [[A.Check, [1]]], { args: [kind, b, part] },
      () => { A.Flags.add(kind).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(part); });
  } else {
    // An episode finished (signal type 1) or story 804's type-2 signal.
    const [id, ty] = rnd(6) ? [pick(LINE_SIGNALS), 1] : [804, 2], t = transient();
    lineVars();
    stage('flag', [[A.Check, [1]]], { args: [12, id, ty] },
      () => { if (t) t[0](); A.Flags.add(12).writeU8(1); A.Signal.writeS32(id); A.Signal.add(4).writeS32(ty); },
      () => { if (t) t[1](); });
  }
}

// ---------------------------------------------------------------- staged calls, st_Multi_include (--keep multi)
// The multiplayer modes 1001..1006 started, their counters run out, ground battles on their planets,
// colonisations (special 10), the radar offer's answers, the purchase (special 15), fleets arriving at the
// capture-the-flag bases, space battles with empires 1 and 2.
const MULTI = [1001, 1002, 1003, 1004, 1005, 1006];
const MULTI_COUNTERS = [10011, 10021, 10031, 10033, 10034];
function multiPlanets() {
  const out = [];
  for (const id of [10011, 10021, 10031, 10032, 10033, 10051, 10052]) { const p = planetOfId(id); if (p) out.push(p); }
  return out;
}
const ftou = (x) => { const b = new ArrayBuffer(4); new Float32Array(b)[0] = x; return new Uint32Array(b)[0]; };
// A value written for one record and put back afterwards.
function transientU8(sets, q, v) { let was = 0; sets.push([() => { was = q.readU8(); q.writeU8(v); }, () => q.writeU8(was)]); }
function transientU32(sets, q, v) { let was = 0; sets.push([() => { was = q.readU32(); q.writeU32(v); }, () => q.writeU32(was)]); }
function multiFuzz() {
  const c = rnd(16), en = A.EN.readS32(), np = nPlanets(), running = MULTI.filter(bucketHas);
  const sets = [];
  const prep = () => sets.forEach((x) => x[0]()), after = () => sets.slice().reverse().forEach((x) => x[1]());
  // The modes' planets change owners for a record now and then (a colonisation or a conquest elsewhere).
  const shuffleOwners = () => {
    for (const p of multiPlanets()) if (rnd(3) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([0, en, en, 1, 2, 3, 4]));
  };
  if (c < 2 || !running.length) {
    const ids = MULTI.filter((id) => !bucketHas(id));
    if (!ids.length) return;
    // 1003 and 1005 more often: their starts search, draw and build the most.
    const id = pick(ids.concat(ids.filter((x) => x === 1003 || x === 1005).flatMap((x) => [x, x, x])));
    if (rnd(2)) vSet(10010, rnd(4));
    if (rnd(2)) vSet(10020, rnd(4));
    if (rnd(3) === 0) { vSet(10059, rnd(3) ? 0 : 1 + rnd(400)); vSet(10060, rnd(3) ? 0 : 1 + rnd(90)); }
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] });
  } else if (c < 5) {
    // A mode's counter runs out (with the planets' owners perhaps changed).
    shuffleOwners();
    const id = pick(MULTI_COUNTERS);
    stage('flag', [[A.Check, [1]]], { args: [5, id] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(id); }, after);
  } else if (c < 6) {
    // A ground battle ends on one of the modes' planets (flag 4, GrWarBolygo 0xF26544).
    shuffleOwners();
    const ps = multiPlanets(), p = ps.length && rnd(4) ? pick(ps) : 1 + rnd(np);
    stage('flag', [[A.Check, [1]]], { args: [4, p] }, () => { prep(); A.Flags.add(4).writeU8(1); A.Gr.add(12).writeS32(p); }, after);
  } else if (c < 7) {
    // A colonisation (special 10): elsewhere, or now and then on a capture-the-flag base.
    shuffleOwners();
    const bases = [planetOfId(10051), planetOfId(10052)].filter((p) => p), p = bases.length && rnd(4) === 0 ? pick(bases) : 1 + rnd(np);
    const args = [10, rnd(3) ? en : 1 + rnd(8), p, 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args }, prep, after);
  } else if (c < 8) {
    // The radar offer: answered (part 2 sends the purchase, part 4 asks later), promoted.
    const [kind, part] = pick([[6, 2], [6, 4], [6, 1], [7, 0]]);
    stage('flag', [[A.Check, [1]]], { args: [kind, 0x2347, part] }, () => { A.Flags.add(kind).writeU8(1); A.Msg.writeS32(0x2347); A.Msg.add(4).writeS32(part); });
  } else if (c < 9) {
    // The purchase (special 15 from MULTIGET_Story): the human's or another player's.
    const args = [15, pick([en, en, 0, 1 + rnd(8)]), 10031, 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args });
  } else if (c < 11) {
    // The radar counter with the human owning the planets and enemy fleets around them.
    for (const id of [10031, 10032, 10033]) {
      const p = planetOfId(id);
      if (!p) continue;
      const b = A.Bolygok.add(0x188 * p);
      if (rnd(4)) transientU8(sets, b.add(0x20), en);
      const lf = liveFleets().filter((f) => fleet(f).add(0x15).readU8() !== en);
      for (let k = rnd(3); k > 0 && lf.length; k--) {
        const q = fleet(pick(lf)), d = rnd(130);
        transientU32(sets, q.add(0x30), ftou(b.add(0x18).readFloat() + d * (rnd(2) ? 0.7 : -0.7)));
        transientU32(sets, q.add(0x34), ftou(b.add(0x1c).readFloat() + d * (rnd(2) ? 0.7 : -0.7)));
      }
    }
    stage('flag', [[A.Check, [1]]], { args: [5, 10033] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(10033); }, after);
  } else if (c < 14) {
    // A fleet of empire 1 or 2 arrives at a base (or elsewhere).
    const bases = [planetOfId(10051), planetOfId(10052)].filter((p) => p), lf = liveFleets();
    if (!lf.length) return;
    const f = pick(lf), q = fleet(f), tgt = bases.length && rnd(5) ? pick(bases) : 1 + rnd(np);
    transientU8(sets, q.add(0x15), pick([1, 2, 1, 2, q.add(0x15).readU8()]));
    if (rnd(3) === 0) vSet(rnd(2) ? 10053 : 10054, rnd(2));
    let save = null;
    stage('arrive', [[A.ArrivedSave, [f]], [A.ArrivedCheck, []]], { args: [f, 1, tgt] },
      () => { prep(); save = [q.add(0x42).readS16(), q.add(0x44).readFloat()]; q.add(0x42).writeS16(1); q.add(0x44).writeFloat(tgt); },
      () => { q.add(0x42).writeS16(save[0]); q.add(0x44).writeFloat(save[1]); after(); });
  } else if (c === 14 && rnd(2)) {
    // A mode ends (so its start runs again later).
    const id = pick(running.concat(running.filter((x) => x === 1003 || x === 1005)));
    const h = rnd(2);
    stage('win', [[A.Win, [id, h]]], { args: [id, h] });
  } else {
    // A space battle ends with empires 1 and 2 (fake participants, CTF_value written for the record).
    // Two or three participants: the game never ends a battle with one (st_Multi reads the second regardless).
    const n = 2 + rnd(2), list = [];
    for (let i = 0; i < n; i++) {
      const owner = pick([1, 2, 1, 2, en, rnd(9)]), p = fake.add(0x30 * i);
      p.add(0x18).writeS32(owner); p.add(0x20).writeS32(0); p.add(0x24).writeS32(0); list.push([owner, 0]);
    }
    for (let i = 0; i < 3; i++) transientU32(sets, A.Ctf.add(4 * i), rnd(3) ? rnd(500) : (rnd(100000) - 50000) >>> 0);
    if (rnd(2)) vSet(10053, rnd(2));
    if (rnd(2)) vSet(10054, rnd(2));
    stage('spwfinished', [[A.SpwFinished, [fake.toUInt32(), n, 1]]], { args: list }, prep, after);
  }
}

// ---------------------------------------------------------------- staged calls, st_BRnd_include (--keep brnd)
// The random stories 220..239 (+ 2301, 2311) started, their counters run out (with the world they test set up
// for the record: techs done, a strong fleet, ships of the story designs at the right planets, more planets),
// their messages answered or promoted, daily checks, space battles starting with their fleets, specials 0x11
// and 0x19.
const BRND = [220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 2301, 231, 2311, 232, 233, 234, 235, 236, 237, 238, 239];
const MERC = { 220: 2200, 221: 2210, 222: 2220, 223: 2230, 224: 2240, 226: 2260, 227: 2270, 228: 2280, 229: 2290 };
const MERC_TECH = { 220: 7, 221: 8, 222: 4, 224: 8, 228: 6, 229: 9 };
const BRND_COUNTERS = { 230: [2300, 2302], 2301: [23011], 231: [2310], 2311: [23111], 233: [2331], 234: [2340, 2341],
  235: [2350, 2351], 236: [2360, 2361, 2362], 237: [2370], 238: [2380] };
const BRND_MESSAGES = { 230: [2300], 231: [2310], 233: [2330], 234: [2340], 235: [2350], 236: [2360, 2361], 238: [2380], 239: [2390] };
// A value written for one record and put back afterwards unless the game changed it meanwhile.
function transientS16(sets, q, v) { let was = 0; sets.push([() => { was = q.readS16(); q.writeS16(v); }, () => { if (q.readS16() === v) q.writeS16(was); }]); }
function transientS32(sets, q, v) { let was = 0; sets.push([() => { was = q.readS32(); q.writeS32(v); }, () => { if (q.readS32() === v) q.writeS32(was); }]); }
function humanShips() {
  const en = A.EN.readS32(), hn = A.HajoCount.readS32(), hp = A.Hajok.readPointer(), out = [];
  for (let k = 1; k <= hn; k++) { const f = hp.add(6 * k).readS16(), d = hp.add(6 * k + 2).readS16(); if (d !== 0x7fff && f > 0 && f <= nFleets() && fleet(f).add(0x15).readU8() === en) out.push([k, f]); }
  return out;
}
// A ship of the human's of design `d` in a fleet next to planet `p` (heading for it).
function shipAt(sets, d, p) {
  const ships = humanShips();
  if (!ships.length || !p) return;
  const [k, f] = pick(ships), q = fleet(f), b = A.Bolygok.add(0x188 * p), hp = A.Hajok.readPointer();
  transientS16(sets, hp.add(6 * k + 2), d);
  // Offsets from the planet's truncated coordinates around st_tav's limit of 20 (12/16 → 20, 6/19 → 19.9).
  const [dx, dy] = rnd(3) ? pick([[12, 16], [6, 19], [0, 20], [0, 19], [14, 14], [16, 12], [3, 4]]) : [rnd(30) - 15, rnd(30) - 15];
  const sx = rnd(2) ? 1 : -1, sy = rnd(2) ? 1 : -1;
  transientU32(sets, q.add(0x30), ftou(Math.trunc(b.add(0x18).readFloat()) + sx * dx + 0.25));
  transientU32(sets, q.add(0x34), ftou(Math.trunc(b.add(0x1c).readFloat()) + sy * dy + 0.25));
  if (rnd(4)) transientS16(sets, q.add(0x42), 1);
  if (rnd(4)) transientU32(sets, q.add(0x44), ftou(p));
  if (rnd(4) && q.add(0x40).readS16() === 0) transientS16(sets, q.add(0x40), 1);
}
// The stories whose branches need several steps: picked more often.
const BRND_RARE = [230, 231, 233, 234, 235, 236, 239];
const pickRare = (ids) => { const r = ids.filter((x) => BRND_RARE.includes(x)); return r.length && rnd(3) ? pick(r) : pick(ids); };
let wormholeMade = false;
function brndFuzz() {
  // --inline narrows the stories staged (a focused run).
  const pool = cfg.inline && cfg.inline.length ? BRND.filter((x) => cfg.inline.includes(x)) : BRND;
  const c = rnd(20), en = A.EN.readS32(), np = nPlanets(), running = pool.filter(bucketHas);
  // A wormhole for story 239 (the campaign maps start without one): made once, natively, not recorded.
  if (!wormholeMade && A.WormholeN.readS32() === 0 && np > 2) {
    wormholeMade = true;
    const a = A.Bolygok.add(0x188 * (1 + rnd(np))), b = A.Bolygok.add(0x188 * (1 + rnd(np)));
    call(va(0x53aec0), [a.add(0x18).readFloat() | 0, a.add(0x1c).readFloat() | 0, b.add(0x18).readFloat() | 0, b.add(0x1c).readFloat() | 0], null, false);
    return;
  }
  const sets = [];
  const prep = () => sets.forEach((x) => x[0]()), after = () => sets.slice().reverse().forEach((x) => x[1]());
  // The human's planet count for the record (233..236 need 3, 5, 10).
  const morePlanets = () => { if (rnd(2)) transientS32(sets, A.HanyB.add(4 * en), pick([3, 5, 10, 12, 60])); };
  const ownSome = () => {
    for (let k = rnd(4); k > 0; k--) { const p = 1 + rnd(np - 1); if (A.Bolygok.add(0x188 * p + 0x20).readU8() !== en) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), en); }
  };
  if (c < 3 || !running.length) {
    const ids = pool.filter((id) => !bucketHas(id));
    if (!ids.length) return;
    const id = pickRare(ids);
    morePlanets();
    if (id === 233 || id === 234 || id === 235) ownSome();
    if (id === 230 || id === 231) for (let e = 1; e < 9; e++) if (e !== en && rnd(3) === 0) transientU8(sets, A.Ismeri.add(19 * en + e), rnd(2));
    if (id === 239 && A.WormholeN.readS32() > 0 && rnd(3)) { const k = 1 + rnd(A.WormholeN.readS32()); transientU8(sets, A.Wormholes.add(0x1c * k + 8), 1); transientU8(sets, A.Wormholes.add(0x1c * k + 0x14), 1); }
    if (id === 232) for (let k = rnd(6); k > 0; k--) transientS16(sets, A.Bolygok.add(0x188 * (1 + rnd(np)) + 0xd8 + 2 * en), 0);
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
  } else if (c < 5) {
    // A counter runs out at the next tick.
    const id = pickRare(running), list = MERC[id] ? [MERC[id] + 9, MERC[id] + 1].concat(id === 227 ? [2272] : []) : BRND_COUNTERS[id];
    if (list) szStart(pick(list), rnd(2));
  } else if (c < 10) {
    // A staged counter with the world it tests set up.
    const id = pickRare(running);
    let ctr = 0;
    if (MERC[id]) {
      ctr = rnd(3) ? MERC[id] + 9 : id === 227 && rnd(2) ? 2272 : MERC[id] + 1;
      // 227's chase with no fleet of the human's left (st_fEliminate).
      if (ctr === 2272 && rnd(2)) for (const f of liveFleets()) if (fleet(f).add(0x15).readU8() === en) transientU8(sets, fleet(f).add(0x15), 9);
      if (MERC_TECH[id] && rnd(3)) transientU8(sets, A.Techs.add(0x114 * MERC_TECH[id] + 0x45 + en), 5);
      if (rnd(2)) {
        // A strong fleet: one of the human's ships' design gets a big attack.
        const ships = humanShips();
        if (ships.length) { const [k] = pick(ships), d = A.Hajok.readPointer().add(6 * k + 2).readS16(); transientS32(sets, A.Tervek.add(4 * en).readPointer().add(0x84 * d + 0x2c), pick([800, 3000, 9000, 20000])); }
      }
    } else if (BRND_COUNTERS[id]) {
      ctr = pick(BRND_COUNTERS[id]);
      if (ctr === 2300 && rnd(4)) { const p = 1 + rnd(np); transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), varOf(2302)); shipAt(sets, varOf(2301), p); }
      if (ctr === 2302 && rnd(2)) { const s2 = humanShips(); if (s2.length) transientS16(sets, A.Hajok.readPointer().add(6 * pick(s2)[0] + 2), varOf(2301)); }
      if (ctr === 2341) for (let k = rnd(8); k > 0; k--) transientU8(sets, A.Techs.add(0x114 * (1 + rnd(176)) + 0x45 + en), 1 + rnd(3));
      if (ctr === 2362) for (let k = rnd(6); k > 0; k--) transientS16(sets, A.Bolygok.add(0x188 * (1 + rnd(np)) + 0xd8 + 2 * en), 0);
    }
    if (!ctr) return;
    stage('flag', [[A.Check, [1]]], { args: [5, ctr] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(ctr); }, after);
  } else if (c < 15) {
    // An answer (flag 6), a promotion (7) or the disable query (8).
    const id = pickRare(running), ms = MERC[id] ? [MERC[id], MERC[id] + 1] : BRND_MESSAGES[id];
    if (!ms) return;
    const b = pick(ms), kind = rnd(5) ? 6 : rnd(3) ? 7 : 8, part = kind === 6 ? pick([2, 2, 3, 3, 4, 4, 1, 5]) : 0;
    // 235's payment: the human's planets worth more for the record.
    if (b === 2350 && kind === 6) for (let p = 1; p <= np; p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === en && rnd(2)) transientS32(sets, A.Bolygok.add(0x188 * p + 0x28), 20000 + rnd(400000));
    // st_GetUnknownPlanet: the campaign map's planets 11001..11015 unknown to the human.
    if (id === 236 && b === 2361 && part === 2) for (let k = 11001; k <= 11015; k++) { const p = planetOfId(k); if (p && rnd(3)) transientS16(sets, A.Bolygok.add(0x188 * p + 0xd8 + 2 * en), -1); }
    if (id === 234 || id === 235) ownSome();
    stage('flag', [[A.Check, [1]]], { args: [kind, b, part] }, () => { prep(); A.Flags.add(kind).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(part); }, after);
  } else if (c < 17) {
    // A daily check (231's and 239's tests), a ship of 231's design next to its planet now and then.
    if (bucketHas(231) && rnd(2)) { let p = planetOfId(2310); if (p) shipAt(sets, varOf(2310), p); }
    stage('flag', [[A.Check, [1]]], { args: [0x10, 0] }, () => { prep(); A.Flags.add(0x10).writeU8(1); }, after);
  } else if (c < 18) {
    // A space battle starts: spFlottak holds a fleet of the human's (one with the hired mercenaries mostly).
    const own = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en), all = liveFleets();
    if (!all.length) return;
    const a = own.length && rnd(3) ? pick(own) : pick(all), b = pick(all);
    transientS16(sets, A.SpFlottak, rnd(2) ? a : b); transientS16(sets, A.SpFlottak.add(6), rnd(2) ? b : a);
    stage('flag', [[A.Check, [1]]], { args: [1, a, b] }, () => { prep(); A.Flags.add(1).writeU8(1); }, after);
  } else if (c < 19) {
    // Special 0x11 (239's wormhole) for a live fleet.
    const lf = liveFleets();
    if (!lf.length) return;
    const args = [0x11, pick(lf), 0, 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args });
  } else {
    // Special 0x19 for a fleet of the human's and one of its designs (a story design 2200..2299 now and then).
    const own = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en), tn = A.TervCount.add(4 * en).readS32();
    if (!own.length || tn < 1) return;
    const d = 1 + rnd(tn);
    if (rnd(2)) transientS32(sets, A.Tervek.add(4 * en).readPointer().add(0x84 * d + 0x80), pick([2200, 2215, 2299, 2300, 2190, 0]));
    const args = [0x19, pick(own), d, 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args }, prep, after);
  }
}

// ---------------------------------------------------------------- staged calls, st_Begin_include (--keep begin)
// The opening episodes 1..24 (+ 51) started, their counters run out (with the world they test set up for the
// record), their messages read, answered or promoted, daily checks, specials 10/0xd/0x10/0x13, fleet arrivals,
// space battles starting and ending with their fleets, ground battles ending on their planets, the radar reaching
// the base, spy news.
const BEGIN = [1, 4, 5, 51, 6, 7, 8, 9, 10, 11, 12, 19, 20, 21, 22, 23, 24];
const spwBufs = [];
const BEGIN_COUNTERS = { 1: [18, 12, 19, 11], 4: [49], 5: [59], 6: [61, 62], 7: [71, 72, 73], 9: [91], 10: [109, 101],
  11: [111], 12: [129], 19: [196, 195, 197, 198, 191], 20: [201], 21: [211], 23: [231], 24: [241, 242] };
// [kind (6 answer, 7 promoted, 9 read), block, part]
const BEGIN_MESSAGES = { 1: [[9, 11]], 4: [[6, 41, 4], [6, 41, 6], [6, 41, 7], [7, 41], [6, 41, 2]], 5: [[9, 51]], 6: [[9, 61]],
  7: [[6, 72, 2], [6, 72, 3], [7, 72], [6, 73, 2], [6, 73, 3]], 8: [[9, 81]],
  9: [[6, 91, 2], [6, 91, 3], [6, 91, 4], [6, 92, 2], [6, 92, 3], [6, 92, 4], [7, 91], [7, 92], [8, 91]],
  10: [[9, 111], [9, 101]], 11: [[9, 111], [7, 111]] };
const BEGIN_FLEETS = [11, 12, 14, 41, 51, 63, 69, 231, 232, 241];
const BEGIN_RARE = [1, 4, 5, 6, 7, 10, 19, 20, 21, 23, 24];
const pickBegin = (ids) => { const r = ids.filter((x) => BEGIN_RARE.includes(x)); return r.length && rnd(3) ? pick(r) : pick(ids); };
function fleetOfId(id) { for (let f = 1; f <= nFleets(); f++) if (fleet(f).add(0x16).readS16() === id && fleet(f).add(0x15).readU8() !== 0xff) return f; return 0; }
function ownPlanets() { const en = A.EN.readS32(), o = []; for (let p = 1; p <= nPlanets(); p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === en) o.push(p); return o; }
const kemRec = (e, k) => A.Kemek.add((e * 13 + k) * 0x4c);
// The human's spies for the record: alive, at home on counter-espionage (19) or with a target (21).
function stageSpies(sets, kind) {
  const en = A.EN.readS32(), n = A.KemN.add(4 * en).readS32();
  if (n < 2 && rnd(2)) transientS32(sets, A.KemN.add(4 * en), 2 + rnd(4));
  const m = Math.max(n, 3);
  for (let k = 1; k < m; k++) {
    if (rnd(3) === 0) continue;
    const r = kemRec(en, k);
    transientS16(sets, r.add(0x10), kind === 19 ? pick([3, 3, 1, 0]) : pick([1, 1, 2, 3, 0]));
    if (kind === 19) { transientS16(sets, r.add(0x1a), pick([13, 13, 2])); transientU8(sets, r.add(0x32), rnd(10)); transientU8(sets, r.add(0x33), rnd(10)); }
    if (kind === 21 && rnd(4) === 0) transientU8(sets, r.add(0x18), 1 + rnd(8));
  }
}
function beginFuzz() {
  const pool = cfg.inline && cfg.inline.length ? BEGIN.filter((x) => cfg.inline.includes(x)) : BEGIN;
  const c = rnd(24), en = A.EN.readS32(), np = nPlanets(), running = pool.filter(bucketHas);
  const sets = [];
  const prep = () => sets.forEach((x) => x[0]()), after = () => sets.slice().reverse().forEach((x) => x[1]());
  const flagRec = (fl, extra, write) => stage('flag', [[A.Check, [1]]], { args: [fl].concat(extra || []) }, () => { prep(); A.Flags.add(fl).writeU8(1); if (write) write(); }, after);
  if (c < 4 || !running.length) {
    const ids = pool.filter((id) => !bucketHas(id));
    if (!ids.length) return;
    const id = pickBegin(ids);
    if (id === 6 && !planetOfId(61)) {
      const cand = []; for (let p = 1; p <= np; p++) { const o = A.Bolygok.add(0x188 * p + 0x20).readU8(); if (o !== en && o !== 0 && o <= 8) cand.push(p); }
      if (cand.length) bAddAzon(pick(cand), 61);
    }
    if (id === 10 && rnd(3)) transientS32(sets, A.HanyB.add(4 * en), 1 + rnd(2));
    if (id === 9) for (let e = 1; e < 9; e++) if (e !== en && rnd(3) === 0) transientU8(sets, A.Ismeri.add(19 * en + e), rnd(2));
    if (id === 19 || id === 21) stageSpies(sets, id);
    if (id === 24) {
      // A trader of empire 9 (ids 10000..11000) near one of the human's planets.
      const nine = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === 9), own = ownPlanets();
      if (own.length && rnd(2)) {
        const f = nine.length && rnd(2) ? pick(nine) : pick(liveFleets()), b = A.Bolygok.add(0x188 * pick(own));
        transientU8(sets, fleet(f).add(0x15), 9); transientS16(sets, fleet(f).add(0x16), 10000 + rnd(1001));
        transientU32(sets, fleet(f).add(0x30), ftou(b.add(0x18).readFloat() + rnd(120) - 60));
        transientU32(sets, fleet(f).add(0x34), ftou(b.add(0x1c).readFloat() + rnd(120) - 60));
      }
    }
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
  } else if (c < 6) {
    const id = pickBegin(running), list = BEGIN_COUNTERS[id];
    if (list) szStart(pick(list), rnd(2));
  } else if (c < 11) {
    // A staged counter with the world it tests set up.
    const id = pickBegin(running), list = BEGIN_COUNTERS[id];
    if (!list) return;
    const ctr = pick(list);
    if (id === 19) stageSpies(sets, 19);
    if (id === 1 && ctr === 12 && rnd(2)) { const p = planetOfId(11); if (p) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), rnd(2) ? 10 : en); }
    if (id === 6 && rnd(2)) { const p = planetOfId(61); if (p) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([10, 10, en, 3])); }
    if (id === 1 || id === 5 || id === 6) for (const fid of [11, 12, 14, 51, 63, 69]) { const f = fleetOfId(fid); if (f && rnd(2)) transientS16(sets, fleet(f).add(0x40), rnd(2) ? 0 : 1); }
    if (id === 4 && rnd(2)) { const p = planetOfId(41); if (p) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), rnd(3) ? en : 3); }
    if (id === 10 && ctr === 101 && rnd(2)) { const p = planetOfId(101); if (p) transientS32(sets, A.Bolygok.add(0x188 * p + 0x28), pick([1000, 4999, 5000, 7000, 8000, 10000, 20000])); }
    flagRec(5, [ctr], () => A.SzVegzett.writeS32(ctr));
  } else if (c < 15) {
    // A message read (9), answered (6), promoted (7) or the disable query (8).
    const id = pickBegin(running), ms = BEGIN_MESSAGES[id];
    if (!ms) return;
    const [kind, b, part] = pick(ms);
    if (id === 4 && rnd(2)) { const p = planetOfId(41); if (p) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), rnd(3) ? en : 3); }
    flagRec(kind, [b, part || 0], () => { A.Msg.writeS32(b); A.Msg.add(4).writeS32(part || 0); });
  } else if (c < 17) {
    // A daily check; the fleets standing or not, story 7's troops on the planet, 21's spy.
    for (const fid of BEGIN_FLEETS) { const f = fleetOfId(fid); if (f && rnd(3) === 0) transientS16(sets, fleet(f).add(0x40), rnd(2) ? 0 : 1); }
    if (bucketHas(7) && rnd(2)) {
      // Units of size 19 on planet 73: a design of the human's made a size-19 ground unit, a Tankok record there.
      const p = planetOfId(73), tn = A.TervCount.add(4 * en).readS32(), n = A.TankN.readS32();
      if (p && tn > 0 && n > 0) {
        const d = 1 + rnd(tn), tp = A.Tervek.add(4 * en).readPointer().add(0x84 * d);
        transientS16(sets, tp.add(0x16), 2); transientS32(sets, tp.add(0x44), 19);
        for (let k = rnd(3) + 1; k > 0; k--) {
          const t = A.Tanks.add(8 * (1 + rnd(n)));
          transientS16(sets, t, -p); transientS16(sets, t.add(2), d); transientS32(sets, t.add(4), pick([1, 2, 3, 4, 5, 8]));
        }
      }
    }
    if (bucketHas(21) && rnd(2)) { const k = varOf(211); if (k > 0 && k < 13) transientS16(sets, kemRec(en, k).add(0x10), rnd(2) ? 0 : 1); }
    if (bucketHas(10) && rnd(2)) { const p = planetOfId(101); if (p) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), rnd(2) ? en : 3); }
    flagRec(0x10, [0]);
  } else if (c < 18) {
    // Specials: 10 (a colonisation, by the human mostly), 0xd (research done), 0x13 (a design bought), 0x10.
    const t = pick([10, 10, 0xd, 0x13, 0x10]);
    let args;
    if (t === 10) { const p = rnd(2) && planetOfId(221) ? planetOfId(221) : 1 + rnd(np); args = [10, rnd(4) ? en : 1 + rnd(8), p, 0, 0, 0, 0, 0]; }
    else if (t === 0xd) args = [0xd, en, rnd(2) ? varOf(121) : 1 + rnd(192), 0, 0, 0, 0, 0];
    else if (t === 0x13) { const tn = A.TervCount.add(4 * en).readS32(); let d = 1 + rnd(Math.max(1, tn)); for (let k = 40; k <= tn; k++) if (A.Tervek.add(4 * en).readPointer().add(0x84 * k + 0x80).readS32() === 81 && rnd(2)) d = k; args = [0x13, en, d, 0, 0, 0, 0, 0]; }
    else args = [0x10, pick([51, 231, 7]), 0, 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args });
  } else if (c < 19) {
    // A fleet arrives: 41 at a point, the human's at planet 221, 232 at 231, the human's at 241.
    const own = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en);
    const kind = pick([0, 1, 2, 3]);
    let a;
    if (kind === 0) { const f = fleetOfId(41) || pick(liveFleets()); a = [41, f, 0, 0, 0]; }
    else if (kind === 1) { const f = own.length ? pick(own) : pick(liveFleets()); a = [fleet(f).add(0x16).readS16(), f, planetOfId(221) || 1 + rnd(np), 1, 0]; }
    else if (kind === 2) { const f = fleetOfId(232) || pick(liveFleets()); a = [232, f, fleetOfId(231), 2, 231]; }
    else { const f = own.length ? pick(own) : pick(liveFleets()); a = [fleet(f).add(0x16).readS16(), f, fleetOfId(241), 2, 241]; }
    flagRec(0, a, () => { for (let i = 0; i < 5; i++) A.Arr.add(4 * i).writeS32(a[i]); });
  } else if (c < 20) {
    // A space battle starts with the base's fleets (spFlottak).
    const all = liveFleets();
    if (!all.length) return;
    const ids = [11, 12].map(fleetOfId).filter((f) => f);
    const a = ids.length && rnd(3) ? pick(ids) : pick(all), b = pick(all);
    transientS16(sets, A.SpFlottak, rnd(2) ? a : b); transientS16(sets, A.SpFlottak.add(6), rnd(2) ? b : a);
    flagRec(1, [a, b]);
  } else if (c < 22) {
    // A space battle ends with the stories' fleets among the participants; the winner (SpWarKiNyert) set.
    // A buffer of its own: several staged records can wait for the same frame.
    const n = 2 + rnd(2), list = [], buf = Memory.alloc(0x30 * 4);
    spwBufs.push(buf);
    if (spwBufs.length > 64) spwBufs.shift();
    for (let i = 0; i < n; i++) {
      const id = rnd(3) ? pick(BEGIN_FLEETS) : 0, owner = rnd(2) ? en : pick([10, 13, 9, 3, 1]);
      const p = buf.add(0x30 * i); p.add(0x18).writeS32(owner); p.add(0x20).writeS32(id); p.add(0x24).writeS32(id); list.push([owner, id]);
    }
    transientS32(sets, A.SpWin, rnd(2) ? list[0][0] : list[1][0]);
    // The fleet may be gone already (it lost).
    for (const [, id] of list) { const f = fleetOfId(id); if (f && rnd(2)) transientS16(sets, fleet(f).add(0x16), 0); }
    stage('spwfinished', [[A.SpwFinished, [buf.toUInt32(), n, 1]]], { args: list }, prep, after);
  } else if (c < 23) {
    // A ground battle ends on a story planet (flag 4).
    const p = pick([11, 51, 61, 61].map(planetOfId).filter((x) => x).concat([1 + rnd(np)]));
    const fid = pick([0, 69, 11, 51]), f = fleetOfId(fid), won = rnd(2);
    flagRec(4, [fid, p, won], () => {
      A.Gr.writeS32(f); A.Gr.add(4).writeS32(fid); A.Gr.add(8).writeS32(en); A.Gr.add(12).writeS32(p); A.Gr.add(16).writeS32(A.Bolygok.add(0x188 * p + 0x20).readU8());
      A.GrWin.writeU8(won); A.GrWin.add(1).writeU8(1 - won);
    });
  } else {
    // The radar reaches the base (flag 11), or spy news (flag 15).
    if (rnd(2)) {
      const p = rnd(3) ? planetOfId(11) : 1 + rnd(np);
      flagRec(11, [p], () => A.RadarB.writeS32(p));
    } else {
      const spy = pick([varOf(211), 1 + rnd(6), 0]), mission = pick([4, 8, 8, 3]), target = rnd(3) ? varOf(201) : 1 + rnd(8);
      const techs = []; for (let i = 0; i < 5; i++) techs.push(rnd(4) ? 1 + rnd(192) : 0);
      const tech = rnd(2) ? varOf(203) : 1 + rnd(192), died = rnd(3) === 0 ? 1 : 0, ok = rnd(4) ? 1 : 0;
      for (const t of techs) if (t && rnd(2)) transientU8(sets, A.Techs.add(0x114 * t + 0x45 + en), 1 + rnd(5));
      flagRec(15, [spy, mission, target, tech, died, ok].concat(techs), () => {
        A.KemTipus.writeS32(3);
        const k = A.KemNews;
        k.writeS32(1); k.add(4).writeS32(spy); k.add(8).writeS32(died); k.add(0xc).writeU8(ok);
        k.add(0x14).writeS32(mission); k.add(0x18).writeS32(target); k.add(0x1c).writeS32(tech);
        for (let i = 0; i < 5; i++) k.add(0x24 + 4 * i).writeS32(techs[i]);
      });
    }
  }
}

// ---------------------------------------------------------------- staged calls, st_Rnd_include (--keep rnd)
// The stories of st_Rnd_include the port has (part 1); the script's other stories are kept from running: their
// `st_Story(id, 1)` calls from inside the script get an id no story has (no side effect: not found leaves
// StoryLastChecked alone). Staged: starts (with the world they test set up), counters, answers, promotions,
// daily checks, finished space battles with a pirate (118), specials 0x14/0x15/0x16/0x18/0x19, battle starts.
const RND = [101, 102, 108, 116, 117, 118, 124, 127, 128, 129, 131, 132, 133, 134, 136, 137, 138, 139, 142, 144, 145, 146, 147,
  148, 149, 150, 151, 152, 153, 155, 156, 161, 163, 164, 165, 166, 169, 170, 171, 172, 173, 174, 176, 177, 178, 179, 180, 181, 182,
  183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 199, 242];
const RND_COUNTERS = { 144: [0x5a1, 0x5a2], 146: [0x5bd, 0x5b5], 149: [0x5d3], 170: [0x6a5], 171: [0x6af, 0x6b0],
  172: [0x6b9, 0x6ba], 173: [0x6c3, 0x6c4], 242: [0x975],
  127: [0x4f7], 128: [0x509, 0x501, 0x502], 129: [0x50f, 0x50b, 0x50c], 133: [0x533, 0x534, 0x53b], 136: [0x554, 0x551, 0x552],
  139: [0x56f], 145: [0x5ab, 0x5ac, 0x5ad], 147: [0x5bf], 148: [0x5c9], 153: [0x5fb], 161: [0x64b], 165: [0x673],
  101: [0x3fb, 0x3f3], 102: [0x405, 0x3fd], 108: [0x441], 124: [0x4e1], 131: [0x51f] };
const RND_MESSAGES = { 170: [[0x6a5, [2, 3, 4]], [0x6a6, [2, 4]], [0x125d, [2, 3]], [0x125e, [2, 4]]],
  171: [[0x6af, [2, 3, 4]], [0x6b0, [2, 3]]], 172: [[0x6b9, [2, 3, 4]], [0x6ba, [3, 5, 6]]],
  173: [[0x6c3, [4, 5, 6]], [0x6c4, [2, 3]]], 242: [[0x975, [2, 2, 3]]],
  116: [[0x489, [2, 3]], [0x425, [1]]], 117: [[0x493, [2, 3]]], 127: [[0x4f7, [1]]], 128: [[0x501, [2, 3, 4]]],
  129: [[0x50b, [2, 3]]], 132: [[0x529, [2, 3]]], 133: [[0x533, [2, 3, 4]]],
  136: [[0x551, [2, 3]], [0x552, [2, 3, 4]], [0x553, [2, 4, 5, 6]]], 139: [[0x570, [2, 3, 4]]], 145: [[0x5ab, [2, 3, 4]]],
  147: [[0x5bf, [2, 3]], [0x5c7, [2, 3]]], 148: [[0x5c9, [2, 3]]], 101: [[0x3f3, [2, 3]]], 102: [[0x3fd, [2, 3]]] };
const RND_RARE = [118, 149, 170, 242, 101, 102, 108, 124, 131, 151, 127, 133, 136, 145, 147, 148];
// st_Rnd part 2: the stories' planet ids (arrivals, radar contacts) and fleet ids (battles, arrivals).
const RND_PLANETS = { 128: [0x501], 129: [0x50b], 132: [0x529], 133: [0x533], 134: [0x53d], 136: [0x551, 0x552], 137: [0x55b],
  139: [0x56f], 145: [0x5ab], 101: [0x3f3, 0x3f4], 102: [0x3fd, 0x3fe], 131: [0x51f, 0x520], 151: [0x5e7] };
const RND_FLEETS = { 133: [0x534, 0x535], 136: [0x552], 145: [0x5ab], 147: [0x5bf], 148: [0x5c9], 101: [0x3f3, 0x3f4],
  102: [0x3fd, 0x3fe], 108: [0x439, 0x43a], 124: [0x4d9, 0x4da], 131: [0x51f, 0x520], 151: [0x5e7] };
// The part-1 stories (part 2's staging leaves them to rndFuzz).
const RND_ONE = [118, 142, 144, 146, 149, 150, 163, 164, 169, 170, 171, 172, 173, 199, 242];
function setupRndFilter() {
  const lo = va(0x5f0e20), hi = va(0x601580);
  Interceptor.attach(A.StoryAt, {
    onEnter(args) {
      const ra = this.returnAddress;
      if (ra.compare(lo) < 0 || ra.compare(hi) >= 0 || args[1].toInt32() !== 1) return;
      if (!RND.includes(args[0].toInt32())) args[0] = ptr(0x7fffff00);
    },
  });
}
const heroId = (k) => A.RndHosok.add(0x1c * k).readS32();
const heroWord = (k, i) => A.RndHosok.add(0x1c * k + 4 * i).readS32();
function humanPlanets() { const en = A.EN.readS32(), o = []; for (let p = 1; p <= nPlanets(); p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === en) o.push(p); return o; }
function techOf(sets, t, v) { transientU8(sets, A.Techs.add(0x114 * t + 0x45 + A.EN.readS32()), v); }
// A strong fleet: one of the human's ships' design gets a big attack.
function strongFleet(sets) {
  const ships = humanShips(), en = A.EN.readS32();
  if (!ships.length) return;
  const [k] = pick(ships), d = A.Hajok.readPointer().add(6 * k + 2).readS16();
  transientS32(sets, A.Tervek.add(4 * en).readPointer().add(0x84 * d + 0x2c), pick([800, 3000, 9000, 20000, 60000]));
}
// The human's spy slots 1..n for 127's counter: on its mission (14 + k for StoryKuldetes[k] == 0x4f7), at home.
function rndSpies(sets) {
  const en = A.EN.readS32();
  let k = 0; while (k < 12 && A.StoryKuld.add(4 * k).readS32() !== 0x4f7) k++;
  const n = A.KemN.add(4 * en).readS32(), want = rnd(5) ? 1 + rnd(3) : 0;
  let top = 0;
  for (let i = 1, got = 0; i < 6 && got < want; i++) {
    if (rnd(3) === 0) continue;
    const r = kemRec(en, i);
    got++;
    top = i;
    transientS16(sets, r.add(0x10), pick([3, 3, 3, 1])); transientS16(sets, r.add(0x1a), rnd(6) ? 14 + k : 13);
    transientU8(sets, r.add(0x40), pick([0, 1, 2, 2, 3, 5]));
  }
  if (n < top) transientS32(sets, A.KemN.add(4 * en), top + rnd(2));
}
// A human planet with a building of `kind` (152..166): one to three buildings' kind written for the record.
function rndBuilding(sets, kind, extra) {
  const own = humanPlanets().filter((p) => A.HanyEpulet.add(4 * p).readS32() > 0);
  if (!own.length) return;
  const p = pick(own), n = A.HanyEpulet.add(4 * p).readS32(), base = A.Epuletek.add(4 * p).readPointer();
  for (let k = rnd(3) + 1; k > 0; k--) {
    const r = base.add(0x20 * (1 + rnd(n)));
    if (r.readU8() === 0) continue;
    transientU8(sets, r, kind);
    if (extra) extra(r);
  }
}
// st_Rnd part 2's own records: reads (9), radar contacts (10, 11), diplomacy answers (13), spy news (15), arrivals,
// battles, colonisations; starts and counters with the world they test set up.
function rnd2Fuzz(pool, running, sets, prep, after) {
  const en = A.EN.readS32(), np = nPlanets(), c = rnd(12);
  const two = running.filter((x) => !RND_ONE.includes(x));
  const flagRec = (fl, extra, write) => stage('flag', [[A.Check, [1]]], { args: [fl].concat(extra || []) }, () => { prep(); A.Flags.add(fl).writeU8(1); if (write) write(); }, after);
  const levels = () => {
    for (let e = 1; e < 9; e++) {
      if (e === en || rnd(3)) continue;
      transientU8(sets, A.Viszony.add((en * 19 + e) * 0x26 + 0x25), pick([1, 2, 3, 4]));
      if (rnd(2)) transientU8(sets, A.El.add(e), 1);
    }
  };
  const storyPlanets = () => { const o = []; for (const id of two) for (const b of RND_PLANETS[id] || []) o.push(b); return o; };
  const storyFleets = () => { const o = []; for (const id of two) for (const fid of RND_FLEETS[id] || []) o.push(fid); return o; };
  if (c < 2 || !two.length) {
    // A start with the world it tests.
    const ids = pool.filter((id) => !bucketHas(id) && !RND_ONE.includes(id));
    if (!ids.length) return;
    const id = pick(ids);
    if (id === 116 || id === 117) levels();
    if (id === 117) {
      for (let e = 1; e < 9; e++) {
        if (rnd(3)) continue;
        transientU8(sets, A.Ismeri.add(19 * en + e), rnd(2));
        transientU8(sets, A.Viszony.add((en * 19 + e) * 0x26 + 0x24), rnd(3) ? 0 : 1);
      }
    }
    if (id === 153 || id === 161) {
      techOf(sets, id === 153 ? 133 : 134, pick([5, 5, 4]));
      if (rnd(3)) rndBuilding(sets, id === 153 ? 0x35 : 0x36, (r) => transientU8(sets, r.add(0x18), rnd(256)));
    }
    if (id === 155) {
      transientS32(sets, A.HanyB.add(4 * en), pick([5, 6, 12, 39, 40]));
      for (const p of humanPlanets()) if (rnd(2)) transientS32(sets, A.Bolygok.add(0x188 * p + 0x24), pick([50, 100, 101, 400]));
      if (rnd(2)) rndBuilding(sets, 0x13 + rnd(4));
    }
    if (id === 165) rndBuilding(sets, 0x13 + rnd(4), (r) => transientU8(sets, r.add(0x19), rnd(4)));
    if ((id === 101 || id === 102) && rnd(4)) transientS32(sets, A.HanyB.add(4 * en), 3 + rnd(13));
    if (id === 131) {
      techOf(sets, 57, pick([5, 5, 5, 3]));
      if (rnd(4)) transientS32(sets, A.Research.add(4 * en), pick([0, 1 + rnd(192), varOf(0x527) || 7]));
      for (let k = rnd(4); k > 0; k--) rndBuilding(sets, 14 + rnd(3), (r) => transientS16(sets, r.add(8), rnd(4) ? 0 : 1));
    }
    // The nearest-unknown stories (129, 132, 133, 134, 136): some of planets 11001..11015 unknown to the human again.
    if ([129, 132, 133, 134, 136].includes(id)) {
      for (let k = 1; k <= 15; k++) {
        const p = planetOfId(11000 + k);
        if (p && rnd(3) === 0) transientS16(sets, A.Bolygok.add(0x188 * p + 0xd8 + 2 * en), -1);
      }
    }
    if (id === 166 && rnd(2)) rndBuilding(sets, 13, (r) => transientS16(sets, r.add(8), rnd(2)));
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
  } else if (c < 4) {
    // A counter with the world it tests.
    const ids = two.filter((x) => RND_COUNTERS[x]);
    if (!ids.length) return;
    const id = pick(ids), ctr = pick(RND_COUNTERS[id]);
    if (id === 127) rndSpies(sets);
    if (id === 128 && rnd(2)) techOf(sets, varOf(0x501), pick([5, 3]));
    if (id === 153 || id === 161) {
      techOf(sets, id === 153 ? 133 : 134, pick([5, 5, 4]));
      if (rnd(3)) rndBuilding(sets, id === 153 ? 0x35 : 0x36, (r) => transientU8(sets, r.add(0x18), rnd(256)));
    }
    stage('flag', [[A.Check, [1]]], { args: [5, ctr] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(ctr); }, after);
  } else if (c < 5) {
    // A daily check: the levels (116), trade agreements (117), the story planets taken.
    if (two.includes(116) || two.includes(117)) levels();
    if (two.includes(117) && rnd(2)) { const e = varOf(0x493); if (e > 0 && e < 19) transientU8(sets, A.Viszony.add((en * 19 + e) * 0x26 + 0x24), rnd(2)); }
    if (two.includes(116) && rnd(2)) { const e = varOf(0x489); if (e > 0 && e < 19) transientU8(sets, A.Viszony.add((en * 19 + e) * 0x26 + 0x25), pick([1, 2, 3, 4])); }
    for (const b of storyPlanets()) { const p = planetOfId(b); if (p && rnd(3) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([0, en, 3])); }
    flagRec(0x10, [0]);
  } else if (c < 6) {
    // 127: the message read (9) or spy news (15).
    if (!two.includes(127) && rnd(4)) return;
    if (rnd(2)) {
      const b = rnd(4) ? 0x4f7 : 0x4f8;
      flagRec(9, [b], () => A.Msg.writeS32(b));
    } else {
      const id = rnd(3) ? 0x4f7 : 0x4f8, kind = rnd(4) ? 2 : 3;
      transientS32(sets, A.KemValasz, 1 + rnd(3));
      flagRec(15, [kind, id], () => { A.KemTipus.writeS32(kind); A.KemAzon.writeS32(id); });
    }
  } else if (c < 7) {
    // Radar contacts: a story planet (11) or 136's fleet (10).
    const ids = storyPlanets();
    if (!ids.length) return;
    const b = pick(ids);
    if (b === 0x552 && rnd(2)) {
      const f = fleetOfId(0x552);
      flagRec(10, [f], () => A.RadarF.writeS32(f));
    } else {
      const p = planetOfId(b) || 1 + rnd(np);
      if (rnd(3) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([0, en, 3]));
      // 139's other branch: v(1395) == 1 (nothing in the script sets it).
      if (b === 0x56f && rnd(2)) vSet(0x573, 1);
      flagRec(11, [p], () => A.RadarB.writeS32(p));
    }
  } else if (c < 9) {
    // Arrivals: a fleet of the human's at a story planet, a story fleet at a planet.
    const own = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en);
    let a;
    if (rnd(2)) {
      const ids = storyPlanets();
      if (!ids.length || !own.length) return;
      const b = pick(ids), f = rnd(5) ? pick(own) : pick(liveFleets()), p = planetOfId(b) || 1 + rnd(np);
      if (rnd(3) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([0, en, 3]));
      a = [fleet(f).add(0x16).readS16(), f, p, 1, 0];
    } else {
      const ids = storyFleets();
      if (!ids.length) return;
      const fid = pick(ids), f = fleetOfId(fid) || pick(liveFleets()), p = rnd(2) ? planetOfId(fid) || 1 + rnd(np) : 1 + rnd(np);
      if (rnd(3) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([0, en, 3]));
      a = [fid, f, p, 1, 0];
    }
    flagRec(0, a, () => { for (let i = 0; i < 5; i++) A.Arr.add(4 * i).writeS32(a[i]); });
  } else if (c < 10) {
    // A space battle ends with a story fleet and the human.
    const ids = storyFleets();
    if (!ids.length) return;
    const n = 2 + rnd(2), list = [], buf = Memory.alloc(0x30 * 4);
    spwBufs.push(buf);
    if (spwBufs.length > 64) spwBufs.shift();
    for (let i = 0; i < n; i++) {
      const id = i === 0 || rnd(2) ? pick(ids.concat([0x5c9])) : 0, owner = i === 1 || rnd(3) === 0 ? en : pick([10, 3, 9, 1]);
      const q = buf.add(0x30 * i); q.add(0x18).writeS32(owner); q.add(0x20).writeS32(id); q.add(0x24).writeS32(id); list.push([owner, id]);
    }
    transientS32(sets, A.KiNyert, rnd(2) ? list[0][0] : list[1][0]);
    for (const [, id] of list) { const f = fleetOfId(id); if (f && rnd(2)) transientS16(sets, fleet(f).add(0x16), 0); }
    stage('spwfinished', [[A.SpwFinished, [buf.toUInt32(), n, 1]]], { args: list }, prep, after);
  } else if (c < 11 && rnd(4) && two.some((x) => [101, 102, 108, 124, 131, 151].includes(x))) {
    // Part 2b: a battle starting with the escorts (spFlottak), 108's pirate reaching the trader, 124's radar
    // contacts, 151's special 0x12 (data 2: 1 the nearest fleet, 2 a fleet of the human's), 101's trader's ships.
    // The kind follows the running stories.
    const kinds = [];
    if (two.some((x) => [101, 102, 131, 124].includes(x))) kinds.push(0, 0);
    if (two.includes(108)) kinds.push(1, 1);
    if (two.includes(124)) kinds.push(2, 2);
    if (two.includes(151)) kinds.push(3);
    if (two.includes(101) || two.includes(102)) kinds.push(4);
    const k = kinds.length ? pick(kinds) : rnd(5), own = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en);
    if (k === 0) {
      // Both fleets of an escort story (or 124's pirate) as the battle's two, a fleet of the human's near them.
      const pairs = [[0x3f3, 0x3f4], [0x3fd, 0x3fe], [0x51f, 0x520], [0x4d9, 0x4da]].map((q) => q.map(fleetOfId)).filter((q) => q[0]);
      const all = liveFleets();
      if (!all.length) return;
      const pr = pairs.length && rnd(5) ? pick(pairs) : [pick(all), pick(all)];
      const a = pr[0], b = pr[1] || pick(all);
      transientS16(sets, A.SpFlottak, a); transientS16(sets, A.SpFlottak.add(6), b);
      for (let i = 1 + rnd(3); i > 0 && own.length; i--) { const f = pick(own), q = fleet(a); transientU32(sets, fleet(f).add(0x30), ftou(q.add(0x30).readFloat() + rnd(9) - 4)); transientU32(sets, fleet(f).add(0x34), ftou(q.add(0x34).readFloat() + rnd(9) - 4)); }
      transientS32(sets, A.SpFlottakN, rnd(3));
      flagRec(1, [a, b]);
    } else if (k === 1) {
      const f = fleetOfId(0x43a) || pick(liveFleets()), a = [rnd(5) ? 0x43a : 0x439, f, fleetOfId(0x439), 2, rnd(5) ? 0x439 : 0x43a];
      if (own.length && rnd(2)) { const g = pick(own), q = fleet(f); transientU32(sets, fleet(g).add(0x30), ftou(q.add(0x30).readFloat() + rnd(10) - 5)); transientU32(sets, fleet(g).add(0x34), ftou(q.add(0x34).readFloat() + rnd(10) - 5)); }
      flagRec(0, a, () => { for (let i = 0; i < 5; i++) A.Arr.add(4 * i).writeS32(a[i]); });
    } else if (k === 2) {
      const f = pick([fleetOfId(0x4d9), fleetOfId(0x4da), fleetOfId(0x4da), pick(liveFleets())].filter((x) => x));
      if (!f) return;
      flagRec(10, [f], () => A.RadarF.writeS32(f));
    } else if (k === 3) {
      const p = planetOfId(0x5e7) || 1 + rnd(np), n = A.HanyEpulet.add(4 * p).readS32();
      if (n > 0 && rnd(2)) {
        const base = A.Epuletek.add(4 * p).readPointer();
        for (let i = rnd(4); i > 0; i--) { const r = base.add(0x20 * (1 + rnd(n))); if (r.readU8()) { transientU8(sets, r, 0x1c + rnd(4)); transientU8(sets, r.add(0x19), rnd(2)); transientS16(sets, r.add(8), rnd(3) ? 0 : 1); } }
      }
      if (rnd(2)) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([en, en, 3]));
      const d2 = rnd(3) ? 1 : 2, d3 = own.length ? pick(own) : 0;
      if (d3 && rnd(2)) transientS32(sets, fleet(d3).add(0x60), pick([10, 120, 199, 200, 500, 5000]));
      if (rnd(2)) vSet(0x5e7, pick([200, 150, 40, 1]));
      const args = [0x12, d3, d2, d3, 0, 0, 0, 0];
      stage('special', [[A.Special, args]], { args }, prep, after);
    } else {
      const f = fleetOfId(pick([0x3f3, 0x3fd]));
      if (f) transientS32(sets, fleet(f).add(0x54), pick([1, 2, 5]));
      const p = planetOfId(pick([0x3f3, 0x3f4, 0x3fd, 0x3fe])) || 1 + rnd(np), a = [pick([0x3f3, 0x3fd]), f || 1, p, 1, 0];
      flagRec(0, a, () => { for (let i = 0; i < 5; i++) A.Arr.add(4 * i).writeS32(a[i]); });
    }
  } else if (c < 11) {
    // Diplomacy answers (147, 148), colonisations (133, 139).
    if (rnd(2)) {
      const which = pick([0x5bf, 0x5c9, 0x5bf + rnd(3)]), e = 1 + rnd(8);
      flagRec(13, [which, e], () => { A.Dipl.writeS32(which); A.Dipl.add(4).writeS32(e); });
    } else {
      const p = rnd(2) ? planetOfId(0x533) || planetOfId(0x56f) || 1 + rnd(np) : 1 + rnd(np);
      const args = [10, rnd(4) ? en : 1 + rnd(8), p, 0, 0, 0, 0, 0];
      stage('special', [[A.Special, args]], { args }, prep, after);
    }
  } else {
    // An answer or a promotion of part 2's messages.
    const ids = two.filter((x) => RND_MESSAGES[x]);
    if (!ids.length) return;
    const id = pick(ids), [b, parts] = pick(RND_MESSAGES[id]);
    const kind = rnd(4) && b !== 0x425 ? 6 : 7, part = kind === 6 ? pick(parts.concat([1, 5])) : 0;
    if (id === 117 && rnd(2)) { const e = varOf(0x493); if (e > 0 && e < 19) transientU8(sets, A.Viszony.add((en * 19 + e) * 0x26 + 0x24), 0); }
    stage('flag', [[A.Check, [1]]], { args: [kind, b, part] }, () => { prep(); A.Flags.add(kind).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(part); }, after);
  }
}
// st_Rnd part 2c: 138's spy recruits, 174's colony, the heroes 176..192. Staged by each story's state: an offer's
// counter with the world it tests (techs, a strong fleet, money), the hire, the fee's counter and answers, battles
// starting and ending with the hired fleet, arrivals (138's planet, 178's and 184's fleets), ground battles (174,
// 178), specials 0x19 (a hero's design deleted) and 0x10 (190's arrival), 190's radar contact, 174's spy news,
// daily checks, and every message's answers and promotions now and then.
const RND2C = [138, 174, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192];
const RND2C_HEROES = [176, 177, 178, 179, 180, 181, 182, 184];
// The texts the tracer dumps: fleet and design names (`_s(5, id)`), 138's planet name.
const RND2C_TEXTS = [0x565, 0x6e1, 0x6eb, 0x6f5, 0x6ff, 0x709, 0x713, 0x71d, 0x731];
// A hero's story design key (its ships are found by it).
const HERO_KEY = { 176: 0x6e1, 177: 0x6eb, 178: 0x6f5, 179: 0x6ff, 180: 0x709, 181: 0x713, 182: 0x71d, 184: 0x731 };
// The hired fleets' steps: the offer's counter, the offer and the answers that hire, the fee's counter, its
// messages and the variable that says it pays at once.
const HERO = {
  176: { offer: 0x6e9, msg: 0x6e1, hire: [2], fee: 0x6e1, fees: [0x6e2], paid: 0x6e8 },
  177: { offer: 0x6f3, msg: 0x6eb, hire: [2], fee: 0x6eb, fees: [0x6ec, 0x6f1, 0x6ee], paid: 0x6f2 },
  178: { offer: 0x6fd, msg: 0x6f5, hire: [6], fee: 0x6f5, fees: [], paid: 0 },
  179: { offer: 0x707, msg: 0x6ff, hire: [2], fee: 0x6ff, fees: [0x700], paid: 0x706 },
  181: { offer: 0x71b, msg: 0x713, hire: [2, 5], fee: 0x713, fees: [0x714, 0x716], paid: 0x71a },
  182: { offer: 0x725, msg: 0x71d, hire: [2], fee: 0x71d, fees: [0x71e], paid: 0x724 },
  184: { offer: 0x739, msg: 0x731, hire: [2, 3], fee: 0x731, fees: [0x732], paid: 0x738 },
};
const C_TECHS = { 177: [7], 178: [8, 148], 179: [78], 181: [9], 182: [10], 184: [11] };
const C_COUNTERS = { 174: [0x6d0, 0x6d1, 0x6cd], 177: [0x6ec], 178: [0x6f6], 181: [0x715], 183: [0x727, 0x728], 185: [0x73b],
  186: [0x746], 187: [0x750], 188: [0x759, 0x75b, 0x75c, 0x75d, 0x75e], 189: [0x764, 0x763], 190: [0x76f, 0x76d, 0x76e, 0x770],
  192: [0x1fc3] };
const C_MESSAGES = { 138: [[0x566, [2, 2, 3]], [0x567, [2, 2, 3]]], 176: [[0x6e1, [2, 2, 3]], [0x6e2, [2, 3, 4]]],
  177: [[0x6eb, [2, 2, 3]], [0x6ee, [3, 4, 2]], [0x6ec, [2, 3]], [0x6f1, [2, 3, 4]]], 178: [[0x6f5, [6, 6, 3, 5, 7]]],
  179: [[0x6ff, [2, 2, 3, 4]], [0x700, [2, 3, 4]]], 180: [[0x709, [2, 4, 5]]], 181: [[0x713, [2, 5, 4, 6]], [0x714, [2, 3, 4]], [0x716, [2, 3]]],
  182: [[0x71d, [2, 2, 3]], [0x71e, [2, 3, 4]]], 183: [[0x727, [4, 7, 9, 4, 7, 3, 6, 10]], [0x728, [2, 4, 6, 5]], [0x72a, [2, 4, 3]]],
  184: [[0x731, [2, 3, 4]], [0x732, [2, 3, 4]]], 185: [[0x73b, [2, 2, 3]], [0x73c, [2, 3]]], 186: [[0x745, [2, 2, 3]], [0x746, [2, 3]]],
  187: [[0x74f, [2, 2, 3]], [0x750, [2, 3, 4]]], 188: [[0x759, [3, 6, 5, 7, 5, 7]], [0x75a, [2, 6, 3, 5]], [0x75c, [2, 3]]],
  189: [[0x763, [1]]], 190: [[0x76e, [2, 3]], [0x770, [1]]] };
// The human's ships of story design `key` and their fleets.
function heroFleets(key) {
  const en = A.EN.readS32(), out = [], tp = A.Tervek.add(4 * en).readPointer(), tn = A.TervCount.add(4 * en).readS32();
  for (const [k, f] of humanShips()) {
    const d = A.Hajok.readPointer().add(6 * k + 2).readS16();
    if (d >= 0 && d <= tn && tp.add(0x84 * d + 0x80).readS32() === key && !out.includes(f)) out.push(f);
  }
  return out;
}
function rnd2cFuzz(pool, running, sets, prep, after) {
  const en = A.EN.readS32(), np = nPlanets();
  const ids2c = running.filter((x) => RND2C.includes(x));
  const flagRec = (fl, extra, write) => stage('flag', [[A.Check, [1]]], { args: [fl].concat(extra || []) }, () => { prep(); A.Flags.add(fl).writeU8(1); if (write) write(); }, after);
  const counter = (ctr) => stage('flag', [[A.Check, [1]]], { args: [5, ctr] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(ctr); }, after);
  const answer = (b, part) => stage('flag', [[A.Check, [1]]], { args: [6, b, part] }, () => { prep(); A.Flags.add(6).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(part); }, after);
  const promote = (b) => stage('flag', [[A.Check, [1]]], { args: [7, b, 0] }, () => { prep(); A.Flags.add(7).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(0); }, after);
  const own = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en);
  // The world an offer tests: its techs, a strong fleet, money (181); mostly met.
  const offerWorld = (id, met) => {
    for (const t of C_TECHS[id] || []) techOf(sets, t, met ? 5 : pick([5, 3]));
    if (met || rnd(2)) strongFleet(sets);
    if (id === 181) { const m = A.Money.add(8 * en); let was = 0; const x = met ? 300000 : pick([50000, 50001, 1000]); sets.push([() => { was = m.readDouble(); m.writeDouble(x); }, () => m.writeDouble(was)]); }
  };
  const knowAll = () => { for (let e = 1; e < 9; e++) if (e !== en && rnd(4)) transientU8(sets, A.Ismeri.add(19 * en + e), 1); };
  const c = rnd(20);
  if (c < 3 || !ids2c.length) {
    // A start with the world it tests.
    const ids = pool.filter((id) => RND2C.includes(id) && !bucketHas(id));
    if (!ids.length) return;
    const id = pick(ids);
    offerWorld(id, rnd(4) !== 0);
    if (id === 138) for (let k = 1; k <= 15; k++) { const p = planetOfId(11000 + k); if (p && rnd(3) === 0) transientS16(sets, A.Bolygok.add(0x188 * p + 0xd8 + 2 * en), -1); }
    if (id === 183 || id === 186 || id === 190) knowAll();
    if (id === 191) for (const p of humanPlanets()) if (rnd(2)) transientU8(sets, A.Bolygok.add(0x188 * p + 0x22), pick([1, 2, 5, 3, 4, 7]));
    if (id === 192 && rnd(4)) vSet(0x781, rnd(4) ? 1 : 0);
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
    return;
  }
  const id = pick(ids2c), h = HERO[id];
  if (h && c < 14) {
    // A hired fleet's next step.
    const hired = id === 178 ? varOf(0x6fc) !== 0 || varOf(0x6fd) !== 0 : id === 184 ? heroFleets(0x731).length > 0 : varOf(id === 181 ? 0x71b : h.msg) !== 0;
    if (!hired) {
      if (rnd(2)) { offerWorld(id, rnd(5) !== 0); counter(h.offer); } else answer(h.msg, pick(h.hire));
      return;
    }
    const k = rnd(10);
    if (k < 3) {
      if (h.paid && rnd(3) === 0) vSet(h.paid, rnd(2));
      if (rnd(5) === 0) { const f = pick(heroFleets(HERO_KEY[id]).concat([0])); if (f) transientU8(sets, fleet(f).add(0x15), 0xff); }
      counter(h.fee);
    } else if (k < 6 && h.fees.length) {
      const b = pick(h.fees);
      if (rnd(4)) answer(b, pick([2, 2, 3, 4])); else promote(b);
    } else if (k < 8) {
      // A battle starts with it (or 181's ship, the slips of 182 and 184).
      const fs = heroFleets(HERO_KEY[id]).concat(heroFleets(0x713)), all = liveFleets();
      if (!all.length) return;
      const a = fs.length && rnd(5) ? pick(fs) : pick(all), b = pick(all);
      transientS16(sets, A.SpFlottak, rnd(2) ? a : b); transientS16(sets, A.SpFlottak.add(6), rnd(2) ? b : a);
      flagRec(1, [a, b]);
    } else {
      // A battle ends with it (its slot at +0x20; 178's id at +0x24).
      const n = 2 + rnd(2), list = [], buf = Memory.alloc(0x30 * 4);
      spwBufs.push(buf);
      if (spwBufs.length > 64) spwBufs.shift();
      const fs = heroFleets(HERO_KEY[id]);
      for (let i = 0; i < n; i++) {
        const fid = id === 178 && rnd(2) ? 0x6f5 : 0, owner = i === 1 || rnd(3) === 0 ? en : pick([10, 3, 9]);
        const slot = fs.length && rnd(3) ? pick(fs) : fid ? fleetOfId(fid) : rnd(nFleets() + 1);
        const q = buf.add(0x30 * i); q.add(0x18).writeS32(owner); q.add(0x20).writeS32(slot); q.add(0x24).writeS32(fid); list.push([owner, slot, fid]);
      }
      transientS32(sets, A.KiNyert, rnd(2) ? list[0][0] : list[1][0]);
      stage('spwfinished', [[A.SpwFinished, [buf.toUInt32(), n, 1]]], { args: list }, prep, after);
    }
    return;
  }
  if (c < 9) {
    // An answer or a promotion of the story's messages, or one of its counters.
    if (C_COUNTERS[id] && rnd(2)) { if (id === 177 || id === 178) offerWorld(id, true); counter(pick(C_COUNTERS[id])); return; }
    const ms = C_MESSAGES[id];
    if (!ms) return;
    const [b, parts] = pick(ms);
    if (id === 189 && rnd(2)) { flagRec(9, [b], () => A.Msg.writeS32(b)); return; }
    if (id === 138 && rnd(2)) {
      // One recruit of the human's alive (138's one-offer branch) or two.
      const n = A.KemN.add(4 * en).readS32();
      for (let i = 1; i <= Math.min(n, 2); i++) if (rnd(2)) { const r = kemRec(en, i); transientU8(sets, r.add(0x12), 14); transientS16(sets, r.add(0x10), 1); }
    }
    if (rnd(5)) answer(b, pick(parts)); else promote(b);
  } else if (c < 11) {
    // 138's planet reached by a fleet of the human's (one or two recruits alive), 184's fleet at its target.
    if (id === 184 || (rnd(4) === 0 && bucketHas(184))) {
      const f = fleetOfId(0x732) || pick(liveFleets()), p = planetOfId(0x731) || 1 + rnd(np);
      if (!f) return;
      const a = [0x732, f, p, 1, 0];
      flagRec(0, a, () => { for (let i = 0; i < 5; i++) A.Arr.add(4 * i).writeS32(a[i]); });
      return;
    }
    const p = planetOfId(0x565) || 1 + rnd(np), f = own.length && rnd(5) ? pick(own) : pick(liveFleets());
    if (!f) return;
    // The recruits alive: exactly 0, 1 or 2 (138's three offers) now and then.
    const n = A.KemN.add(4 * en).readS32();
    if (rnd(3)) {
      let want = rnd(3);
      for (let i = 0; i <= Math.min(n, 12); i++) {
        const r = kemRec(en, i);
        if (want > 0 && i > 0) { transientU8(sets, r.add(0x12), 14); transientS16(sets, r.add(0x10), 1); want--; } else if (r.add(0x12).readU8() === 14) transientU8(sets, r.add(0x12), 1);
      }
    }
    const a = [fleet(f).add(0x16).readS16(), f, p, 1, 0];
    flagRec(0, a, () => { for (let i = 0; i < 5; i++) A.Arr.add(4 * i).writeS32(a[i]); });
  } else if (c < 12) {
    // A daily check: 138's planet taken, 178's fleet idle.
    const p = planetOfId(0x565);
    if (p && rnd(3) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([0, en, 3]));
    for (const fid of [0x6f5, 0x732]) { const f = fleetOfId(fid); if (f && rnd(2)) transientS16(sets, fleet(f).add(0x40), 0); }
    flagRec(0x10, [0]);
  } else if (c < 14) {
    // 174: spy news (15), a ground battle on its planet (4), its planet lost.
    if (rnd(3)) {
      const n = A.KemN.add(4 * en).readS32(), slot = n > 0 ? 1 + rnd(n) : 0, r = kemRec(en, slot);
      for (const o of [0x31, 0x32, 0x33]) if (rnd(2)) transientU8(sets, r.add(o), rnd(11));
      let k = 0; while (k < 12 && A.StoryKuld.add(4 * k).readS32() !== 0x6cd) k++;
      if (k < 12 && rnd(4) === 0) transientU8(sets, A.KemKuld.add(0x20 * (14 + k)), rnd(2));
      const kind = rnd(5) ? 2 : 3, sid = rnd(5) ? 0x6cd : 0x4f7;
      flagRec(15, [kind, sid, slot], () => { A.KemTipus.writeS32(kind); A.KemAzon.writeS32(sid); A.KemTulaj.writeS32(en); A.KemAktKem.writeS32(slot); A.KemValasz.writeS32(-1); });
    } else {
      const won = rnd(2), p = rnd(3) ? planetOfId(0x6cd) || 1 + rnd(np) : 1 + rnd(np);
      const hf = heroFleets(0x6f5), f = hf.length && rnd(2) ? pick(hf) : own.length ? pick(own) : 0;
      flagRec(4, [f, p, won], () => {
        A.Gr.writeS32(f); A.Gr.add(4).writeS32(f ? fleet(f).add(0x16).readS16() : 0); A.Gr.add(8).writeS32(en); A.Gr.add(12).writeS32(p);
        A.Gr.add(16).writeS32(A.Bolygok.add(0x188 * p + 0x20).readU8()); A.GrWin.writeU8(won); A.GrWin.add(1).writeU8(1 - won);
      });
    }
  } else if (c < 16) {
    // 190: its spy ship in a battle starting with the human, the battle's end, its arrival (0x10), radar (10).
    const spy = fleetOfId(0x76d), k = rnd(4), all = liveFleets();
    if (!all.length) return;
    if (k === 0) {
      const a = spy || pick(all), b = own.length ? pick(own) : pick(all);
      transientS16(sets, A.SpFlottak, rnd(2) ? a : b); transientS16(sets, A.SpFlottak.add(6), rnd(2) ? b : a);
      transientS32(sets, A.SpFlottakN, rnd(3));
      flagRec(1, [a, b]);
    } else if (k === 1) {
      const n = 2 + rnd(2), list = [], buf = Memory.alloc(0x30 * 4);
      spwBufs.push(buf);
      if (spwBufs.length > 64) spwBufs.shift();
      for (let i = 0; i < n; i++) {
        const fid = i === 0 || rnd(2) ? pick([0x76d, 0x76d, 0x76e, 0]) : 0, owner = i === 1 || rnd(3) === 0 ? en : pick([10, 9, 3]);
        const q = buf.add(0x30 * i); q.add(0x18).writeS32(owner); q.add(0x20).writeS32(fleetOfId(fid)); q.add(0x24).writeS32(fid); list.push([owner, fleetOfId(fid), fid]);
      }
      for (const fid of [0x76d, 0x76e]) { const f = fleetOfId(fid); if (f && rnd(3) === 0) transientS16(sets, fleet(f).add(0x16), 0); }
      transientS32(sets, A.KiNyert, rnd(2) ? list[0][0] : list[1][0]);
      stage('spwfinished', [[A.SpwFinished, [buf.toUInt32(), n, 1]]], { args: list }, prep, after);
    } else if (k === 2) {
      if (rnd(3) === 0) vSet(0x76d, rnd(2));
      const args = [0x10, rnd(5) ? 0x76d : 0, 0, 0, 0, 0, 0, 0];
      stage('special', [[A.Special, args]], { args }, prep, after);
    } else {
      const f = spy || pick(all);
      flagRec(10, [f], () => A.RadarF.writeS32(f));
    }
  } else {
    // Special 0x19: a design of the human's with a hero's key deleted.
    if (!own.length) return;
    const tn = A.TervCount.add(4 * en).readS32(), tp = A.Tervek.add(4 * en).readPointer();
    if (tn < 1) return;
    const keyed = []; for (let d = 1; d <= tn; d++) { const k = tp.add(0x84 * d + 0x80).readS32(); if (Object.values(HERO_KEY).includes(k)) keyed.push(d); }
    const d = keyed.length && rnd(4) ? pick(keyed) : 1 + rnd(tn);
    if (!keyed.includes(d) && rnd(2)) transientS32(sets, tp.add(0x84 * d + 0x80), HERO_KEY[pick(RND2C_HEROES)]);
    const args = [0x19, pick(own), d, 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args }, prep, after);
  }
}
function rndFuzz() {
  const pool = cfg.inline && cfg.inline.length ? RND.filter((x) => cfg.inline.includes(x)) : RND;
  const c = rnd(20), en = A.EN.readS32(), np = nPlanets(), running = pool.filter(bucketHas);
  // A wormhole for story 142 (the campaign maps start without one): made once, natively, not recorded.
  if (!wormholeMade && A.WormholeN.readS32() === 0 && np > 2) {
    wormholeMade = true;
    const a = A.Bolygok.add(0x188 * (1 + rnd(np))), b = A.Bolygok.add(0x188 * (1 + rnd(np)));
    call(va(0x53aec0), [a.add(0x18).readFloat() | 0, a.add(0x1c).readFloat() | 0, b.add(0x18).readFloat() | 0, b.add(0x1c).readFloat() | 0], null, false);
    return;
  }
  const sets = [];
  const prep = () => sets.forEach((x) => x[0]()), after = () => sets.slice().reverse().forEach((x) => x[1]());
  const pickR = (ids) => { const r = ids.filter((x) => RND_RARE.includes(x)); return r.length && rnd(3) ? pick(r) : pick(ids); };
  if (pool.some((x) => RND2C.includes(x)) && rnd(5) < 4) { rnd2cFuzz(pool, running, sets, prep, after); return; }
  if (pool.some((x) => !RND_ONE.includes(x) && !RND2C.includes(x)) && rnd(3)) { rnd2Fuzz(pool.filter((x) => !RND2C.includes(x)), running, sets, prep, after); return; }
  const wormholeEnds = () => {
    for (let k = 1; k <= A.WormholeN.readS32(); k++) {
      const q = A.Wormholes.add(0x1c * k), m = rnd(4);
      transientU8(sets, q.add(8), m & 1); transientU8(sets, q.add(0x14), m >> 1);
    }
  };
  const morale = (n) => { for (let k = rnd(n); k > 0; k--) transientS32(sets, A.Bolygok.add(0x188 * (1 + rnd(np)) + 0x30), pick([100, 1500, 1999, 2000, 2500, 3000, 3001, 3500, 349, 350, 900])); };
  if (c < 3 || !running.length) {
    const ids = pool.filter((id) => !bucketHas(id));
    if (!ids.length) return;
    const id = pickR(ids);
    if (rnd(2)) transientS32(sets, A.HanyB.add(4 * en), pick([3, 7, 8, 9, 12, 40]));
    if (id === 142 && rnd(2)) wormholeEnds();
    if (id === 163) for (let k = rnd(6); k > 0; k--) techOf(sets, 1 + rnd(192), pick([4, 4, 3, 5]));
    if (id === 164) { const own = humanPlanets(); if (own.length && rnd(2)) transientS16(sets, A.Bolygok.add(0x188 * pick(own) + 0x16a), 0x60f); }
    if (id === 149) for (const p of humanPlanets()) if (rnd(4) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x22), 7);
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
  } else if (c < 5) {
    // A counter runs out at the next tick (a hired hero's fee counter now and then).
    const id = pickR(running);
    let list = RND_COUNTERS[id];
    if (id === 242 && rnd(2)) list = [heroId(rnd(9))];
    if (list) szStart(pick(list), rnd(2));
  } else if (c < 9) {
    // A staged counter with the world it tests set up.
    const id = pickR(running);
    let ctr = RND_COUNTERS[id] ? pick(RND_COUNTERS[id]) : 0;
    if (id === 144 || id === 146) morale(8);
    if (id === 146) for (const p of humanPlanets()) if (rnd(2)) transientS16(sets, A.Bolygok.add(0x188 * p + 0xd8 + 2 * en), pick([0, 1, -1]));
    if (id === 149) for (let k = rnd(4); k > 0; k--) transientS16(sets, A.Bolygok.add(0x188 * (1 + rnd(np)) + 0x16a + 2 * rnd(5)), pick([0x5d3, 0]));
    if (id === 170 && rnd(2)) techOf(sets, varOf(0x6a5), pick([5, 3]));
    if (id === 242) {
      if (rnd(3)) { strongFleet(sets); for (let k = 0; k < 9; k++) if (rnd(3) === 0) techOf(sets, heroWord(k, 1) + pick([3, 7]), pick([5, 1])); }
      // A hired hero's yearly fee.
      if (rnd(3) === 0) ctr = heroId(rnd(9));
    }
    if (!ctr) return;
    stage('flag', [[A.Check, [1]]], { args: [5, ctr] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(ctr); }, after);
  } else if (c < 13) {
    // An answer (flag 6) or a promotion (7).
    const id = pickR(running), ms = RND_MESSAGES[id];
    if (!ms) return;
    const [b, parts] = pick(ms), kind = rnd(4) ? 6 : 7, part = kind === 6 ? pick(parts.concat([1, 5])) : 0;
    stage('flag', [[A.Check, [1]]], { args: [kind, b, part] }, () => { prep(); A.Flags.add(kind).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(part); }, after);
  } else if (c < 15) {
    // A daily check: 142's wormholes, 144's planet owner, 170's tech done.
    if (bucketHas(142) && rnd(2)) wormholeEnds();
    if (bucketHas(170) && rnd(2)) techOf(sets, varOf(0x6a5), pick([5, 2]));
    stage('flag', [[A.Check, [1]]], { args: [0x10, 0] }, () => { prep(); A.Flags.add(0x10).writeU8(1); }, after);
  } else if (c < 16) {
    // 118: a space battle ends with the human and a pirate (owner 9) among the participants.
    const n = 2 + rnd(2), list = [];
    const owners = [en, 9, pick([en, 9, 3, 0])];
    if (rnd(2)) owners.reverse();
    for (let i = 0; i < n; i++) {
      const owner = rnd(6) ? owners[i] : rnd(19), id = pick([10000, 10500, 11000, 11001, 9999, 10000 + rnd(1001), rnd(5000), varOf(0xbcd)]);
      const p = fake.add(0x30 * i); p.add(0x18).writeS32(owner); p.add(0x20).writeS32(id); p.add(0x24).writeS32(id); list.push([owner, id]);
    }
    transientS32(sets, A.KiNyert, rnd(4) ? en : pick([9, 0, 3]));
    stage('spwfinished', [[A.SpwFinished, [fake.toUInt32(), n, 1]]], { args: list }, prep, after);
  } else if (c < 18) {
    // Specials: 0x14 (142), 0x15/0x16 (144), 0x18 (199).
    const t = pick([0x14, 0x14, 0x15, 0x16, 0x18, 0x18]);
    const d1 = t === 0x15 || t === 0x16 ? 1 + rnd(np) : rnd(4) ? en : rnd(9);
    if (t === 0x18 && rnd(2)) vSet(0x7c7, pick([3, 1, 0]));
    const args = [t, d1, rnd(3), 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args }, prep, after);
  } else if (c < 19) {
    // Special 0x19 for a fleet of the human's and one of its designs (a hero's design now and then).
    const own = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en), tn = A.TervCount.add(4 * en).readS32();
    if (!own.length || tn < 1) return;
    const d = 1 + rnd(tn);
    if (rnd(2)) transientS32(sets, A.Tervek.add(4 * en).readPointer().add(0x84 * d + 0x80), rnd(4) ? heroId(rnd(9)) : pick([2200, 0]));
    const args = [0x19, pick(own), d, 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args }, prep, after);
  } else {
    // A space battle starts: spFlottak holds a fleet of the human's (a hero's mostly).
    const own = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en), all = liveFleets();
    if (!all.length) return;
    const a = own.length && rnd(3) ? pick(own) : pick(all), b = pick(all);
    transientS16(sets, A.SpFlottak, rnd(2) ? a : b); transientS16(sets, A.SpFlottak.add(6), rnd(2) ? b : a);
    stage('flag', [[A.Check, [1]]], { args: [1, a, b] }, () => { prep(); A.Flags.add(1).writeU8(1); }, after);
  }
}

// ---------------------------------------------------------------- staged calls, st_Elem_include (--keep elem)
// The storyline episodes of part 1 (805..868) started, their counters run out with the world they test set up,
// their messages read (flag 9), answered (6) or promoted (7), ground battles on their planets (4), space battles
// with the raider (3), radar contacts (11), arrivals (0), specials 10 and 13, story 816's finish (12), daily checks.
const ELEM = [801, 802, 804, 805, 806, 807, 808, 809, 810, 811, 813, 814, 815, 816, 817, 818, 820, 821, 824, 830, 831, 832, 834, 835, 836, 837, 839, 840, 841, 842, 843, 844, 850, 852, 854, 856, 857, 858, 860, 861, 862, 863, 864, 865, 866, 867, 868];
const ELEM_COUNTERS = { 811: [8110], 813: [8131], 816: [8183, 8189, 5619], 817: [8179], 820: [8201, 8209], 830: [8302, 8301],
  831: [8312, 8311], 835: [8351, 8350], 837: [8370], 841: [8411], 842: [8429], 864: [8640, 8642, 8643], 867: [8671] };
// Planet ids the episodes read: given to some other planet when the campaign map lacks them.
const ELEM_PLANETS = [8160, 8200, 8301, 8302, 8011, 8010, 8040, 8041, 8042, 8091, 8092, 8093, 8094, 8140, 8150, 8151, 8152, 8320,
  8321, 8322, 8340, 8560, 8044];
// The multi-step episodes: picked more often.
const ELEM_RARE = [816, 820, 830, 831, 835, 842, 864, 866, 802, 824, 850, 852, 861];
// Part 2a: the spy episodes.
const ELEM2A = [802, 824, 850, 852, 861];
const ELEM2A_COUNTERS = { 802: [8020, 8021, 8022, 8023, 8025, 8026, 8029], 824: [8241, 8243, 8244], 850: [8110, 5313, 5311, 8507, 5314],
  852: [8100, 8520, 6705, 6706], 861: [8611, 8612] };
const ELEM2A_ANSWERS = { 802: [[6, 8020, 2], [6, 9020, 2], [7, 8020, 0], [7, 9020, 0]],
  824: [[7, 8241, 0], [6, 8241, 4], [7, 8242, 0], [6, 8242, 4], [6, 8241, 3], [6, 8242, 3], [6, 8251, 2], [6, 8251, 4], [6, 8251, 5],
    [6, 8252, 2], [6, 8252, 4], [6, 8252, 5]],
  850: [[6, 5310, 2], [6, 5310, 3], [7, 5310, 0], [6, 5311, 2], [6, 5311, 3], [7, 5311, 0], [6, 8112, 2], [6, 8112, 3], [7, 8112, 0], [6, 5320, 2]],
  852: [[6, 6701, 2], [6, 6703, 2]],
  861: [[6, 8611, 2], [6, 8611, 3], [6, 8611, 4], [6, 8611, 5], [7, 8611, 0], [6, 8613, 2], [6, 8613, 3], [7, 8613, 0]] };
const ELEM2A_MISSIONS = { 802: [8021], 824: [8241, 8242, 8243], 850: [8501], 852: [8521], 861: [8611] };
function aliveEmpires(en) { const o = []; for (let e = 1; e < 9; e++) if (e !== en && A.El.add(e).readU8()) o.push(e); return o; }
// Part 2a's staging: starts with the world they read, counters, answers, the spy news of their missions (kind 1 = caught?,
// 2 = success; the spy's skills and the mission switch), ground battles starting and ending on 8021/8241/the human's
// planets, 802's fleet in battles, arriving and special 0x10, tech 183 (special 13, daily), daily checks.
function elem2aFuzz(pool, running, sets, prep, after) {
  const en = A.EN.readS32(), np = nPlanets(), own = humanPlanets(), live = running.filter((x) => ELEM2A.includes(x));
  const flagRec = (fl, extra, write) => stage('flag', [[A.Check, [1]]], { args: [fl].concat(extra || []) }, () => { prep(); A.Flags.add(fl).writeU8(1); if (write) write(); }, after);
  const c = rnd(22);
  if (c < 4 || !live.length) {
    const ids = pool.filter((id) => ELEM2A.includes(id) && !bucketHas(id));
    if (!ids.length) return;
    const id = pick(ids), al = aliveEmpires(en);
    if (id === 824) { vSet(5208, al.length && rnd(6) ? pick(al) : rnd(9)); vSet(8082, rnd(2)); }
    if (id === 852) vSet(8521, al.length ? pick(al) : 3);
    if (id === 850 && rnd(2)) vSet(8001, pick([0, 2]));
    if (id === 861) for (const e of [1, 5]) if (e !== en) transientU8(sets, A.El.add(e), rnd(5) ? 1 : 0);
    if (id === 802 && rnd(3) === 0) { const p = own.length ? pick(own) : 1 + rnd(np); bAddAzon(p, 8020); }
    // 850 needs three planets of the human's (st_bGet3Nearest): some others become its own for the record.
    if (id === 850 && own.length < 3 && rnd(4)) {
      for (let k = 3 + rnd(3); k > 0; k--) { const q = 1 + rnd(np - 1); transientU8(sets, A.Bolygok.add(0x188 * q + 0x20), en); transientS16(sets, A.Bolygok.add(0x188 * q + 0xd8 + 2 * en), 1); }
      transientS32(sets, A.HanyB.add(4 * en), 3 + rnd(3));
    }
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
    return;
  }
  const id = pick(live);
  if (c < 8) {
    const ctr = pick(ELEM2A_COUNTERS[id]);
    if (ctr === 8611) for (const e of [1, 5]) if (e !== en && rnd(4) === 0) transientU8(sets, A.El.add(e), 0);
    if (ctr === 8244 || ctr === 8243) {
      const e = varOf(5208);
      if (e > 0 && e < 19 && rnd(3) === 0) transientU8(sets, A.Viszony.add((en * 19 + e) * 0x26 + 0x25), pick([1, 3, 5]));
      if (e > 0 && e < 9 && rnd(4) === 0) transientU8(sets, A.El.add(e), 0);
    }
    if (ctr === 8520 || ctr === 5313) { const n = A.KemN.add(4 * en).readS32(); for (let k = 1; k <= n; k++) if (rnd(3) === 0) transientS16(sets, kemRec(en, k).add(0x10), pick([1, 1, 2, 3])); }
    if (ctr === 8110 && rnd(2)) transientS32(sets, A.Adat.add(0x2c), rnd(3) ? rnd(100000) : 0);
    if (ctr === 8110 && rnd(3) === 0) vSet(8110, pick([10, 11, 82, 83, rnd(20)]));
    if (ctr === 8020 && rnd(2)) vSet(8022, 0);
    stage('flag', [[A.Check, [1]]], { args: [5, ctr] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(ctr); }, after);
  } else if (c < 12) {
    // 824's refusal (answer 3: the spy missions) more often than its other answers.
    const [kind, b, part] = id === 824 && rnd(2) ? pick([[6, 8241, 3], [6, 8242, 3]]) : pick(ELEM2A_ANSWERS[id]);
    if (b === 8613) for (const e of [1, 5]) if (rnd(3) === 0) transientU8(sets, A.Viszony.add((en * 19 + e) * 0x26 + 0x25), 1 + rnd(5));
    if (b === 6701 && rnd(4) === 0) vSet(8522, 0);
    if (b === 5320 && rnd(4) === 0) vSet(8502, 0);
    stage('flag', [[A.Check, [1]]], { args: [kind, b, part] }, () => { prep(); A.Flags.add(kind).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(part); }, after);
  } else if (c < 16) {
    // Spy news for a mission of the story's (or another's).
    const n = A.KemN.add(4 * en).readS32(), slot = n > 0 ? 1 + rnd(n) : 0, r = kemRec(en, slot);
    for (const o of [0x31, 0x32, 0x33]) if (rnd(2)) transientU8(sets, r.add(o), rnd(11));
    const sid = rnd(6) ? pick(ELEM2A_MISSIONS[id]) : pick([8021, 8241, 8242, 8243, 8501, 8521, 8611]);
    let k = 0; while (k < 12 && A.StoryKuld.add(4 * k).readS32() !== sid) k++;
    // The mission's switch: mostly on (the success paths need it), sometimes off.
    if (k < 12) { if (rnd(5) === 0) transientU8(sets, A.KemKuld.add(0x20 * (14 + k)), 0); else if (rnd(3)) transientU8(sets, A.KemKuld.add(0x20 * (14 + k)), 1); }
    const kind = rnd(3) ? 2 : 1;
    if (id === 850 && rnd(2)) vSet(8056, rnd(2));
    // 802's success searches the nearest empire's planet (st_GetNearestBirZachkinhoz): more empires' planets and the
    // human's knowledge of them, for the record.
    if (sid === 8021 && rnd(4)) {
      for (let j = 4 + rnd(10); j > 0; j--) transientU8(sets, A.Bolygok.add(0x188 * (1 + rnd(np)) + 0x20), pick([2, 4, 6, 7, 8, 3, 1, 5, 0, 9]));
      for (let e = 1; e < 9; e++) if (e !== en && rnd(3) === 0) transientU8(sets, A.Ismeri.add(19 * en + e), rnd(2));
    }
    flagRec(15, [kind, sid, slot], () => { A.KemTipus.writeS32(kind); A.KemAzon.writeS32(sid); A.KemTulaj.writeS32(en); A.KemAktKem.writeS32(slot); A.KemValasz.writeS32(-1); });
  } else if (c < 18) {
    // A ground battle starts (2) or ends (4): on 8021, 8241 or a planet of the human's.
    const ids = (id === 824 ? [8241] : id === 802 ? [8021] : [8021, 8241]).map(planetOfId).filter((p) => p);
    const p = ids.length && (id !== 850 || rnd(4) === 0) && rnd(4) ? pick(ids) : own.length && rnd(3) ? pick(own) : 1 + rnd(np);
    const fl = rnd(2) ? 2 : 4, owner = rnd(4) ? en : pick([3, 10, 0]), won = rnd(2);
    if (fl === 4 && rnd(2)) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([en, 0, varOf(5208) & 0xff, varOf(8029) & 0xff]));
    if (id === 802 && rnd(2)) szStart(8022, rnd(3) ? 10 : 0);
    if (id === 850 && rnd(2)) vSet(8055, 1);
    // 850's madness needs units of the human's on the battlefield: the exe's TankNew(−p, 1, n) first (not recorded).
    if (id === 850 && fl === 2 && rnd(3) === 0 && A.Bolygok.add(0x188 * p + 0x20).readU8() === en) call(va(0x541750), [-p, 1, 2 + rnd(4)], null, false);
    flagRec(fl, [p, owner, won], () => { A.Gr.writeS32(0); A.Gr.add(4).writeS32(0); A.Gr.add(8).writeS32(owner); A.Gr.add(12).writeS32(p); A.GrWin.writeU8(won); A.GrWin.add(1).writeU8(1 - won); });
  } else if (c < 21 && id === 802) {
    // 802's fleet: a battle starting (1) or finished, its arrival at 8022 (0) or special 0x10.
    const f = fleetOfId(8020), all = liveFleets(), k = rnd(4);
    if (k === 0 && all.length) {
      const a = f || pick(all), mine = all.filter((x) => fleet(x).add(0x15).readU8() === en), b = mine.length ? pick(mine) : pick(all);
      transientS16(sets, A.SpFlottak, rnd(2) ? a : b); transientS16(sets, A.SpFlottak.add(6), rnd(2) ? b : a);
      flagRec(1, [a, b]);
    } else if (k === 1) {
      const n = 2 + rnd(2), list = [];
      for (let i = 0; i < n; i++) list.push([i === 1 || rnd(3) === 0 ? en : pick([10, 3, varOf(8029) & 0xff]), i === 0 || rnd(3) === 0 ? 8020 : rnd(9000)]);
      const write = () => list.forEach(([owner, fid], i) => { const q = fake.add(0x30 * i); q.add(0x18).writeS32(owner); q.add(0x20).writeS32(fleetOfId(fid)); q.add(0x24).writeS32(fid); });
      transientS32(sets, A.KiNyert, rnd(2) ? list[0][0] : list[1][0]);
      stage('spwfinished', [[A.SpwFinished, [fake.toUInt32(), n, 1]]], { args: list.map(([o, fid]) => [o, fleetOfId(fid), fid]) }, () => { prep(); write(); }, after);
    } else if (k === 2) {
      const p = rnd(4) ? planetOfId(8022) : 1 + rnd(np), fid = rnd(4) ? 8020 : 0;
      const a = [fid, f, p, rnd(5) ? 1 : 0, 0];
      flagRec(0, a, () => { for (let i = 0; i < 5; i++) A.Arr.add(4 * i).writeS32(a[i]); });
    } else {
      const args = [0x10, rnd(4) ? 8020 : 0, 0, 0, 0, 0, 0, 0];
      stage('special', [[A.Special, args]], { args }, prep, after);
    }
  } else {
    // Tech 183 (special 13 or daily), 824's daily (a fleet of the human's near its planet), the wars 824/852 test.
    techOf(sets, 183, rnd(2) ? 5 : 3);
    if (rnd(4) === 0) vSet(8026, rnd(2));
    const p = planetOfId(8241);
    if (p && rnd(2)) {
      const mine = liveFleets().filter((x) => fleet(x).add(0x15).readU8() === en);
      if (mine.length) {
        const q = fleet(pick(mine)), b = A.Bolygok.add(0x188 * p);
        let wx = 0, wy = 0;
        const nx = b.add(0x18).readFloat() + rnd(15) - 7, ny = b.add(0x1c).readFloat() + rnd(15) - 7;
        sets.push([() => { wx = q.add(0x30).readFloat(); wy = q.add(0x34).readFloat(); q.add(0x30).writeFloat(nx); q.add(0x34).writeFloat(ny); },
          () => { q.add(0x30).writeFloat(wx); q.add(0x34).writeFloat(wy); }]);
      }
    }
    for (const v of [8521, 5208]) { const e = varOf(v); if (e > 0 && e < 19 && rnd(3) === 0) transientU8(sets, A.Viszony.add((en * 19 + e) * 0x26 + 0x25), pick([1, 3])); }
    if (rnd(2)) {
      const args = [0xd, rnd(4) ? en : 3, rnd(4) ? 183 : 100, 0, 0, 0, 0, 0];
      stage('special', [[A.Special, args]], { args }, prep, after);
    } else stage('flag', [[A.Check, [1]]], { args: [0x10, 0] }, () => { prep(); A.Flags.add(0x10).writeU8(1); }, after);
  }
}
// Part 2b: the ground-war episodes 808 (planet 8080 of empire v(5208)) and 854 (the riots).
const ELEM2B = [808, 854];
// 808/854 staging: starts (808 with an empire and sometimes a planet for 8080), the counters, daily checks, ground
// battles starting and ending on 8080 / the human's planets (854's riot needs a planet of the human's with more than 4
// vehicles, few police and the human's units on its battlefield: TankNew(−p, 1, n) first, not recorded), message
// 8540 read.
function elem2bFuzz(pool, running, sets, prep, after) {
  const en = A.EN.readS32(), np = nPlanets(), own = humanPlanets(), live = running.filter((x) => ELEM2B.includes(x));
  const flagRec = (fl, extra, write) => stage('flag', [[A.Check, [1]]], { args: [fl].concat(extra || []) }, () => { prep(); A.Flags.add(fl).writeU8(1); if (write) write(); }, after);
  const ctrRec = (ctr) => flagRec(5, [ctr], () => A.SzVegzett.writeS32(ctr));
  const grRec = (fl, p, owner, won) => flagRec(fl, [p, owner, won], () => {
    A.Gr.writeS32(0); A.Gr.add(4).writeS32(0); A.Gr.add(8).writeS32(owner); A.Gr.add(12).writeS32(p); A.Gr.add(16).writeS32(A.Bolygok.add(0x188 * p + 0x20).readU8());
    A.GrWin.writeU8(won); A.GrWin.add(1).writeU8(1 - won);
  });
  // TankNew(−p, 1, n) faults for an owner without a design table (free planets, empires ≥ 9 without designs).
  // The madness a staged battle start sets stays without the battle the game would run: unmarked after the record
  // (left set, the planet counts as at war, and a later end's BolygoElfoglal re-ran Story_Check from inside it until
  // the stack ran out).
  const calmAfter = (p) => sets.push([() => {}, () => {
    const w = wf(p), n = w.add(0x180).readS32();
    if (n <= 0 || n > 2000) return;
    try { const u = w.add(0x330).readPointer(); for (let k = 0; k < n; k++) { const q = u.add(4 * k).readPointer().add(0x188); q.writeU32(q.readU32() & ~4); } } catch (e) {}
  }]);
  const atWar = (p) => A.Bolygok.add(0x188 * p + 0x64).readU8() !== 0;
  // 854's guards count intact police only: a police station (kind 6) damaged (+8) for the record now and then.
  // The riot skips units with +0x188 & 0x4020000: the bit set on some of the human's units for the record now and
  // then (only the added bit is taken back after it).
  const flagUnits = (q) => {
    if (!q || rnd(3)) return;
    const w = wf(q), n = w.add(0x180).readS32();
    if (n <= 0 || n > 2000) return;
    try {
      const u = w.add(0x330).readPointer();
      for (let k = 0; k < n; k++) {
        const r = u.add(4 * k).readPointer();
        if (r.add(0x194).readU32() !== en || rnd(3)) continue;
        const f = r.add(0x188), bit = pick([0x20000, 0x4000000]);
        if (f.readU32() & bit) continue;
        sets.push([() => f.writeU32(f.readU32() | bit), () => f.writeU32(f.readU32() & ~bit)]);
      }
    } catch (e) {}
  };
  const damagePolice = (q) => {
    if (!q || rnd(3)) return;
    const n = A.HanyEpulet.add(4 * q).readS32(), e = A.Epuletek.add(4 * q).readPointer();
    for (let j = 1; j <= n && j <= 2000; j++) if (e.add(0x20 * j).readU8() === 6 && rnd(2)) transientS16(sets, e.add(0x20 * j + 8), 1 + rnd(200));
  };
  const units = (p, n) => {
    const o = A.Bolygok.add(0x188 * p + 0x20).readU8();
    if (o >= 1 && o < 19 && !A.Tervek.add(4 * o).readPointer().isNull() && A.TervCount.add(4 * o).readS32() >= 1) call(va(0x541750), [-p, 1, n], null, false);
  };
  const others = []; for (let p = 1; p <= np; p++) { const o = A.Bolygok.add(0x188 * p + 0x20).readU8(); if (o !== en && o > 0 && o < 9) others.push(p); }
  // A riot candidate: a planet of the human's it sees, crowded (+0x58 > 4) and with people enough (+0x28).
  const crowd = (q) => {
    const b = A.Bolygok.add(0x188 * q);
    if (b.add(0xd8 + 2 * en).readS16() <= 0) transientS16(sets, b.add(0xd8 + 2 * en), 1);
    transientS32(sets, b.add(0x58), pick([3, 4, 5, 6, 7, 9, 12]));
    if (rnd(3)) transientS32(sets, b.add(0x28), pick([40000, 100000, 1000000]));
  };
  const c = rnd(20);
  if (c < 3 || !live.length) {
    const ids = pool.filter((id) => ELEM2B.includes(id) && !bucketHas(id));
    if (!ids.length) return;
    const id = pick(ids), al = aliveEmpires(en);
    if (id === 808) {
      // An empire with a design table (AutoAddtank's st_GetTerv makes designs: a map's inactive empire has none and
      // TervNew faults).
      const designs = (e) => !A.Tervek.add(4 * e).readPointer().isNull() && A.TervCount.add(4 * e).readS32() >= 1;
      const owners = others.map((p) => A.Bolygok.add(0x188 * p + 0x20).readU8()).filter(designs), al2 = al.filter(designs);
      vSet(5208, owners.length && rnd(6) ? pick(owners) : al2.length ? pick(al2) : 3);
      const mine = others.filter((p) => designs(A.Bolygok.add(0x188 * p + 0x20).readU8()));
      if (mine.length && rnd(3) === 0) bAddAzon(pick(mine), 8080);
      vSet(8080, 0); if (rnd(3) === 0) vSet(6400, 1);
      if (cfg.debug) { const e = varOf(5208), q = planetOfId(8080); emit({ k: 'dbg', what: '808start', e, q, owner: q ? A.Bolygok.add(0x188 * q + 0x20).readU8() : -1, tn: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map((x) => [A.Tervek.add(4 * x).readPointer().isNull() ? 0 : 1, A.TervCount.add(4 * x).readS32()]) }); }
    }
    if (id === 854 && own.length && rnd(2)) { const q = pick(own); crowd(q); units(q, 1 + rnd(3)); }
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
    return;
  }
  const id = pick(live);
  if (id === 808) {
    const k = rnd(8), p = planetOfId(8080), owner = p ? A.Bolygok.add(0x188 * p + 0x20).readU8() : 0;
    if (k < 2) {
      const ctr = pick([8080, 8081, 8081, 8088]);
      if (ctr === 8081) vSet(8089, rnd(2));
      if (ctr === 8080 && rnd(2)) vSet(6400, rnd(2));
      ctrRec(ctr);
    } else if (k < 3) {
      // Daily: a building of kind 0x20..0x23 on 8080's planet first (EpuletNewSearchLocation, not recorded).
      if (p && owner !== en && owner > 0 && rnd(2)) call(va(0x53cca0), [p, 0x20 + rnd(4), 1, 0, -1, -1], null, false);
      if (p && rnd(3) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([en, owner]));
      flagRec(0x10, []);
    } else if (k < 6 || !p || owner === 0 || atWar(p)) {
      // The human lands on 8080 (or elsewhere); after the 153 days 10 more vehicles land.
      const q = p && rnd(5) ? p : 1 + rnd(np);
      if (rnd(2)) vSet(8080, 1);
      if (q && rnd(3) === 0) units(q, 1 + rnd(3));
      // A building the start deletes (kinds 0x1c..0x1f) first, not recorded.
      if (q === p && owner > 0 && rnd(3) === 0) call(va(0x53cca0), [q, 0x1c + rnd(4), 1, 0, -1, -1], null, false);
      // The start sets StoryAdat+0x40 (the planet) and +0x88 for the battle (GroundWarIndit, SpaceWarIndit read them):
      // without the battle that follows in the game, cleared after the record (left set, the game soon reset itself).
      sets.push([() => {}, () => { A.Adat.add(0x40).writeS32(0); A.Adat.add(0x88).writeS32(0); }]);
      calmAfter(q);
      grRec(2, q, en, 0);
    } else {
      // The battle ends: the buildings' sums against the start's. As in the game the human has just taken the
      // planet: its owner byte is the human's for the record (handing the empire its own planet again crashed; a
      // real BolygoElfoglal(p, EN) first made the campaign reset itself).
      // The sums at the end (808's: the kinds 0xe..0x10 count EpuletekLeiro[kind].+8 into both, the others it less
      // their damage +4): mostly v(8083) set round the bar total = (v(8083) − 2·v(8084))·4/5, so either result shows.
      let total = 0;
      const n8 = A.HanyEpulet.add(4 * p).readS32(), e8 = A.Epuletek.add(4 * p).readPointer();
      for (let j = 1; j <= n8 && j <= 2000; j++) {
        const kd = e8.add(0x20 * j).readU8();
        if (kd) total += va(0x8eba40).add(0x24 * kd).readS32() - ((kd >= 0xe && kd <= 0x10) ? 0 : e8.add(0x20 * j + 4).readS32());
      }
      const b84 = rnd(3) === 0 ? pick([0, 1, 300]) : 0;
      const near = Math.floor((total * 5) / 4) + 2 * b84 + rnd(9) - 4;
      vSet(8084, b84);
      vSet(8083, rnd(4) ? near : pick([0, 1, 500, 5000, 50000, rnd(20000), 1000000, 10000000, rnd(3000000)]));
      if (owner !== en) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), en);
      // The human knows the empire it fought: BolygoElfoglal's contact block posts special 0xB before it marks an
      // unknown empire known, and 808's end (flag 4 still set) runs again inside it until the stack runs out.
      const e = varOf(5208);
      if (e > 0 && e < 19) transientU8(sets, A.Ismeri.add(19 * en + e), 1);
      grRec(4, p, en, rnd(2));
    }
    return;
  }
  const k = rnd(12), riotP = varOf(8549), p8540 = planetOfId(8540);
  if (k < 4) {
    // The riot's counter: v(8540) = 3 ends it.
    vSet(8540, pick([0, 0, 1, 2, 3]));
    if (own.length && rnd(4)) { const q = p8540 && rnd(3) === 0 ? p8540 : pick(own); crowd(q); damagePolice(q); flagUnits(q); if (rnd(4)) units(q, 1 + rnd(3)); }
    ctrRec(8540);
  } else if (k < 5) {
    ctrRec(8549);
  } else if (k < 7) {
    const q = riotP > 0 && riotP <= np && rnd(4) ? riotP : own.length ? pick(own) : 1 + rnd(np);
    grRec(4, q, rnd(2) ? en : 10, rnd(2));
  } else if (k < 9) {
    // A ground battle starts on a planet of the human's after a riot (v(8548)).
    const q = p8540 && rnd(2) ? p8540 : own.length ? pick(own) : 1 + rnd(np);
    if (rnd(2)) vSet(8548, 1);
    if (rnd(2)) units(q, 1 + rnd(3));
    damagePolice(q);
    // Few people for the police: the madness chance reaches its upper clamp (75).
    if (rnd(3) === 0) transientS32(sets, A.Bolygok.add(0x188 * q + 0x28), pick([0, 100, 3000, 20000]));
    calmAfter(q);
    grRec(2, q, rnd(3) ? 10 : en, 0);
  } else if (k < 10) {
    flagRec(9, [8540], () => { A.Msg.writeS32(8540); A.Msg.add(4).writeS32(0); });
  } else {
    if (rnd(2)) vSet(8548, 1);
    if (p8540 && rnd(3) === 0) transientS32(sets, A.Bolygok.add(0x188 * p8540 + 0x28), pick([0, 100, 4000, 100000]));
    damagePolice(p8540);
    if (p8540 && rnd(4) === 0) transientU8(sets, A.Bolygok.add(0x188 * p8540 + 0x20), pick([0, 3]));
    flagRec(0x10, []);
  }
}
function transientF32(sets, q, v) { let was = 0; sets.push([() => { was = q.readFloat(); q.writeFloat(v); }, () => q.writeFloat(was)]); }
// Part 2c: the diplomacy episodes 810 (protection of v(8109)), 857 (an alliance from the second strongest enemy),
// 858 (empire 5's planet 8580, then the others' non-aggression pacts) and 860 (empire 1's tribute).
const ELEM2C = [810, 857, 858, 860];
const ELEM2C_COUNTERS = { 810: [6011, 6011, 6602, 6701, 6705], 857: [8570, 8571, 8571, 8572], 858: [8580, 8589], 860: [8600, 8600, 8601, 8601, 8602] };
const ELEM2C_ANSWERS = { 810: [[6, 6601, 2], [6, 6800, 2], [6, 6601, 3], [6, 6800, 3], [7, 6601, 0], [7, 6800, 0], [6, 6602, 2], [6, 6802, 2],
  [6, 6602, 3], [6, 6802, 3], [7, 6602, 0], [7, 6802, 0], [6, 6701, 2], [6, 6702, 2], [6, 6703, 2]],
  857: [[6, 8573, 2]], 858: [[6, 8580, 2]],
  860: [[6, 8601, 5], [7, 8601, 0], [6, 8601, 6], [6, 8601, 7], [6, 8601, 8], [6, 8601, 9], [6, 8601, 2]] };
// Staging: starts with the world they read (the empires alive, known, at war, their strengths, the human's planets),
// the counters (with the money, the tribute, the levels), the answers, the diplomacy specials answered (flag 13 with
// the empire), ground battles ending on 8580 / the empire's planets with a fleet of the human's or another's, daily
// checks.
function elem2cFuzz(pool, running, sets, prep, after) {
  const en = A.EN.readS32(), np = nPlanets(), own = humanPlanets(), live = running.filter((x) => ELEM2C.includes(x));
  const flagRec = (fl, extra, write) => stage('flag', [[A.Check, [1]]], { args: [fl].concat(extra || []) }, () => { prep(); A.Flags.add(fl).writeU8(1); if (write) write(); }, after);
  const level = (e, l) => { if (e > 0 && e < 19 && e !== en) transientU8(sets, A.Viszony.add((en * 19 + e) * 0x26 + 0x25), l); };
  const alive = (e, x) => { if (e > 0 && e < 9 && e !== en) transientU8(sets, A.El.add(e), x); };
  const ownerOf = (p) => A.Bolygok.add(0x188 * p + 0x20).readU8();
  // Three planets of the human's it sees (860's st_bGet3Nearest) for the record.
  const threeOwn = () => {
    if (own.length >= 3 || !rnd(4)) return;
    for (let k = 3 + rnd(3); k > 0; k--) { const q = 1 + rnd(np - 1); transientU8(sets, A.Bolygok.add(0x188 * q + 0x20), en); transientS16(sets, A.Bolygok.add(0x188 * q + 0xd8 + 2 * en), 1); }
    transientS32(sets, A.HanyB.add(4 * en), 3 + rnd(3));
  };
  const c = rnd(24);
  if (c < 4 || !live.length) {
    const ids = pool.filter((id) => ELEM2C.includes(id) && !bucketHas(id));
    if (!ids.length) return;
    const id = pick(ids), al = aliveEmpires(en);
    if (id === 810) {
      vSet(8109, rnd(3) ? 0 : al.length ? pick(al) : 3);
      for (const e of al) if (rnd(4) === 0) level(e, pick([1, 3, 4]));
      for (const e of al) if (rnd(4) === 0) transientU8(sets, A.Ismeri.add(19 * en + e), rnd(2));
    }
    if (id === 857) {
      // GetLegerosebb(0, 0) over the strengths (f32), then the strongest known one but empire 5 and it.
      for (const e of al) if (rnd(3) === 0) transientF32(sets, A.Ero.add(4 * e), rnd(2) ? rnd(100000) : rnd(50));
      for (const e of al) if (rnd(3) === 0) transientU8(sets, A.Ismeri.add(19 * en + e), rnd(3) ? 1 : 0);
    }
    if (id === 858) {
      vSet(8589, rnd(5) ? 1 : 0);
      alive(5, rnd(6) ? 1 : 0);
      // Empire 5's planets the human sees (+0xd8[EN] >= 0) with their people (+0x28): a few more for the record.
      if (rnd(2)) for (let k = 1 + rnd(4); k > 0; k--) { const q = 1 + rnd(np); if (ownerOf(q) === en) continue; transientU8(sets, A.Bolygok.add(0x188 * q + 0x20), 5); transientS16(sets, A.Bolygok.add(0x188 * q + 0xd8 + 2 * en), rnd(4) ? 1 : -1); }
      for (let q = 1; q <= np; q++) if (ownerOf(q) === 5 && rnd(3) === 0) transientS32(sets, A.Bolygok.add(0x188 * q + 0x28), pick([0, 1, 5000, rnd(1000000)]));
    }
    if (id === 860) { alive(1, rnd(6) ? 1 : 0); alive(5, rnd(6) ? 1 : 0); level(1, pick([1, 2, 3, 4])); if (rnd(3) === 0) transientU8(sets, A.Ismeri.add(19 * en + 1), 0); }
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
    return;
  }
  const id = pick(live);
  if (c < 10) {
    const ctr = pick(ELEM2C_COUNTERS[id]);
    if (ctr === 6011) { vSet(6700, rnd(2)); if (rnd(2)) vSet(6701, rnd(2)); const e = varOf(8109); if (rnd(2)) level(e, pick([1, 2, 3, 5])); }
    if (ctr === 8570 || ctr === 8571) {
      const m = A.Money.add(8 * en), x = pick([0, 1999999.5, 2000000, 2000001, 5e6, rnd(3000000)]);
      let was = 0;
      if (rnd(2)) sets.push([() => { was = m.readDouble(); m.writeDouble(x); }, () => m.writeDouble(was)]);
      const e = varOf(8579);
      if (ctr === 8571) { if (rnd(4) === 0) alive(e, 0); if (rnd(3) === 0) level(e, pick([4, 5])); }
    }
    if (ctr === 8580) alive(5, rnd(3) ? 1 : 0);
    if (ctr === 8600) { alive(1, rnd(6) ? 1 : 0); threeOwn(); }
    if (ctr === 8601) {
      alive(1, rnd(8) ? 1 : 0);
      if (rnd(3)) transientS32(sets, A.Adat.add(0x2c), rnd(3) ? rnd(200000) : pick([0, 9, 10, -25, 0x7fffffff]));
      if (rnd(2)) vSet(8600, pick([0, 9, 10, 11, 12, rnd(11)]));
    }
    stage('flag', [[A.Check, [1]]], { args: [5, ctr] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(ctr); }, after);
  } else if (c < 14) {
    const [kind, b, part] = pick(ELEM2C_ANSWERS[id]);
    if (b === 8601 && part >= 6) for (let k = 0; k < 3; k++) { const q = planetOfId(8600 + k); if (q && rnd(4) === 0) transientU8(sets, A.Bolygok.add(0x188 * q + 0x20), pick([en, 1, 0])); }
    stage('flag', [[A.Check, [1]]], { args: [kind, b, part] }, () => { prep(); A.Flags.add(kind).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(part); }, after);
  } else if (c < 17 && id !== 810) {
    // A diplomacy special answered (flag 13): 857's 8571, 858's 8581 (by any empire, the level before the pact),
    // 860's 8601 (or another's).
    const which = rnd(6) ? { 857: 8571, 858: 8581, 860: 8601 }[id] : pick([8571, 8581, 8601, 1234]);
    const al = aliveEmpires(en), e = al.length && rnd(5) ? pick(al) : 1 + rnd(8);
    if (which === 8581 && rnd(2)) level(e, pick([1, 2, 3, 4, 5]));
    flagRec(13, [which, e], () => { A.Dipl.writeS32(which); A.Dipl.add(4).writeS32(e); });
  } else if (c < 20 && id !== 810 && id !== 860) {
    // A ground battle ends (4): on 8580 (858) or a planet of 857's empire, the battle's fleet the human's (read live,
    // Flottak[GrWarFlotta].+0x15), the old owner the empire's.
    const all = liveFleets(), mine = all.filter((x) => fleet(x).add(0x15).readU8() === en);
    const f = mine.length && rnd(5) ? pick(mine) : all.length ? pick(all) : 0;
    const e = varOf(8579), p858 = planetOfId(8580);
    let p = 1 + rnd(np);
    if (id === 858 && p858 && rnd(5)) p = p858;
    const oldOwner = id === 857 && rnd(4) ? e : ownerOf(p);
    // 857 ends when the empire has no planet left: sometimes none for the record.
    if (id === 857 && e > 0 && e < 19 && rnd(3) === 0) for (let q = 1; q <= np; q++) if (ownerOf(q) === e) transientU8(sets, A.Bolygok.add(0x188 * q + 0x20), en);
    if (id === 858 && rnd(3) === 0) vSet(8582, rnd(2));
    const won = rnd(4) ? 1 : 0;
    // GrWarFlottaTulaj (+8) apart from the fleet's live owner now and then (857/858 read the fleet's).
    const gOwner = rnd(4) === 0 ? pick([en, 10, 3, 5]) : f ? fleet(f).add(0x15).readU8() : en;
    flagRec(4, [p, f, oldOwner, won], () => {
      A.Gr.writeS32(f); A.Gr.add(4).writeS32(f ? fleet(f).add(0x16).readS16() : 0); A.Gr.add(8).writeS32(gOwner);
      A.Gr.add(12).writeS32(p); A.Gr.add(16).writeS32(oldOwner); A.GrWin.writeU8(won); A.GrWin.add(1).writeU8(1 - won);
    });
  } else {
    // Daily: 810 with its empire dead; 858's pacts (v(8590 + e)) and empire 5 dead after v(8588).
    if (id === 810 && rnd(3) === 0) alive(varOf(8109), 0);
    if (id === 858) {
      if (rnd(2)) vSet(8599, rnd(3) ? 1 : 0);
      for (let e = 1; e < 9; e++) if (rnd(3) === 0) vSet(8590 + e, rnd(2));
      if (rnd(2)) vSet(8588, 1);
      alive(5, rnd(3) ? 1 : 0);
    }
    flagRec(0x10, []);
  }
}
// Part 2d: 801 (the evacuation of 8010), 804 (empire 10's bases 8040..8042), 806 (the police strike), 807 (the
// pirates' ultimatum), 809 (empire 13's blockade).
// Part 2f: 836 (empire 1's three waves on 8351), 839 (empire 10's hunter fleet 8390 round 8391..8393), 840 (the
// colonisation race, empire 17), 843 (the raiders 8430..8432 past planet 8044).
const ELEM2G = [844, 856, 862, 863];
// The fleets' names (844's 8443 and 8441, 856's 8561, 862's 8621 and 8623, 863's 8638; 856's colony text 8560 is in
// ELEM2E_TEXTS).
const ELEM2G_TEXTS = [0x20f9, 0x20fb, 0x2171, 0x21ad, 0x21af, 0x21be];
const ELEM2G_COUNTERS = { 844: [8441], 856: [8560, 8561, 8563, 8563, 8564, 8565, 8565, 8569], 863: [8630, 8631, 8632, 8632, 8633, 8634, 8636, 8638] };
const ELEM2G_ACTS = { 844: ['ctr', 'arrive', 'arrive', 'ss', 'se', 'se', 'daily', 'daily'],
  856: ['ctr', 'ctr', 'ctr', 'ans', 'ans', 'ans', 'letilt', 'special', 'arrive', 'arrive', 'daily'],
  862: ['read', 'daily', 'daily', 'daily', 'arrive', 'arrive', 'gstart'], 863: ['ctr', 'ctr', 'ctr', 'ground', 'daily'] };
const ELEM2G_ANSWERS = [[8562, 2], [8562, 3], [8562, 4], [8563, 3], [8563, 4], [8563, 5], [8563, 6], [8563, 7], [8564, 2], [8564, 3],
  [8567, 2], [8567, 3]];
function elem2gFuzz(pool, running, sets, prep, after) {
  const en = A.EN.readS32(), np = nPlanets(), live = running.filter((x) => ELEM2G.includes(x));
  const flagRec = (fl, extra, write) => stage('flag', [[A.Check, [1]]], { args: [fl].concat(extra || []) }, () => { prep(); A.Flags.add(fl).writeU8(1); if (write) write(); }, after);
  const ownerOf = (p) => A.Bolygok.add(0x188 * p + 0x20).readU8();
  const setOwner = (p, o) => { if (p) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), o); };
  const fxy = (f) => [fleet(f).add(0x30).readFloat(), fleet(f).add(0x34).readFloat()];
  const mine = () => liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en);
  // Empires 1 and 5 alive for the record (862 and 863 need both); some of the others' colonies lent to them (never a
  // free planet: a conquest of one hangs the game).
  const lend = (e, n) => {
    const c = []; for (let p = 1; p <= np; p++) { const o = ownerOf(p); if (o !== 0 && o !== en && o !== e && o < 9) c.push(p); }
    for (let k = n; k > 0 && c.length; k--) setOwner(c.splice(rnd(c.length), 1)[0], e);
  };
  const alive = (e) => { if (!A.El.add(e).readU8()) transientU8(sets, A.El.add(e), 1); };
  // A fleet of the human's holds story id `fid` for the record (856's 8562), its vehicles' designs' classes varied;
  // now and then its vehicle designs made fighters (+0x16 ≠ 2) with +0x58 their sum (TankDelete lowers it then).
  const holdFleet = (fid, fighters) => {
    let f = fleetOfId(fid);
    const m = mine();
    if ((!f || rnd(4) === 0) && m.length) { f = pick(m); transientS16(sets, fleet(f).add(0x16), fid); }
    if (!f) return 0;
    const own = fleet(f).add(0x15).readU8(), tp = A.Tervek.add(4 * own).readPointer(), tkn = A.TankN.readS32();
    for (let k = 1; k <= tkn; k++) {
      const t = A.Tanks.add(8 * k), d = t.add(2).readS16();
      if (d === 0x7fff || t.readS16() !== f || tp.isNull()) continue;
      if (rnd(2)) transientS32(sets, tp.add(0x84 * d + 0x44), 1 + rnd(4));
      if (rnd(3) === 0) transientU8(sets, tp.add(0x84 * d + 0x36), pick([0, 1, 2]));
    }
    if ((fighters || rnd(2)) && !tp.isNull()) {
      let n = 0;
      for (let k = 1; k <= tkn; k++) {
        const t = A.Tanks.add(8 * k), d = t.add(2).readS16();
        if (d === 0x7fff || t.readS16() !== f) continue;
        const kind = tp.add(0x84 * d + 0x16);
        if (kind.readS16() === 2) transientS16(sets, kind, pick([0, 1]));
        if (tp.add(0x84 * d + 0x36).readU8() === 0) transientU8(sets, tp.add(0x84 * d + 0x36), 1);
        n += t.add(4).readS32();
      }
      if (n) transientS32(sets, fleet(f).add(0x58), n);
    }
    return f;
  };
  if (rnd(6) === 0 || !live.length) {
    const ids = pool.filter((id) => ELEM2G.includes(id) && !bucketHas(id));
    if (!ids.length) return;
    const id = pick(ids);
    if (id === 862 || id === 863) {
      if (rnd(6)) { alive(1); alive(5); }
      if (rnd(4)) lend(5, 1 + rnd(4));
      if (id === 863 && rnd(4)) lend(1, 1 + rnd(3));
    }
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
    return;
  }
  const id = pick(live), act = pick(ELEM2G_ACTS[id]);
  if (act === 'ctr') {
    const ctr = pick(ELEM2G_COUNTERS[id]);
    // 844: the raid's two empires alive or not.
    if (ctr === 8441) for (const v of [8448, 8449]) { const e = varOf(v); if (e > 0 && e < 19 && rnd(4) === 0) transientU8(sets, A.El.add(e), rnd(2)); }
    // 856: the toll in vehicles (v(8563): 5, 20 or all) or ships, from the held fleet.
    if (ctr >= 8563 && ctr <= 8565) { if (rnd(2)) vSet(8563, pick([1, 2, 3, 0])); if (rnd(8)) holdFleet(8562, rnd(4) !== 0); }
    // 856: empire 5 dead now and then (no raider).
    if (ctr === 8569 && rnd(4) === 0) transientU8(sets, A.El.add(5), 0);
    // 863: the human's money round 500,000, empire 5's planet count (3, 6: the waves) and the human's.
    if (ctr === 8630) { const m = A.Money.add(8 * en), x = pick([100000, 500000, 500001, 2000000]); let was = 0; sets.push([() => { was = m.readDouble(); m.writeDouble(x); }, () => m.writeDouble(was)]); }
    if (ctr === 8631 || ctr === 8632) {
      if (rnd(2)) lend(5, ctr === 8632 ? 4 + rnd(6) : rnd(8));
      if (rnd(3) === 0) transientS32(sets, A.HanyB.add(4 * 5), pick([0, 1, 2, 3, 5, 6, 9]));
      else if (ctr === 8632 && rnd(2)) transientS32(sets, A.HanyB.add(4 * 5), pick([6, 7, 9]));
      if (rnd(6) === 0) transientS32(sets, A.HanyB.add(4 * en), pick([1, 2, 3]));
    }
    // 863: the marked planets (8632..8635, 8633) empire 5's or not.
    if (ctr === 8633 || ctr === 8634) for (const b of [8632, 8633, 8634, 8635]) { const p = planetOfId(b); if (p && rnd(3) === 0) setOwner(p, pick([5, en, 1])); }
    if (ctr === 8634 && rnd(4) === 0) transientU8(sets, A.El.add(5), 0);
    stage('flag', [[A.Check, [1]]], { args: [5, ctr] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(ctr); }, after);
  } else if (act === 'ans') {
    const [b, part] = pick(ELEM2G_ANSWERS);
    if (b === 8567 && rnd(2)) holdFleet(8562);
    stage('flag', [[A.Check, [1]]], { args: [6, b, part] }, () => { prep(); A.Flags.add(6).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(part); }, after);
  } else if (act === 'letilt') {
    // st_Letilt(8563): the answers the held fleet's vehicles can't pay are disabled.
    if (rnd(4)) holdFleet(8562);
    stage('flag', [[A.Check, [1]]], { args: [8, 8563, 0] }, () => { prep(); A.Flags.add(8).writeU8(1); A.Msg.writeS32(8563); A.Msg.add(4).writeS32(0); }, after);
  } else if (act === 'read') {
    flagRec(9, [8620, 0], () => { A.Msg.writeS32(8620); A.Msg.add(4).writeS32(0); });
  } else if (act === 'special') {
    // A colonisation (special 10) of planet 8560 (or another) by the human or another empire.
    const p = planetOfId(8560), args = [10, rnd(3) ? en : 1 + rnd(8), p && rnd(4) ? p : 1 + rnd(np), 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args });
  } else if (act === 'arrive') {
    // 844's envoy at its point (type 0) or the raid at its planet (type 1); 856's raider at its point, a fleet of the
    // human's (or not) at 8560 (v(8560) and v(8562) varied); 862's escort at 8620. Another fleet takes a missing id.
    const all = liveFleets();
    if (!all.length) return;
    let fid, type, tgt = 0, f;
    if (id === 844) { [fid, type] = rnd(2) ? [8443, 0] : [8441, 1]; }
    else if (id === 856) {
      if (rnd(3) === 0) { fid = 8561; type = 0; }
      else {
        const m = mine(); f = m.length && rnd(5) ? pick(m) : pick(all); fid = fleet(f).add(0x16).readS16(); type = 1; tgt = planetOfId(8560) || 1 + rnd(np);
        if (rnd(3) === 0) vSet(8560, pick([0, 1, 1])); if (rnd(4) === 0) vSet(8562, rnd(2));
        if (rnd(2)) transientS32(sets, fleet(f).add(0x58), pick([0, 3, 12, 30]));
      }
    } else { fid = 8623; type = 1; tgt = planetOfId(8620) || 1 + rnd(np); }
    if (rnd(8) === 0) type = 1 - type;
    if (!f) { f = fleetOfId(fid) || pick(all); if (fleet(f).add(0x16).readS16() !== fid) transientS16(sets, fleet(f).add(0x16), fid); }
    if (type === 1 && !tgt) tgt = 1 + rnd(np);
    const a = [fid, f, tgt, type, 0];
    flagRec(0, a, () => { for (let i = 0; i < 5; i++) A.Arr.add(4 * i).writeS32(a[i]); });
  } else if (act === 'ss') {
    // 844: a space battle starts with the envoy or the raid among spFlottak's two.
    const all = liveFleets();
    if (all.length < 2) return;
    const fid = pick([8443, 8441]);
    let g = fleetOfId(fid);
    if (!g) { g = pick(all); transientS16(sets, fleet(g).add(0x16), fid); }
    const o = pick(all.filter((x) => x !== g));
    keepRegion(sets, A.SpFl0, 0x2c); keepRegion(sets, A.Adat.add(0x94), 8);
    const [s1, s2] = rnd(2) ? [g, o] : [o, g];
    transientS16(sets, A.SpFlottak, s1); transientS16(sets, A.SpFlottak.add(6), s2);
    flagRec(1, [s1, s2]);
  } else if (act === 'se') {
    // 844: a space battle ends with the envoy or the raid among the participants, lost or won.
    const n = 2 + rnd(2), list = [], buf = Memory.alloc(0x30 * 4);
    spwBufs.push(buf);
    if (spwBufs.length > 64) spwBufs.shift();
    for (let i = 0; i < n; i++) {
      const fid = pick([0, 8443, 8441, 8441]), owner = rnd(2) ? en : pick([9, 10, 1]);
      const q = buf.add(0x30 * i); q.add(0x18).writeS32(owner); q.add(0x20).writeS32(fid); q.add(0x24).writeS32(fid); list.push([owner, fid]);
    }
    transientS32(sets, A.SpWin, rnd(2) ? list[0][0] : list[1][0]);
    stage('spwfinished', [[A.SpwFinished, [buf.toUInt32(), n, 1]]], { args: list }, prep, after);
  } else if (act === 'gstart' || act === 'ground') {
    // 862: a ground battle starts on 8620 (its fleet deleted); 863: one ends anywhere (the waves' checks).
    const p = id === 862 && rnd(4) ? planetOfId(8620) || 1 + rnd(np) : 1 + rnd(np);
    const m = mine(), f = m.length ? pick(m) : 0, won = rnd(3) ? 1 : 0, fl = act === 'gstart' ? 2 : 4;
    if (id === 863) { if (rnd(2)) vSet(8639, 1); for (const b of [8632, 8633, 8634, 8635]) { const q = planetOfId(b); if (q && rnd(3) === 0) setOwner(q, pick([5, en])); } }
    flagRec(fl, [p, f, won], () => {
      A.Gr.writeS32(f); A.Gr.add(4).writeS32(f ? fleet(f).add(0x16).readS16() : 0); A.Gr.add(8).writeS32(en);
      A.Gr.add(12).writeS32(p); A.Gr.add(16).writeS32(ownerOf(p)); A.GrWin.writeU8(won); A.GrWin.add(1).writeU8(1 - won);
    });
  } else {
    // Daily: 844's raid wandering (empire v(8448) without planets now and then); 856's raider hunting near 8560; 862's
    // raiders and escort (v(8621) 0, 2, 3; empires 1 and 5 without planets); 863 with empire 5 dead.
    if (id === 844) {
      // The raid (another empire's fleet takes id 8441 for the record), heading for a planet of v(8448)'s or not.
      let g = fleetOfId(8441);
      const others = liveFleets().filter((x) => fleet(x).add(0x15).readU8() !== en);
      if (!g && others.length && rnd(2)) { g = pick(others); transientS16(sets, fleet(g).add(0x16), 8441); }
      if (g && rnd(2)) { transientS16(sets, fleet(g).add(0x40), pick([0, 1, 1])); transientS16(sets, fleet(g).add(0x42), pick([0, 1, 1, 2])); }
    }
    if (id === 844) { if (rnd(2)) vSet(8441, 1); const e = varOf(8448); if (e > 0 && e < 19 && rnd(5) === 0) transientS32(sets, A.HanyB.add(4 * e), 0); }
    if (id === 856 && rnd(2)) vSet(8561, rnd(3) ? 1 : 0);
    if (id === 862) {
      if (rnd(2)) vSet(8621, pick([0, 0, 2, 3]));
      if (rnd(3) === 0) vSet(8628, rnd(2));
      const f = fleetOfId(8621), p = planetOfId(8620);
      // The raiders near the planet or far, the human's fleets near them now and then.
      if (f && p && rnd(2)) { const b = A.Bolygok.add(0x188 * p); transientF32(sets, fleet(f).add(0x30), b.add(0x18).readFloat() + pick([5, 60, 150])); transientF32(sets, fleet(f).add(0x34), b.add(0x1c).readFloat()); }
      if (f && rnd(3) === 0) { const [x, y] = fxy(f); for (const g of mine().slice(0, 2)) { transientF32(sets, fleet(g).add(0x30), x + pick([3, 20, 45])); transientF32(sets, fleet(g).add(0x34), y); } }
    }
    if (id === 863 && rnd(3) === 0) transientU8(sets, A.El.add(5), 0);
    flagRec(0x10, []);
  }
}
const ELEM2F = [836, 839, 840, 843];
// The fleets' names (836's 8360..8362, 839's 8390, 840's 8401; 843 uses 834's 8340..8342, in ELEM2E_TEXTS).
const ELEM2F_TEXTS = [0x20a8, 0x20a9, 0x20aa, 0x20c6, 0x20d1];
const ELEM2F_COUNTERS = { 836: [8360, 8361, 8363, 8364], 839: [8398, 8398, 8391, 8392, 8399, 8399], 840: [8400, 8400, 8401, 8402, 8407, 8407, 8408, 8409],
  843: [8435, 8436, 8430] };
const ELEM2F_ACTS = { 836: ['ctr', 'ctr', 'ground', 'ground', 'daily', 'daily'], 839: ['ctr', 'ctr', 'ctr', 'ans', 'special', 'arrive', 'arrive', 'se', 'se'],
  840: ['ctr', 'ctr', 'ctr', 'special', 'special', 'ground', 'daily', 'daily'], 843: ['ctr', 'ctr', 'arrive', 'arrive', 'arrive', 'se', 'se'] };
function elem2fFuzz(pool, running, sets, prep, after) {
  const en = A.EN.readS32(), np = nPlanets(), live = running.filter((x) => ELEM2F.includes(x));
  const flagRec = (fl, extra, write) => stage('flag', [[A.Check, [1]]], { args: [fl].concat(extra || []) }, () => { prep(); A.Flags.add(fl).writeU8(1); if (write) write(); }, after);
  const ownerOf = (p) => A.Bolygok.add(0x188 * p + 0x20).readU8();
  const setOwner = (p, o) => { if (p) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), o); };
  const mine = () => liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en);
  // Empire e's planets given away for the record now and then (b(-e) finds none).
  const strip = (e) => { for (let p = 1; p <= np; p++) if (ownerOf(p) === e) setOwner(p, pick([en, 0])); };
  if (rnd(6) === 0 || !live.length) {
    const ids = pool.filter((id) => ELEM2F.includes(id) && !bucketHas(id));
    if (!ids.length) return;
    const id = pick(ids);
    // 836's target: planet 8351 the human's, another's or missing; empire 1 without planets now and then.
    if (id === 836) { const p = planetOfId(8351); if (p && rnd(3) === 0) setOwner(p, pick([en, 1, 0])); if (rnd(6) === 0) strip(1); }
    // 839's planets: kept (shown), one of them taken, or missing.
    if (id === 839) for (const b of [8391, 8392, 8393]) { const p = planetOfId(b); if (p && rnd(4) === 0) setOwner(p, pick([en, 10, 0])); }
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
    return;
  }
  const id = pick(live), act = pick(ELEM2F_ACTS[id]);
  if (act === 'ctr') {
    const ctr = pick(ELEM2F_COUNTERS[id]);
    if (id === 836 && ctr !== 8361 && rnd(4) === 0) strip(1);
    if (ctr === 8398) {
      for (const b of [8391, 8392, 8393]) { const p = planetOfId(b); if (p && rnd(3) === 0) setOwner(p, pick([en, 10, 0, 0])); }
      if (rnd(2)) vSet(8399, rnd(2));
    }
    if (ctr === 8399) {
      const f = fleetOfId(8390);
      if (f && rnd(3) === 0) transientS16(sets, fleet(f).add(0x16), 0);
      // The human's fleets far away (st_fGetNearest finds none: the hunter is sent off).
      if (rnd(4) === 0) for (const g of mine()) transientU8(sets, fleet(g).add(0x15), 0xff);
    }
    if (ctr === 8400) {
      if (rnd(2)) vSet(8400, pick([0, 1, 3]));
      // Fewer than 3 planets the human sees now and then; else others' planets given to the human for the record (840
      // hands the two poorest to empire 17), their +0x28 varied (equal values now and then).
      if (rnd(4) === 0) { const m = []; for (let p = 1; p <= np; p++) if (ownerOf(p) === en) m.push(p); for (const p of m.slice(rnd(3))) transientS16(sets, A.Bolygok.add(0x188 * p + 0xd8 + 2 * en), -1); }
      else {
        // Colonies of the others (a free planet has no colony data: BolygoElfoglal on it hung the game).
        const free = []; for (let p = 1; p <= np; p++) if (ownerOf(p) !== 0 && ownerOf(p) !== en && ownerOf(p) < 19) free.push(p);
        for (let k = 2 + rnd(3); k > 0 && free.length; k--) {
          const p = free.splice(rnd(free.length), 1)[0], b = A.Bolygok.add(0x188 * p);
          setOwner(p, en); transientS16(sets, b.add(0xd8 + 2 * en), pick([0, 1, 1])); transientS32(sets, b.add(0x28), pick([0, 100, 100, 2500, 50000]));
        }
      }
    }
    stage('flag', [[A.Check, [1]]], { args: [5, ctr] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(ctr); }, after);
  } else if (act === 'ans') {
    // 839's message read (flag 9).
    stage('flag', [[A.Check, [1]]], { args: [9, 8390, 0] }, () => { prep(); A.Flags.add(9).writeU8(1); A.Msg.writeS32(8390); A.Msg.add(4).writeS32(0); }, after);
  } else if (act === 'special') {
    // A colonisation (special 10) by the human or another empire; 839's planets taken, 840's v(8401) varied.
    if (id === 839) for (const b of [8391, 8392, 8393]) { const p = planetOfId(b); if (p && rnd(3) === 0) setOwner(p, pick([en, 10])); }
    if (id === 839 && rnd(2)) vSet(8399, rnd(2));
    if (id === 840 && rnd(4) === 0) vSet(8401, rnd(2));
    if (id === 840 && rnd(3) === 0) vSet(8400, pick([3, 4, 5, 6]));
    const args = [10, rnd(4) ? en : 1 + rnd(8), 1 + rnd(np), 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args });
  } else if (act === 'ground') {
    // A ground battle ends (flag 4) on 8351 (836) or anywhere (840), the attacker winning or not; 840: empire 17's
    // planets gone now and then, v(8409) 1.
    const p = id === 836 && rnd(4) ? planetOfId(8351) || 1 + rnd(np) : 1 + rnd(np);
    // 836 strips the ids of fleets 8350..8354: some fleets get them for the record.
    if (id === 836) { const all = liveFleets(); for (let k = rnd(3); k > 0 && all.length; k--) transientS16(sets, fleet(pick(all)).add(0x16), 8350 + rnd(5)); }
    if (id === 840) { if (rnd(2)) strip(17); if (rnd(2)) vSet(8409, pick([1, 1, 2])); }
    const m = mine(), f = m.length ? pick(m) : 0, won = rnd(3) ? 1 : 0;
    flagRec(4, [p, f, won], () => {
      A.Gr.writeS32(f); A.Gr.add(4).writeS32(f ? fleet(f).add(0x16).readS16() : 0); A.Gr.add(8).writeS32(en);
      A.Gr.add(12).writeS32(p); A.Gr.add(16).writeS32(ownerOf(p)); A.GrWin.writeU8(won); A.GrWin.add(1).writeU8(1 - won);
    });
  } else if (act === 'arrive') {
    // 839's fleet at a planet (type 1); 843's raiders at their point (type 0). Another fleet takes a missing id.
    const all = liveFleets();
    if (!all.length) return;
    const fid = id === 839 ? 8390 : 8430 + rnd(3), f = fleetOfId(fid) || pick(all), type = id === 839 ? (rnd(8) ? 1 : 0) : (rnd(8) ? 0 : 1);
    if (fleet(f).add(0x16).readS16() !== fid) transientS16(sets, fleet(f).add(0x16), fid);
    const a = [fid, f, type === 1 ? 1 + rnd(np) : 0, type, 0];
    flagRec(0, a, () => { for (let i = 0; i < 5; i++) A.Arr.add(4 * i).writeS32(a[i]); });
  } else if (act === 'se') {
    // A space battle ends with 839's fleet (gone now and then) or 843's raiders among the participants.
    const n = 2 + rnd(2), list = [], buf = Memory.alloc(0x30 * 4);
    spwBufs.push(buf);
    if (spwBufs.length > 64) spwBufs.shift();
    for (let i = 0; i < n; i++) {
      const fid = id === 839 && rnd(2) ? 8390 : pick([0, 8430, 8431, 8432, 8202]), owner = rnd(2) ? en : pick([10, 1]);
      const q = buf.add(0x30 * i); q.add(0x18).writeS32(owner); q.add(0x20).writeS32(fid); q.add(0x24).writeS32(fid); list.push([owner, fid]);
    }
    transientS32(sets, A.SpWin, rnd(2) ? list[0][0] : list[1][0]);
    const ids = id === 839 ? [8390] : [8430, 8431, 8432];
    for (const x of ids) { const f = fleetOfId(x); if (f && rnd(2)) transientS16(sets, fleet(f).add(0x16), 0); }
    stage('spwfinished', [[A.SpwFinished, [buf.toUInt32(), n, 1]]], { args: list }, prep, after);
  } else {
    // Daily: 836's fleets with ground units aboard or not, idle or not; 840 with empire 17's planets gone.
    if (id === 836) for (let k = 0; k < 3; k++) {
      const f = fleetOfId(8360 + k);
      if (f && rnd(2)) transientS32(sets, fleet(f).add(0x5c), pick([0, 0, 3]));
      if (f && rnd(2)) transientS16(sets, fleet(f).add(0x40), pick([0, 1]));
    }
    if (id === 840) { if (rnd(2)) strip(17); if (rnd(2)) vSet(8409, pick([1, 1, 2, 0])); }
    flagRec(0x10, []);
  }
}
const ELEM2E = [814, 815, 821, 832, 834];
// 814's ticker 8140, the fleets' names (821's convoy 8206 and pirates 8202, 832's and 834's fleets), 834's notice 8560.
const ELEM2E_TEXTS = [0x1fcc, 0x200e, 0x200a, 0x2080, 0x2081, 0x2082, 0x2094, 0x2095, 0x2096, 0x2097, 0x2098, 0x2170];
const ELEM2E_COUNTERS = { 814: [8140, 8141, 8141], 815: [8150, 8151, 8159], 821: [8219, 8219, 8200, 8200], 832: [8327, 8327, 8321, 8329],
  834: [8340, 8341, 8342, 8343, 8344] };
// What gets staged per running story (starts aside): counters, answers, ground battles, arrivals, space battle starts
// and ends, specials, daily checks.
const ELEM2E_ACTS = { 814: ['ctr', 'ctr', 'ans', 'ans', 'ground', 'ground', 'ground', 'daily'], 815: ['ctr', 'ctr', 'ground', 'ground', 'daily', 'daily', 'daily'],
  821: ['ctr', 'ctr', 'ctr', 'arrive', 'arrive', 'ss', 'ss', 'ss', 'se', 'se', 'se', 'daily', 'daily', 'daily'],
  832: ['ctr', 'ctr', 'ans', 'ground', 'ground', 'se', 'se', 'daily', 'daily', 'daily'], 834: ['ctr', 'ctr', 'ctr', 'ans', 'arrive', 'arrive', 'arrive', 'se', 'se', 'special', 'special', 'daily'] };
const ELEM2E_ANSWERS = { 814: [[6, 8140, 2], [6, 8140, 3], [6, 8140, 4], [7, 8140, 0], [8, 8140, 0], [8, 8140, 0]], 815: [], 821: [],
  832: [[9, 8320, 0]], 834: [[9, 8340, 0]] };
// Saved before the record and put back after it (the record sees the story's own writes).
function keepRegion(sets, q, n) { let was = null; sets.push([() => { was = q.readByteArray(n); }, () => q.writeByteArray(was)]); }
function elem2eFuzz(pool, running, sets, prep, after) {
  const en = A.EN.readS32(), np = nPlanets(), live = running.filter((x) => ELEM2E.includes(x));
  const flagRec = (fl, extra, write) => stage('flag', [[A.Check, [1]]], { args: [fl].concat(extra || []) }, () => { prep(); A.Flags.add(fl).writeU8(1); if (write) write(); }, after);
  const ownerOf = (p) => A.Bolygok.add(0x188 * p + 0x20).readU8();
  const setOwner = (p, o) => { if (p) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), o); };
  const mine = () => liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en);
  // A fleet of the human's put next to (x, y) for the record (within `r`, sometimes just beyond it).
  const near = (x, y, r) => {
    const m = mine();
    if (!m.length) return 0;
    const f = pick(m), q = fleet(f), a = rnd(360) * Math.PI / 180, d = pick([0, r * 0.3, r * 0.9, r - 0.5, r + 0.5, r * 1.5]);
    transientF32(sets, q.add(0x30), x + d * Math.cos(a)); transientF32(sets, q.add(0x34), y + d * Math.sin(a));
    return f;
  };
  const pxy = (p) => [A.Bolygok.add(0x188 * p + 0x18).readFloat(), A.Bolygok.add(0x188 * p + 0x1c).readFloat()];
  const fxy = (f) => [fleet(f).add(0x30).readFloat(), fleet(f).add(0x34).readFloat()];
  if (rnd(6) === 0 || !live.length) {
    const ids = pool.filter((id) => ELEM2E.includes(id) && !bucketHas(id));
    if (!ids.length) return;
    const id = pick(ids);
    // 815's planets: the human's view of them varied (b(·, 1) finds them anyway).
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
    return;
  }
  const id = pick(live), act = pick(ELEM2E_ACTS[id]);
  if (act === 'ctr') {
    const ctr = pick(ELEM2E_COUNTERS[id]);
    if (ctr === 8219 && rnd(2)) transientS32(sets, A.HanyB.add(4 * en), pick([1, 2, 3]));
    if (ctr === 8200) { if (rnd(2)) setOwner(planetOfId(8200), pick([10, 10, en, 0])); if (rnd(2)) vSet(8201, pick([0, 1, 2, 3])); if (rnd(3) === 0) vSet(8211, rnd(3)); }
    if (ctr === 8150 || ctr === 8151) vSet(8150, rnd(2));
    if (ctr === 8327 || ctr === 8329) {
      // Empire 13's planets taken now and then (b(-13, 1) finds none).
      if (rnd(2)) for (let p = 1; p <= np; p++) if (ownerOf(p) === 13) setOwner(p, pick([en, 0]));
    }
    if (ctr >= 8340 && ctr <= 8344 && rnd(4) === 0) for (let p = 1; p <= np; p++) if (ownerOf(p) === 1) setOwner(p, pick([en, 0, 1]));
    stage('flag', [[A.Check, [1]]], { args: [5, ctr] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(ctr); }, after);
  } else if (act === 'ans') {
    const [kind, b, part] = pick(ELEM2E_ANSWERS[id]);
    if (kind === 8 && rnd(2)) transientS32(sets, A.Adat.add(0x58), rnd(2));
    stage('flag', [[A.Check, [1]]], { args: [kind, b, part] }, () => { prep(); A.Flags.add(kind).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(part); }, after);
  } else if (act === 'ground') {
    // A ground battle starts (flag 2: 814) or ends (flag 4) on 814's base, 815's planets or anywhere (832, the old owner
    // 13 now and then); the doomsday building ('+') intact or not.
    const base = id === 814 ? [8140] : id === 815 ? [8150, 8151, 8152] : [8320, 8321, 8322];
    const p = rnd(4) ? planetOfId(pick(base)) || 1 + rnd(np) : 1 + rnd(np), fl = id === 814 && rnd(3) === 0 ? 2 : 4;
    if (rnd(2)) setOwner(p, pick([en, en, id === 814 ? 10 : id === 815 ? 12 : 13, 0]));
    if (id === 815) for (const b of base) if (rnd(3) === 0) setOwner(planetOfId(b), pick([en, 12]));
    if (id === 814 && rnd(2)) {
      const n = A.HanyEpulet.add(4 * p).readS32(), e = A.Epuletek.add(4 * p).readPointer(), slots = [];
      for (let k = 1; k <= n && k < 400; k++) if (e.add(0x20 * k).readU8() !== 0) slots.push(k);
      for (let k = rnd(3); k > 0 && slots.length; k--) { const r = e.add(0x20 * pick(slots)); transientU8(sets, r, 0x2b); transientS16(sets, r.add(8), pick([0, 0, 1])); }
      // The doomsday building ('+') wiped out (every one damaged) or some intact.
      const gone = rnd(2);
      for (const k of slots) if (e.add(0x20 * k).readU8() === 0x2b && (gone || rnd(2))) transientS16(sets, e.add(0x20 * k + 8), gone ? pick([1, 30]) : pick([0, 1, 30]));
    }
    if (id === 832 && rnd(2)) for (let q = 1; q <= np; q++) if (ownerOf(q) === 13 && rnd(2)) setOwner(q, pick([en, 0]));
    const m = mine(), f = m.length ? pick(m) : 0, won = rnd(2), old = id === 832 && rnd(2) ? 13 : pick([en, 10, 12, 0]);
    flagRec(fl, [p, f, won, old], () => {
      A.Gr.writeS32(f); A.Gr.add(4).writeS32(f ? fleet(f).add(0x16).readS16() : 0); A.Gr.add(8).writeS32(en);
      A.Gr.add(12).writeS32(p); A.Gr.add(16).writeS32(old); A.GrWin.writeU8(won); A.GrWin.add(1).writeU8(1 - won);
    });
  } else if (act === 'arrive') {
    // Arrivals at a planet: 821's convoy (anywhere; v(8218) varied) or its pirates going home (8203 at 8200); one of
    // 834's fleets at 8340 with fleets of the human's (or others) next to it.
    const all = liveFleets();
    if (!all.length) return;
    let fid, f, tgt;
    if (id === 821) {
      fid = rnd(3) ? 8207 : 8203; f = fleetOfId(fid) || pick(all);
      tgt = fid === 8203 && rnd(2) ? planetOfId(8200) || 1 + rnd(np) : 1 + rnd(np);
      if (rnd(3) === 0) vSet(8218, rnd(2));
      if (rnd(3) === 0) transientS32(sets, A.HanyB.add(4 * en), pick([1, 2, 3]));
    } else {
      fid = 8340 + rnd(5); f = fleetOfId(fid) || pick(all);
      tgt = rnd(4) ? planetOfId(8340) || 1 + rnd(np) : 1 + rnd(np);
      const p = planetOfId(8340);
      if (p) { const [x, y] = pxy(p); for (let k = rnd(3); k > 0; k--) near(x, y, 20); }
    }
    if (fleet(f).add(0x16).readS16() !== fid) transientS16(sets, fleet(f).add(0x16), fid);
    const a = [fid, f, tgt, rnd(8) ? 1 : 0, 0];
    flagRec(0, a, () => { for (let i = 0; i < 5; i++) A.Arr.add(4 * i).writeS32(a[i]); });
  } else if (act === 'ss') {
    // A space battle starts between the pirates and the convoy (spFlottak), the human's fleets round the convoy; the
    // list, its count and the flags the escort sets are put back after the record.
    const all = liveFleets();
    if (all.length < 2) return;
    // The convoy and the pirates (another fleet takes the id for the record when one is missing).
    let cv = fleetOfId(8207), pr = fleetOfId(8202);
    if (!cv && rnd(2)) { cv = pick(all); transientS16(sets, fleet(cv).add(0x16), 8207); }
    if (!pr && rnd(2)) { pr = pick(all.filter((f) => f !== cv)); transientS16(sets, fleet(pr).add(0x16), 8202); }
    const a = cv && rnd(6) ? cv : pick(all), b = pr && rnd(6) ? pr : pick(all);
    if (cv) { const [x, y] = fxy(cv); for (let k = rnd(4); k > 0; k--) near(Math.trunc(x), Math.trunc(y), 10); }
    keepRegion(sets, A.SpFl0, 0x2c); keepRegion(sets, va(0xf26174), 4); keepRegion(sets, A.Adat.add(0x88), 4); keepRegion(sets, A.Adat.add(0x94), 8);
    const [s1, s2] = rnd(2) ? [a, b] : [b, a];
    transientS16(sets, A.SpFlottak, s1); transientS16(sets, A.SpFlottak.add(6), s2);
    flagRec(1, [s1, s2]);
  } else if (act === 'se') {
    // A space battle ends with the convoy (or anything) among the participants; the convoy gone now and then.
    const n = 2 + rnd(2), list = [], buf = Memory.alloc(0x30 * 4);
    spwBufs.push(buf);
    if (spwBufs.length > 64) spwBufs.shift();
    for (let i = 0; i < n; i++) {
      const fid = id === 821 && rnd(2) ? 8207 : pick([0, 8202, 8320, 8340 + rnd(5)]), owner = rnd(2) ? en : pick([9, 10, 13, 1]);
      const q = buf.add(0x30 * i); q.add(0x18).writeS32(owner); q.add(0x20).writeS32(fid); q.add(0x24).writeS32(fid); list.push([owner, fid]);
    }
    transientS32(sets, A.SpWin, rnd(2) ? list[0][0] : list[1][0]);
    const cv = fleetOfId(8207);
    if (id === 821 && cv && rnd(2)) transientS16(sets, fleet(cv).add(0x16), 0);
    if (id === 832 && rnd(2)) for (let p = 1; p <= np; p++) if (ownerOf(p) === 13) setOwner(p, pick([en, 0]));
    // Empire 13's fleets gone (832) / 834's five fleets gone (their ids taken for the record), v(8349) set.
    if (id === 832 && rnd(2)) for (const f of liveFleets()) if (fleet(f).add(0x15).readU8() === 13) transientU8(sets, fleet(f).add(0x15), pick([0xff, en]));
    if (id === 834 && rnd(2)) { if (rnd(2)) vSet(8349, 1); for (let k = 0; k < 5; k++) { const f = fleetOfId(8340 + k); if (f && rnd(4)) transientS16(sets, fleet(f).add(0x16), 0); } }
    if (id === 821 && !cv && rnd(2)) { const all = liveFleets(); if (all.length) transientS16(sets, fleet(pick(all)).add(0x16), 8207); }
    if (id === 821 && rnd(2) && !fleetOfId(8202)) { const all = liveFleets(); if (all.length) transientS16(sets, fleet(pick(all)).add(0x16), 8202); }
    stage('spwfinished', [[A.SpwFinished, [buf.toUInt32(), n, 1]]], { args: list }, prep, after);
  } else if (act === 'special') {
    // A colonisation (special 10) of planet 8340, by the human or another empire.
    const p = planetOfId(8340), args = [10, rnd(3) ? en : 1 + rnd(8), p && rnd(4) ? p : 1 + rnd(np), 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args });
  } else {
    // Daily: 815's zone (fleets of the human's inside the triangle; tech 177 found), 821's convoy (idle, moving to a
    // planet of the human's or not; the base taken), 832's guards (the human's fleets near them; their planets
    // lost), 834's fleets idle.
    if (id === 815) {
      const ax = [0, 1, 2].map((k) => A.Adat.add(8 + 4 * k).readFloat()), ay = [0, 1, 2].map((k) => A.Adat.add(0x14 + 4 * k).readFloat());
      const cx = (ax[0] + ax[1] + ax[2]) / 3, cy = (ay[0] + ay[1] + ay[2]) / 3;
      for (let k = rnd(3); k > 0; k--) { const m = mine(); if (!m.length) break; const q = fleet(pick(m)), t = rnd(2) ? 0.5 * rnd(3) : 1.2; const v = rnd(3); transientF32(sets, q.add(0x30), cx + t * (ax[v] - cx)); transientF32(sets, q.add(0x34), cy + t * (ay[v] - cy)); }
      if (rnd(3) === 0) vSet(8159, rnd(2));
      if (rnd(3) === 0) vSet(8152, 1);
      if (rnd(3) === 0) transientU8(sets, A.Techs.add(0x114 * 177 + 0x45 + en), pick([3, 5, 1]));
      if (rnd(4) === 0) vSet(8158, 1);
    }
    if (id === 821) {
      const cv = fleetOfId(8207);
      if (cv && rnd(2)) { const q = fleet(cv); transientS16(sets, q.add(0x40), pick([0, 1, 1, 2])); transientS16(sets, q.add(0x42), pick([0, 1, 1, 2])); transientF32(sets, q.add(0x44), 1 + rnd(np)); }
      if (rnd(3) === 0) vSet(8217, pick([0, 1 + rnd(np)]));
      if (rnd(4) === 0) vSet(8219, rnd(2));
      if (rnd(4) === 0) vSet(8218, rnd(2));
      if (rnd(3) === 0) setOwner(planetOfId(8200), pick([10, en]));
      for (const fid of [8202, 8203]) { const f = fleetOfId(fid); if (f && rnd(2)) transientS16(sets, fleet(f).add(0x40), pick([0, 1])); }
      if (rnd(3) === 0) transientS32(sets, A.HanyB.add(4 * en), pick([1, 2, 3]));
    }
    if (id === 832) for (const fid of [8320, 8321, 8322]) {
      const f = fleetOfId(fid), p = planetOfId(fid);
      if (f && rnd(2)) { const [x, y] = fxy(f); near(x, y, 20); }
      if (p && rnd(4) === 0) setOwner(p, pick([en, 13, 0]));
      if (f && rnd(3) === 0) transientS32(sets, fleet(f).add(0x5c), rnd(2));
    }
    if (id === 834) for (let k = 0; k < 5; k++) { const f = fleetOfId(8340 + k); if (f && rnd(2)) transientS16(sets, fleet(f).add(0x40), pick([0, 1])); }
    flagRec(0x10, []);
  }
}
const ELEM2D = [801, 804, 806, 807, 809];
const ELEM2D_TEXTS = [0x1f4a, 0x1f4b];
const ELEM2D_COUNTERS = { 801: [8001, 8002, 8003], 804: [8044, 8045, 8046, 8049, 8049], 806: [8069, 8069, 8069, 8060, 8060, 8061],
  807: [8078, 8070, 8071, 8071, 8071, 8074, 8079], 809: [8099, 8099, 8090, 8091] };
const ELEM2D_ANSWERS = { 801: [[9, 5001, 0], [9, 5001, 0], [7, 5001, 0]], 804: [], 806: [], 809: [],
  807: [[6, 6840, 4], [6, 6840, 5], [6, 6840, 6], [6, 6841, 3], [6, 6841, 4], [6, 8074, 3], [6, 8074, 4], [6, 8074, 5], [6, 8074, 2]] };
// Buildings for a planet of the human's (806 needs more than 10 intact ones with police, shields now and then):
// EpuletNewSearchLocation(p, kind, true, 0, -1, -1) called natively before the record (not recorded).
function elem2dBuild(p) {
  const kinds = [7, 7, 0x20 + rnd(4), 2, 5, 6, 2, 5, 6, 3, 4, 7];
  for (let k = 2 + rnd(10); k > 0; k--) call(va(0x53cca0), [p, pick(kinds), 1, 0, -1, -1], null, false);
}
function slotReadable(k) { try { fleet(k).add(0x7f).readU8(); return true; } catch (e) { return false; } }
function elem2dFuzz(pool, running, sets, prep, after) {
  const en = A.EN.readS32(), np = nPlanets(), own = humanPlanets(), live = running.filter((x) => ELEM2D.includes(x));
  const flagRec = (fl, extra, write) => stage('flag', [[A.Check, [1]]], { args: [fl].concat(extra || []) }, () => { prep(); A.Flags.add(fl).writeU8(1); if (write) write(); }, after);
  const ownerOf = (p) => A.Bolygok.add(0x188 * p + 0x20).readU8();
  const ships = () => { const o = [], n = A.HajoCount.readS32(), hp = A.Hajok.readPointer(); for (let k = 1; k <= n; k++) o.push([k, hp.add(6 * k).readS16(), hp.add(6 * k + 2).readS16()]); return o; };
  const shipDesign = (k, d) => transientS16(sets, A.Hajok.readPointer().add(6 * k + 2), d);
  const mine = () => liveFleets().filter((f) => fleet(f).add(0x15).readU8() === en);
  // 806's counters with planet 8060's id gone (its daily check lost the planet): C(8060) divides by the slot count of
  // planet 0 (0) and C(8061) hands planet 0 to empire 10, which the original does too; stopped here, not recorded.
  if (bucketHas(806) && !planetOfId(8060)) {
    const n = va(0xf23a0c).readS32(), tab = va(0xf24388); // sHanySzamlalo, sSzamlalok {id, ticks} from 1
    for (let k = 1; k <= n && k <= 300; k++) { const id = tab.add(8 * k).readS32(); if ((id === 8060 || id === 8061) && tab.add(8 * k + 4).readS32() >= 0) tab.add(8 * k + 4).writeS32(-1); }
  }
  const c = rnd(24);
  if (c < 4 || !live.length) {
    const ids = pool.filter((id) => ELEM2D.includes(id) && !bucketHas(id));
    if (!ids.length) return;
    const id = pick(ids);
    // 804's bases belong to empire 10 first (StoryInit804, exe 0x5c49c0: AutoAddtank for another owner may crash the
    // game). Done natively, not recorded.
    if (id === 804 && ownerOf(planetOfId(8040)) !== 10) { call(va(0x5c49c0), [], null, false); return; }
    if (id === 806 && own.length && rnd(2)) elem2dBuild(pick(own));
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
    return;
  }
  const id = pick(live);
  if (c < 11) {
    const ctr = pick(ELEM2D_COUNTERS[id]);
    if (ctr === 8001) vSet(8001, pick([0, 1, 2]));
    if (ctr === 8002 || ctr === 8003) {
      // The loaded ships: live ones, free ones (design 0x7fff) or none.
      const sh = ships().filter((s) => slotReadable(s[0])), used = sh.filter((s) => s[2] !== 0x7fff), free = sh.filter((s) => s[2] === 0x7fff);
      for (const v of [8017, 8018]) if (rnd(2)) vSet(v, pick([0, 0, used.length ? pick(used)[0] : 0, free.length ? pick(free)[0] : 0]));
    }
    if (ctr >= 8044 && ctr <= 8046) { const p = planetOfId(ctr - 4); if (p && rnd(3) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([10, en, 0])); }
    if (ctr === 8049) {
      // Raiders standing on a base without an id (st_fMarkNearPlanet), two of them close (OsszevonasCheck).
      const ten = liveFleets().filter((f) => fleet(f).add(0x15).readU8() === 10);
      for (let k = rnd(3); k > 0 && ten.length; k--) {
        const f = pick(ten), q = fleet(f), b = planetOfId(8040 + rnd(3));
        if (rnd(2)) transientS16(sets, q.add(0x16), pick([0, 8049]));
        if (b && rnd(2)) { transientF32(sets, q.add(0x30), A.Bolygok.add(0x188 * b + 0x18).readFloat()); transientF32(sets, q.add(0x34), A.Bolygok.add(0x188 * b + 0x1c).readFloat()); }
        else if (rnd(2)) {
          // Next to another raider: dy² + dx² round the 10 the merge tests.
          const g = pick(ten), [dx, dy] = pick([[3, 1], [1, 3], [3, 0], [2, 2], [0, 0], [3, 2], [rnd(4), -rnd(3)]]);
          transientF32(sets, q.add(0x30), fleet(g).add(0x30).readFloat() + dx); transientF32(sets, q.add(0x34), fleet(g).add(0x34).readFloat() + dy);
        }
      }
    }
    if (ctr === 8069 && own.length && rnd(3)) elem2dBuild(pick(own));
    if (ctr === 8069) for (const p of own) if (rnd(3) === 0) transientS16(sets, A.Bolygok.add(0x188 * p + 0xd8 + 2 * en), pick([0, 1, 5]));
    // Intact counts of 10 or 11 exactly: other intact slots damaged (+8, i16) for the record.
    if (ctr === 8069) for (const p of own) if (rnd(4)) {
      const n = A.HanyEpulet.add(4 * p).readS32(), e = A.Epuletek.add(4 * p).readPointer(), intact = [];
      for (let k = 1; k <= n && k < 400; k++) { const r = e.add(0x20 * k); if (r.readU8() !== 0 && r.add(8).readS16() === 0) intact.push(k); }
      // Keep one police station intact; any other slot may go.
      const keep = intact.find((k) => e.add(0x20 * k).readU8() === 7), others = intact.filter((k) => k !== keep);
      for (let x = intact.length - pick([10, 11]); x > 0 && others.length; x--) transientS16(sets, e.add(0x20 * others.splice(rnd(others.length), 1)[0] + 8), pick([1, 50]));
    }
    if (ctr === 8061 || ctr === 8060) {
      const p = planetOfId(8060);
      if (!p) return;
      if (rnd(4) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([en, 10, 0]));
    }
    if (ctr === 8071) {
      // Fleets of the human's next to planet 8070, some of the human's designs' hull classes changed for the record.
      const p = planetOfId(8070), m = mine();
      if (p && m.length) for (let k = 1 + rnd(3); k > 0; k--) {
        const g = pick(m), q = fleet(g);
        // Its ships' designs mostly of the hull classes 807 lets through (5..8; else 11 or others).
        if (rnd(2)) { const tp = A.Tervek.add(4 * en).readPointer(); for (const s of ships()) if (s[1] === g && s[2] > 0 && s[2] !== 0x7fff) transientS32(sets, tp.add(0x84 * s[2] + 0x48), pick([5, 6, 5, 6, 7, 8, 11])); }
        transientF32(sets, q.add(0x30), A.Bolygok.add(0x188 * p + 0x18).readFloat() + rnd(30) - 10);
        transientF32(sets, q.add(0x34), A.Bolygok.add(0x188 * p + 0x1c).readFloat() + rnd(30) - 10);
      }
      if (rnd(3) === 0) { const tn = A.TervCount.add(4 * en).readS32(), tp = A.Tervek.add(4 * en).readPointer(); for (let k = rnd(4); k > 0; k--) transientS32(sets, tp.add(0x84 * (1 + rnd(tn)) + 0x48), pick([5, 6, 7, 8, 11, 2, 0])); }
      // The vehicles of the human's fleets: counts round 20, their designs' +0x36 byte (0, 1, 2: the exe tests the byte).
      if (rnd(4)) {
        const tkn = A.TankN.readS32(), tp = A.Tervek.add(4 * en).readPointer();
        for (let k = 1; k <= tkn && k < 2000; k++) {
          const r = A.Tanks.add(8 * k), pl = r.readS16(), d = r.add(2).readS16();
          if (d === 0x7fff || !m.includes(pl)) continue;
          transientS32(sets, r.add(4), pick([19, 20, 21, 20, 21, 1]));
          if (d > 0 && rnd(4)) transientU8(sets, tp.add(0x84 * d + 0x36), pick([0, 1, 2, 2, 2]));
        }
      }
    }
    if (ctr === 8079 && rnd(3) === 0) {
      // A ship of one of the pirates' story designs (7801, 7811, 7821) in a fleet of the human's.
      const tn = A.TervCount.add(4 * en).readS32(), tp = A.Tervek.add(4 * en).readPointer(), m = mine(), sh = ships().filter((s) => m.includes(s[1]));
      if (sh.length && tn >= 40) { const d = 40 + rnd(tn - 39); transientS32(sets, tp.add(0x84 * d + 0x80), pick([7801, 7811, 7821])); shipDesign(pick(sh)[0], d); }
    }
    if (ctr === 8099) {
      const p = planetOfId(8099);
      if (p && rnd(2)) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([en, en, 13, 0]));
      if (p && rnd(2)) transientS32(sets, A.Bolygok.add(0x188 * p + 0x28), pick([1000, 1001, 1031, 5000, rnd(200000)]));
    }
    stage('flag', [[A.Check, [1]]], { args: [5, ctr] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(ctr); }, after);
  } else if (c < 14 && ELEM2D_ANSWERS[id].length) {
    const [kind, b, part] = pick(ELEM2D_ANSWERS[id]);
    stage('flag', [[A.Check, [1]]], { args: [kind, b, part] }, () => { prep(); A.Flags.add(kind).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(part); }, after);
  } else if (c < 20 && id === 801) {
    // A fleet of the human's arrives at 8010 (its ships of design A) or at a planet of the human's / a free one it sees
    // (its ships of design B, v(8017)/v(8018) on them).
    const m = mine(), all = liveFleets();
    if (!all.length) return;
    // The transports' fleets (named _s(5, 8010) by a load) now and then: their landing tests fleet slot k's name.
    const tq = Memory.alloc(16); sCall(tq, 5, 0x1f4a);
    const tname = tq.readPointer().isNull() ? '' : latin1(tq.readPointer(), 0x15), named = m.filter((x) => latin1(fleet(x), 0x15) === tname);
    const f = named.length && rnd(2) ? pick(named) : m.length && rnd(6) ? pick(m) : pick(all), da = A.Adat.add(0x3c).readS32(), db = A.Adat.add(0x64).readS32();
    // Only ships whose index is a fleet slot too: 801 renames fleet slot k (the ship's index) and reads past the fleet
    // array for a larger one (the original crashes the same way).
    const fs = ships().filter((s) => s[1] === f && slotReadable(s[0]));
    let tgt;
    if (rnd(2)) {
      tgt = planetOfId(8010) || 1 + rnd(np);
      for (const s of fs) if (rnd(3)) shipDesign(s[0], da);
      if (rnd(4)) vSet(8009, rnd(6) ? 1 : 0);
      // Without its custom name (bit 11) the loaded fleet gets the transports' name.
      if (rnd(2)) { const q = fleet(f).add(0x52); transientS16(sets, q, q.readU16() & ~0x800); }
      if (rnd(3) === 0) vSet(8019, pick([0, 1, 2, 3]));
      if (rnd(3) === 0 && fs.length) vSet(pick([8017, 8018]), pick(fs)[0]);
    } else {
      const vis = []; for (let p = 1; p <= np; p++) { const o = ownerOf(p); if ((o === en || o === 0) && A.Bolygok.add(0x188 * p + 0xd8 + 2 * en).readS16() >= 0) vis.push(p); }
      const ownVis = vis.filter((p) => ownerOf(p) === en);
      tgt = ownVis.length && rnd(2) ? pick(ownVis) : vis.length && rnd(5) ? pick(vis) : 1 + rnd(np);
      const chosen = fs.filter(() => rnd(2)).slice(0, 2);
      for (const s of chosen) shipDesign(s[0], db);
      if (chosen.length) { vSet(8017, chosen[0][0]); vSet(8018, chosen.length > 1 ? chosen[1][0] : pick([0, 0, chosen[0][0]])); }
      if (rnd(4)) vSet(8015, pick([0, 0, tgt, 1 + rnd(np)]));
    }
    const a = [fleet(f).add(0x16).readS16(), f, tgt, rnd(8) ? 1 : 0, 0];
    flagRec(0, a, () => { for (let i = 0; i < 5; i++) A.Arr.add(4 * i).writeS32(a[i]); });
  } else if (c < 20 && (id === 807 || id === 809)) {
    // A space battle ends with fleet 8070 / 8091 among the participants (the winner set; the fleet gone now and then).
    const n = 2 + rnd(2), list = [], buf = Memory.alloc(0x30 * 4);
    spwBufs.push(buf);
    if (spwBufs.length > 64) spwBufs.shift();
    const sid = id === 807 ? 8070 : 8091;
    for (let i = 0; i < n; i++) {
      const fid = rnd(2) ? sid : pick([0, 8072, 8092]), owner = rnd(2) ? en : pick([10, 13, 9]);
      const q = buf.add(0x30 * i); q.add(0x18).writeS32(owner); q.add(0x20).writeS32(fid); q.add(0x24).writeS32(fid); list.push([owner, fid]);
    }
    transientS32(sets, A.SpWin, rnd(2) ? list[0][0] : list[1][0]);
    const f = fleetOfId(sid);
    if (f && rnd(2)) transientS16(sets, fleet(f).add(0x16), 0);
    stage('spwfinished', [[A.SpwFinished, [buf.toUInt32(), n, 1]]], { args: list }, prep, after);
  } else if (c < 22 && (id === 807 || id === 809)) {
    // A space battle starts: spFlottak with an empire 13 fleet (809) or a fleet of the human's (807's pirate ships).
    const all = liveFleets();
    if (!all.length) return;
    const pref = all.filter((f) => fleet(f).add(0x15).readU8() === (id === 809 ? 13 : en));
    const a = pref.length && rnd(3) ? pick(pref) : pick(all), b = pick(all);
    transientS16(sets, A.SpFlottak, rnd(2) ? a : b); transientS16(sets, A.SpFlottak.add(6), rnd(2) ? b : a);
    flagRec(1, [a, b]);
  } else if (c < 23 && (id === 804 || id === 809)) {
    // A ground battle ends (flag 4) on a base / an empire 13 planet; the bases sometimes no longer theirs.
    const base = id === 804 ? [8040, 8041, 8042] : [8091, 8092, 8093, 8094], owner = id === 804 ? 10 : 13;
    for (const b of base) { const p = planetOfId(b); if (p && rnd(2)) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), rnd(3) ? en : owner); }
    if (id === 804 && rnd(3) === 0) vSet(8049, rnd(2));
    const p = rnd(4) ? planetOfId(pick(base)) || 1 + rnd(np) : 1 + rnd(np), m = mine(), f = m.length ? pick(m) : 0, won = rnd(2);
    flagRec(4, [p, f, won], () => {
      A.Gr.writeS32(f); A.Gr.add(4).writeS32(f ? fleet(f).add(0x16).readS16() : 0); A.Gr.add(8).writeS32(en);
      A.Gr.add(12).writeS32(p); A.Gr.add(16).writeS32(owner); A.GrWin.writeU8(won); A.GrWin.add(1).writeU8(1 - won);
    });
  } else {
    // Daily: 806's police (working bit 0, strike bit 4 of +0x19) on 8060.
    if (id === 806) {
      const p = planetOfId(8060);
      if (p && rnd(4) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([en, 10]));
      if (p && rnd(2)) {
        const n = A.HanyEpulet.add(4 * p).readS32(), e = A.Epuletek.add(4 * p).readPointer();
        for (let k = 1; k <= n && k < 400; k++) if (e.add(0x20 * k).readU8() === 7 && rnd(2)) transientU8(sets, e.add(0x20 * k + 0x19), pick([1, 0x11, 0, 1]));
      }
    }
    flagRec(0x10, []);
  }
}
function elemFuzz() {
  const pool = cfg.inline && cfg.inline.length ? ELEM.filter((x) => cfg.inline.includes(x)) : ELEM;
  const c = rnd(24), en = A.EN.readS32(), np = nPlanets(), running = pool.filter(bucketHas);
  for (const id of ELEM_PLANETS) {
    if (planetOfId(id)) continue;
    const other = []; for (let p = 1; p <= np; p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() !== en) other.push(p);
    if (other.length) bAddAzon(pick(other), id);
  }
  const sets = [];
  const prep = () => sets.forEach((x) => x[0]()), after = () => sets.slice().reverse().forEach((x) => x[1]());
  const pickRare = (ids) => { const r = ids.filter((x) => ELEM_RARE.includes(x)); return r.length && rnd(2) ? pick(r) : pick(ids); };
  if (pool.some((x) => ELEM2G.includes(x)) && rnd(3)) { elem2gFuzz(pool, running, sets, prep, after); return; }
  if (pool.some((x) => ELEM2F.includes(x)) && rnd(3)) { elem2fFuzz(pool, running, sets, prep, after); return; }
  if (pool.some((x) => ELEM2E.includes(x)) && rnd(3)) { elem2eFuzz(pool, running, sets, prep, after); return; }
  if (pool.some((x) => ELEM2D.includes(x)) && rnd(3)) { elem2dFuzz(pool, running, sets, prep, after); return; }
  if (pool.some((x) => ELEM2C.includes(x)) && rnd(3)) { elem2cFuzz(pool, running, sets, prep, after); return; }
  if (pool.some((x) => ELEM2B.includes(x)) && rnd(3)) { elem2bFuzz(pool, running, sets, prep, after); return; }
  if (pool.some((x) => ELEM2A.includes(x)) && rnd(3)) { elem2aFuzz(pool, running, sets, prep, after); return; }
  // Empires 1 and 5 alive or not, story 864's relation and the human's money, for one record.
  const empires = () => {
    for (const e of [1, 5]) if (e !== en && rnd(4) === 0) transientU8(sets, A.El.add(e), rnd(2));
    if (rnd(4) === 0) transientU8(sets, A.Viszony.add((en * 19 + 5) * 0x26 + 0x25), 1 + rnd(5));
    if (rnd(3) === 0) {
      const m = A.Money.add(8 * en), x = pick([50000, 100000, 100001, 250000.5, 3e6]);
      let was = 0;
      sets.push([() => { was = m.readDouble(); m.writeDouble(x); }, () => m.writeDouble(was)]);
    }
  };
  if (c < 3 || !running.length) {
    // 808 and 854 start only from their own staging (808 needs an empire with designs: AutoAddtank).
    const ids = pool.filter((id) => !bucketHas(id) && !ELEM2B.includes(id) && !ELEM2C.includes(id) && !ELEM2D.includes(id) && !ELEM2E.includes(id) && !ELEM2F.includes(id) && !ELEM2G.includes(id));
    if (!ids.length) return;
    const id = pickRare(ids);
    // 816 (and 818, which starts it) belong to a storyline that first gives planet 8160 to empire 10
    // (StoryInit8160, exe 0x5c4d40); its tanks for any other owner crash the game. Done natively, not recorded.
    if ((id === 816 || id === 818) && A.Bolygok.add(0x188 * planetOfId(8160) + 0x20).readU8() !== 10) { call(va(0x5c4d40), [], null, false); return; }
    if (id === 835) vSet(8359, pick([1 + rnd(8), 10, 11, 16]));
    if (id === 817) { vSet(7250, rnd(2)); const q = planetOfId(8011); if (q && rnd(2)) transientU8(sets, A.Bolygok.add(0x188 * q + 0x20), en); }
    if (id === 864 || id === 865 || id === 866) empires();
    if (id === 831 && rnd(2)) transientU8(sets, A.Techs.add(0x114 * 162 + 0x45 + en), pick([1, 5]));
    if (id === 841 && rnd(2)) transientU8(sets, A.Techs.add(0x114 * 6 + 0x45 + en), pick([1, 5]));
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
  } else if (c < 5) {
    // A counter of a running episode runs out at the next tick.
    const id = pickRare(running), list = ELEM_COUNTERS[id];
    if (list) szStart(pick(list), rnd(2));
  } else if (c < 10) {
    // A staged counter with the world it tests set up.
    const id = pickRare(running), list = ELEM_COUNTERS[id];
    if (!list) return;
    const ctr = pick(list);
    if (ctr === 8183 || ctr === 8209) { const p = planetOfId(ctr === 8183 ? 8160 : 8200); if (p && rnd(2)) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([10, en, 0])); }
    if (ctr === 8201) transientS32(sets, A.HanyB.add(4 * en), pick([1, 2, 3]));
    // 837: most of the owned planets the human sees become its own now and then (more than two thirds wins).
    if (ctr === 8370) {
      if (rnd(2)) for (let p = 1; p <= np; p++) { const b = A.Bolygok.add(0x188 * p); if (b.add(0x20).readU8() !== 0 && b.add(0xd8 + 2 * en).readS16() >= 0 && rnd(5)) transientU8(sets, b.add(0x20), en); }
      else for (let k = rnd(12); k > 0; k--) transientU8(sets, A.Bolygok.add(0x188 * (1 + rnd(np)) + 0x20), pick([en, en, en, 0, 1 + rnd(8)]));
    }
    if (ctr === 8311) {
      for (let k = rnd(4); k > 0; k--) { const t = 161 + rnd(11); transientU8(sets, A.Techs.add(0x114 * t + 0x45 + en), pick([1, 4, 5])); }
      // The human's research time left on one of them (status other than 4).
      if (rnd(2)) { const t = 161 + rnd(11); transientS32(sets, A.Techs.add(0x114 * t + 0x58 + 4 * en), pick([0, 1, 50])); }
    }
    if (ctr === 8640) empires();
    // 835's st_bGetLegerosebb: the human holds several (built-up) planets it sees for the record.
    if (ctr === 8351 && rnd(3)) for (let k = 2 + rnd(5); k > 0; k--) { const b = A.Bolygok.add(0x188 * (1 + rnd(np))); transientU8(sets, b.add(0x20), en); if (rnd(4)) transientS16(sets, b.add(0xd8 + 2 * en), pick([1, 2, 0])); }
    stage('flag', [[A.Check, [1]]], { args: [5, ctr] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(ctr); }, after);
  } else if (c < 13) {
    // A message read (9), answered (6) or promoted (7).
    const msgs = [];
    if (bucketHas(820)) msgs.push([9, 8206, 0]);
    if (bucketHas(830)) msgs.push([9, 8300, 0]);
    if (bucketHas(831)) msgs.push([9, 8310, 0]);
    if (bucketHas(864)) msgs.push([6, 8641, 2], [6, 8641, 3], [6, 8641, 4], [7, 8641, 0], [6, 8644, 2], [6, 8644, 3], [7, 8644, 0]);
    if (!msgs.length) return;
    const [kind, b, part] = pick(msgs);
    if (b === 8641) empires();
    stage('flag', [[A.Check, [1]]], { args: [kind, b, part] }, () => { prep(); A.Flags.add(kind).writeU8(1); A.Msg.writeS32(b); A.Msg.add(4).writeS32(part); }, after);
  } else if (c < 15) {
    // A ground battle ends (flag 4) on one of the episodes' planets (816, 820, 830, 835), attacker won or not.
    const ids = [8160, 8200, 8301, 8302].map(planetOfId).filter((p) => p), p = ids.length && rnd(4) ? pick(ids) : 1 + rnd(np);
    if (rnd(2)) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([10, 16, en, 0]));
    if (bucketHas(835) && rnd(2)) { const e = varOf(8359); for (let k = rnd(3); k > 0; k--) transientU8(sets, A.Bolygok.add(0x188 * (1 + rnd(np)) + 0x20), e); }
    const won = rnd(2);
    stage('flag', [[A.Check, [1]]], { args: [4, p, won] }, () => { prep(); A.Flags.add(4).writeU8(1); A.Gr.add(4).writeS32(0); A.Gr.add(12).writeS32(p); A.GrWin.writeU8(won); }, after);
  } else if (c < 16) {
    // A radar contact with a planet (flag 11): 820's and 830's.
    const ids = [8200, 8302].map(planetOfId).filter((p) => p), p = ids.length && rnd(4) ? pick(ids) : 1 + rnd(np);
    stage('flag', [[A.Check, [1]]], { args: [11, p] }, () => { A.Flags.add(11).writeU8(1); A.RadarB.writeS32(p); });
  } else if (c < 18) {
    // A daily check (817 at difficulty 4 with empire 11's planets, 835, 841, 842, 864..866).
    if (bucketHas(817) && rnd(2)) { transientS32(sets, A.Nehezseg, 4); if (rnd(2)) A.Bolygok.add(0x188 * (1 + rnd(np)) + 0x20).writeU8(11); }
    if (bucketHas(841) && rnd(3) === 0) transientU8(sets, A.Techs.add(0x114 * 6 + 0x45 + en), 5);
    if (bucketHas(842) && rnd(2)) {
      // A ship of the human's of a design with +0x34 set.
      const ships = humanShips();
      if (ships.length) { const [k] = pick(ships), d = A.Hajok.readPointer().add(6 * k + 2).readS16(); transientU8(sets, A.Tervek.add(4 * en).readPointer().add(0x84 * d + 0x34), pick([1, 2, 0x80])); }
    }
    if (bucketHas(865) || bucketHas(866) || bucketHas(864)) {
      empires(); if (rnd(3) === 0) vSet(8651, rnd(2)); if (rnd(3) === 0) vSet(8661, rnd(2));
      // Story 405 running (865/866 test it): started directly, its start silenced by v(4059) (not recorded).
      if (!bucketHas(405) && rnd(6) === 0) { vSet(4059, 1); A.Adat.add(0xcc).writeS32(5); startStory(405, 1); }
    }
    if (bucketHas(835) && rnd(2)) { const e = varOf(8359); for (let k = rnd(3); k > 0; k--) transientU8(sets, A.Bolygok.add(0x188 * (1 + rnd(np)) + 0x20), e); }
    stage('flag', [[A.Check, [1]]], { args: [0x10, 0] }, () => { prep(); A.Flags.add(0x10).writeU8(1); }, after);
  } else if (c < 19) {
    // Specials: 10 (a colonisation; 842) and 13 (a tech researched; 831).
    const args = rnd(2) ? [10, rnd(4) ? en : 1 + rnd(8), 1 + rnd(np), 0, 0, 0, 0, 0] : [13, rnd(4) ? en : 1 + rnd(8), 155 + rnd(20), 0, 0, 0, 0, 0];
    stage('special', [[A.Special, args]], { args });
  } else if (c < 20) {
    // A space battle ends with the human and 816's raider (8185) among the participants.
    // The list is written when the job runs: several jobs may be queued at once and `fake` is shared.
    const n = 2 + rnd(2), list = [];
    for (let i = 0; i < n; i++) list.push([pick([en, en, 10, rnd(9)]), pick([8185, 8185, 0, rnd(9000)])]);
    const write = () => list.forEach(([owner, id], i) => { const q = fake.add(0x30 * i); q.add(0x18).writeS32(owner); q.add(0x20).writeS32(id); q.add(0x24).writeS32(id); });
    stage('spwfinished', [[A.SpwFinished, [fake.toUInt32(), n, 1]]], { args: list }, write);
  } else if (c < 22) {
    // Story 816 finished (818's signal), or a fleet arrives at a point (816's raider 8185, 864's fleet 8642).
    if (rnd(3) === 0) {
      const ty = pick([1, 1, 2]);
      stage('flag', [[A.Check, [1]]], { args: [12, 816, ty] }, () => { A.Flags.add(12).writeU8(1); A.Signal.writeS32(816); A.Signal.add(4).writeS32(ty); });
      return;
    }
    const lf = liveFleets();
    if (!lf.length) return;
    let f = 0;
    for (const k of lf) { const id = fleet(k).add(0x16).readS16(); if (id === 8185 || id === 8642) f = k; }
    if (!f || rnd(3) === 0) f = pick(lf);
    const q = fleet(f), fid = q.add(0x16).readS16();
    if (fid !== 8185 && fid !== 8642 && rnd(2)) transientS16(sets, q.add(0x16), pick([8185, 8642]));
    let save = null;
    stage('arrive', [[A.ArrivedSave, [f]], [A.ArrivedCheck, []]], { args: [f, 0, 0] },
      () => { prep(); save = [q.add(0x42).readS16(), q.add(0x44).readFloat()]; q.add(0x42).writeS16(0); q.add(0x44).writeFloat(0); },
      () => { q.add(0x42).writeS16(save[0]); q.add(0x44).writeFloat(save[1]); after(); });
  } else {
    // An episode ends (WIN) so its start runs again later.
    if (!running.length) return;
    const id = pick(running), h = rnd(2);
    stage('win', [[A.Win, [id, h]]], { args: [id, h] });
  }
}

// ---------------------------------------------------------------- staged calls, st_Fable_include (--keep fable)
// The random fables 301..316: started (with the world they look for set up), their timers, answers, battles
// (fake participants with the merchants' ids), the arrival at the escorted merchant, the agent's death.
const FABLE = [301, 302, 303, 304, 305, 307, 308, 309, 310, 311, 312, 313, 314, 315, 316];
const FABLE_COUNTERS = [3011, 3031, 3032, 3051, 3053, 3082, 3091, 3101, 3120, 3140, 3150, 3151, 3161];
const FABLE_ANSWERS = [[3051, 3], [3051, 4], [3051, 5], [3071, 3], [3071, 4], [3071, 5], [3081, 2], [3081, 3], [3091, 2],
  [3091, 3], [3091, 4], [3092, 2], [3095, 2], [3095, 3], [3120, 2], [3121, 2], [3120, 3], [3121, 4], [3120, 4]];
const sCall = new NativeFunction(A.S, 'pointer', ['pointer', 'int', 'int'], { abi: 'mscdecl' });
function sText(group, id) { const out = Memory.alloc(16); sCall(out, group, id); const q = out.readPointer(); return q.isNull() ? '' : q.readCString(); }
const flottaNew = new NativeFunction(A.FlottaNew, 'int', ['pointer', 'int', 'int', 'float', 'float', 'float'], { abi: 'mscdecl' });
const fableName = Memory.allocAnsiString('Kereskedo');
function traders() { return liveFleets().filter((f) => fleet(f).add(0x15).readU8() === 9 && fleet(f).add(0x16).readS16() >= 10000 && fleet(f).add(0x16).readS16() <= 11000); }
function fableFuzz() {
  const c = rnd(20), en = A.EN.readS32(), np = nPlanets(), sets = [];
  const prep = () => sets.forEach((x) => x[0]()), after = () => sets.slice().reverse().forEach((x) => x[1]());
  const own = []; for (let p = 1; p <= np; p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === en) own.push(p);
  // Merchants of empire 9 (the campaign maps have few): a new one now and then near the human's planets.
  if (traders().length < 4 && own.length && rnd(3) === 0) {
    const b = A.Bolygok.add(0x188 * pick(own));
    flottaNew(fableName, 9, 10000 + rnd(1001), b.add(0x18).readFloat() + rnd(500) - 250, b.add(0x1c).readFloat() + rnd(500) - 250, 1.0);
  }
  const tr = traders();
  if (tr.length && rnd(4) === 0) fleet(pick(tr)).add(0x18).writeU8(rnd(3) ? 0 : 1); // traded or not
  const running = FABLE.filter(bucketHas);
  if (c < 4 || !running.length) {
    const ids = FABLE.filter((id) => !bucketHas(id));
    if (!ids.length) return;
    const id = pick(ids);
    if (id >= 309 && id <= 311 && rnd(4)) vSet(3099, 0);
    if (id === 311 && rnd(3)) transientU32(sets, A.HanyB.add(4 * en), 4 + rnd(3));
    if (id === 310 && rnd(4) === 0) transientU32(sets, A.KemN.add(4 * en), 12);
    if (id === 316) for (const p of own) if (rnd(2)) transientU8(sets, A.Bolygok.add(0x188 * p + 0x6e), rnd(4));
    if (id === 307) for (let k = 1; k <= A.WormN.readS32(); k++) if (rnd(2)) transientU8(sets, A.Worm.add(0x1c * k + (rnd(2) ? 8 : 0x14)), 0);
    if (id === 308) for (let e = 1; e < 9; e++) if (rnd(4) === 0 && e !== en) transientU8(sets, A.El.add(e), 0);
    stage('start', [[A.StartStory, [id, 0]], [A.Check, [1]]], { args: [id] }, prep, after);
  } else if (c < 7) {
    // A timer runs out: at the next tick, or staged (the labs and research perhaps changed).
    const id = pick(FABLE_COUNTERS);
    if (rnd(2)) { szStart(id, rnd(2)); return; }
    if (id === 3151) { for (let k = 0; k < 4; k++) if (rnd(2)) transientU32(sets, A.Labor.add(16 * en + 4 * k), rnd(3)); if (rnd(2)) transientU32(sets, A.ResAkt.add(4 * en), rnd(3)); }
    if (id === 3082 || id === 3140 || id === 3161) for (const p of own.concat([1 + rnd(np)])) if (rnd(3) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), pick([en, 0, 1 + rnd(8)]));
    if (id === 3101) { const kn = A.KemN.add(4 * en).readS32(); for (let k = 0; k <= kn; k++) if (rnd(4) === 0) transientU8(sets, A.Kemek.add(0x3dc * en + 0x4c * k + 0x10), rnd(2)); }
    stage('flag', [[A.Check, [1]]], { args: [5, id] }, () => { prep(); A.Flags.add(5).writeU8(1); A.SzVegzett.writeS32(id); }, after);
  } else if (c < 10) {
    // An answer (6), a promotion (7) or a read message (9).
    const [b, part] = pick(FABLE_ANSWERS), kind = rnd(5) ? 6 : pick([7, 9]);
    const blk = kind === 9 ? pick([3022, 4022, 3022, b]) : b;
    stage('flag', [[A.Check, [1]]], { args: [kind, blk, part] }, () => { A.Flags.add(kind).writeU8(1); A.Msg.writeS32(blk); A.Msg.add(4).writeS32(part); });
  } else if (c < 13) {
    // A space battle ends: the human, empire 9 and the stories' fleets among the participants.
    const n = 2 + rnd(2), list = [], lf = liveFleets();
    const ids = [3011, 3041, 3111, varOf(3021), varOf(3051)].concat(tr.map((f) => fleet(f).add(0x16).readS16()));
    for (let i = 0; i < n; i++) {
      const owner = pick([en, 9, en, 9, 10, rnd(9)]), slot = lf.length ? pick(lf) : 0, id = rnd(4) ? pick(ids) : rnd(12000);
      list.push([owner, slot, id]);
    }
    // Written at the call (staged calls can queue up while the game isn't calling the frame hook).
    sets.push([() => list.forEach(([owner, slot, id], i) => {
      const q = fake.add(0x30 * i); q.writeAnsiString('P' + i); q.add(0x18).writeS32(owner); q.add(0x20).writeS32(slot); q.add(0x24).writeS32(id);
    }), () => {}]);
    transientU32(sets, A.KiNyert, pick([en, 9, 10, rnd(9)]));
    if (rnd(3) === 0) vSet(3011, 0);
    if (rnd(4) === 0) vSet(3052, rnd(2));
    stage('spwfinished', [[A.SpwFinished, [fake.toUInt32(), n, 1]]], { args: list }, prep, after);
  } else if (c < 15) {
    // A space battle starts between the pirate 3111 and the trader 4041, the human's fleets around 4041.
    const a = fleetOfId(3111), b = fleetOfId(4041), lf = liveFleets();
    const s1 = a && rnd(4) ? a : pick(lf), s2 = b && rnd(4) ? b : pick(lf);
    transientU32(sets, A.SpFlottakN, 2);
    const w16 = (q, v) => { let was = 0; sets.push([() => { was = q.readS16(); q.writeS16(v); }, () => q.writeS16(was)]); };
    w16(A.SpFl0.add(6), rnd(2) ? s1 : s2); w16(A.SpFl0.add(12), rnd(2) ? s2 : s1);
    if (b) {
      const mine = lf.filter((f) => fleet(f).add(0x15).readU8() === en), q = fleet(b);
      for (let k = rnd(3); k > 0 && mine.length; k--) {
        const m = fleet(pick(mine)), d = rnd(16);
        transientU32(sets, m.add(0x30), ftou(q.add(0x30).readFloat() + (rnd(2) ? d : -d) * 0.7));
        transientU32(sets, m.add(0x34), ftou(q.add(0x34).readFloat() + (rnd(2) ? d : -d) * 0.7));
      }
    }
    for (let e = 0; e < 19; e++) transientU32(sets, A.Pact.add(4 * e), 0);
    stage('flag', [[A.Check, [1]]], { args: [1] }, () => { prep(); A.Flags.add(1).writeU8(1); }, after);
  } else if (c < 16) {
    // A fleet of the human's (or another) arrives at the escorted merchant 3031.
    const t = fleetOfId(3031), lf = liveFleets();
    if (!t || !lf.length) return;
    const mine = lf.filter((f) => fleet(f).add(0x15).readU8() === en), f = mine.length && rnd(4) ? pick(mine) : pick(lf), q = fleet(f);
    let save = null;
    stage('arrive', [[A.ArrivedSave, [f]], [A.ArrivedCheck, []]], { args: [f, 2, t] },
      () => { save = [q.add(0x42).readS16(), q.add(0x44).readFloat()]; q.add(0x42).writeS16(2); q.add(0x44).writeFloat(t); },
      () => { q.add(0x42).writeS16(save[0]); q.add(0x44).writeFloat(save[1]); });
  } else if (c < 17) {
    // A spy report: the agent (or another spy) dead (KemEsemeny kind 1, outcome 1), or another kind.
    const kn = A.KemN.add(4 * en).readS32(), k = rnd(Math.max(kn, 0) + 1), kind = rnd(3) ? 1 : rnd(4), b = rnd(3) ? 1 : rnd(3);
    stage('flag', [[A.Check, [1]]], { args: [15, kind, k, b] }, () => {
      A.Flags.add(15).writeU8(1); A.KemTipus.writeS32(rnd(5) ? 3 : 1); A.KemEv.writeS32(kind); A.KemEv.add(4).writeS32(k); A.KemEv.add(8).writeS32(b);
    });
  } else if (c < 19) {
    // A raw daily check (305's trader sold, 308's planets changing hands, 310/311's losses).
    const t = A.Adat.add(0x28).readS32();
    if (t > 0 && t <= nFleets() && rnd(3) === 0) transientU8(sets, fleet(t).add(0x15), pick([9, 1, 10]));
    const p = 1 + rnd(np); if (rnd(3) === 0) transientU8(sets, A.Bolygok.add(0x188 * p + 0x20), rnd(9));
    stage('flag', [[A.Check, [1]]], { args: [0x10] }, () => { prep(); A.Flags.add(0x10).writeU8(1); }, after);
  } else {
    const id = pick(running), h = rnd(2);
    stage('win', [[A.Win, [id, h]]], { args: [id, h] });
  }
}

let frames = 0, started = false, loadedAt = 0, initAt = 1e9, lastIdo = -1, sameFor = 0, homeKep = -1, wantSpeed = 0;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (!started && frames === cfg.startAfterFrames) {
      started = true; fz = cfg.seed;
      if (cfg.campaign) {
        setupActs();
        for (const name in UNPORTED) if (!cfg.keep.includes(name)) Memory.patchCode(va(UNPORTED[name]), 1, (code) => code.writeU8(0xc3));
        if (cfg.keep.includes('rnd')) setupRndFilter();
        // --keep elem: the script's blocks not ported yet test a story id nothing starts (st_Story's `push id`).
        if (cfg.keep.includes('elem')) for (const site of ELEM_UNPORTED) Memory.patchCode(va(site + 1), 4, (code) => code.writeU32(0x7fff0000));
        // st_Fable's pirates attack the human's fleets: auto-resolve two-sided battles (the debug switch "always
        // simulate", space-combat.md §4), so the clock doesn't stop on the battle screen.
        if (cfg.keep.includes('fable')) va(0xb77230).writeS32(1);
        // st_Elem part 2d: 807's pirates and 809's blockade fight the human's fleets too.
        if (cfg.keep.includes('elem') && (cfg.inline || []).some((x) => ELEM2D.includes(x) || ELEM2E.includes(x) || ELEM2F.includes(x) || ELEM2G.includes(x))) va(0xb77230).writeS32(1);
        // The race menu's choice: campaign N (race index N - 1), the difficulty selector (0 → 1, 2 → 2, 1 → 4).
        A.FajStart.writeU8(1); A.FajSzetnyilas.writeFloat(100.0); A.FajRace.writeS32(cfg.campaign - 1);
        A.FajNehezseg.writeS32({ 1: 0, 2: 2, 4: 1 }[cfg.difficulty] ?? 2); A.DebugSeed.writeS32(cfg.seed);
        // MainScr_Kilep (the main screen's update) calls CampaignGameStart on its own once these are set.
        installStub();
      } else {
        betolt(Memory.allocAnsiString(cfg.scenario));
        setSpeed(cfg.speed, 1);
      }
      loadedAt = frames;
      emit({ k: 'loaded' });
      return;
    }
    if (started && phase === 0 && !cfg.campaign && frames >= loadedAt + 300) {
      homeKep = A.AktKep.readS32(); wantSpeed = A.Speed.readS32(); phase = 1;
      emit({ k: 'setup', init: snap(), data: storyData(), campaign: cfg.campaign });
    }
    if (cfg.campaign && frames === initAt + 300) {
      setSpeed(cfg.speed, 1);
      homeKep = A.AktKep.readS32(); wantSpeed = A.Speed.readS32();
    }
    if (phase === 1 && cfg.fuzz && (!cfg.campaign || wantSpeed) && frames % cfg.every === 0) {
      try {
        if (cfg.campaign && cfg.keep.includes('elem')) elemFuzz();
        else if (cfg.campaign && cfg.keep.includes('begin')) beginFuzz();
        else if (cfg.campaign && cfg.keep.includes('rnd')) rndFuzz();
        else if (cfg.campaign && cfg.keep.includes('brnd')) brndFuzz();
        else if (cfg.campaign && cfg.keep.includes('multi')) multiFuzz();
        else if (cfg.campaign && cfg.keep.includes('fable') && rnd(5)) fableFuzz();
        else if (cfg.campaign && cfg.keep.includes('line') && rnd(3)) lineFuzz();
        else if (cfg.campaign) campaignFuzz();
        else fuzzStep();
      } catch (e) { emit({ k: 'error', where: 'fuzz', e: String(e), stack: e.stack }); }
    }
    if (phase === 1 && wantSpeed && frames % 300 === 0 && A.Speed.readS32() !== wantSpeed) setSpeed(cfg.speed, 1);
    // --keep brnd: story 227's enemy fleets (owner 10) would fight the human in real time and stop the clock.
    if (phase === 1 && cfg.keep.includes('brnd') && frames % 20 === 0) for (const f of liveFleets()) if (fleet(f).add(0x15).readU8() === 10) fleet(f).add(0x3c).writeFloat(0);
    // --keep rnd: the story fleets of part 2 (owner 10, ids 1332..1333, 1362, 1451, 1471, 1481) likewise.
    if (phase === 1 && cfg.keep.includes('rnd') && frames % 20 === 0) {
      for (const f of liveFleets()) {
        const o = fleet(f).add(0x15).readU8(), id = fleet(f).add(0x16).readS16();
        if (o === 10 || o === 0 || [0x534, 0x535, 0x552, 0x5ab, 0x5bf, 0x5c9, 0x3f3, 0x3fd, 0x439, 0x4da, 0x51f].includes(id)) fleet(f).add(0x3c).writeFloat(0);
      }
    }
    // --keep begin: the base's and the hunter's fleets (owners 10, 13) likewise.
    if (phase === 1 && cfg.keep.includes('begin') && frames % 20 === 0) for (const f of liveFleets()) { const o = fleet(f).add(0x15).readU8(); if (o === 10 || o === 13) fleet(f).add(0x3c).writeFloat(0); }
    // --keep elem: the episodes' raiders and guards (owners 10 and 11) likewise.
    if (phase === 1 && cfg.keep.includes('elem') && frames % 20 === 0) for (const f of liveFleets()) { const o = fleet(f).add(0x15).readU8(); if (o === 10 || o === 11) fleet(f).add(0x3c).writeFloat(0); }
    if (phase === 1 && wantSpeed) {
      const ido = A.Ido.readS32();
      sameFor = ido === lastIdo ? sameFor + 1 : 0;
      lastIdo = ido;
      if (sameFor === 200 || (sameFor > 200 && sameFor % 600 === 0)) {
        emit({ k: 'stall', seq, ido, kep: A.AktKep.readS32(), speed: A.Speed.readS32() });
        A.AktKepKell.writeS32(homeKep); setPause(0, 0); setSpeed(cfg.speed, 1);
      }
    }
    if (queue.length) armTrampoline();
  },
});

// --debug: report a crash (exe VA, the faulting access, registers, a fuzzy stack) before the process dies, with the
// records still buffered.
let cxxThrows = 0;
Process.setExceptionHandler((d) => {
  const rel = (p) => { try { const o = p.sub(mod.base).toInt32(); return o >= 0 && o < mod.size ? '0x' + (o + 0x400000).toString(16) : String(p); } catch (e) { return String(p); } };
  // 'system' = RaiseException: OutputDebugString's (0x40010006, 0x4001000a: the game's log lines, skipped) and C++
  // throws (the stories' aborts among them; the first 300 reported with their throw site).
  let sys = null;
  if (d.type === 'system') {
    const bt = Thread.backtrace(d.context, Backtracer.FUZZY).slice(0, 8).map(rel);
    if (bt[0] === '0x40010006' || bt[0] === '0x4001000a' || !cfg.debug || cxxThrows++ > 300) return false;
    sys = bt;
  }
  if (cfg.debug) {
    if (sys) { send({ type: 'batch', events: [{ k: 'throw', stack: sys, cur: cur ? [cur.call, cur.args, cur.acts.slice(-6)] : null }] }); return false; }
    flush();
    send({ type: 'batch', events: [{ k: 'crash', type: d.type, address: rel(d.address), mem: d.memory ? [d.memory.operation, String(d.memory.address)] : null,
      ctx: { eax: String(d.context.eax), ecx: String(d.context.ecx), edx: String(d.context.edx), esi: String(d.context.esi), edi: String(d.context.edi) },
      stack: Thread.backtrace(d.context, Backtracer.FUZZY).slice(0, 16).map(rel), cur: cur ? [cur.call, cur.args] : null }] });
  }
  return false;
});
// --debug: who ends the game (BirodalomMeghal 0x53b630, MEGNYERES 0x53f6b0, the game-over screen request): the
// stack and the record's calls so far.
// The `AktKepKell := 0xF` sites (game over / won: 0x53b702 the human's death, 0x53f6fb/0x53f71f MEGNYERES,
// 0x53f998/0x53fa2e MegnyeresEllenoriz's inlined wins, 0x4db68d, 0x4e303d, 0x60c988).
for (const [a_, name] of [[0x53b630, 'meghal'], [0x53f6b0, 'megnyeres'], [0x5117f0, 'exitmain'], [0x4db68d, 'kep15'], [0x4e303d, 'kep15'], [0x53b702, 'kep15'],
  [0x53f6fb, 'kep15'], [0x53f71f, 'kep15'], [0x53f998, 'kep15'], [0x53fa2e, 'kep15'], [0x60c988, 'kep15']]) {
  Interceptor.attach(va(a_), {
    onEnter() {
      if (!cfg.debug) return;
      const rel = (p) => { const o = p.sub(mod.base).toInt32(); return o >= 0 && o < mod.size ? '0x' + (o + 0x400000).toString(16) : String(p); };
      flush();
      send({ type: 'batch', events: [{ k: 'dbg', what: name, at: '0x' + a_.toString(16), arg: argI(this.context, 0), stack: Thread.backtrace(this.context, Backtracer.ACCURATE).slice(0, 16).map(rel),
        cur: cur ? [cur.call, cur.args, cur.acts.slice(-30)] : null }] });
    },
  });
}
// --debug: the game's (re-)initialisation 0x4dfa00 after the start (the campaign reset itself in some traces).
let inits = 0;
Interceptor.attach(va(0x4dfa00), {
  onEnter() {
    if (!cfg.debug || inits++ === 0) return;
    const rel = (p) => { const o = p.sub(mod.base).toInt32(); return o >= 0 && o < mod.size ? '0x' + (o + 0x400000).toString(16) : String(p); };
    flush();
    send({ type: 'batch', events: [{ k: 'dbg', what: 'reinit', frames, stack: Thread.backtrace(this.context, Backtracer.ACCURATE).slice(0, 24).map(rel),
      fuzzy: Thread.backtrace(this.context, Backtracer.FUZZY).slice(0, 40).map(rel), cur: cur ? [cur.call, cur.args, cur.acts.slice(-20)] : null }] });
  },
});
// --debug: when the frame hook stops running, sample every thread's stack (exe VAs).
let wdFrames = -1, wdSame = 0;
setInterval(() => {
  if (!cfg.debug || phase !== 1) return;
  wdSame = frames === wdFrames ? wdSame + 1 : 0;
  wdFrames = frames;
  if (wdSame === 3 || wdSame === 6) {
    const rel = (p) => { const o = p.sub(mod.base).toInt32(); return o >= 0 && o < mod.size ? '0x' + (o + 0x400000).toString(16) : String(p); };
    const th = Process.enumerateThreads().map((t) => ({ id: t.id, pc: rel(t.context.pc), bt: Thread.backtrace(t.context, Backtracer.FUZZY).slice(0, 16).map(rel) }));
    flush();
    send({ type: 'batch', events: [{ k: 'hang', frames, seq, threads: th }] });
  }
}, 2000);
send({ type: 'ready', base: mod.base.toString() });
