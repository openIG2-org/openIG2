// Frida agent for reference screenshots of the original's screens (re/screens/*.md): starts scenario/quick.scn,
// loads save<slot> with the exe's `LoadSave(slot, name, 1)` (exe 0x543c80), pauses the clock and asks for the
// screen the config names (`AktKepKell` 0xBAE250 / `AktPlanetKell` 0xBAE254: 3 = a planet's colony screen), then
// tells the driver when to grab the window. The driver (run_screen.py) captures it. Output: game imagery, keep it
// under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  LoadSave: va(0x543c80), AktKep: va(0xbae0fc), AktKepKell: va(0xbae250), AktPlanetKell: va(0xbae254), AktPlanet: va(0xb9933c),
  Ido2: va(0xbae050),
};

const nf = (p, r, a) => new NativeFunction(p, r, a, { abi: 'mscdecl', traps: 'all' });
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const setPause = nf(A.SetIdoPause, 'void', ['int', 'int']);
const loadSave = nf(A.LoadSave, 'void', ['int', 'pointer', 'int']);

// `Ev`/`Honap`/`Nap` (0xBAE040/44/48).
const date = () => [0xbae040, 0xbae044, 0xbae048].map((x) => va(x).readS32());

// Skip the title video: `MainScr_Belep` (exe 0x4fe3c0) plays `title.bik` once (`VoltGTLogo` 0xBBB22F clear) and the
// player's click ends it; `PlayBink` runs the frames itself, so no screen request is taken until it ends.
va(0xbbb22f).writeU8(1);

let cfg = null;
recv('config', (m) => {
  cfg = m.payload;
  if (cfg.rop) hookRop();
  if (cfg.ropMenu || cfg.menu) hookMenus();
  if (cfg.spyFrame !== null) hookSpyFrame();
  if (cfg.resFrame !== null && cfg.resFrame !== undefined) hookResFrame();
  if (cfg.lightProbe) hookLightProbe();
  if (cfg.verdict) hookVerdict();
  if (cfg.hide) hideInfoPlanet(cfg.hide);
  infoLeft = (cfg.infoPress || []).slice();
  if (infoLeft.length || cfg.infoSelect) hookInfoMain();
});

// --info-press IDS: on the info screen (`AktKep` 9) press its hot-spots in turn through the game's own input
// (`AktIkonKell` 0xB77208 = hot-spot + 0x1e, as --verdict), 30 frames apart: e.g. 9 a row's detail mode, 31 the list
// down, 15 the star log, 37 back from the detail (re/screens/info.md §6.5–§6.8).
// `AktIkonKell` sets `AktIkon2` but not `Mouse_Lenyom` (0xDB465F, the press edge `Info_Main` exe 0x4e81c0 checks):
// set it at `Info_Main`'s entry in the frame the staged hot-spot arrives.
let infoLeft = [], infoWait = 0, infoPending = 0;
// --info-select K,X,Y,I: once the presses are done, hold the planet-settings menu of hot-spot K open at (X, Y) on
// item I (re/screens/info.md §7.1: `InfoSelectTax`/`AutoBuild`/`Priority` 0xB79F8A/0xB79F8B/0xB79FB0, `infoSelectX/Y`
// 0x90B6FC/0x90B700, `infosmSelected` 0xB84C14), written after every `Info_Main` (which leaves them alone while the
// pointer isn't on 26..28).
const INFO_SELECT = { 26: 0xb79f8a, 27: 0xb79f8b, 28: 0xb79fb0 };
function hookInfoMain() {
  Interceptor.attach(va(0x4e81c0), {
    onEnter() {
      if (infoPending && va(0xbae240).readS32() === infoPending) {
        va(0xdb465f).writeU8(1);
        send({ k: 'info-taken', id: infoPending, frames });
        infoPending = 0;
      }
    },
    onLeave() {
      const s = cfg.infoSelect;
      if (!s || infoLeft.length || infoPending) return;
      for (const k of [26, 27, 28]) va(INFO_SELECT[k]).writeU8(k === s[0] ? 1 : 0);
      va(0x90b6fc).writeS32(s[1]); va(0x90b700).writeS32(s[2]); va(0xb84c14).writeS32(s[3]);
    },
  });
}

// --hide clouds|planet: the long info's planet without its clouds (`HideClouds` exe 0x59c600 skips its unhide:
// `JZ` at 0x59c63e made a `JMP`) or the clouds alone (`HideObj` exe 0x4524c0 on `PlanetId` 0xF18BD0 + 8·(AktPlanet − 1)
// after it), to compare the two meshes one at a time (re/screens/starmap.md §3.3).
function hideInfoPlanet(what) {
  if (what === 'clouds') {
    Memory.patchCode(va(0x59c63e), 1, (c) => c.writeU8(0xeb));
  } else {
    const hideObj = nf(va(0x4524c0), 'void', ['pointer']);
    Interceptor.attach(va(0x59c600), { onLeave() { hideObj(va(0xf18bd0 + (A.AktPlanet.readS32() - 1) * 8)); } });
  }
}

