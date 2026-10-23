// Frida agent: produce original save games on the GOG ig2.exe and snapshot the saved globals at the same moment
// (spec re/formats/save.md). After loading the scenario it waits for the configured days; at each one it reads the
// source memory of the save's sections, then calls the exe's own `LoadSave(slot, name, 2)` (exe 0x543c80) from the
// frame hook (same thread, nothing runs in between), and reports the snapshot. The Python side copies the written
// `save<slot>.sav` to _local/saves/ and removes it from the game's data directory.
// --ships: give empires with designs a few warships (as spy.js) and hand the human's empire to the AI.
// --refit: at each day, perturb the buildings (positions, damage, work, holes made with the exe's `EpuletDelete`) and
// the ground units' cells, write a save, call `EpuleteketJoHelyreTesz` (exe 0x53cd00, the end of `LoadScenario`;
// re/rules/colony-placement.md §6) and write a second save: index 2k before, 2k + 1 after.
// --cursor: at each day, move units near buildings, write a save and probe the build cursor's search (exe 0x486460,
// colony-placement.md §8) on every owned planet; grids and probes go into the save's JSON.
// --wormholes: at each day, set `OsszHanyWormhole` (0xBB61A0) to a start value (0 the first day), write a save, call
// the exe's `AddWormhole` (0x53aec0) with fuzzed coordinates until one returns 0 (the call after the fifth wormhole)
// and write a second save (2k / 2k + 1); the calls and their results go into the second save's JSON
// (re/rules/fleet-movement.md "Opening a wormhole").
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  LoadSave: va(0x543c80), GetSavedGame: va(0x543670), MultiplayerSave: va(0xe40c2c), FlottaNew: va(0x53d400), HajoNew: va(0x53e800),
  Ev: va(0xbae040), Honap: va(0xbae044), Nap: va(0xbae048), Ido: va(0xbae04c), Ido2: va(0xbae050), OldNap: va(0xbae058),
  Nehezseg: va(0x92827c), EN: va(0x912a3c), CampaignType: va(0x928278),
  AktKep: va(0xbae0fc), AktKepKell: va(0xbae250), Speed: va(0xbae25c),
  PlayerKi: va(0xbae560), PlayerFaj: va(0xbae574), El: va(0xbae6ac), MIzik: va(0x915f58), PP: va(0x912acc),
  Money: va(0xbad168), Pools: va(0xbad200), RandSeed: va(0xbaf2d0), Ismeri: va(0xbaebb8), Viszony: va(0xdabf38),
  Ossz: va(0xbb1f84), Kemek: va(0xbafcc8), Techs: va(0xb9c0c8),
  Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8), Hajok: va(0xbb4100), HajoCount: va(0xbb4104),
  Tankok: va(0xbb41a8), TankCount: va(0xbb4154), Waypoints: va(0xbb41a4), WaypointCount: va(0xbb60f0),
  Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8), Tervek: va(0xb9aa18), TervCount: va(0xb9c028),
  HanyEpulet: va(0xba9518), HanyEpuletLefoglalva: va(0xba9840), HanyEpuletBlokk: va(0xba9b68), Epuletek: va(0xba9e90),
  MiGyarthato: va(0xdb28e8), StoryAdat: va(0xf273f8),
  AddWormhole: va(0x53aec0), WormholeCount: va(0xbb61a0),
  Refit: va(0x53cd00), EpuletDelete: va(0x53c730), WfCount: va(0x9c33c4), WfUsed: va(0x9c33cc), WfUnits: va(0x9c3574),
  CursorSearch: va(0x486460), Decode2LargeMap: va(0x46d8a0), Allocate: va(0x45f3d0), MapAlloc: va(0x998260),
  AktPlanet: va(0xb9933c), AktEpuletTipus: va(0xb9c0c4), Footprints: va(0x8f1f80),
};

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, days: [3, 40, 150], slot: 97, ships: false, mp: false, seed: 4242, refit: false, wormholes: false, cursor: false };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 4) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

function hexOf(p, size) {
  if (size <= 0 || p.isNull()) return '';
  const b = new Uint8Array(p.readByteArray(size));
  let s = '';
  for (let i = 0; i < b.length; i++) s += (b[i] < 16 ? '0' : '') + b[i].toString(16);
  return s;
}
let fz = 1;
const rnd = (n) => { fz ^= fz << 13; fz >>>= 0; fz ^= fz >>> 17; fz ^= fz << 5; fz >>>= 0; return fz % n; };

