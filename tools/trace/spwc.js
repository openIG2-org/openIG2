// Frida agent: the real-time space battle (SpwC) on the GOG ig2.exe (spec re/rules/space-combat.md §7).
// Loads quick.scn, gives every empire warships (as spacewar.js does), then starts real-time battles between
// a human fleet and an AI fleet through SpaceWarIndit (0x5a7670, with the "never lopsided" debug switch).
// The game switches to the battle screen (AktKep 0xB) and runs FRISSIT_SpaceWar -> SpwC::frissit ->
// SpwC::SpaceWarFrissit (0x56dff0) once per battle tick. That function is hooked: the first call of a
// battle emits every memory region the step reads or writes ('init'); every call emits the byte runs that
// changed in those regions ('step'). Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0),
  SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  SpaceWarIndit: va(0x5a7670), SpaceWarFrissit: va(0x56dff0),
  RandSeed: va(0xbaf2d0),
  Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8),
  Hajok: va(0xbb4100), HajoCount: va(0xbb4104),
  Tankok: va(0xbb41a8), TankCount: va(0xbb4154),
  Tervek: va(0xb9aa18), TervCount: va(0xb9c028),
  Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8), HanyEpulet: va(0xba9840), Epuletek: va(0xba9e90),
  PlayerKi: va(0xbae560), PlayerFaj: va(0xbae574), EN: va(0x912a3c), MP: va(0xbae2c0),
  DebugAlwaysSim: va(0xb77230), DebugNoLopsided: va(0xb77238),
  AktKep: va(0xbae0fc), AktKepKell: va(0xbae250), IdoPause: va(0xbae264),
  BattleSpeed: va(0xbae260),     // IdoSpeedSzorzoBattle: SpaceWarFrissit calls per frame
  FlottaNew: va(0x53d400), HajoNew: va(0x53e800), TankNew: va(0x541750),
  Forms: va(0x9256f8),           // fegyveralakok: 40 x 0x78 (loaded +0, slots +0x14, sprites +0x3c, pool +0x74)
  Weapons: va(0x91d858),         // fegyverek: 40 x 0x5c (recomputed by fegyverszamol at InitSpaceWar)
  StoryBonus: va(0xf27470),      // StoryAdat +0x78..+0x84 (hit chance and damage bonuses)
};
const GLOBALS = [[0xbae264, 4], [0xbae2c0, 4], [0xb77244, 4], [0xe72dd3, 1], [0x926b3c, 4], [0xe72e14, 4],
  [0xf27470, 16], [0xbae24d, 1], [0xbae0fc, 4], [0xbae250, 4], [0x912a3c, 4], [0xbae560, 19],
  [0xdb2848, 76], [0xdb2898, 76], [0xf2748c, 8], [0xf6915d, 1], [0xf66f2a, 1]];
const SPW0 = va(0xe417c0), SPW_STRIDE = 0x2d9c;   // SpwC spw[10]; active flag +0x1f2e
const activeSlots = () => { let n = 0; for (let b = 0; b < 10; b++) if (SPW0.add(SPW_STRIDE * b).add(0x1f2e).readU8()) n++; return n; };
const OBJ = 0x2d9c, UNIT = 600, FIGHTER = 0xa8, GUN = 0xa8, SHOT = 0xa8, SPRITE = 0x4c, WEAPON = 0x70;

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, steps: 3000, battles: 20, seed: 4242,
  fuzz: false, aiHuman: false, planets: false, join: false, fuzzDesigns: false, every: 4, speed: 4, perBattle: 1500,
  orders: 0, orderEvery: 5 };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 4) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

function hexOf(bytes) {
  let s = '';
  for (let i = 0; i < bytes.length; i++) s += (bytes[i] < 16 ? '0' : '') + bytes[i].toString(16);
  return s;
}
let fz = 1;
const rnd = (n) => { fz ^= fz << 13; fz >>>= 0; fz ^= fz >>> 17; fz ^= fz << 5; fz >>>= 0; return fz % n; };

const betolt = new NativeFunction(A.Betolt_Scenario, 'void', ['pointer'], { abi: 'mscdecl', traps: 'all' });
const setSpeed = new NativeFunction(A.SetIdoSpeed, 'void', ['int', 'int'], { abi: 'mscdecl', traps: 'all' });
const setPause = new NativeFunction(A.SetIdoPause, 'void', ['int', 'int'], { abi: 'mscdecl', traps: 'all' });
const indit = new NativeFunction(A.SpaceWarIndit, 'void', ['int', 'int', 'int'], { abi: 'mscdecl', traps: 'all' });
const flottaNew = new NativeFunction(A.FlottaNew, 'int', ['pointer', 'int', 'int', 'float', 'float', 'float'], { abi: 'mscdecl', traps: 'all' });
const hajoNew = new NativeFunction(A.HajoNew, 'void', ['int', 'int'], { abi: 'mscdecl', traps: 'all' });
const tankNew = new NativeFunction(A.TankNew, 'void', ['int', 'int', 'int'], { abi: 'mscdecl', traps: 'all' });
const fleetName = Memory.allocAnsiString('Probe');