let frames = 0, phase = 0, at = 0, shot = 0, ropDone = false, setupDone = false, staged = false, mainCalls = 0, gfx1Calls = 0, spyStage = null;

// --spies MODE: a roster for the human's spy screen (`Kemek` 0xBAFCC8 [9][13] × 0x4c, `OsszHanyKem` 0xBB1F84;
// re/rules/espionage.md §1.1): five spies of different races and states (at home with skill points, on a mission
// against the AI, travelling home, captured, at home), the human knowing the AI (`BirodalomIsmeri` 0xBAEBB8). MODE
// also queues one report (`KemBeszolasok` 0xBAF338, count 0xBAF334; info-messages.md §3.3) the screen shows first:
// roster none, report a finished empire report (kind 1), verdict an AI spy caught by the human (kind 3), applicant
// a new recruit (kind 2).
function stageSpies(mode) {
  const en = va(0x912a3c).readS32(), ai = cfg.aiEmpire;
  const kemek = va(0xbafcc8), ossz = va(0xbb1f84);
  const rec = (e, k) => kemek.add((e * 13 + k) * 0x4c);
  // [name, state, race, mission, target, p1, left, skills, points, xp, rank, applicant]
  const plan = [
    ['Agent Alpha', 1, 1, 0, 0, 0, 0, [40, 30, 20, 50], 2, 4000, 3, 0],
    ['Agent Bravo', 3, 4, 2, ai, 0, 400, [60, 10, 30, 40], 0, 9000, 5, 0],
    ['Agent Charlie', 2, 7, 0, 0, 0, 100, [20, 20, 20, 20], 0, 500, 1, 0],
    ['Agent Delta', 4, 13, 0, 0, 0, 0, [10, 50, 70, 5], 0, 1200, 2, 0],
    ['Agent Echo', 1, 14, 0, 0, 0, 0, [90, 90, 90, 90], 0, 50000, 9, 0],
  ];
  if (mode === 'applicant') plan.push(['Agent Foxtrot', 1, 6, 0, 0, 0, 0, [30, 30, 30, 30], 0, 3000, 2, 1]);
  plan.forEach(([name, st, race, m, t, p1, left, sk, pts, xp, rank, appl], i) => {
    const r = rec(en, i + 1);
    r.writeByteArray(new Array(0x4c).fill(0));
    r.writeUtf8String(name);
    r.add(0x10).writeS16(st); r.add(0x12).writeU8(race); r.add(0x16).writeS16(appl);
    r.add(0x1a).writeU16(m); r.add(0x1c).writeU8(t); r.add(0x1e).writeS16(p1);
    r.add(0x24).writeS32(left); r.add(0x28).writeS32(left); r.add(0x2c).writeS32(-1000);
    r.add(0x31).writeByteArray(sk); r.add(0x38).writeS32(pts); r.add(0x3c).writeS32(xp); r.add(0x40).writeU8(rank);
  });
  ossz.add(4 * en).writeS32(plan.length);
  // One AI spy at home (the verdict's prisoner).
  const n = ossz.add(4 * ai).readS32() + 1;
  const r = rec(ai, n);
  r.writeByteArray(new Array(0x4c).fill(0));
  r.writeUtf8String('Agent Spectre');
  r.add(0x10).writeS16(4); r.add(0x12).writeU8(3); r.add(0x31).writeByteArray([50, 50, 50, 30]); r.add(0x40).writeU8(4);
  ossz.add(4 * ai).writeS32(n);
  va(0xbaebb8).add(19 * en + ai).writeU8(1);
  const reports = va(0xbaf338), count = va(0xbaf334);
  const put = (kind, a, b, c, x) => {
    const q = reports.add(0x30);
    q.writeByteArray(new Array(0x30).fill(0));
    q.writeU8(kind); q.add(4).writeS32(a); q.add(8).writeU8(b); q.add(9).writeU8(c);
    x.forEach((v, i) => q.add(0xc + 4 * i).writeS32(v));
    count.writeS32(1);
  };
  count.writeS32(0);
  if (mode === 'report') put(1, 2, 4, 1, [2, ai, 0, 0, 7, 1234567, 23, 61, 0]);
  if (mode === 'verdict') put(3, 1, 0, 0, [0, ai, n, 0, 0, 0, 0, 0, 0]);
  if (mode === 'applicant') put(2, plan.length, 0, 0, [0, en, 0, 0, 0, 0, 0, 0, 0]);
  return { en, ai, spies: plan.length, prisoner: n };
}