const nf = (p, r, a) => new NativeFunction(p, r, a, { abi: 'mscdecl', traps: 'all' });
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const setPause = nf(A.SetIdoPause, 'void', ['int', 'int']);
const loadSave = nf(A.LoadSave, 'void', ['int', 'pointer', 'int']);
const getSavedGame = nf(A.GetSavedGame, 'pointer', ['int']);
const flottaNew = nf(A.FlottaNew, 'int', ['pointer', 'int', 'int', 'float', 'float', 'float']);
const hajoNew = nf(A.HajoNew, 'void', ['int', 'int']);
const refit = nf(A.Refit, 'void', []);
const addWormhole = nf(A.AddWormhole, 'int', ['int', 'int', 'int', 'int']);
const epuletDelete = nf(A.EpuletDelete, 'void', ['int', 'int']);

// --refit's perturbation: on every owned planet some buildings move (whole and fractional cells, a few near the
// edges, some onto another building), get damage and unfinished work, some slots are emptied by `EpuletDelete`
// (holes the refit fills); units on the battlefield move onto building cells (the search's vehicle pass).
function fuzzBuildings() {
  const out = { moved: 0, holes: 0, units: 0 };
  const np = A.HanyBolygo.readS32();
  for (let p = 1; p <= np; p++) {
    if (A.Bolygok.add(0x188 * p + 0x20).readU8() === 0) continue;
    const n = A.HanyEpuletLefoglalva.add(4 * p).readS32();
    const cells = [];
    for (let j = 1; j <= n; j++) {
      const b = A.Epuletek.add(4 * p).readPointer().add(0x20 * j);
      if (b.readU8() === 0) continue;
      cells.push([b.add(0xc).readFloat(), b.add(0x10).readFloat()]);
      const r = rnd(12);
      if (r < 3) {
        const edge = rnd(5) === 0;
        b.add(0xc).writeFloat(edge ? rnd(2) ? 3 + rnd(20) : 180 + rnd(19) : 20 + rnd(160) + rnd(4) * 0.25);
        b.add(0x10).writeFloat(edge ? rnd(2) ? 3 + rnd(20) : 180 + rnd(19) : 20 + rnd(160) + rnd(4) * 0.25);
        out.moved++;
      } else if (r === 3 && cells.length > 1) {
        const c = cells[rnd(cells.length - 1)];
        b.add(0xc).writeFloat(c[0] + rnd(3) - 1); b.add(0x10).writeFloat(c[1] + rnd(3) - 1);
        out.moved++;
      } else if (r === 4) {
        epuletDelete(p, j);
        out.holes++;
        continue;
      }
      if (rnd(4) === 0) b.add(4).writeS32(rnd(200));
      if (rnd(4) === 0) b.add(8).writeU16(rnd(3000));
    }
    const nu = A.WfCount.add(0x8ec * p).readS32(), used = A.WfUsed.add(0x8ec * p).readPointer(), units = A.WfUnits.add(0x8ec * p).readPointer();
    if (nu < 1 || used.isNull() || units.isNull() || !cells.length) continue;
    for (let i = 0; i < nu; i++) {
      if (!used.add(i).readU8() || rnd(2)) continue;
      const u = units.add(4 * i).readPointer(), c = cells[rnd(cells.length)];
      u.add(0x1a0).writeS32(Math.trunc(c[0]) + rnd(5) - 2); u.add(0x1a4).writeS32(Math.trunc(c[1]) + rnd(5) - 2);
      out.units++;
    }
  }
  return out;
}

const cursorSearch = nf(A.CursorSearch, 'int', ['pointer', 'pointer']);
const decode2LargeMap = nf(A.Decode2LargeMap, 'void', ['int', 'pointer', 'int', 'int']);
const allocate = nf(A.Allocate, 'void', ['int', 'int', 'int', 'int', 'int']);

