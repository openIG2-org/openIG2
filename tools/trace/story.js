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
  // The race menu's state CampaignGameStart reads.
  FajStart: va(0xbbb17c), FajSzetnyilas: va(0xbbb16c), FajRace: va(0xbbb178), FajNehezseg: va(0xbbb180),
};
// The scripts the port doesn't have yet (Story_Check calls them; campaign mode patches them to `ret`).
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
  };
}
function varOf(id) {
  const n = A.nV.readS32();
  for (let i = 1; i <= n; i++) if (A.sValtozok.add(8 * i).readS32() === id) return A.sValtozok.add(8 * i + 4).readS32();
  return 0;
}
function world() {
  const en = A.EN.readS32(), np = nPlanets(), nfl = nFleets(), planets = [], fleets = [], multi = cfg.keep.includes('multi');
  const brnd = cfg.keep.includes('brnd');
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
    }
    planets.push(row);
  }
  for (let f = 0; f <= nfl; f++) {
    const q = fleet(f), row = [q.add(0x15).readU8(), q.add(0x16).readS16()];
    if (cfg.campaign) row.push(f32bits(q.add(0x30)), f32bits(q.add(0x34)), q.add(0x42).readS16(), q.add(0x5c).readS32(), f32bits(q.add(0x3c)));
    // st_BRnd: +0x40 ships, +0x44 the order's target, +0x18, +0x52.
    if (cfg.campaign && brnd) row.push(q.add(0x40).readS16(), f32bits(q.add(0x44)), q.add(0x18).readU8(), q.add(0x52).readU16());
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
    // Ships and the human's designs only while story 404 (the refit trader) or the multiplayer modes run.
    if (bucketHas(404) || multi || brnd) {
      const ships = [], hn = A.HajoCount.readS32(), hp = A.Hajok.readPointer();
      for (let k = 0; k <= hn; k++) ships.push([hp.add(6 * k).readS16(), hp.add(6 * k + 2).readS16()]);
      const designs = [], tn = A.TervCount.add(4 * en).readS32(), tp = A.Tervek.add(4 * en).readPointer();
      // st_BRnd also: +0x20 cost, +0x2c attack, +0x80 story key.
      for (let d = 0; d <= tn; d++) designs.push(brnd ? [tp.add(0x84 * d + 0x1c).readS32(), tp.add(0x84 * d + 0x24).readS32(), tp.add(0x84 * d + 0x20).readS32(), tp.add(0x84 * d + 0x2c).readS32(), tp.add(0x84 * d + 0x80).readS32()] : [tp.add(0x84 * d + 0x1c).readS32(), tp.add(0x84 * d + 0x24).readS32()]);
      out.ships = ships;
      out.designs = designs;
    }
    if (brnd) {
      // The human's tech statuses and every tech's prerequisites (st_GetRandomTalalmany), spFlottak, the
      // wormholes' shown ends, the global morale.
      const t = []; for (let k = 0; k < 193; k++) { const r = A.Techs.add(0x114 * k); t.push(r.add(0x45 + en).readU8(), r.add(0x2d).readU8(), r.add(0x2e).readU8(), r.add(0x2f).readU8()); }
      out.techs = t.map((x) => (x < 16 ? '0' : '') + x.toString(16)).join('');
      out.spf = [A.SpFlottak.readS16(), A.SpFlottak.add(6).readS16()];
      const wh = []; for (let k = 1; k <= A.WormholeN.readS32(); k++) wh.push([A.Wormholes.add(0x1c * k + 8).readU8(), A.Wormholes.add(0x1c * k + 0x14).readU8()]);
      out.wh = wh;
      out.gmoral = [A.GMoral.readS32(), A.GMoralSpeed.readS32()];
    }
  }
  return out;
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
function act(name, args) { if (!cur) return null; const e = [name, args, actDepth - cur.d0, null]; cur.acts.push(e); return e; }
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
    const po = p.sub(A.PlayerNev).toInt32();
    if (po >= 0 && po % 11 === 0 && po / 11 < 19) return [argI(c, 0), 'player', po / 11];
    return [argI(c, 0), 'other', p.readAnsiString()];
  });
  logged(va(0x506200), 'rmmessage', ints(1));
  logged(va(0x5063d0), 'disablepart', ints(1));
  logged(va(0x53c0b0), 'inforemove', ints(3));
  logged(va(0x4c1d60), 'discover', ints(2));
  logged(va(0x604040), 'unhide', ints(1));
  if (cfg.keep.includes('multi') || cfg.keep.includes('brnd')) setupMultiActs(ints);
  if (cfg.keep.includes('brnd')) {
    // st_BRnd_include's calls: designs deleted, number parameters, techs given, the ticker.
    logged(va(0x541d50), 'tervdelete', ints(4));
    logged(va(0x509500), 'paramint', ints(2));
    logged(va(0x602920), 'techstate', ints(3));
    logged(va(0x556b50), 'scroll', (c) => { const q = c.esp.add(4).readPointer(); return [q.isNull() ? '' : q.readAnsiString(), argI(c, 1)]; });
  }
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
      emit({ k: 'setup', init: snap(), data: storyData(), campaign: cfg.campaign, keep: cfg.keep, infos });
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
  logged(va(0x513840), 'multisend', (c) => [argI(c, 0), argI(c, 1), argI(c, 2), c.esp.add(16).readPointer().readS32()]);
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
  if (cfg.debug) emit({ k: 'dbg', what: 'arm', n: armed.length, frames });
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

let frames = 0, started = false, loadedAt = 0, initAt = 1e9, lastIdo = -1, sameFor = 0, homeKep = -1, wantSpeed = 0;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (!started && frames === cfg.startAfterFrames) {
      started = true; fz = cfg.seed;
      if (cfg.campaign) {
        setupActs();
        for (const name in UNPORTED) if (!cfg.keep.includes(name)) Memory.patchCode(va(UNPORTED[name]), 1, (code) => code.writeU8(0xc3));
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
        if (cfg.campaign && cfg.keep.includes('brnd')) brndFuzz();
        else if (cfg.campaign && cfg.keep.includes('multi')) multiFuzz();
        else if (cfg.campaign && cfg.keep.includes('line') && rnd(3)) lineFuzz();
        else if (cfg.campaign) campaignFuzz();
        else fuzzStep();
      } catch (e) { emit({ k: 'error', where: 'fuzz', e: String(e), stack: e.stack }); }
    }
    if (phase === 1 && wantSpeed && frames % 300 === 0 && A.Speed.readS32() !== wantSpeed) setSpeed(cfg.speed, 1);
    // --keep brnd: story 227's enemy fleets (owner 10) would fight the human in real time and stop the clock.
    if (phase === 1 && cfg.keep.includes('brnd') && frames % 20 === 0) for (const f of liveFleets()) if (fleet(f).add(0x15).readU8() === 10) fleet(f).add(0x3c).writeFloat(0);
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

send({ type: 'ready', base: mod.base.toString() });