const fleetPtr = (i) => A.Flottak.readPointer().add(0x80 * i);
const owner = (i) => fleetPtr(i).add(0x15).readU8();
const nFleets = () => A.FlottaCount.readS32();
const designs = (e) => A.Tervek.add(4 * e).readPointer();

function populate() {
  const np = A.HanyBolygo.readS32();
  for (let e = 1; e <= 8; e++) {
    const cnt = A.TervCount.add(4 * e).readS32();
    if (cnt < 1) continue;
    const ships = [], vehicles = [], tanks = [];
    for (let d = 1; d <= cnt; d++) {
      const p = designs(e).add(0x84 * d), cls = p.add(0x16).readS16();
      if (p.add(0x2c).readS32() <= 0 && p.add(0x30).readS32() <= 0) continue;
      if (cls === 1 && p.add(0x36).readU8() === 0) ships.push(d); else if (cls === 1) vehicles.push(d);
      else if (cls === 2) tanks.push(d);
    }
    if (!ships.length) continue;
    const mine = [];
    for (let q = 1; q <= np; q++) if (A.Bolygok.add(0x188 * q + 0x20).readU8() === e) mine.push(q);
    if (!mine.length) continue;
    for (let k = 0; k < 8; k++) {
      const q = A.Bolygok.add(0x188 * mine[rnd(mine.length)]);
      const f = flottaNew(fleetName, e, 0, q.add(0x18).readFloat() + rnd(9) - 4, q.add(0x1c).readFloat() + rnd(9) - 4, 0);
      if (f < 1) continue;
      // Half the fleets repeat one design, so the battle groups them into multi-ship units.
      const same = rnd(2) ? ships[rnd(ships.length)] : 0;
      // A few big fleets: more ships than formation slots, so units hold several ships.
      const n = same && rnd(3) === 0 ? 20 + rnd(25) : 2 + rnd(8);
      for (let j = 0; j < n; j++) hajoNew(f, same || ships[rnd(ships.length)]);
      // --fuzz-designs: some carriers with many fighters (more than 60 in a battle scales the groups).
      if (vehicles.length && rnd(3)) tankNew(f, vehicles[rnd(vehicles.length)], cfg.fuzzDesigns && rnd(3) === 0 ? 30 + rnd(50) : 1 + rnd(8));
      if (vehicles.length && rnd(3) === 0) tankNew(f, vehicles[rnd(vehicles.length)], 1 + rnd(8));
      if (tanks.length && rnd(2)) tankNew(f, tanks[rnd(tanks.length)], 1 + rnd(4));
    }
  }
  emit({ k: 'populated', fleets: nFleets(), ships: A.HajoCount.readS32() });
}