// --spy-frame F [--spy-closing]: hold the spy screen's `spyframe` (f32 0xE75504) at F before every `Spying3DUpdate`
// (exe 0x590410), the opening's pose (with --spy-closing `spyBECSUKAS` 0xE75383 set: the closing's); re/screens/spy.md
// §1.1, §2.2. `spyKELLKINYITAS` 0xE75430 (= `KELLKINYITASDEFAULT` 0x912A39, the registry's) is reported.
// --res-frame F [--res-closing]: hold the research screen's `resframe` (f32 0xDC5368) at F before every
// `Research3DUpdate` (exe 0x51fd10); with --res-closing `resBECSUKAS` 0xDDF323 set and `resKINYITVA` 0xDDF322 cleared,
// as `Research_Kilep` leaves them (re/screens/research.md §5); `reskeringeto` 0, `resatlatszosag` 0.85.
function hookResFrame() {
  Interceptor.attach(va(0x51fd10), {
    onEnter() {
      if (A.AktKep.readS32() !== 4) return;
      va(0xdc5368).writeFloat(cfg.resFrame);
      // The side lights' clock `reskeringeto` (f32 0xDDF6BC, read by the inlined `resMozgatMainFenyek`) and the
      // glass's transparency `resatlatszosag` (f32 0xDC5370, from `res_xy`) pinned too, so the client can match them.
      va(0xddf6bc).writeFloat(cfg.resKeringeto || 0);
      va(0xdc5370).writeFloat(0.85);
      if (resFrames3d < 1000) resFrames3d++;
      if (cfg.resClosing) { va(0xddf323).writeU8(1); va(0xddf322).writeU8(0); }
    },
  });
}

// --light-probe (research screen): the engine's per-vertex lighting `fenyekszamolasa` (ELF 0x606530, exe 0x52f190,
// cdecl (pos, vertexnormal[], n, spsorttype, XVOStruct, opacity); re/screens/research.md §2.1) on the screen's first
// lit frames: per call the object's rotation chain (sp +0x24), flags (+0x68), its lights (+0x6c/+0x70: type, colour
// word, the light vector +0x28) and every vertex's normal and the colour it got (vertexnormal 0x14 B: xyz, colour
// +0xc), plus the shade table once. Records {k: 'light', ...} go to --jsonl.
let resFrames3d = 0, shadeSent = false;
function hookLightProbe() {
  Interceptor.attach(va(0x52f190), {
    onEnter(args) {
      this.on = A.AktKep.readS32() === 4 && resFrames3d >= 1 && resFrames3d <= 3 && va(0xdff940).readU32() !== 0;
      if (!this.on) return;
      this.vn = args[1]; this.n = args[2].toInt32(); this.sp = args[3]; this.xvo = args[4]; this.op = args[5].toInt32();
    },
    onLeave() {
      if (!this.on) return;
      const sp = this.sp, m = [];
      for (let i = 0; i < 9; i++) m.push(sp.add(0x24 + 4 * i).readFloat());
      const flags = sp.add(0x68).readU32(), nl = sp.add(0x70).readS32(), lights = [];
      for (let i = 0; i < nl && i < 16; i++) {
        const l = sp.add(0x6c).readPointer().add(4 * i).readPointer();
        lights.push({ type: l.readS32(), color: l.add(4).readU32(), dir: [0x18, 0x1c, 0x20].map((o) => l.add(o).readFloat()),
          vec: [0x28, 0x2c, 0x30].map((o) => l.add(o).readFloat()), spec: l.add(0x44).readU8() });
      }
      const verts = [];
      for (let i = 0; i < this.n && i < 3000; i++) {
        const v = this.vn.add(0x14 * i);
        verts.push([v.readFloat(), v.add(4).readFloat(), v.add(8).readFloat(), v.add(0xc).readU32()]);
      }
      const mat = this.xvo.add(0x34).readPointer().readPointer();
      const rec = { k: 'light', frame: resFrames3d, sp: sp.toString(), m, flags, opacity: this.op, lights, verts, keringeto: va(0xddf6bc).readFloat() };
      if (!shadeSent) {
        const t = mat.add(0x34).readPointer(), sp2 = mat.add(0x38).readPointer();
        rec.shade = Array.from(new Uint16Array(t.readByteArray(0x4000 + 0x4000)));
        rec.specular = sp2.isNull() ? null : sp2.toString();
        rec.shadeAt = t.toString();
        shadeSent = true;
      }
      send(rec);
    },
  });
}

function hookSpyFrame() {
  Interceptor.attach(va(0x590410), {
    onEnter() {
      if (A.AktKep.readS32() !== 7) return;
      va(0xe75504).writeFloat(cfg.spyFrame);
      if (cfg.spyClosing) va(0xe75383).writeU8(1);
    },
  });
}

