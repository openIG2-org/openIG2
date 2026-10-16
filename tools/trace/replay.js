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
];
// Story queues outside the save ({ids, dual, n}).
const QUEUES = [['vegzes', 0xf43290], ['talal', 0xf4329c]];

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, slot: 99, name: 'Replay', speed: 3, ticks: 40,
  seed: 4242 };
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
  if (full) v.mem.push(['techs', 0xb9c0c8, hexOf(va(0xb9c0c8), 193 * 0x114)]);
  for (const [n, a] of QUEUES) {
    const q = va(a), ids = q.readPointer(), dual = q.add(4).readPointer(), cnt = q.add(8).readS32(), out = [];
    for (let i = 0; i < cnt; i++) out.push([ids.add(4 * i).readS32(), dual.add(4 * i).readS32()]);
    v.queues[n] = out;
  }
  const p = A.AlapNev.readPointer();
  v.alapnev = p.isNull() ? '' : p.readCString();
  return v;
}

let k = 0, frames = 0, phase = 0, loadedAt = 0, homeKep = -1, busy = false;
function save(where) {
  busy = true;
  const slot = 100 + k, ido2 = A.Ido2.readS32();
  const seed = (cfg.seed + ido2) >>> 0;
  const vol = volatile(k === 0);
  loadSave(slot, Memory.allocAnsiString(cfg.name + ' ' + k), 2);
  emit({ k: 'save', idx: k, slot, where, ido: A.Ido.readS32(), ido2, seed, vol });
  flush();
  busy = false;
  k++;
}

Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    if (busy) return;
    frames++;
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
    save('tick');
    srand((cfg.seed + A.Ido2.readS32()) >>> 0);
    if (k > cfg.ticks) { phase = 3; emit({ k: 'done' }); flush(); }
  },
});

send({ type: 'ready', base: mod.base.toString() });
