// Frida agent: the planet passability maps `PlanetAtjarasMap::GenerateMap` (exe 0x479300) on the GOG ig2.exe
// (spec re/rules/ground-combat.md §11.13).
// From the frame hook, after quick.scn: per probe a planet p and a colony map m (planet +0x23 := m for the
// call, restored after), then GenerateMap(p - 1, 0) as thiscall on a scratch 0x24-byte object (so the
// planet's own maps stay untouched). Recorded: the live colony-map record m (0x2714 bytes: masks, heights
// +0x138c, base +0x84), the 102 bytes after its height grid (the head of record m + 1, which the height
// sampler reads past the grid), the flood-fill start globals 0x99644C/0x996450 before and after, the
// object's pointers and the four maps (run-length coded u16 [value, count] pairs).
// --fuzz perturbs the record (heights: noise, cliffs, plateaus, random fields; obstacle mask B; the tail)
// for the call and restores it after.
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330),
  GenerateMap: va(0x479300), Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8), Terepek: va(0x99825c),
  StartX: va(0x99644c), StartY: va(0x996450),
};
const TEREP = 0x2714, HEIGHTS = 0x138c, MASK_B = 0x330, TAIL = 102;

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, count: 150, fuzz: false, every: 2,
  seed: 4242, maps: 75 };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 2) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

function hex(p, n) {
  const b = new Uint8Array(p.readByteArray(n));
  let s = '';
  for (let i = 0; i < b.length; i++) s += (b[i] < 16 ? '0' : '') + b[i].toString(16);
  return s;
}
function rle(p) {
  const v = new Uint16Array(p.readByteArray(80000));
  const out = [];
  for (let i = 0; i < v.length;) {
    let j = i;
    while (j < v.length && v[j] === v[i]) j++;
    out.push(v[i], j - i);
    i = j;
  }
  return out;
}
let fz = 1;
const rnd = (n) => { fz ^= fz << 13; fz >>>= 0; fz ^= fz >>> 17; fz ^= fz << 5; fz >>>= 0; return fz % n; };

const nf = (a, r, args, abi) => new NativeFunction(a, r, args, { abi: abi || 'mscdecl', traps: 'all' });
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const generate = nf(A.GenerateMap, 'void', ['pointer', 'int', 'int'], 'thiscall');
const crt = Process.getModuleByName('msvcr120.dll');
const cfree = nf(crt.getExportByName('free'), 'void', ['pointer']);

// Temporary record changes (restored after the call), newest first on undo.
function fuzzRecord(rec, m, restore) {
  const save = (p, n) => restore.push([p, p.readByteArray(n)]);
  const h = rec.add(HEIGHTS);
  save(h, 5000);
  save(rec.add(MASK_B), 650);
  const kind = rnd(6);
  const get = (x, y) => h.add(2 * (y * 50 + x)).readS16();
  const set = (x, y, v) => h.add(2 * (y * 50 + x)).writeS16(Math.max(-32768, Math.min(32767, v)));
  if (kind === 0) {                                  // noise of a random amplitude
    const amp = [4, 16, 48, 120][rnd(4)];
    for (let y = 0; y < 50; y++) for (let x = 0; x < 50; x++) set(x, y, get(x, y) + rnd(2 * amp + 1) - amp);
  } else if (kind === 1) {                           // cliffs and pits
    for (let n = 1 + rnd(12); n > 0; n--) {
      const x0 = rnd(50), y0 = rnd(50), w = 1 + rnd(12), hh = 1 + rnd(12), d = rnd(3000) - 1500;
      for (let y = y0; y < Math.min(50, y0 + hh); y++) for (let x = x0; x < Math.min(50, x0 + w); x++) set(x, y, get(x, y) + d);
    }
  } else if (kind === 2) {                           // a rough field with smooth pockets (the 7000 retries)
    // One pocket is big enough (>= 22 x 22 height cells, 7744 grid cells): without a region of 7000 cells
    // the exe floods from (100, 100), which runs over the map buffer or never ends (spec 11.13).
    const base = get(25, 25);
    for (let y = 0; y < 50; y++) for (let x = 0; x < 50; x++) set(x, y, base + rnd(4001) - 2000);
    for (let n = 1 + rnd(6); n > 0; n--) {
      const big = n === 1, r = big ? 12 + rnd(6) : 2 + rnd(10);
      const x0 = big ? r + rnd(50 - 2 * r) : rnd(50), y0 = big ? r + rnd(50 - 2 * r) : rnd(50), v = base + rnd(200) - 100;
      for (let y = Math.max(0, y0 - r); y < Math.min(50, y0 + r); y++) for (let x = Math.max(0, x0 - r); x < Math.min(50, x0 + r); x++) set(x, y, v + rnd(5));
    }
  } else if (kind === 3) {                           // a flat plateau (a scattered one fragments the map)
    const v = get(rnd(50), rnd(50)), x0 = rnd(40), y0 = rnd(40), w = 5 + rnd(45), hh = 5 + rnd(45);
    for (let y = y0; y < Math.min(50, y0 + hh); y++) for (let x = x0; x < Math.min(50, x0 + w); x++) set(x, y, v);
  } else if (kind === 4) {                           // a slope
    const sx = rnd(81) - 40, sy = rnd(81) - 40;
    for (let y = 0; y < 50; y++) for (let x = 0; x < 50; x++) set(x, y, get(x, y) + sx * x + sy * y);
  }
  if (rnd(2)) {                                      // obstacle blobs in mask B (applied after the fill)
    const mb = rec.add(MASK_B);
    for (let n = rnd(40); n > 0; n--) { const o = rnd(650); mb.add(o).writeU8(mb.add(o).readU8() | (1 + rnd(255))); }
    if (rnd(8) === 0) for (let o = 0; o < 650; o++) mb.add(o).writeU8(0);
  }
  if (m + 1 < cfg.maps && rnd(3) === 0) {            // the head of the next record
    const t = rec.add(TEREP);
    save(t, TAIL);
    for (let o = 0; o < TAIL; o += 2) if (rnd(2)) t.add(o).writeU16(rnd(65536));
  }
  return kind;
}