// ---------------------------------------------------------------- regions
// Every region the battle step reads or writes, keyed by a stable name. Pointer-held arrays are keyed by
// owner index so a re-allocation shows up as a new key.
function regions(self) {
  const out = [];
  const add = (key, p, n) => {
    if (p.isNull() || n <= 0) return;
    try { p.readU8(); p.add(n - 1).readU8(); out.push([key, p, n]); } catch (e) { /* stale pointer */ }
  };
  add('obj', self, OBJ);
  const units = self.add(0xc20).readPointer(), nu = units.isNull() ? 0 : self.add(0xc28).readS32();
  add('units', units, UNIT * nu);
  for (let u = 0; u < nu; u++) {
    const p = units.add(UNIT * u);
    add('w' + u + '@' + p.add(0x21c).readPointer(), p.add(0x21c).readPointer(), WEAPON * p.add(0x218).readS32());
    add('s' + u + '@' + p.add(0x28).readPointer(), p.add(0x28).readPointer(), 4 * Math.max(p.add(0x1b0).readS32(), 1));
  }
  // Battle-AI squads: per empire (+0x6c e) a count (+4) and an array (+8) of 0x1c-byte squads, each with
  // a member count (+0) and member list (+0x18).
  for (let e = 0; e < 19; e++) {
    const n = self.add(0x6c * e + 4).readS32(), sp = self.add(0x6c * e + 8).readPointer();
    if (n <= 0 || n > 200 || sp.isNull()) continue;
    add('sq' + e + '@' + sp, sp, 0x1c * n);
    for (let g = 0; g < n; g++) {
      const q = sp.add(0x1c * g), m = q.readS32(), mp = q.add(0x18).readPointer();
      if (m > 0 && m < 200) add('sm' + e + '_' + g + '@' + mp, mp, 4 * m);
    }
  }
  add('fighters', self.add(0xc24).readPointer(), FIGHTER * self.add(0xc2c).readS32());
  add('guns', self.add(0xc94).readPointer(), GUN * self.add(0xc90).readS32());
  for (let f = 0; f < 40; f++) {
    const fp = A.Forms.add(0x78 * f), n = fp.add(0x14).readS32();
    add('form' + f, fp, 0x78);
    if (fp.readU8()) {
      add('pool' + f + '@' + fp.add(0x74).readPointer(), fp.add(0x74).readPointer(), SHOT * n);
      add('spr' + f + '@' + fp.add(0x3c).readPointer(), fp.add(0x3c).readPointer(), SPRITE * n);
    }
  }
  add('seeds', A.RandSeed, 4 * 21);
  add('hajok@' + A.Hajok.readPointer(), A.Hajok.readPointer(), 6 * (A.HajoCount.readS32() + 1));
  add('hajocount', A.HajoCount, 4);
  add('flottak@' + A.Flottak.readPointer(), A.Flottak.readPointer(), 0x80 * (nFleets() + 1));
  add('tankok', A.Tankok, 8 * (A.TankCount.readS32() + 1));
  add('tankcount', A.TankCount, 4);
  const pl = self.add(0x1f28).readS32();
  if (pl > 0 && pl <= A.HanyBolygo.readS32()) {
    add('planet' + pl, A.Bolygok.add(0x188 * pl), 0x188);
    const nb = A.HanyEpulet.add(4 * pl).readS32();
    if (nb > 0) add('buildings' + pl + '@' + A.Epuletek.add(4 * pl).readPointer(), A.Epuletek.add(4 * pl).readPointer(), 0x20 * (nb + 1));
  }
  for (const [g, n] of GLOBALS) add('g' + g.toString(16), va(g), n);
  return out;
}
// Static data the step reads: designs of the participants, ship-type records, weapons.
function statics(self) {
  const st = { weapons: hexOf(new Uint8Array(A.Weapons.readByteArray(40 * 0x5c))), types: {}, designs: {},
    buildingHp: hexOf(new Uint8Array(va(0x8eba40).readByteArray(0x24 * 64))) };
  const units = self.add(0xc20).readPointer(), nu = units.isNull() ? 0 : self.add(0xc28).readS32();
  for (let u = 0; u < nu; u++) {
    const t = units.add(UNIT * u).add(0x214).readPointer();
    if (!t.isNull() && !(t.toString() in st.types)) st.types[t.toString()] = hexOf(new Uint8Array(t.readByteArray(0x40)));
  }
  for (let e = 1; e <= 18; e++) {
    const c = A.TervCount.add(4 * e).readS32();
    if (c > 0 && c < 300) st.designs[e] = hexOf(new Uint8Array(designs(e).readByteArray(0x84 * (c + 1))));
  }
  st.formsBase = A.Forms.toString();
  st.base = mod.base.toString();
  return st;
}

function grab(rs) {
  const m = {};
  for (const [k, p, n] of rs) m[k] = { p: p.toString(), b: new Uint8Array(p.readByteArray(n)) };
  return m;
}
function diff(prev, cur) {
  const out = {};
  for (const k in cur) {
    const a = prev[k], b = cur[k].b;
    if (!a || a.b.length !== b.length || a.p !== cur[k].p) { out[k] = { p: cur[k].p, full: hexOf(b) }; continue; }
    const runs = [];
    let i = 0;
    while (i < b.length) {
      if (a.b[i] === b[i]) { i++; continue; }
      let j = i + 1, gap = 0;
      while (j < b.length && gap < 8) { if (a.b[j] === b[j]) gap++; else gap = 0; j++; }
      const end = j - gap;
      runs.push([i, hexOf(b.subarray(i, end))]);
      i = end;
    }
    if (runs.length) out[k] = { r: runs };
  }
  for (const k in prev) if (!(k in cur)) out[k] = { gone: 1 };
  return out;
}

// A won planet battle with ground units aboard starts the ground war (another screen): not here.
Interceptor.replace(va(0x59c2d0), new NativeCallback(function () { emit({ k: 'groundwar' }); }, 'void', ['int', 'int'], 'mscdecl'));