// --verdict N (with --spies verdict): N presses of the verdict buttons 0x14..0x17 (re/screens/spy.md §6.2) through
// the game's own input (`AktIkonKell` 0xB77208 = hot-spot + 0x1e, as `IG2_gameUpdate` takes it), each on a fresh
// copy of the report (kind 3, `KemBeszolasok[1]` 0xBAF368, the dialog `SpyBeszolasMi` 0xE75384) with the prisoner's
// loyalty (`Kemek[x7][x8] + 0x34`), double-agent field (`+0x18`) and stream 0's seed (`RandSeed` 0xBAF2D0) fuzzed.
// `Spy_Main` (exe 0x58d410) is hooked: the seed before and after its press, the report's new kind; `Multi_Send`
// (exe 0x513840) gives the fate it sends (0x4a). Records {k: 'verdict', choice, owner, spy, loyalty, master,
// seed0, seed1, kind, dialog, fate}.
let verdictLeft = 0, verdictPending = null, verdictWait = 0, verdictRnd = 0;
function vrnd(n) {
  verdictRnd = (Math.imul(verdictRnd, 1103515245) + 12345) >>> 0;
  return (verdictRnd >>> 8) % n;
}
function hookVerdict() {
  verdictLeft = cfg.verdict;
  verdictRnd = cfg.verdictSeed >>> 0;
  Interceptor.attach(va(0x58d410), {
    onEnter() {
      const id = va(0xbae240).readS32();
      this.press = verdictPending && id >= 0x14 && id <= 0x17 ? verdictPending : null;
      if (!this.press) return;
      // The last press's fate (`MULTIGET_Spy` 0x4a, applied a frame or more later) may have rewritten the
      // prisoner since the staging: put its fields back here, where the roll reads them.
      const p = this.press, r = va(0xbafcc8).add((p.owner * 13 + p.spy) * 0x4c);
      r.add(0x34).writeU8(p.loyalty); r.add(0x18).writeU8(p.master);
      p.seed0 = va(0xbaf2d0).readU32();
    },
    onLeave() {
      const p = this.press;
      if (!p) return;
      p.seed1 = va(0xbaf2d0).readU32();
      p.kind = va(0xbaf368).readU8();
      p.dialog = va(0xe75384).readS32();
      send(p);
      verdictPending = null;
      verdictLeft--;
      verdictWait = 10;
    },
  });
  Interceptor.attach(va(0x513840), {
    onEnter() {
      if (!verdictPending) return;
      const s = this.context.esp;
      if ((s.add(4).readS32() & 0xffff) === 0x4a) verdictPending.fate = s.add(16).readPointer().readS32();
    },
  });
}
function stageVerdict(stage) {
  const owner = stage.ai, k = stage.prisoner;
  const r = va(0xbafcc8).add((owner * 13 + k) * 0x4c);
  const loyalty = vrnd(4) ? vrnd(12) : vrnd(256), master = vrnd(3) ? 0 : 1 + vrnd(8);
  r.add(0x34).writeU8(loyalty); r.add(0x18).writeU8(master);
  const q = va(0xbaf338).add(0x30);
  q.writeByteArray(new Array(0x30).fill(0));
  q.writeU8(3); q.add(4).writeS32(1); q.add(0x10).writeS32(owner); q.add(0x14).writeS32(k);
  va(0xbaf334).writeS32(1);
  va(0xe75384).writeS32(3);
  if (vrnd(2)) va(0xbaf2d0).writeU32((vrnd(0x10000) << 16 | vrnd(0x10000)) >>> 0);
  const choice = 1 + vrnd(4);
  verdictPending = { k: 'verdict', choice, owner, spy: k, loyalty, master, fate: null };
  va(0xb77208).writeS32(0x14 + choice - 1 + 0x1e);
}

// --fleets N: N fleets of the human (`EN` 0x912A3C) beside the shown planet, built like production does (`FlottaNew`
// 0x53d400, `HajoNew` 0x53e800, `TankNew` 0x541750): ships of the human's first two ship designs and, when it has
// one, a vehicle entry; quick-1.sav's human has no fleet of its own.
const flottaNew = nf(va(0x53d400), 'int', ['pointer', 'int', 'int', 'float', 'float', 'float']);
const hajoNew = nf(va(0x53e800), 'void', ['int', 'int']);
const tankNew = nf(va(0x541750), 'void', ['int', 'int', 'int']);
const fleets = [];
function populate(n) {
  const e = va(0x912a3c).readS32(), cnt = va(0xb9c028).add(4 * e).readS32(), designs = va(0xb9aa18).add(4 * e).readPointer();
  const ships = [], vehicles = [];
  for (let d = 1; d <= cnt; d++) {
    const p = designs.add(0x84 * d), cls = p.add(0x16).readS16();
    if (cls === 1 && p.add(0x36).readU8() === 0) ships.push(d); else if (cls === 2) vehicles.push(d);
  }
  const q = va(0xb85ea8).add(0x188 * cfg.planet);
  for (let k = 0; k < n && ships.length; k++) {
    const f = flottaNew(Memory.allocAnsiString('Fleet ' + (k + 1)), e, 0, q.add(0x18).readFloat() + 2 * k, q.add(0x1c).readFloat() + 1, 0);
    if (f < 1) continue;
    for (let j = 0; j < 3 + k; j++) hajoNew(f, ships[j % Math.min(2, ships.length)]);
    if (vehicles.length) tankNew(f, vehicles[0], 2 + k);
    fleets.push(f);
  }
  send({ k: 'fleets', e, ships, vehicles, fleets });
}