// --cursor: units move near building cells (half of them, ± 6), then the save is written; afterwards, on every
// owned planet, `MapAlloc` is built as `Init3dColony` builds it (`Decode2LargeMap(m, MapAlloc, 0, p)`, then
// `Allocate((int)x, (int)y, w, h, 1)` per building in slot order) and the build cursor's search (exe 0x486460,
// `LocateActBuildingSuitableLocation`) runs from fuzzed starts for fuzzed kinds; `MapAlloc`, `AktPlanet` and
// `AktEpuletTipus` are restored. The grid (before the units' +8) and the probes go into the save's JSON.
function fuzzUnits() {
  let moved = 0;
  const np = A.HanyBolygo.readS32();
  for (let p = 1; p <= np; p++) {
    if (A.Bolygok.add(0x188 * p + 0x20).readU8() === 0) continue;
    const n = A.HanyEpuletLefoglalva.add(4 * p).readS32(), cells = [];
    for (let j = 1; j <= n; j++) {
      const b = A.Epuletek.add(4 * p).readPointer().add(0x20 * j);
      if (b.readU8() !== 0) cells.push([Math.trunc(b.add(0xc).readFloat()), Math.trunc(b.add(0x10).readFloat())]);
    }
    const nu = A.WfCount.add(0x8ec * p).readS32(), used = A.WfUsed.add(0x8ec * p).readPointer(), units = A.WfUnits.add(0x8ec * p).readPointer();
    if (nu < 1 || used.isNull() || units.isNull() || !cells.length) continue;
    for (let i = 0; i < nu; i++) {
      if (!used.add(i).readU8() || rnd(2)) continue;
      const u = units.add(4 * i).readPointer(), c = cells[rnd(cells.length)];
      u.add(0x1a0).writeS32(c[0] + rnd(13) - 6); u.add(0x1a4).writeS32(c[1] + rnd(13) - 6);
      moved++;
    }
  }
  return moved;
}

function cursorProbes(perPlanet) {
  const saved = A.MapAlloc.readByteArray(40000), aktP = A.AktPlanet.readS32(), aktT = A.AktEpuletTipus.readS32();
  const xy = Memory.alloc(8), out = [];
  const np = A.HanyBolygo.readS32();
  for (let p = 1; p <= np; p++) {
    const pl = A.Bolygok.add(0x188 * p);
    if (pl.add(0x20).readU8() === 0) continue;
    decode2LargeMap(pl.add(0x23).readU8(), A.MapAlloc, 0, p);
    const n = A.HanyEpuletLefoglalva.add(4 * p).readS32(), cells = [];
    for (let j = 1; j <= n; j++) {
      const b = A.Epuletek.add(4 * p).readPointer().add(0x20 * j), k = b.readU8();
      if (k === 0) continue;
      const fp = A.Footprints.add(0x6c * (k - 1));
      const x = Math.trunc(b.add(0xc).readFloat()), y = Math.trunc(b.add(0x10).readFloat());
      allocate(x, y, fp.readS32(), fp.add(4).readS32(), 1);
      cells.push([x, y]);
    }
    const grid = hexOf(A.MapAlloc, 40000);
    A.AktPlanet.writeS32(p);
    const probes = [];
    for (let i = 0; i < perPlanet; i++) {
      const t = 1 + rnd(60), r = rnd(10);
      let sx, sy;
      if (r < 7 && cells.length) { const c = cells[rnd(cells.length)]; sx = c[0] + rnd(17) - 8; sy = c[1] + rnd(17) - 8; }
      else if (r < 9) { sx = rnd(200); sy = rnd(200); }
      else { sx = rnd(2) ? rnd(14) - 4 : 186 + rnd(20); sy = rnd(210) - 5; }
      A.AktEpuletTipus.writeS32(t);
      xy.writeS32(sx); xy.add(4).writeS32(sy);
      const ok = cursorSearch(xy, xy.add(4)) & 0xff;
      probes.push([t, sx, sy, ok, xy.readS32(), xy.add(4).readS32()]);
    }
    out.push({ p, grid, probes });
    A.MapAlloc.writeByteArray(saved);
  }
  A.AktPlanet.writeS32(aktP); A.AktEpuletTipus.writeS32(aktT);
  return out;
}