// ---------------------------------------------------------------- battle hook
let warnedMulti = false;
let cur = null, battleSerial = 0, steps = 0, lastSelf = null, battleSteps = 0;
Interceptor.attach(A.SpaceWarFrissit, {
  onEnter() {
    this.on = false;
    if (steps >= cfg.steps) return;
    const self = this.context.ecx;
    if (!self.add(0x1f2e).readU8()) return;
    // The projectile pools are global and every active slot moves every projectile: record one battle at a time.
    if (activeSlots() !== 1) { if (!warnedMulti) { warnedMulti = true; emit({ k: 'multi', n: activeSlots() }); } return; }
    this.on = true;
    this.self = self;
    if (!lastSelf || !lastSelf.equals(self) || cur === null) {
      battleSerial++;
      lastSelf = self;
      battleSteps = 0;
      // --ai-human: the battle AI also plays the human's side (otherwise its units only shoot back).
      if (cfg.aiHuman) self.add(0x6c * self.add(0xc3c).readS32() + 0x50).writeU8(1);
      if (cfg.fuzz) fuzzBattle(self);
      cur = grab(regions(self));
      const full = {};
      for (const k in cur) full[k] = { p: cur[k].p, full: hexOf(cur[k].b) };
      emit({ k: 'init', battle: battleSerial, self: self.toString(), statics: statics(self), regions: full });
    } else {
      // State changed between steps (the UI tick, our own fuzzing): record it as a pre-step diff.
      const now = grab(regions(self));
      const d = diff(cur, now);
      if (Object.keys(d).length) emit({ k: 'between', battle: battleSerial, d });
      cur = now;
    }
    if (cfg.orders && battleSteps % cfg.orderEvery === 0) stageOrders(self);
  },
  onLeave() {
    if (!this.on) return;
    const now = grab(regions(this.self));
    emit({ k: 'step', battle: battleSerial, n: steps, d: diff(cur, now) });
    cur = now;
    steps++;
    battleSteps++;
    if (steps >= cfg.steps) emit({ k: 'done' });
  },
});

// ---------------------------------------------------------------- staged orders (MULTIGET_SpaceWar)
// --orders N: every --order-every ticks, N random battle-screen orders go through the exe's own MULTIGET_SpaceWar
// (0x5615b0, spec §7.12) between two ticks: its Multi_GetNextTipus calls (0x513340) are pointed at a stub that
// returns the staged type once, only to a call whose range holds it; its Multi_Get calls (0x513160) at a stub that
// writes the staged ints to the out-parameters ([esp + 8 + 8i]). Each order is an 'order' record: type, values,
// and the byte diff it made.
const MULTIGET = va(0x5615b0);
const NEXT_CALLS = [0x5615d9, 0x561a9e, 0x561ab9, 0x561c0b].map(va);
const GET_CALLS = [0x56160f, 0x5616a5, 0x56172d, 0x561783, 0x5617e8, 0x561850, 0x5618a1, 0x5619b4, 0x561a5a,
  0x561ae2, 0x561b3b, 0x561bb1].map(va);
