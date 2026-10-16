// Frida agent: produce original save games on the GOG ig2.exe and snapshot the saved globals at the same moment
// (spec re/formats/save.md). After loading the scenario it waits for the configured days; at each one it reads the
// source memory of the save's sections, then calls the exe's own `LoadSave(slot, name, 2)` (exe 0x543c80) from the
// frame hook (same thread, nothing runs in between), and reports the snapshot. The Python side copies the written
// `save<slot>.sav` to _local/saves/ and removes it from the game's data directory.
// --ships: give empires with designs a few warships (as spy.js) and hand the human's empire to the AI.
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
};

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, days: [3, 40, 150], slot: 97, ships: false, mp: false, seed: 4242 };
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