// --rop N: press the starmap's icon N once (`AktRopIkon` 0xE83654: 1 tanks/transfer, 2 production, 0xd split, 0xe
// transfer between two fleets), as a click would. `Starmap_Main` (exe 0x5b2b70) clears it on entry, so the value is
// put in right after that store.
function hookRop() {
  const target = va(0xe83654), b = [0xc7, 0x05];
  for (let i = 0; i < 4; i++) b.push(target.shr(8 * i).and(0xff).toInt32());
  const pattern = b.concat([0, 0, 0, 0]).map((x) => ('0' + x.toString(16)).slice(-2)).join(' ');
  const hit = Memory.scanSync(va(0x5b2b70), 0x200, pattern)[0];
  if (!hit) { send({ k: 'error', what: 'AktRopIkon store not found' }); return; }
  Interceptor.attach(hit.address.add(10), function () {
    if (cfg.ropNow && !ropDone) { target.writeS32(cfg.rop); ropDone = true; send({ k: 'rop', rop: cfg.rop }); }
  });
}
// --rop-menu X,Y: hold the radial menu open at the virtual point (`StarmapRopIkonMod` 0xF16F18, `RopIkonFazis`
// 0xEF1154, `RopIkonFrame` f32 0xE8634C = 50, `LastMouseX/Y` 0xE75A74/78 = the point − (82, 84)); `StarmapGfx2D_1`
// (exe 0x5a87d0) clears `StarmapVoltRopikonAlap` 0xE75A73 without a held button, so it is set again after it.
// --menu KIND@X,Y: a select menu held open (re/screens/starmap.md §16): its flag, `smSelectX/Y` 0x927078/0x92707C
// and the hot-spot as `AktIkon2` 0xBAE240, so `Starmap_Main` (exe 0x5b2b70) keeps it and picks the item under the
// pointer (--mouse).
const MENUS = { tax: [0xf1737f, 0x5a], autobuild: [0xf176e8, 0x5e], priority: [0xf176e9, 0x60], sale: [0xf176ea, 0x65], planets: [0xf176eb, 0x32] };
function stageMenus() {
  if (cfg.ropMenu) {
    va(0xf16f18).writeU8(1); va(0xef1154).writeS32(1); va(0xe8634c).writeFloat(50);
    va(0xe75a74).writeS32(cfg.ropMenu[0] - 82); va(0xe75a78).writeS32(cfg.ropMenu[1] - 84);
  }
  if (cfg.menu) {
    const [flag, spot] = MENUS[cfg.menu.kind];
    va(0x927078).writeS32(cfg.menu.x); va(0x92707c).writeS32(cfg.menu.y);
    va(flag).writeU8(1);
    va(0xbae240).writeS32(spot + (cfg.menu.kind === 'sale' ? cfg.menu.n - 1 : 0));
    // `smSelected` 0xE75A68 as `Starmap_Main` would pick it (it doesn't run while the clock is paused):
    // the row under --mouse (§16), −1 without one.
    let sel = -1;
    if (cfg.mouse && cfg.menu.kind !== 'planets') {
      const [mx, my] = cfg.mouse, { x, y } = cfg.menu;
      const k = Math.floor((my - y) / 13), w = cfg.menu.kind === 'sale' ? (cfg.menu.w || 200) : 120;
      const first = cfg.menu.kind === 'priority' || cfg.menu.kind === 'sale' ? 1 : 0;
      const last = { tax: 7, autobuild: 8, priority: 3, sale: 1 }[cfg.menu.kind];
      if (mx >= x && mx <= x + w && my >= y && k + first <= last) sel = k + first;
    }
    if (cfg.menu.kind === 'planets') {
      // `smSelectChr/Ma/Sz` 0xE75A5C/60/64 as `Starmap_Main` sets them (§17): the longest name a, the longest
      // production text b (`pct%-design`), ⌈n/32⌉ columns.
      const e = va(0x912a3c).readS32(), n = va(0x912ac8).readS32(), designs = va(0xb9aa18).add(4 * e).readPointer();
      let a = 0, b = 0, cnt = 0;
      for (let p = 1; p <= n; p++) {
        const q = va(0xb85ea8).add(0x188 * p);
        if (q.add(0x20).readU8() !== e) continue;
        cnt++;
        a = Math.max(a, q.readCString().length);
        const d = q.add(0x78).readS16();
        if (d > 0) {
          const rec = designs.add(0x84 * d), time = rec.add(0x20).readS32();
          const pct = Math.min(99, 100 - Math.trunc(q.add(0xa0).readS32() * 100 / time));
          b = Math.max(b, String(pct).padStart(2, ' ').length + 2 + rec.readCString().length);
        }
      }
      const sz = Math.max(1, Math.trunc((cnt + 31) / 32));
      va(0xe75a5c).writeS32(a + b); va(0xe75a64).writeS32(sz); va(0xe75a60).writeS32(Math.trunc((cnt + sz - 1) / sz));
    }
    va(0xe75a68).writeS32(sel);
  }
}
function hookMenus() {
  // `StarmapGfx2D_1` (exe 0x5a87d0) runs every drawn frame, `Starmap_Main` not while the clock is paused: stage the
  // state before it and set `StarmapVoltRopikonAlap` (which wants a held button) after it.
  Interceptor.attach(va(0x5a87d0), {
    onEnter() { if (staged) { mainCalls++; stageMenus(); } },
    onLeave() { if (staged) { gfx1Calls++; if (cfg.ropMenu) va(0xe75a73).writeU8(1); } },
  });
}
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    if (!cfg) return;
    frames++;
    // `IsActive` (exe 0x912A38) is clear while the window has no focus; `IG2_gameUpdate` then skips `UpdateFrame`,
    // which also finishes the screen fades, so no screen change completes. The capture runs in the background.
    va(0x912a38).writeU8(1);
    // `MainScr_Kilep` (exe 0x4fecc0) leaves the main menu by looping `UpdateFrame` + `FRISSITES` until its fade
    // counter (f32 0xBBB360, from 100) runs out, flagged by 0xBBB22A; the counter doesn't run in a background window.
    if (va(0xbbb22a).readU8() !== 0) va(0xbbb360).writeFloat(0);
    // Keep the pointer in the middle of the 640 × 480 screen (`Mouse_Akt_x/y` 0x916008/0x91600C, `Mouse_x/y`
    // 0xDB4654/0xDB4658), so the colony screen's edge scrolling (`Colony_Autoscroll`) stays still.
    // --cam: pin `ColonyXStart/YStart/Zoom/Pitch/Rotate` (0xBA90DC, 0xBA9514, 0xBA9E8C, 0xBA9B64, 0xBA983C).
    if (phase >= 3 && cfg.cam && A.AktKep.readS32() === 3) {
      [0xba90dc, 0xba9514, 0xba9e8c, 0xba9b64, 0xba983c].forEach((x, i) => va(x).writeFloat(cfg.cam[i]));
    }
    // Keep the clock paused (`IdoPause` 0xBAE264) once the screen is asked for (the loading screen needs it running,
    // and the screens' `_Belep` can restart it), so the save written at the grab is the moment on screen.
    if (phase >= 3 && !cfg.run && va(0xbae264).readU8() === 0) va(0xbae264).writeU8(1);
    // --mouse X,Y puts it elsewhere (a menu's hover; on the spy screen a hot-spot's tooltip).
    if (phase >= 2) {
      const [mx, my] = phase >= 3 && cfg.mouse && (staged || ((cfg.kep === 7 || cfg.kep === 4) && A.AktKep.readS32() === cfg.kep)) ? cfg.mouse : [320, 240];
      va(0x916008).writeS32(mx); va(0x91600c).writeS32(my);
      va(0xdb4654).writeS32(mx); va(0xdb4658).writeS32(my);
    }
    // --debug: the dispatcher's state (`AktKep`, `AktKepKell` 0xBAE250, `Atlepes` 0xBAE24D, `Belep` 0xBAE24C,
    // `OldKep` 0xBAE19C, `IdoPause`) every 120 frames.
    if (cfg.debug && frames % 120 === 0) {
      send({ k: 'state', frames, phase, kep: A.AktKep.readS32(), kell: A.AktKepKell.readS32(), atlepes: va(0xbae24d).readU8(),
        belep: va(0xbae24c).readU8(), old: va(0xbae19c).readS32(), pause: va(0xbae264).readU8(), date: date(),
        stack: Thread.backtrace(this.context, Backtracer.FUZZY).slice(0, 16).map((p) => '0x' + p.sub(mod.base).add(IMAGE_BASE).toString(16)) });
    }
    if (phase === 0 && frames === 1200) {  // the menu needs ~1200 frames (replay.js)
      betolt(Memory.allocAnsiString('scenario/quick.scn'));
      at = frames; phase = 1;
      send({ k: 'scenario' });
    } else if (phase === 1 && frames >= at + 300) {
      loadSave(cfg.slot, Memory.allocAnsiString('screen'), 1);
      setSpeed(1, 1);
      setPause(cfg.run ? 0 : 1, 0);
      send({ k: 'loaded', kep: A.AktKep.readS32(), ido2: A.Ido2.readS32(), date: date() });
      at = frames; phase = 2;
    } else if (phase === 2 && frames >= at + 60 && (A.AktKep.readS32() === 2 || frames >= at + 300)) {
      // `LoadSave` leaves the game on the main menu screen (15) while the clock is paused (with the clock running it
      // goes on to the starmap); ask for ours from there.
      if (cfg.spies) {
        spyStage = stageSpies(cfg.spies);
        send({ k: 'spies', stage: spyStage });
      }
      // --fleets on the info screen: the human's fleets first (the list's fleet rows, re/screens/info.md §6.6).
      if (cfg.fleets && cfg.kep === 9) populate(cfg.fleets);
      // --info-starlog N: N lines in the unsaved `StarLog` (0xB82990, 200 × 0x2c, the newest last; colour +0, text +8;
      // `StarlogHanysorfeltoltve` 0xB84BF4), so the star log has something to show (re/screens/info.md §6.8).
      if (cfg.infoStarlog) {
        const cols = [1, 2, 3, 6, 0x21, 0x22, 0x23, 7];
        for (let k = 0; k < cfg.infoStarlog; k++) {
          const r = va(0xb82990 + 0x2c * (200 - cfg.infoStarlog + k));
          r.writeS32(cols[k % cols.length]);
          r.add(8).writeByteArray(Array.from('LINE ' + (k + 1) + ' OF THE STAR LOG, 33 CHARS', (c) => c.charCodeAt(0)).slice(0, 33).concat([0]));
        }
        va(0xb84bf4).writeS32(cfg.infoStarlog);
      }
      if (cfg.kep >= 0) {
        A.AktKepKell.writeS32(cfg.kep);
        A.AktPlanetKell.writeS32(cfg.planet);
      }
      at = frames; phase = 3;
    } else if (phase === 3 && !setupDone && (cfg.rop || cfg.fleets || cfg.ropMenu || cfg.menu) && A.AktKep.readS32() === 2 && frames >= at + 60) {
      // The starmap is up: make the fleets, select one (--rop-fleet K: `StarmapMode` 0xB9998C = 2, `AktFleet`
      // 0xB99664 = the K-th made), then press the icon and wait from there.
      if (cfg.fleets) populate(cfg.fleets);
      if (cfg.ropFleet && fleets[cfg.ropFleet - 1]) { va(0xb9998c).writeS32(2); va(0xb99664).writeS32(fleets[cfg.ropFleet - 1]); }
      if (cfg.rop) cfg.ropNow = true;
      setupDone = true;
      at = frames;
    } else if (phase === 3 && setupDone && !staged && (cfg.ropMenu || cfg.menu) && (!cfg.rop || ropDone) && frames >= at + 60) {
      // The icon's view has settled (the long info's zoom, the production view): stage the menus from here.
      staged = true;
      at = frames;
      send({ k: 'staged' });
    } else if (phase === 3 && cfg.verdict && verdictLeft > 0 && A.AktKep.readS32() === 7 && frames >= at + 300) {
      // --verdict: one press at a time; a press `Spy_Main` didn't take within 60 frames is reported and dropped.
      if (verdictPending) {
        if (++verdictPending.waited > 60) { send({ k: 'error', what: 'verdict press not taken', p: verdictPending }); verdictPending = null; verdictLeft--; }
      } else if (verdictWait > 0) {
        verdictWait--;
      } else {
        stageVerdict(spyStage);
        verdictPending.waited = 0;
      }
      if (verdictLeft <= 0) at = frames - cfg.wait + 30;
    } else if (phase === 3 && infoLeft.length > 0 && A.AktKep.readS32() === 9 && frames >= at + 300) {
      if (infoWait > 0) {
        infoWait--;
      } else {
        const id = infoLeft.shift();
        va(0xb77208).writeS32(id + 0x1e);
        infoPending = id;
        send({ k: 'info-press', id, frames });
        infoWait = 30;
      }
      if (infoLeft.length === 0) at = frames - 300;
    } else if (phase === 3 && (!cfg.rop || ropDone) && (!(cfg.ropMenu || cfg.menu) || staged) && (!cfg.verdict || verdictLeft <= 0) && infoLeft.length === 0 && frames >= at + cfg.wait) {
      const f = (x) => va(x).readFloat();
      const p = A.AktPlanet.readS32();
      const wf = 0x9c3244 + p * 0x8ec;
      // `ColonyXStart/YStart/Zoom/Pitch/Rotate`, the camera points (`forgaspont` 0xDE0130, `kamerapont` 0xDE0148,
      // (h, x, y)), the battlefield's saved camera (`WF+0x1a8..`) and its centring flag `WF+0x184`.
      const cam = { x: f(0xba90dc), y: f(0xba9514), zoom: f(0xba9e8c), pitch: f(0xba9b64), rotate: f(0xba983c),
        pivot: [f(0xde0130), f(0xde0134), f(0xde0138)], eye: [f(0xde0148), f(0xde014c), f(0xde0150)],
        saved: [0, 4, 8, 12, 16].map((o) => f(wf + 0x1a8 + o)), centre: va(wf + 0x184).readU8() };
      // --state-out: write the state on screen with the exe's own `LoadSave(slot, name, 2)`, so the client can render
      // the very same moment.
      if (cfg.stateSlot) loadSave(cfg.stateSlot, Memory.allocAnsiString('screendiff'), 2);
      const menus = { mainCalls, gfx1Calls, mod: va(0xf16f18).readU8(), frame: va(0xe8634c).readFloat(), alap: va(0xe75a73).readU8(),
        places: [0, 4, 8, 12].map((o) => va(0xe75a30 + o).readS32()), last: [va(0xe75a74).readS32(), va(0xe75a78).readS32()],
        hover: va(0xf16f14).readS32(), ikon2: va(0xbae240).readS32(), sel: va(0xe75a68).readS32(), sx: va(0x927078).readS32(), sy: va(0x92707c).readS32(),
        info: va(0xf16ef7).readU8(), mode: va(0xb9998c).readS32() };
      const spy = { kell: va(0xe75430).readU8(), frame: va(0xe75504).readFloat(), open: va(0xe75436).readU8(), closing: va(0xe75383).readU8(), dialog: va(0xe75384).readS32() };
      // --particles: the starmap's particle sets `FStars` (11 × 0x38: +0 records, 0x4c bytes each) and `DiffStars`
      // (8 × 0x38), the records of every set, `HANYFELHOPARTICLE` 0x92712C and `OptionsFogResolution` 0xDB9764.
      if (cfg.particles) {
        const hex = (p, n) => Array.from(new Uint8Array(p.readByteArray(n)), (b) => b.toString(16).padStart(2, '0')).join('');
        const sets = (base, n) => Array.from({ length: n }, (_, i) => {
          const h = va(base + i * 0x38);
          const recs = h.readPointer();
          const head = hex(h, 0x38);
          const count = h.add(0x10).readS32();
          return { head, count, recs: recs.isNull() || count <= 0 || count > 5000 ? '' : hex(recs, count * 0x4c) };
        });
        send({ k: 'particles', fstars: sets(0xf16f38, 11), diff: sets(0xf171a0, 8), hany: va(0x92712c).readS32(), fog: va(0xdb9764).readS32(), clouds: va(0xe75918).readS32(),
          futofeny: va(0xf1736c).readFloat(), kis: va(0xe75914).readS32(), erosseg: va(0x927994).readFloat() });
      }
      const res = { kell: va(0x918bd5).readU8(), frame: va(0xdc5368).readFloat(), open: va(0xddf322).readU8(), closing: va(0xddf323).readU8(),
        wire: va(0xdc537c).readFloat(), started: va(0xddf628).readU8() };
      // `GomolygasCounter` 0xE75918 (the clouds' drift), `futofeny` 0xF1736C (the starmap's UI clock): re/screens/starmap.md §6;
      // the remaster's `game::acttime` 0xFD1520 (ms, the planet clouds' drift): §3.3; the info screen's side lights' and
      // chart sweeps' clock `infokeringeto` 0xB85304 (re/screens/info.md §1.1); the info list's `BolygoFlottaPointer`
      // 0xB82984, `KirakBolygoInfo`'s problem step 0xB85E88, `Counter` 0xBAE05C and per slot of `InfoBF` 0xB85B90 (0xB0
      // each) the spin +0x78 and the planet's / fleet's rotation +0x30 / +0x54 (§6.2, §6.4).
      const mat = (o) => Array.from({ length: 9 }, (_, i) => va(o + 4 * i).readFloat());
      // The detail mode (§6.6: `Info_NagyInfoMode` 0xB79FB1, `Info_KilepesNagyinforol` 0xB85258, `InfoBolygoFlottaSelected`
      // 0xB82980, `InfoNKivalasztott` 0xB852B8, `InfoMaxTervekPtr` 0xB84C08 / `InfoMaxTervek` 0xB84C0C, the info-text
      // statics 0x90B8A0 / 0xB85E94 / 0xB85E98), the slots' positions `+0x7c`, home `+0x88`, start `+0x94`, target
      // `+0xa0`, steps `+0xac`, and the star log (§6.8: `Info_StarLogMode` 0xB79FB2, `infoframe` 0xB85260, `StarLog`
      // 0xB82990 (200 × 0x2c: colour, text at +8), `StarLogPointer` 0xB84C10, `StarlogHanysorfeltoltve` 0xB84BF4).
      const hexs = (p, n) => Array.from(new Uint8Array(p.readByteArray(n)), (b) => b.toString(16).padStart(2, '0')).join('');
      const infoList = { pointer: va(0xb82984).readS32(), step: va(0xb85e88).readS32(), counter: va(0xbae05c).readS32(),
        globes: [0, 1, 2, 3].map((k) => { const b = 0xb85b90 + 0xb0 * k; return [va(b + 0x78).readFloat()].concat(mat(b + 0x30), mat(b + 0x54), Array.from({ length: 15 }, (_, i) => va(b + 0x7c + 4 * i).readFloat())); }),
        select: [va(0xb79f8a).readU8() ? 26 : va(0xb79f8b).readU8() ? 27 : va(0xb79fb0).readU8() ? 28 : 0, va(0x90b6fc).readS32(), va(0x90b700).readS32(), va(0xb84c14).readS32()],
        detail: { mode: va(0xb79fb1).readU8(), leaving: va(0xb85258).readU8(), entry: va(0xb82980).readS32(), slot: va(0xb852b8).readS32(),
          ptr: va(0xb84c08).readS32(), n: va(0xb84c0c).readS32(), texts: [va(0x90b8a0).readS32(), va(0xb85e94).readS32(), va(0xb85e98).readS32()] },
        star: { mode: va(0xb79fb2).readU8(), frame: va(0xb85260).readFloat(), pointer: va(0xb84c10).readS32(), filled: va(0xb84bf4).readS32(),
          log: Array.from({ length: 200 }, (_, i) => { const r = va(0xb82990 + 0x2c * i); const t = r.add(8).readByteArray(0x24); const b = new Uint8Array(t); const n = b.indexOf(0); return [r.readS32(), hexs(r.add(8), n < 0 ? 0x24 : n)]; }) } };
      send({ k: 'grab', clouds: va(0xe75918).readS32(), futofeny: va(0xf1736c).readFloat(), acttime: va(0xfd1520).readU32(), infoTurn: va(0xb85304).readFloat(), infoList, menus, spy, res, n: shot, kep: A.AktKep.readS32(), planet: p, ido2: A.Ido2.readS32(), date: date(), paused: va(0xbae264).readU8(), cam });
      shot++;
      at = frames;
      if (shot >= cfg.shots) phase = 4;
    }
  },
});