const multiget = new NativeFunction(MULTIGET, 'void', [], { abi: 'mscdecl', traps: 'all' });
let ostub = null;
function stubs() {
  if (ostub !== null) return;
  ostub = Memory.alloc(Process.pageSize);
  const type = ostub.add(0x200), vals = ostub.add(0x210);
  Memory.patchCode(ostub, 0x180, (code) => {
    const w = new X86Writer(code, { pc: ostub });
    // eax = the staged type; 0 or outside [lo, hi] (the dword arguments) -> 0; else clear it and return it.
    w.putMovRegAddress('ecx', type); w.putMovRegRegPtr('eax', 'ecx');
    w.putCmpRegI32('eax', 0); w.putJccShortLabel('je', 'none', 'no-hint');
    w.putMovRegRegOffsetPtr('edx', 'esp', 4); w.putCmpRegReg('eax', 'edx'); w.putJccShortLabel('jl', 'none', 'no-hint');
    w.putMovRegRegOffsetPtr('edx', 'esp', 8); w.putCmpRegReg('eax', 'edx'); w.putJccShortLabel('jg', 'none', 'no-hint');
    w.putMovRegPtrU32('ecx', 0); w.putRet();
    w.putLabel('none'); w.putXorRegReg('eax', 'eax'); w.putRet();
    w.flush();
    const g = new X86Writer(code.add(0x80), { pc: ostub.add(0x80) });
    // Arguments: type, then (out pointer, size marker) pairs; the list ends with a 0 in a marker's place.
    for (let i = 0; i < 8; i++) {
      g.putMovRegRegOffsetPtr('ecx', 'esp', 8 + 8 * i);
      g.putMovRegAddress('edx', vals.add(4 * i)); g.putMovRegRegPtr('eax', 'edx');
      g.putMovRegPtrReg('ecx', 'eax');
      g.putMovRegRegOffsetPtr('ecx', 'esp', 0xc + 8 * i);
      g.putCmpRegI32('ecx', 0); g.putJccNearLabel('je', 'done', 'no-hint');
    }
    g.putLabel('done'); g.putMovRegU32('eax', 1); g.putRet();
    g.flush();
  });
}
function patchCalls(calls, target) {
  const saved = calls.map((c) => c.readByteArray(5));
  for (const c of calls) Memory.patchCode(c, 5, (code) => { const w = new X86Writer(code, { pc: c }); w.putCallAddress(target); w.flush(); });
  return () => calls.forEach((c, i) => Memory.patchCode(c, 5, (code) => code.writeByteArray(saved[i])));
}
function runOrder(type, vals) {
  stubs();
  ostub.add(0x200).writeS32(type);
  for (let i = 0; i < 8; i++) ostub.add(0x210 + 4 * i).writeS32(i < vals.length ? vals[i] : 0);
  const undoNext = patchCalls(NEXT_CALLS, ostub), undoGet = patchCalls(GET_CALLS, ostub.add(0x80));
  try { multiget(); } finally { undoNext(); undoGet(); }
}
const f32bits = (x) => { const b = new ArrayBuffer(4); new DataView(b).setFloat32(0, x, true); return new DataView(b).getInt32(0, true); };
// A random order for this battle; the units, guns and mounts it names exist.
function makeOrder(self, b) {
  const units = self.add(0xc20).readPointer(), nu = self.add(0xc28).readS32();
  const ng = self.add(0xc90).readS32();
  const unit = (u) => units.add(UNIT * u);
  const all = [], alive = [], owners = [];
  for (let u = 0; u < nu; u++) {
    all.push(u);
    if (unit(u).readU8()) { alive.push(u); const o = unit(u).add(0x14).readS32(); if (o > 0 && !owners.includes(o)) owners.push(o); }
  }
  if (!all.length) return null;
  const pick = (a) => a[rnd(a.length)];
  const some = () => (alive.length && rnd(6)) ? pick(alive) : pick(all);
  const issuer = owners.length ? pick(owners) : 1;
  const r = rnd(100);
  if (r < 22) {
    // A point the screen can send: JobbClick clamps it to the bounds less two cells (x = height: ±9, y/z: ±186).
    const x = (rnd(181) - 90) / 10, y = rnd(372) - 186 + rnd(1000) / 1000, z = rnd(372) - 186 + rnd(1000) / 1000;
    return [0x70, [issuer, f32bits(x), f32bits(y), f32bits(z), some(), b]];
  }
  if (r < 42) return [0x71, [issuer, some(), rnd(10) ? some() : -1, b]];
  if (r < 46) return [0x72, [issuer, b]];
  if (r < 54 && ng > 0) return [0x73, [issuer, some(), rnd(8) ? rnd(ng) : -1, b]];
  if (r < 60 && ng > 0) return [0x74, [issuer, rnd(ng), rnd(8) ? some() : -1, b]];
  if (r < 62) return [0x75, [issuer]];
  if (r < 64) return [0x76, [issuer, b, some()]];
  if (r < 72) return [0x77, [issuer, rnd(101), rnd(19), b]];
  if (r < 88) {
    const armed = all.filter((u) => unit(u).add(0x218).readS32() > 0);
    if (!armed.length) return null;
    const u = rnd(6) ? pick(armed) : pick(all);
    const nm = unit(u).add(0x218).readS32();
    if (nm < 1) return null;
    return [0x44e, [issuer, u, some(), rnd(nm), b]];
  }
  const keep = rnd(3) ? ~(1 << rnd(32)) : rnd(0x7fffffff);
  return [0x44f, [issuer, some(), keep | 0, rnd(4) ? 0 : (1 << rnd(32)) | 0, b]];
}
function stageOrders(self) {
  const b = self.sub(SPW0).toInt32() / SPW_STRIDE;
  for (let i = 0; i < cfg.orders; i++) {
    const o = makeOrder(self, b);
    if (!o) continue;
    runOrder(o[0], o[1]);
    const now = grab(regions(self));
    emit({ k: 'order', battle: battleSerial, n: steps, type: o[0], vals: o[1], d: diff(cur, now) });
    cur = now;
  }
}