function probe(k) {
  const np = A.HanyBolygo.readS32();
  const p = 1 + rnd(np);
  const m = k < cfg.maps ? k : rnd(cfg.maps);
  const planet = A.Bolygok.add(0x188 * p);
  const rec = A.Terepek.readPointer().add(TEREP * m);
  const restore = [[planet.add(0x23), planet.add(0x23).readByteArray(1)]];
  planet.add(0x23).writeU8(m);
  const fuzzed = cfg.fuzz && k >= cfg.maps;
  const kind = fuzzed ? fuzzRecord(rec, m, restore) : -1;
  const start0 = [A.StartX.readS32(), A.StartY.readS32()];
  // Some probes start from a random leftover start (only GenerateMap's own search sets it).
  if (rnd(3) === 0) { A.StartX.writeS32(rnd(200)); A.StartY.writeS32(rnd(200)); }
  const start = [A.StartX.readS32(), A.StartY.readS32()];
  const record = hex(rec, TEREP), tail = hex(rec.add(TEREP), TAIL);
  const obj = Memory.alloc(0x24);
  for (let i = 0; i < 9; i++) obj.add(4 * i).writeU32(0);
  emit({ k: 'start', p, m, kind }); flush();
  const t0 = Date.now();
  generate(obj, p - 1, 0);
  const ms = Date.now() - t0;
  const ptr = Array.from({ length: 9 }, (_, i) => obj.add(4 * i).readU32());
  const maps = [];
  for (let c = 1; c <= 4; c++) maps.push(rle(obj.add(4 * (c + 1)).readPointer()));
  const after = [A.StartX.readS32(), A.StartY.readS32()];
  emit({ k: 'gen', p, m, fuzzed, kind, start0, start, after, record, tail, ptr: ptr.map((x) => x.toString(16)),
    alias: ptr[6] === ptr[2], maps, ms });
  for (let c = 2; c <= 5; c++) cfree(obj.add(4 * c).readPointer());
  for (const [a, b] of restore.reverse()) a.writeByteArray(b);
  A.StartX.writeS32(start0[0]); A.StartY.writeS32(start0[1]);
}

let frames = 0, started = false, phase = 0, done = 0, loadedAt = 0;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (!started && frames === cfg.startAfterFrames) {
      started = true; fz = cfg.seed;
      betolt(Memory.allocAnsiString(cfg.scenario));
      setSpeed(3, 1);
      loadedAt = frames;
      emit({ k: 'loaded' });
      return;
    }
    if (!started || frames < loadedAt + 300 || phase > 0) return;
    if (frames % cfg.every !== 0) return;
    try {
      probe(done);
      done++;
      if (done >= cfg.count) { phase = 1; emit({ k: 'done', done }); }
    } catch (e) { emit({ k: 'error', msg: e.toString(), stack: e.stack }); emit({ k: 'done' }); phase = 1; }
  },
});

send({ type: 'ready', base: mod.base.toString() });
