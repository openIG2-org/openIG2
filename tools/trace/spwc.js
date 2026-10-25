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
  SpaceWarIndit: va(0x5a7670), SpaceWarFrissit: va(0x56dff0), loadfelallas: va(0x578850),
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
  fuzz: false, aiHuman: false, planets: false, join: false, fuzzDesigns: false, weapons: [], every: 4, speed: 4, perBattle: 1500,
  orders: 0, orderEvery: 5, grabTicks: [], grabSettle: 120, stateSlot: 0, script: null, soundTables: null, formation: 0 };
recv('config', (m) => {
  Object.assign(cfg, m.payload);
  if (cfg.script) hookScript();
  if (cfg.soundTables && typeof hookSounds === 'function') hookSounds();
});

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
const indit = new NativeFunction(A.SpaceWarIndit, 'void', ['int', 'int', 'int'], { abi: 'mscdecl', traps: 'all', exceptions: 'propagate' });
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
  // The human's ship list (side +0x5c / +0x60, 0x1c-byte entries): what a formation pick places anew.
  const h = self.add(0xc3c).readS32();
  if (h >= 0 && h < 19) {
    const lp = self.add(0x6c * h + 0x5c).readPointer(), ln = self.add(0x6c * h + 0x60).readS32();
    if (ln > 0 && ln < 1000) add('hlist@' + lp, lp, 0x1c * ln);
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
    if (cfg.grabTicks.includes(battleSteps) && !grabPending) {
      // The battle stops (no ticks a frame) while the screen keeps drawing; SetIdoPause left the battle screen.
      grabPending = { tick: battleSteps, self: this.self, swaps: 0 };
      A.BattleSpeed.writeS32(0);
      if (!swapHooked) { swapHooked = true; hookSwap(); emit({ k: 'grabbing', tick: battleSteps }); }
    }
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
  let shield = !cfg.shield;
  for (let k = 0; k < 4 && nb > 0; k++) {
    const b = A.Epuletek.add(4 * pl).readPointer().add(0x20 * (1 + rnd(nb)));
    if (b.readU8() === 0) continue;
    // Guns 28..31; with --fuzz-designs also techs the setup skips (32..35) and armour (36..38), some switched off;
    // --shield: the first one armour (the shield rings, `karikakirak`).
    b.writeU8(!shield ? 36 + rnd(3) : cfg.fuzzDesigns ? 28 + rnd(11) : 28 + rnd(4));
    shield = true;
    b.add(4).writeS32(0);
    b.add(8).writeS16(0);
    if (cfg.fuzzDesigns) b.add(0x19).writeU8(rnd(5) ? 1 : 0);
    // --shield: the armour is the planet's only gun, so the attackers' gun picks (`Lov2`) land on it.
    if (cfg.shield) break;
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
  if (cfg.fuzzDesigns || cfg.weapons.length) fuzzDesigns(f2 > 0 ? [f1, f2] : [f1], planet);
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
    if (d === 0x7fff || !fleets.includes(fl) || !cfg.fuzzDesigns || rnd(2)) continue;
    if (designs(owner(fl)).add(0x84 * d).add(0x36).readU8()) t.add(4).writeS32(30 + rnd(70));
  }
  const owners = new Set(fleets.map(owner));
  if (planet > 0) owners.add(A.Bolygok.add(0x188 * planet).add(0x20).readU8());
  for (const e of owners) {
    if (e < 1 || e > 18) continue;
    const cnt = A.TervCount.add(4 * e).readS32();
    for (let d = 1; d <= cnt; d++) {
      const p = designs(e).add(0x84 * d);
      if (p.add(0x16).readS16() !== 1) continue;
      const slots = Math.min(p.add(0x4c).readS32(), 12);
      if (cfg.weapons.length) {
        // --weapons: every slot of every warship design takes the listed techs in turn.
        for (let k = 1; k <= slots; k++) {
          p.add(0x50 + k).writeU8(cfg.weapons[(k - 1) % cfg.weapons.length]);
          p.add(0x5d + k).writeU8(2);
        }
        continue;
      }
      if (rnd(2)) continue;
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

// --formation N: on the battle screen, with a chance of 1/N per driver frame (at most 3 a battle), a random
// formation pick 0..8 goes into `+0x2d88`; the exe's own `SpaceWarFrissit2` (exe 0x56d870) then calls
// `loadfelallas` (0x578850) when it differs from `+0x2d8c`, in the phase or not (the check isn't guarded by
// `+0x2d85`). 'preform' / 'form' snapshot the slot (with the human's ship list) and the set-up's globals (the
// ship-type footprints) around that call.
let formsBattle = -1, formsDone = 0;
const formRegions = (self) => setupRegions(self, 0);
function formSnap(k, self) {
  emit({ k, battle: battleSerial, self: self.toString(), pick: self.add(0x2d88).readS32(), loaded: self.add(0x2d8c).readS32(),
    human: self.add(0xc3c).readS32(), statics: statics(self), regions: full(formRegions(self)) });
}
Interceptor.attach(A.loadfelallas, {
  onEnter(args) {
    this.on = cfg.formation > 0 && !cfg.script;
    if (!this.on) return;
    this.self = this.context.ecx;
    this.name = args[0].readAnsiString();
    formSnap('preform', this.self);
  },
  onLeave() {
    if (this.on) formSnap('form', this.self);
  },
});
function maybePick() {
  if (!cfg.formation || !lastSelf || !lastSelf.add(0x1f2e).readU8()) return;
  if (formsBattle !== battleSerial) { formsBattle = battleSerial; formsDone = 0; }
  if (formsDone >= 3 || rnd(cfg.formation)) return;
  formsDone++;
  const pick = rnd(9);
  emit({ k: 'pick', battle: battleSerial, pick, was: lastSelf.add(0x2d8c).readS32(), tick: battleSteps });
  lastSelf.add(0x2d88).writeS32(pick);
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
  if (cfg.planets && (rnd(3) || cfg.shield)) {
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
    if (cfg.script) {
      // The capture runs in the background: `IsActive` (0x912A38) kept set, the pointer the script's.
      va(0x912a38).writeU8(1);
      if (scriptT !== null) {
        // What the frame before left in the pointer and the hot-spot (`AktIkon` 0xBAE23C): changes are recorded.
        const seen = [va(0xdb4654).readS32(), va(0xdb4658).readS32()], ikon = va(0xbae23c).readS32();
        if (ikon !== lastIkon || seen[0] !== scriptMouse[0] || seen[1] !== scriptMouse[1]) {
          emit({ k: 'ikon', t: scriptT, ikon, seen, mouse: scriptMouse.slice() });
          lastIkon = ikon;
        }
        // The camera each frame (after the frame before): look-at now/wanted (+0xc5c/+0xc6c), zoom, the view
        // animation flag (+0x1d3e), `ControlConfig[0..1]` (0x90B520).
        if (scriptSelf) {
          const s = scriptSelf, fl = (o, n) => Array.from({ length: n }, (_, k) => s.add(o + 4 * k).readFloat());
          emit({ k: 'cam', t: scriptT, pan: fl(0xc5c, 3), want: fl(0xc6c, 3), zoom: s.add(0xc88).readFloat(), yaw: s.add(0xc80).readFloat(),
            anim: s.add(0x1d3e).readU8(), cc: [va(0x90b520).readU8(), va(0x90b521).readU8()], move: [va(0x91d6f8).readFloat(), va(0x91d724).readFloat()] });
        }
        // --globals: the listed exe globals (and `slot+OFF` bytes of the battle) each frame, on change.
        if (cfg.globals) for (const g of cfg.globals) {
          const v = g.slot ? scriptSelf.add(g.addr).readU8() : g.size === 1 ? va(g.addr).readU8() : va(g.addr).readS32();
          if (v !== g.last) { emit({ k: 'glob', t: scriptT, frame: frames, a: g.name, v }); g.last = v; }
        }
        [[0x916008, 0x91600c], [0xdb4654, 0xdb4658]].forEach(([ax, ay]) => { va(ax).writeS32(scriptMouse[0]); va(ay).writeS32(scriptMouse[1]); });
        // The physical pointer (window pixels, 0xDB9408/0C) too: `SpaceWar_Autoscroll` (exe 0x5624c0) pans while it
        // is within 8 pixels of the window's edge, and the real cursor is anywhere.
        const [wx, wy] = toWindow(scriptMouse);
        va(0xdb9408).writeS32(wx); va(0xdb940c).writeS32(wy);
      }
      if (populated && !scriptFinished) { try { scriptFrame(); } catch (e) { emit({ k: 'error', msg: e.toString(), stack: e.stack }); } }
    }
    if (!started && frames === cfg.startAfterFrames) {
      started = true; fz = cfg.seed;
      if (cfg.fuzz) va(0xe72dd2).writeU8(1);   // `asteroidak`: battles get scenery
      // --watch-addr VA[:N]: the writers of an exe global (a string) across the scenario's load and after.
      if (cfg.watchAddr) watchWrites(va(cfg.watchAddr.addr), cfg.watchAddr.n || 40, true);
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
        // --script: the exe's own pause and speed (what the screen does with them is under test).
        if (cfg.script) return;
        if (A.IdoPause.readU8()) setPause(0, 0);
        A.BattleSpeed.writeS32(grabPending ? 0 : cfg.speed);
        maybePick();
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

// ---------------------------------------------------------------- screen grabs (--grab-ticks)
// --grab-ticks T,..: when the battle has run T ticks it stops (`IdoSpeedSzorzoBattle` 0), the screen settles for --grab-settle
// frames (the camera's easing), then the back buffer is read at SwapBuffers (glReadPixels, as screen.js) and the
// battle's full regions are dumped at that moment ('dump', the same shape as 'init'): ig2-client draws the same
// battle from it (re/screens/spacewar.md). The first grab also writes the exe's own save (--state-slot) for the
// top bar. Then the clock runs on to the next grab.
let grabPending = null, grabsDone = 0, swapHooked = false;
const loadSave = new NativeFunction(va(0x543c80), 'void', ['int', 'pointer', 'int'], { abi: 'mscdecl', traps: 'all' });
function hookSwap() {
  const gl = Process.getModuleByName('opengl32.dll'), gdi = Process.getModuleByName('gdi32.dll'), user = Process.getModuleByName('user32.dll');
  const f = (m, n, r, a) => new NativeFunction(m.getExportByName(n), r, a, { abi: 'stdcall' });
  const readPixels = f(gl, 'glReadPixels', 'void', ['int', 'int', 'int', 'int', 'uint', 'uint', 'pointer']);
  const getInt = f(gl, 'glGetIntegerv', 'void', ['uint', 'pointer']);
  const readBuffer = f(gl, 'glReadBuffer', 'void', ['uint']);
  const pixelStore = f(gl, 'glPixelStorei', 'void', ['uint', 'int']);
  const getProc = f(gl, 'wglGetProcAddress', 'pointer', ['pointer']);
  const winOfDc = f(user, 'WindowFromDC', 'pointer', ['pointer']);
  const clientRect = f(user, 'GetClientRect', 'int', ['pointer', 'pointer']);
  let bindFb = null, bindBuf = null;
  const tmp = Memory.alloc(16), rect = Memory.alloc(16);
  const get = (e) => { getInt(e, tmp); return tmp.readS32(); };
  Interceptor.attach(gdi.getExportByName('SwapBuffers'), {
    onEnter(args) {
      if (!grabPending || A.AktKep.readS32() !== 0xb) return;
      if (++grabPending.swaps < cfg.grabSettle) return;
      try {
        if (!bindFb) {
          const p = getProc(Memory.allocAnsiString('glBindFramebuffer')), q = getProc(Memory.allocAnsiString('glBindBuffer'));
          bindFb = p.isNull() ? () => {} : new NativeFunction(p, 'void', ['uint', 'uint'], { abi: 'stdcall' });
          bindBuf = q.isNull() ? () => {} : new NativeFunction(q, 'void', ['uint', 'uint'], { abi: 'stdcall' });
        }
        clientRect(winOfDc(args[0]), rect);
        const w = rect.add(8).readS32(), h = rect.add(12).readS32();
        const fb = get(0x8caa), pack = get(0x88ed), rb = get(0x0c02), align = get(0x0d05);
        bindFb(0x8ca8, 0); bindBuf(0x88eb, 0); readBuffer(0x0405); pixelStore(0x0d05, 1);
        const buf = Memory.alloc(w * h * 4);
        readPixels(0, 0, w, h, 0x1908, 0x1401, buf);
        bindFb(0x8ca8, fb); bindBuf(0x88eb, pack); readBuffer(rb); pixelStore(0x0d05, align);
        const self = grabPending.self, now = grab(regions(self)), full = {};
        for (const k in now) full[k] = { p: now[k].p, full: hexOf(now[k].b) };
        flush();
        send({ type: 'shot', tick: grabPending.tick, n: grabsDone, w, h, battle: battleSerial, self: self.toString(),
          acttime: va(0xfd1520).readU32(), futofeny: va(0xf1736c).readFloat(), kering: va(0xf176dc).readFloat(), clouds: va(0xe75918).readS32(),
          // The smoothed camera `spwkamerapont`/`spwforgaspont` (eye, target: h, x, y, w), `spacewarelfade`, `zbm`.
          cam: hexOf(new Uint8Array(va(0xe72dd4).readByteArray(0x20))), fade: va(0xf16eb4).readFloat(), zbm: va(0x91965c).readFloat(), erosseg: va(0x927994).readFloat(),
          statics: statics(self), regions: full }, buf.readByteArray(w * h * 4));
        if (grabsDone === 0 && cfg.stateSlot) loadSave(cfg.stateSlot, Memory.allocAnsiString('spacewar'), 2);
      } catch (e) { emit({ k: 'error', msg: e.toString(), stack: e.stack }); }
      grabPending = null;
      grabsDone++;
      A.BattleSpeed.writeS32(cfg.speed);
      if (grabsDone >= cfg.grabTicks.length) { emit({ k: 'done' }); steps = cfg.steps; }
    },
  });
}
// opengl32.dll is loaded late: the SwapBuffers hook goes in at the first grab.

// ---------------------------------------------------------------- input scripts (--script; re/screens/spacewar.md §9)
// screen.js's script frames on the battle screen: from the first frame `AktKep` is 0xB, `SDL_GetTicks` (exe
// 0x689e80) returns a clock that moves 10 ms a drawn frame (one UI tick a frame, `Fake_FRISSITES`' 100 Hz loop),
// `scriptT` counts the UI ticks (0xB772A4) from there, and the script's events go in through the game's own SDL2
// event filter `MainCallBackEventHandler` (0x4e2db0) at their tick. At that first frame the battle slot is dumped
// in the grab shape ({type: 'entry'}: regions, statics, the camera globals) and the exe's own save goes to
// --state-out, so `ig2-client --screen spacewar --battle <entry>.json --script S` starts from the same battle.
// Grabs: the frame whose swap comes after exactly t UI ticks → {type: 'frame', t, ...state} + RGBA bottom-up.
// The pause and the speeds are the exe's own throughout (the driver leaves them alone).
let scriptT = null, scriptUi0 = 0, scriptMouse = [320, 240], scriptEvents = [], scriptGrabs = [], scriptEnd = 0, scriptFinished = false;
let vt = 0, vtOn = false, realTicks = null, scriptSelf = null, lastIkon = null;
// SDL2 scancode, keycode.
const SDL_KEYS = { down: [81, 0x40000051], up: [82, 0x40000052], right: [79, 0x4000004f], left: [80, 0x40000050],
  add: [87, 0x40000057], subtract: [86, 0x40000056], escape: [41, 27], backspace: [42, 8], space: [44, 32], enter: [40, 13],
  shift: [225, 0x400000e1], ctrl: [224, 0x400000e0], a: [4, 97], c: [6, 99], r: [21, 114], i: [12, 105], o: [18, 111], p: [19, 112], v: [25, 118] };
for (let d = 1; d <= 9; d++) SDL_KEYS[String(d)] = [29 + d, 48 + d];

function hookScript() {
  const s = cfg.script;
  scriptGrabs = (s.grabs || []).slice().sort((x, y) => x - y);
  for (const e of s.events || []) {
    if (e.press) {
      scriptEvents.push({ t: e.t, mouse: e.press, quiet: true }, { t: e.t, down: e.button || 1, at: e.press });
      scriptEvents.push({ t: e.t + (e.hold || 3), up: e.button || 1, at: e.press });
    } else if (e.drag) {
      // Down at the first point, a step a tick on the line, up at the second (ig2-client's script.rs the same).
      const [[x0, y0], [x1, y1]] = e.drag, b = e.button || 1, hold = Math.max(1, e.hold || 3);
      scriptEvents.push({ t: e.t, mouse: [x0, y0], quiet: true }, { t: e.t, down: b, at: [x0, y0] });
      for (let k = 1; k < hold; k++) scriptEvents.push({ t: e.t + k, mouse: [Math.round(x0 + ((x1 - x0) * k) / hold), Math.round(y0 + ((y1 - y0) * k) / hold)], quiet: true });
      scriptEvents.push({ t: e.t + hold, mouse: [x1, y1], quiet: true }, { t: e.t + hold, up: b, at: [x1, y1] });
    } else if (e.key) {
      scriptEvents.push({ t: e.t, keydown: e.key }, { t: e.t + (e.hold || 3), keyup: e.key });
    } else {
      scriptEvents.push(e);
    }
  }
  // Stable: same-tick events keep their order.
  scriptEvents.sort((x, y) => x.t - y.t);
  scriptEnd = Math.max(0, ...scriptGrabs, ...scriptEvents.map((e) => e.t)) + (s.tail || 30);
  const ticks = va(0x689e80);
  const real = new NativeFunction(ticks, 'uint32', [], { abi: 'mscdecl' });
  realTicks = () => real();
  Interceptor.replace(ticks, new NativeCallback(() => (vtOn ? vt >>> 0 : real()), 'uint32', [], 'mscdecl'));
}

const sdlHandler = new NativeFunction(va(0x4e2db0), 'uint32', ['pointer'], { abi: 'mscdecl', traps: 'all' });
const sdlEvent = Memory.alloc(64);
function sdlSend(type, fill) {
  for (let i = 0; i < 64; i += 4) sdlEvent.add(i).writeU32(0);
  sdlEvent.writeU32(type);
  sdlEvent.add(4).writeU32(vt >>> 0);
  fill(sdlEvent);
  sdlHandler(sdlEvent);
}
function toWindow([x, y]) {
  const w = va(0xb770d0).readS32(), h = va(0xb770d4).readS32(), x0 = va(0xb770d8).readS32(), y0 = va(0xb770dc).readS32();
  return [x0 + Math.ceil((x * w) / 640), y0 + Math.ceil((y * h) / 480)];
}
const sndIn = (ev) => { if (typeof sndInput === 'function') sndInput(ev); };
function applyEvent(e) {
  if (e.mouse) {
    scriptMouse = e.mouse.slice();
    if (!e.quiet) sndIn({ src: 'script', ev: 'move', at: scriptMouse });
  } else if (e.down || e.up) {
    const [wx, wy] = toWindow(e.at);
    sdlSend(e.down ? 0x401 : 0x402, (p) => { p.add(0xc).writeU8(e.down || e.up); p.add(0xd).writeU8(e.down ? 1 : 0); p.add(0xe).writeU8(1); p.add(0x10).writeS32(wx); p.add(0x14).writeS32(wy); });
    sndIn({ src: 'script', ev: e.down ? 'down' : 'up', button: e.down || e.up, at: e.at, win: [wx, wy] });
  } else if (e.keydown || e.keyup) {
    const name = e.keydown || e.keyup, [sc, sym] = SDL_KEYS[name] || [0, 0];
    if (!sc) emit({ k: 'error', msg: 'unknown script key ' + name });
    sdlSend(e.keydown ? 0x300 : 0x301, (p) => { p.add(0xc).writeU8(e.keydown ? 1 : 0); p.add(0x10).writeS32(sc); p.add(0x14).writeS32(sym); });
    sndIn({ src: 'script', ev: e.keydown ? 'keydown' : 'keyup', key: name });
  }
}

// The battle screen's state beside each grab: the battle's clocks, the exe's pause and speeds, the smoothed camera,
// the fade, and the slot itself (the camera fields, the selection, the view animation are offline reads of `obj`).
function screenState(self) {
  return { kep: A.AktKep.readS32(), paused: A.IdoPause.readU8(), ikon: va(0xbae23c).readS32(), seen: [va(0xdb4654).readS32(), va(0xdb4658).readS32()], battleSpeed: A.BattleSpeed.readS32(), speed: va(0xbae25c).readS32(),
    battleSteps, cam: hexOf(new Uint8Array(va(0xe72dd4).readByteArray(0x20))), fade: va(0xf16eb4).readFloat(), zbm: va(0x91965c).readFloat(),
    erosseg: va(0x927994).readFloat(), acttime: va(0xfd1520).readU32(), futofeny: va(0xf1736c).readFloat(), kering: va(0xf176dc).readFloat(), clouds: va(0xe75918).readS32(),
    obj: self ? hexOf(new Uint8Array(self.readByteArray(OBJ))) : null };
}

function activeSelf() {
  for (let b = 0; b < 10; b++) { const p = SPW0.add(SPW_STRIDE * b); if (p.add(0x1f2e).readU8()) return p; }
  return null;
}

// --records: at each script grab the scene records (exe record stride 0x140: +0 translation, +0x10 scale matrix,
// +0x5c rotation, +0x80 their product) of the live debris nodes (+0x2008 + 0x1c*i, counter +0x18), the ships root
// (+0x1c18) and every unit's model (unit +0x34; units at *(+0xc20), stride 600, count +0xc28): what `ScaleObja` and
// `AddToScene` left. A meshid is (key +0, record index +4); the key's XVB is found in the std::map at *0x93DA34, as
// exe 0x451210 does.
function meshRecord(id) {
  const m = va(0x93da34).readPointer();
  if (m.isNull()) return null;
  const head = m.readPointer(), key = id.readU32();
  let n = head.add(4).readPointer(), best = head;
  while (n.add(0xd).readU8() === 0) {
    if (n.add(0x10).readU32() < key) n = n.add(8).readPointer();
    else { best = n; n = n.readPointer(); }
  }
  if (best.equals(head) || key < best.add(0x10).readU32()) return null;
  const xvb = best.add(0x14).readPointer();
  return xvb.add(0x20).readPointer().add(id.add(4).readS32() * 0x140);
}
function sceneRecords(self, t) {
  if (!self) return;
  const fl = (p, o, n) => Array.from({ length: n }, (_, k) => p.add(o + 4 * k).readFloat());
  const rec = (id) => { try { const r = meshRecord(id); return r && { at: fl(r, 0, 3), s: fl(r, 0x10, 9), m: fl(r, 0x80, 9) }; } catch (e) { return { err: e.toString() }; } };
  const debris = [];
  for (let i = 0; i < 40; i++) {
    const d = self.add(0x2008 + 0x1c * i), c = d.add(0x18).readS32();
    if (c === 0) continue;
    // The model's own XVB (record +0x44): its header (+4..+0xc the box half-extents `AddToScene` reads, +0x38 the
    // root scale) and its first two child records.
    let model = null;
    try {
      const r = meshRecord(d), x = r.add(0x44).readPointer(), kids = x.add(0x20).readPointer();
      model = { xvb: x.toString(), box: fl(x, 4, 3), k: x.add(0x38).readFloat(), kids: [0, 1].map((j) => ({ at: fl(kids.add(0x140 * j), 0, 3), s: fl(kids.add(0x140 * j), 0x10, 9), m: fl(kids.add(0x140 * j), 0x80, 9) })) };
    } catch (e) { model = { err: e.toString() }; }
    debris.push({ i, c, spin: fl(d, 8, 3), rec: rec(d), model });
  }
  const units = [], base = self.add(0xc20).readPointer(), nu = self.add(0xc28).readS32();
  for (let i = 0; i < nu && i < 64; i++) units.push({ i, rec: rec(base.add(600 * i + 0x34)) });
  // The shield rings (`karikaload` exe 0x576e40: 20 nodes at +0x1a88 + 0xc*i, +8 the start tick, 0 = free).
  const rings = [];
  for (let i = 0; i < 20; i++) {
    const g = self.add(0x1a88 + 0xc * i), st = g.add(8).readS32();
    if (st !== 0) {
      // The whole record: transparency +0x58 (`SetObjTransparency` exe 0x4533a0), the 8.8 frame +0xdc (`SetFrame`
      // 0x452f30), the property words +0x4c/+0x50.
      let raw = null;
      try { const r = meshRecord(g); raw = r && hexOf(new Uint8Array(r.readByteArray(0x140))); } catch (e) {}
      rings.push({ i, start: st, rec: rec(g), raw });
    }
  }
  emit({ k: 'records', t, root: rec(self.add(0x1c18)), debris, units, rings });
}

let scriptSwapHooked = false;
function hookScriptSwap() {
  const gl = Process.getModuleByName('opengl32.dll'), gdi = Process.getModuleByName('gdi32.dll'), user = Process.getModuleByName('user32.dll');
  const f = (m, n, r, a) => new NativeFunction(m.getExportByName(n), r, a, { abi: 'stdcall' });
  const readPixels = f(gl, 'glReadPixels', 'void', ['int', 'int', 'int', 'int', 'uint', 'uint', 'pointer']);
  const getInt = f(gl, 'glGetIntegerv', 'void', ['uint', 'pointer']);
  const readBuffer = f(gl, 'glReadBuffer', 'void', ['uint']);
  const pixelStore = f(gl, 'glPixelStorei', 'void', ['uint', 'int']);
  const getProc = f(gl, 'wglGetProcAddress', 'pointer', ['pointer']);
  const winOfDc = f(user, 'WindowFromDC', 'pointer', ['pointer']);
  const clientRect = f(user, 'GetClientRect', 'int', ['pointer', 'pointer']);
  let bindFb = null, bindBuf = null;
  const tmp = Memory.alloc(16), rect = Memory.alloc(16);
  const get = (e) => { getInt(e, tmp); return tmp.readS32(); };
  // --records: every `fenyekszamolasa` (exe 0x52f190) call of the grabbed frame: the node (arg 4), its mesh (arg 5),
  // the opacity byte `Kocka` hands it (arg 6) and the node's +0x60 (`Kocka`'s `t`): the shield rings' brightness.
  let lightCalls = [], planetLights = [];
  // ... and every `kirak` (exe 0x531460, cdecl, arg 0 the node) draw of the frame: the node, its 8.8 frame (+0x64),
  // flags (+0x68), `t` (+0x60), the mesh (arg 7), the triangles, and what it hands the rasteriser (`0x4ac580` from
  // inside it: arg 1 the projected vertices, stride 0x28: sx, sy, depth; arg 2 the count): count, screen box, depth.
  let kirakCalls = [], inKirak = null;
  if (cfg.records) {
    Interceptor.attach(va(0x531460), {
      onEnter(a) {
        const n = a[0];
        inKirak = { node: n.toString(), mesh: a[7].toString(), frame: n.add(0x64).readS32(), flags: n.add(0x68).readU32(), t: n.add(0x60).readFloat(), tris: a[1].toInt32(), v: 0, box: [1e9, 1e9, -1e9, -1e9], z: [1e9, -1e9], m: Array.from({ length: 21 }, (_, i) => n.add(4 * i).readFloat()), g: [0xdff8e8, 0xdff8ec, 0xdff910, 0xdff918, 0xe033e4, 0x91965c, 0x919650, 0x919654].map((x) => va(x).readFloat()) };
      },
      onLeave() { if (inKirak && kirakCalls.length < 2000) kirakCalls.push(inKirak); inKirak = null; },
    });
    // The static draw (`0x530b20`, same arguments) of every node: kept as `kirak` records with `static: true`.
    Interceptor.attach(va(0x530b20), {
      onEnter(a) {
        const n = a[0];
        inKirak = { static: true, node: n.toString(), mesh: a[7].toString(), frame: n.add(0x64).readS32(), flags: n.add(0x68).readU32(), t: n.add(0x60).readFloat(), tris: a[1].toInt32(), v: 0, box: [1e9, 1e9, -1e9, -1e9], z: [1e9, -1e9], near: [], m: Array.from({ length: 21 }, (_, i) => n.add(4 * i).readFloat()) };
      },
      onLeave() { if (inKirak && kirakCalls.length < 2000) kirakCalls.push(inKirak); inKirak = null; },
    });
    Interceptor.attach(va(0x4ac580), {
      onEnter(a) {
        if (!inKirak) return;
        const p = a[1], n = a[2].toInt32(), k = inKirak;
        for (let i = 0; i < n && i < 4000; i++) {
          const q = p.add(0x28 * i), x = q.readFloat(), y = q.add(4).readFloat(), z = q.add(8).readFloat();
          k.box = [Math.min(k.box[0], x), Math.min(k.box[1], y), Math.max(k.box[2], x), Math.max(k.box[3], y)];
          k.z = [Math.min(k.z[0], z), Math.max(k.z[1], z)];
          // The fog byte (+0x17, the remaster's `var_specular.w`) and the vertex colour (+0x10) handed on.
          const fog = q.add(0x17).readU8();
          k.fog = [Math.min(k.fog ? k.fog[0] : 255, fog), Math.max(k.fog ? k.fog[1] : 0, fog)];
          if (!k.col) k.col = (q.add(0x10).readU32() >>> 0).toString(16);
          // A static mesh's vertices around the shield capture's rings (virtual 640x480).
          if (k.near && x > 380 && x < 450 && y > 110 && y < 185 && k.near.length < 200) k.near.push([+x.toFixed(1), +y.toFixed(1), +z.toFixed(5)]);
        }
        k.v += n;
      },
    });
    Interceptor.attach(va(0x52f190), {
      onEnter(a) {
        if (lightCalls.length < 4000) lightCalls.push([a[3].toString(), a[4].toString(), a[5].toInt32() & 0xff, a[3].add(0x60).readFloat(), a[3].add(0x68).readU32()]);
        // Objects of 100 vertices or more (the planet, its shells): the lights the node sees (+0x6c/+0x70: type, colour,
        // vector +0x28), the relight flag (0xDFF940) and, after the call, a sample of the colours it left (+0xc).
        const n = a[2].toInt32();
        if (n >= 100 && planetLights.length < 64) {
          const sp = a[3], nl = sp.add(0x70).readS32(), lights = [];
          for (let i = 0; i < nl && i < 16; i++) {
            const l = sp.add(0x6c).readPointer().add(4 * i).readPointer();
            lights.push({ type: l.readS32(), color: l.add(4).readU32() >>> 0, vec: [0x28, 0x2c, 0x30].map((o) => +l.add(o).readFloat().toFixed(4)) });
          }
          this.pl = { n, node: sp.toString(), dirty: va(0xdff940).readU32(), opacity: a[5].toInt32() & 0xff, lights, vn: a[1] };
        }
      },
      onLeave() {
        const k = this.pl;
        if (!k) return;
        k.colors = [];
        for (let i = 0; i < k.n; i += Math.max(1, (k.n / 12) | 0)) k.colors.push((k.vn.add(0x14 * i + 0xc).readU32() >>> 0).toString(16));
        delete k.vn;
        planetLights.push(k);
      },
    });
  }
  Interceptor.attach(gdi.getExportByName('SwapBuffers'), {
    onEnter(args) {
      const lights = lightCalls, kiraks = kirakCalls, plights = planetLights;
      lightCalls = [];
      planetLights = [];
      kirakCalls = [];
      if (scriptT === null || scriptGrabs.length === 0) return;
      const ticks = va(0xb772a4).readS32() - scriptUi0;
      if (ticks < scriptGrabs[0]) return;
      const want = scriptGrabs.shift();
      try {
        if (!bindFb) {
          const p = getProc(Memory.allocAnsiString('glBindFramebuffer')), q = getProc(Memory.allocAnsiString('glBindBuffer'));
          bindFb = p.isNull() ? () => {} : new NativeFunction(p, 'void', ['uint', 'uint'], { abi: 'stdcall' });
          bindBuf = q.isNull() ? () => {} : new NativeFunction(q, 'void', ['uint', 'uint'], { abi: 'stdcall' });
        }
        clientRect(winOfDc(args[0]), rect);
        const w = rect.add(8).readS32(), h = rect.add(12).readS32();
        const fb = get(0x8caa), pack = get(0x88ed), rb = get(0x0c02), align = get(0x0d05);
        bindFb(0x8ca8, 0); bindBuf(0x88eb, 0); readBuffer(0x0405); pixelStore(0x0d05, 1);
        const buf = Memory.alloc(w * h * 4);
        readPixels(0, 0, w, h, 0x1908, 0x1401, buf);
        bindFb(0x8ca8, fb); bindBuf(0x88eb, pack); readBuffer(rb); pixelStore(0x0d05, align);
        if (cfg.records) {
          sceneRecords(activeSelf() || scriptSelf, want);
          emit({ k: 'lights', t: want, calls: lights });
          emit({ k: 'kirak', t: want, calls: kiraks });
          emit({ k: 'planetlights', t: want, calls: plights });
        }
        flush();
        send({ type: 'frame', t: want, ticks, w, h, frames, mouse: scriptMouse, ...screenState(activeSelf() || scriptSelf) }, buf.readByteArray(w * h * 4));
      } catch (e) { emit({ k: 'error', msg: e.toString(), stack: e.stack }); }
    },
  });
}

// --watch T:OFF[:N]: from script tick T a hardware write watchpoint on the battle slot's byte OFF (the main
// thread's), the first N writers as {k: 'write', pc, ret, t, value} (the caller from the stack's top words).
let watchOn = false;
function watchWrites(addr, n, str) {
  watchOn = true;
  const th = Process.enumerateThreads().find((x) => x.id === Process.getCurrentThreadId());
  let left = n;
  Process.setExceptionHandler((d) => {
    if (d.type !== 'single-step' && d.type !== 'breakpoint') return false;
    if (left-- > 0) {
      const sp = d.context.sp, words = [];
      for (let i = 0; i < 24; i++) { const w = sp.add(4 * i).readPointer(); const m = Process.findModuleByAddress(w); if (m && m.name === 'ig2.exe') words.push('0x' + (w.sub(mod.base).toInt32() + IMAGE_BASE).toString(16)); }
      emit({ k: 'write', pc: '0x' + (d.context.pc.sub(mod.base).toInt32() + IMAGE_BASE).toString(16), stack: words.slice(0, 6), t: scriptT, frame: frames, value: str ? addr.readCString() : addr.readFloat() });
    }
    if (left <= 0) th.unsetHardwareWatchpoint(0);
    return true;
  });
  th.setHardwareWatchpoint(0, addr, 4, 'w');
  emit({ k: 'watch', addr: addr.toString(), thread: th.id });
}

function scriptFrame() {
  if (scriptT === null) {
    if (A.AktKep.readS32() !== 0xb) return;
    scriptSelf = activeSelf();
    if (!scriptSelf) return;
    scriptT = 0;
    scriptUi0 = va(0xb772a4).readS32();
    vt = realTicks();
    va(0xbae278).writeU32((vt - 1) >>> 0);
    vtOn = true;
    if (!scriptSwapHooked) { hookScriptSwap(); scriptSwapHooked = true; }
    const now = grab(regions(scriptSelf)), full = {};
    for (const k in now) full[k] = { p: now[k].p, full: hexOf(now[k].b) };
    flush();
    send({ type: 'entry', tick: battleSteps, n: 0, battle: battleSerial, self: scriptSelf.toString(), frames, ui: scriptUi0,
      ...screenState(scriptSelf), statics: statics(scriptSelf), regions: full });
    if (cfg.stateSlot) loadSave(cfg.stateSlot, Memory.allocAnsiString('spacewar'), 2);
  } else {
    scriptT = va(0xb772a4).readS32() - scriptUi0;
  }
  vt += 10;
  if (cfg.watch && !watchOn && scriptT >= cfg.watch.t) watchWrites(scriptSelf.add(cfg.watch.off), cfg.watch.n || 40);
  for (const e of scriptEvents) if (!e.done && e.t <= scriptT) { e.done = true; applyEvent(e); }
  if (scriptT >= scriptEnd && scriptGrabs.length === 0) {
    scriptFinished = true;
    flush();
    send({ type: 'scriptDone', scriptT, vt, ...screenState(null) });
    emit({ k: 'done' });
    steps = cfg.steps;
  }
}

send({ type: 'ready', base: mod.base.toString() });