// ---------------------------------------------------------------- fuzzing a fresh battle
const PODS = [0x20, 0x40, 0x80, 0x100, 0x200, 0x400];
function fuzzBattle(self) {
  const units = self.add(0xc20).readPointer(), nu = self.add(0xc28).readS32();
  for (let u = 0; u < nu; u++) {
    const p = units.add(UNIT * u);
    if (p.add(0x14).readS32() === 0) continue;
    let pods = 0;
    for (const f of PODS) if (rnd(3) === 0) pods |= f;
    p.add(0x1a8).writeU32(pods);
    if (rnd(2)) p.add(0x1a4).writeS32(rnd(400));                    // experience
    if (rnd(2)) p.add(0x24).writeS32(rnd(p.add(0x1c).readS32() + 1)); // shield
    // Re-arm with random weapons (the mount copies its kind, reload and range from the live table).
    const nw = p.add(0x218).readS32(), w = p.add(0x21c).readPointer();
    for (let k = 0; k < nw; k++) {
      if (rnd(3) === 0) continue;
      const id = rnd(3) === 0 ? 9 + rnd(4) : rnd(30), wr = A.Weapons.add(0x5c * id), m = w.add(0x70 * k);
      m.writeS32(id); m.add(4).writeS32(wr.readS32()); m.add(0x14).writeS32(wr.add(4).readS32());
      m.add(0x60).writeS32(wr.add(0x50).readS32());
      m.add(0xc).writeS32(rnd(60) - 1);
      if (rnd(2)) m.add(8).writeS32(1 + rnd(6));
    }
    // Recompute the range span the way fegyverhozzaad does.
    let lo = 0, hi = 0;
    for (let k = 0; k < nw; k++) {
      const m = w.add(0x70 * k);
      if (m.add(4).readS32() === 2) continue;
      const r = m.add(0x60).readS32();
      if (lo === 0 || r < lo) lo = r;
      if (r > hi) hi = r;
    }
    p.add(0x168).writeFloat(lo); p.add(0x16c).writeFloat(hi);
  }
  for (let e = 0; e < 19; e++) {
    const r = self.add(0x6c * e);
    r.add(0x40).writeS32(rnd(101));
    if (rnd(3) === 0) r.add(0x58).writeS32(r.add(0x54).readS32() * (1 + rnd(3)));  // provoke retreats
  }
  A.StoryBonus.writeS32(rnd(3) ? 0 : rnd(50)); A.StoryBonus.add(8).writeS32(rnd(3) ? 0 : rnd(50));
}

// Give a planet defence ships (its owner's designs) and finished planet guns (building types 28..31).
function armPlanet(pl) {
  const p = A.Bolygok.add(0x188 * pl), e = p.add(0x20).readU8();
  const cnt = A.TervCount.add(4 * e).readS32();
  const ships = [];
  for (let d = 1; d <= cnt; d++) {
    const q = designs(e).add(0x84 * d);
    if (q.add(0x16).readS16() === 1 && q.add(0x36).readU8() === 0 && q.add(0x2c).readS32() > 0) ships.push(d);
  }
  if (ships.length) {
    const n = 1 + rnd(6);
    p.add(0x114).writeS32(n);
    for (let k = 1; k <= n; k++) p.add(0xfe + 2 * k).writeS16(ships[rnd(ships.length)]);
  }
  const nb = A.HanyEpulet.add(4 * pl).readS32();
  for (let k = 0; k < 4 && nb > 0; k++) {
    const b = A.Epuletek.add(4 * pl).readPointer().add(0x20 * (1 + rnd(nb)));
    if (b.readU8() === 0) continue;
    // Guns 28..31; with --fuzz-designs also techs the setup skips (32..35) and armour (36..38), some switched off.
    b.writeU8(cfg.fuzzDesigns ? 28 + rnd(11) : 28 + rnd(4));
    b.add(4).writeS32(0);
    b.add(8).writeS16(0);
    if (cfg.fuzzDesigns) b.add(0x19).writeU8(rnd(5) ? 1 : 0);
  }
}

// ---------------------------------------------------------------- setup snapshots
// The strategic state SpaceWar_NewBattle reads (spec §7.10) on top of a slot's regions: relations, who is
// human, the switches, the participant list, the story pacts, the ship-type and hull-weapon tables, the
// building techs and the battle planet. 'presetup' is taken just before SpaceWarIndit, 'setup' right after
// (the hooks can't see inside our own NativeFunction call, and SpaceWarIndit only clears orders, rolls the
// story and the diplomatic incident before SpaceWar_NewBattle, so the story arrays and relations are read
// from 'setup').
const SETUP_GLOBALS = [[0xdabf38, 19 * 19 * 0x26], [0xbae560, 19], [0x912a3c, 4], [0xbae2c0, 4], [0xe72dd2, 2],
  [0xbaa1c0, 0x30], [0xb9c027, 1], [0xf26150, 0xa0], [0x91f488, 0x2d00], [0xe5e08c, 0x23a0], [0x8eba38, 0x24 * 64],
  [0xe72e14, 4], [0xe72dc8, 4], [0xf691e8, 12]];