// The save's sections, read from their source globals (names = re/formats/save.md and ig2-formats::save).
function snapshot() {
  const s = {};
  const put = (name, p, size) => { s[name] = hexOf(p, size); };
  for (const [n, p] of [['ev', A.Ev], ['honap', A.Honap], ['nap', A.Nap], ['ido', A.Ido], ['ido2', A.Ido2],
    ['old_nap', A.OldNap], ['difficulty', A.Nehezseg], ['en', A.EN], ['campaign_type', A.CampaignType]]) s[n] = p.readS32();
  put('money', A.Money, 0x98); put('money_pools', A.Pools, 0x390); put('rand_seed', A.RandSeed, 0x54);
  put('player_ki', A.PlayerKi, 0x13); put('player_faj', A.PlayerFaj, 0x13); put('birodalom_el', A.El, 0x13);
  put('birodalom_ismeri', A.Ismeri, 0x169); put('viszony', A.Viszony, 0x3596); put('story_adat', A.StoryAdat, 0x128);
  put('ossz_hany_kem', A.Ossz, 0x24);
  for (let e = 1; e <= 8; e++) put('kemek' + e, A.Kemek.add(e * 0x3dc), (A.Ossz.add(4 * e).readS32() + 1) * 0x4c);
  const nfl = A.FlottaCount.readS32(), ns = A.HajoCount.readS32(), nt = A.TankCount.readS32(), nw = A.WaypointCount.readS32();
  s.n_fleets = nfl; s.n_ships = ns; s.n_tanks = nt; s.n_waypoints = nw;
  put('fleets', A.Flottak.readPointer(), (nfl + 1) * 0x80); put('ships', A.Hajok.readPointer(), (ns + 1) * 6);
  put('tanks', A.Tankok, (nt + 1) * 8);
  if (nw) put('waypoints', A.Waypoints.readPointer(), (nw + 1) * 0xc);
  const np = A.HanyBolygo.readS32();
  s.n_planets = np;
  put('planets', A.Bolygok, (np + 1) * 0x188);
  const techs = [];
  for (let t = 1; t <= 192; t++) {
    const r = A.Techs.add(t * 0x114);
    techs.push(hexOf(r.add(0x45), 0x13) + hexOf(r.add(0x58), 0x4c) + hexOf(r.add(0xa4), 0x13) + hexOf(r.add(0x2c), 0x10) + hexOf(r.add(0xc0), 0x51));
  }
  s.techs = techs;
  put('design_counts', A.TervCount, 0x4c);
  for (let e = 1; e <= 18; e++) put('designs' + e, A.Tervek.add(4 * e).readPointer(), (A.TervCount.add(4 * e).readS32() + 1) * 0x84);
  put('hany_epulet', A.HanyEpulet, 0x324); put('hany_epulet_lefoglalva', A.HanyEpuletLefoglalva, 0x324);
  put('hany_epulet_blokk', A.HanyEpuletBlokk, 0x324);
  const bld = [];
  for (let p = 1; p <= 200; p++) bld.push(hexOf(A.Epuletek.add(4 * p).readPointer(), (A.HanyEpuletBlokk.add(4 * p).readS32() + 1) * 0x20));
  s.buildings = bld;
  put('mi_gyarthato', A.MiGyarthato, 0x1d64);
  return s;
}

function setupShips() {
  const en = A.EN.readS32();
  A.PlayerKi.add(en).writeU8(0);
  let made = 0;
  for (let e = 1; e <= 8; e++) {
    const cnt = A.TervCount.add(4 * e).readS32(), des = A.Tervek.add(4 * e).readPointer();
    if (cnt < 1 || des.isNull()) continue;
    const ships = [];
    for (let d = 1; d <= cnt; d++) { const p = des.add(0x84 * d); if (p.add(0x16).readS16() === 1 && p.add(0x36).readU8() === 0 && (p.add(0x2c).readS32() > 0 || p.add(0x30).readS32() > 0)) ships.push(d); }
    const mine = []; for (let p = 1; p <= A.HanyBolygo.readS32(); p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === e) mine.push(p);
    if (!ships.length || !mine.length) continue;
    for (let k = 0; k < 3; k++) {
      const q = A.Bolygok.add(0x188 * mine[rnd(mine.length)]);
      const f = flottaNew(Memory.allocAnsiString('Probe'), e, 0, q.add(0x18).readFloat() + rnd(9) - 4, q.add(0x1c).readFloat() + rnd(9) - 4, 0);
      if (f < 1) continue;
      for (let j = 0; j < 1 + rnd(4); j++) hajoNew(f, ships[rnd(ships.length)]);
      made++;
    }
  }
  emit({ k: 'setup', fleets: made });
}

