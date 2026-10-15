// Frida agent: trace the IG2 (GOG ig2.exe, PE32, MSVC 2013) sim clock and seeded RNG.
// Addresses are image VAs from re/symbols + re/rules/time-and-rng.md; rebased at runtime.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  IG2_gameUpdate: va(0x4e0440),   // entered once: the remaster's main loop lives inside it
  Fake_FRISSITES: va(0x4de720),   // once per rendered frame
  rndlog: va(0x542da0),          // rndold inlined
  IdoNapValtozott: va(0x53e970), // called once per running sim tick
  SetIdoSpeed: va(0x541330),
  Betolt_Scenario: va(0x4f5ba0),
  RandSeed: va(0xbaf2d0),
  Ido: va(0xbae04c),
  Ev: va(0xbae040), Honap: va(0xbae044), Nap: va(0xbae048),
  IdoSpeed: va(0xbae258), IdoSpeedSzorzo: va(0xbae25c),
  AktKep: va(0xbae0fc),
};

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 400, speedPlan: [] };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let frames = 0, started = false, t0 = 0;
let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 500) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 500);

const betolt = new NativeFunction(A.Betolt_Scenario, 'void', ['pointer'], 'mscdecl');
const setSpeed = new NativeFunction(A.SetIdoSpeed, 'void', ['int', 'int'], 'mscdecl');
const scnPath = Memory.allocAnsiString(cfg.scenario);
let speedIdx = 0;

Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (!started && frames === cfg.startAfterFrames) {
      started = true;
      t0 = Date.now();
      const p = Memory.allocAnsiString(cfg.scenario);
      emit({ k: 'start', t: 0, frames, scenario: cfg.scenario });
      betolt(p);
      emit({ k: 'loaded', t: Date.now() - t0, ido: A.Ido.readS32(), kep: A.AktKep.readS32(),
             seeds: Array.from({ length: 21 }, (_, i) => A.RandSeed.add(4 * i).readU32()) });
    }
    if (started) {
      const now = Date.now() - t0;
      while (speedIdx < cfg.speedPlan.length && now >= cfg.speedPlan[speedIdx].at) {
        setSpeed(cfg.speedPlan[speedIdx].speed, 1);
        emit({ k: 'speed', t: now, speed: cfg.speedPlan[speedIdx].speed,
               mult: A.IdoSpeedSzorzo.readS32() });
        speedIdx++;
      }
    }
  },
});

Interceptor.attach(A.rndlog, {
  onEnter(args) {
    this.s = args[0].toInt32(); this.lo = args[1].toInt32(); this.hi = args[2].toInt32();
    this.before = (this.s >= 0 && this.s < 21) ? A.RandSeed.add(4 * this.s).readU32() : null;
  },
  onLeave(ret) {
    const after = (this.s >= 0 && this.s < 21) ? A.RandSeed.add(4 * this.s).readU32() : null;
    emit({ k: 'rnd', t: started ? Date.now() - t0 : -1, s: this.s, lo: this.lo, hi: this.hi,
           before: this.before, after, r: ret.toInt32(), ido: A.Ido.readS32() });
  },
});

Interceptor.attach(A.IdoNapValtozott, {
  onEnter() {
    emit({ k: 'tick', t: Date.now() - t0, ido: A.Ido.readS32(), ev: A.Ev.readS32(),
           honap: A.Honap.readS32(), nap: A.Nap.readS32(), mult: A.IdoSpeedSzorzo.readS32(),
           kep: A.AktKep.readS32() });
  },
});

send({ type: 'ready', base: mod.base.toString() });