const slotPtr = (b) => SPW0.add(SPW_STRIDE * b);
function setupRegions(self, planet) {
  const out = regions(self);
  for (const [g, n] of SETUP_GLOBALS) out.push(['g' + g.toString(16), va(g), n]);
  // The bytes before Hajok: a defence ship (−k) without a formation slot reads Hajok[−k].fleet there.
  out.push(['hajokpre', A.Hajok.readPointer().sub(0x30), 0x30]);
  if (planet > 0 && planet !== self.add(0x1f28).readS32()) {
    out.push(['planet' + planet, A.Bolygok.add(0x188 * planet), 0x188]);
    const nb = A.HanyEpulet.add(4 * planet).readS32();
    if (nb > 0) out.push(['buildings' + planet + '@' + A.Epuletek.add(4 * planet).readPointer(), A.Epuletek.add(4 * planet).readPointer(), 0x20 * (nb + 1)]);
  }
  return out;
}
function full(rs) {
  const m = grab(rs), o = {};
  for (const k in m) o[k] = { p: m[k].p, full: hexOf(m[k].b) };
  return o;
}
let setupSerial = 0;
// SpaceWarIndit(f1, f2, planet) with snapshots around it. The slot SpaceWar_NewBattle will use: the active
// one holding f1 or f2 (a join), else the first inactive one.
function inditTraced(f1, f2, planet) {
  let slot = -1, join = false;
  for (let b = 0; b < 10 && slot < 0; b++) {
    const s = slotPtr(b);
    if (!s.add(0x1f2e).readU8()) continue;
    const n = s.add(0x1420).readS32();
    for (let k = 0; k < n; k++) {
      const fl = s.add(0xea0 + 0x30 * k).readS32();
      if (fl === f1 || (f2 > 0 && fl === f2)) { slot = b; join = true; }
    }
  }
  if (slot < 0) for (let b = 0; b < 10 && slot < 0; b++) if (!slotPtr(b).add(0x1f2e).readU8()) slot = b;
  setupSerial++;
  const self = slotPtr(slot);
  if (cfg.fuzzDesigns) fuzzDesigns(f2 > 0 ? [f1, f2] : [f1], planet);
  emit({ k: 'presetup', setup: setupSerial, slot, join, fleets: f2 > 0 ? [f1, f2] : [f1], planet,
    statics: statics(self), regions: full(setupRegions(self, planet)) });
  indit(f1, f2, planet);
  emit({ k: 'setup', setup: setupSerial, slot, active: self.add(0x1f2e).readU8(), statics: statics(self),
    regions: full(setupRegions(self, planet)) });
  // A join changes a running battle: the next tick's 'between' diff carries it to the tick replay.
}

// --fuzz-designs: the participants' designs get random weapons, shields and special pods in their slots
// (quick.scn's designs have none of the latter), so AddUjUrhajo's pod pass is exercised.
const POD_IDS = [0x25, 0x27, 0x28, 0x29, 0x2a, 0x37, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36];
function fuzzDesigns(fleets, planet) {
  // Big fighter wings (AddFleet scales the groups when a battle has more than 60): the setup reads only the
  // Tankok counts (carrier capacity would cap them otherwise).
  const nt = A.TankCount.readS32();
  for (let i = 1; i <= nt; i++) {
    const t = A.Tankok.add(8 * i), fl = t.readS16(), d = t.add(2).readS16();
    if (d === 0x7fff || !fleets.includes(fl) || rnd(2)) continue;
    if (designs(owner(fl)).add(0x84 * d).add(0x36).readU8()) t.add(4).writeS32(30 + rnd(70));
  }
  const owners = new Set(fleets.map(owner));
  if (planet > 0) owners.add(A.Bolygok.add(0x188 * planet).add(0x20).readU8());
  for (const e of owners) {
    if (e < 1 || e > 18) continue;
    const cnt = A.TervCount.add(4 * e).readS32();
    for (let d = 1; d <= cnt; d++) {
      const p = designs(e).add(0x84 * d);
      if (p.add(0x16).readS16() !== 1 || rnd(2)) continue;
      const slots = Math.min(p.add(0x4c).readS32(), 12);
      for (let k = 1; k <= slots; k++) {
        const r = rnd(4);
        if (r === 0) continue;
        const id = r === 1 ? 0x11 + rnd(16) : r === 2 ? 0x41 + rnd(8) : POD_IDS[rnd(POD_IDS.length)];
        p.add(0x50 + k).writeU8(id);
        p.add(0x5d + k).writeU8(1 + rnd(4));
      }
    }
  }
}

// --join: right after a new battle is set up (before its screen loads its graphics), another AI fleet joins it
// (SpaceWar_NewBattle's join path; a join while the battle is shown reloads the graphics and crashed the game
// when called from our frame hook). `onto` is a fleet in the battle, `planet` keeps a planet battle real-time.
function joinNow(onto, planet) {
  if (!cfg.join || rnd(3) === 0) return;
  const en = A.EN.readS32(), cand = [];
  for (let i = 1; i <= nFleets(); i++) {
    const o = owner(i);
    if (o === 0xff || o < 1 || o > 8 || o === en || i === onto) continue;
    if (fleetPtr(i).add(0x54).readS32() > 0) cand.push(i);
  }
  if (!cand.length) return;
  const g = cand[rnd(cand.length)];
  if (planet > 0) {
    const q = A.Bolygok.add(0x188 * planet), fp = fleetPtr(g);
    fp.add(0x30).writeFloat(q.add(0x18).readFloat()); fp.add(0x34).writeFloat(q.add(0x1c).readFloat());
  }
  emit({ k: 'join', battle: battleSerial + 1, fleet: g, onto, planet });
  inditTraced(g, onto, planet);
}