let frames = 0, started = false, loadedAt = 0, phase = 0, day0 = 0, next = 0, busy = false, lastIdo = -1, sameFor = 0, homeKep = -1;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    if (busy) return;
    frames++;
    if (!started && frames === cfg.startAfterFrames) {
      started = true; fz = cfg.seed;
      betolt(Memory.allocAnsiString(cfg.scenario));
      setSpeed(3, 1);
      loadedAt = frames;
      emit({ k: 'loaded' });
      return;
    }
    if (started && phase === 0 && frames >= loadedAt + 300) {
      if (cfg.ships) setupShips();
      homeKep = A.AktKep.readS32(); day0 = A.Ido.readS32(); phase = 1;
    }
    if (phase !== 1) return;
    const ido = A.Ido.readS32();
    sameFor = ido === lastIdo ? sameFor + 1 : 0;
    lastIdo = ido;
    if (sameFor === 200 || (sameFor > 200 && sameFor % 600 === 0)) {
      emit({ k: 'stall', ido, kep: A.AktKep.readS32() });
      A.AktKepKell.writeS32(homeKep); setPause(0, 0); setSpeed(3, 1);
    }
    if (ido - day0 < cfg.days[next] * 50) return;
    busy = true;
    if (cfg.wormholes) {
      const start = next === 0 ? 0 : rnd(6);
      A.WormholeCount.writeS32(start);
      const coord = () => rnd(1400) - 200;
      const calls = [];
      for (const half of [0, 1]) {
        const idx = 2 * next + half, slot = cfg.slot + idx;
        if (half) {
          for (;;) {
            const c = [coord(), coord(), coord(), coord()];
            if (rnd(4) === 0) [c[2], c[3]] = [c[0], c[1]];
            const w = addWormhole(c[0], c[1], c[2], c[3]);
            calls.push([...c, w]);
            if (w === 0) break;
          }
        }
        loadSave(slot, Memory.allocAnsiString('Wormhole ' + idx), 2);
        emit({ k: 'save', idx, slot, name: 'Wormhole ' + idx, frames, snap: {}, mp: '', start, calls: half ? calls : [] });
      }
      flush();
      busy = false;
      next++;
      if (next >= cfg.days.length) { emit({ k: 'done' }); flush(); phase = 2; }
      return;
    }
    if (cfg.cursor) {
      const units = fuzzUnits(), slot = cfg.slot + next, name = 'Cursor ' + next;
      loadSave(slot, Memory.allocAnsiString(name), 2);
      emit({ k: 'save', idx: next, slot, name, frames, snap: {}, mp: '', units, cursor: cursorProbes(40) });
      flush();
      busy = false;
      next++;
      if (next >= cfg.days.length) { emit({ k: 'done' }); flush(); phase = 2; }
      return;
    }
    if (cfg.refit) {
      const fuzz = fuzzBuildings();
      for (const half of [0, 1]) {
        const idx = 2 * next + half, slot = cfg.slot + idx;
        if (half) refit();
        loadSave(slot, Memory.allocAnsiString('Refit ' + idx), 2);
        emit({ k: 'save', idx, slot, name: 'Refit ' + idx, frames, snap: {}, mp: '', fuzz });
      }
      flush();
      busy = false;
      next++;
      if (next >= cfg.days.length) { emit({ k: 'done' }); flush(); phase = 2; }
      return;
    }
    const slot = cfg.slot + next, name = 'Probe ' + next;
    const snap = snapshot();
    loadSave(slot, Memory.allocAnsiString(name), 2);
    // The multiplayer state blob of the same moment (`GetSavedGame(player)`: [clen][dlen][zlib]); it leaves
    // `Multiplayer_save` set, which `LoadSave` clears again at its start.
    let mp = '';
    if (cfg.mp) {
      const blob = getSavedGame(A.EN.readS32());
      if (!blob.isNull()) mp = hexOf(blob, blob.readS32() + 8);
      A.MultiplayerSave.writeU8(0);
    }
    emit({ k: 'save', idx: next, slot, name, frames, snap, mp });
    flush();
    busy = false;
    next++;
    if (next >= cfg.days.length) { emit({ k: 'done' }); flush(); phase = 2; }
  },
});

send({ type: 'ready', base: mod.base.toString() });