// ---------------------------------------------------------------- driver
function startBattle() {
  const en = A.EN.readS32();
  const fl = [];
  for (let i = 1; i <= nFleets(); i++) {
    const o = owner(i);
    if (o === 0xff || o < 1 || o > 8) continue;
    if (fleetPtr(i).add(0x54).readS32() > 0) fl.push(i);
  }
  const human = fl.filter((i) => owner(i) === en), ai = fl.filter((i) => owner(i) !== en);
  if (!human.length || !ai.length) return false;
  const h = human[rnd(human.length)], f = ai[rnd(ai.length)];
  A.DebugNoLopsided.writeS32(1);
  A.DebugAlwaysSim.writeS32(0);
  if (cfg.planets && rnd(3)) {
    // At one of the human's planets, armed for the occasion.
    const np = A.HanyBolygo.readS32(), mine = [];
    for (let q = 1; q <= np; q++) if (A.Bolygok.add(0x188 * q + 0x20).readU8() === en) mine.push(q);
    if (mine.length) {
      const pl = mine[rnd(mine.length)];
      armPlanet(pl);
      const q = A.Bolygok.add(0x188 * pl);
      const fp = fleetPtr(f);
      fp.add(0x30).writeFloat(q.add(0x18).readFloat()); fp.add(0x34).writeFloat(q.add(0x1c).readFloat());
      emit({ k: 'start', attacker: f, planet: pl, aOwner: owner(f) });
      inditTraced(f, 0, pl);
      joinNow(f, pl);
      emit({ k: 'started', kepKell: A.AktKepKell.readS32(), slots: activeSlots() });
      return true;
    }
  }
  emit({ k: 'start', attacker: f, defender: h, aOwner: owner(f), dOwner: owner(h) });
  inditTraced(f, h, 0);
  joinNow(h, 0);
  emit({ k: 'started', kepKell: A.AktKepKell.readS32(), slots: activeSlots() });
  return true;
}

let frames = 0, started = false, loadedAt = 0, populated = false, pending = 0, onScreen = 0;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (!started && frames === cfg.startAfterFrames) {
      started = true; fz = cfg.seed;
      if (cfg.fuzz) va(0xe72dd2).writeU8(1);   // `asteroidak`: battles get scenery
      betolt(Memory.allocAnsiString(cfg.scenario));
      setSpeed(3, 1);
      loadedAt = frames;
      emit({ k: 'loaded' });
      return;
    }
    if (!started || frames < loadedAt + 300 || steps >= cfg.steps) return;
    if (frames % cfg.every !== 0) return;
    try {
      if (!populated) { populate(); populated = true; return; }
      const kep = A.AktKep.readS32();
      if (kep === 0xb) {
        onScreen++;
        pending = 0;
        if (A.IdoPause.readU8()) setPause(0, 0);
        A.BattleSpeed.writeS32(cfg.speed);
        // A long battle is abandoned (its slot closed) so the session sees more of them.
        if (battleSteps >= cfg.perBattle && lastSelf) {
          lastSelf.add(0x1f2e).writeU8(0);
          A.AktKepKell.writeS32(2);
          emit({ k: 'abandoned', battle: battleSerial, steps: battleSteps });
          battleSteps = 0;
        }
        return;
      }
      if (onScreen) {
        onScreen = 0; cur = null;
        // The end (spec §7.11): SpaceWarVege copies the winner and hands the participants to the story.
        emit({ k: 'left', battle: battleSerial, slots: activeSlots(), kiNyert: va(0xbaa1ec).readS32(),
          vesztes: va(0xbaa1de).readU8(), you: va(0xf26531).readU8(), esemeny3: va(0xf2f8af).readU8(),
          num: va(0xf265c8).readS32(), story88: va(0xf27480).readS32() });
      }
      if (pending && frames - pending < 400) return;
      if (activeSlots() > 0) { if (pending) { emit({ k: 'stuck', slots: activeSlots() }); } pending = frames; return; }
      if (battleSerial >= cfg.battles) { emit({ k: 'done' }); steps = cfg.steps; return; }
      startBattle();
      pending = frames;
    } catch (e) { emit({ k: 'error', msg: e.toString(), stack: e.stack }); }
  },
});

send({ type: 'ready', base: mod.base.toString() });
