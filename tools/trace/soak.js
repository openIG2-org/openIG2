// Frida agent: a long AI-only run of a generated galaxy on the GOG ig2.exe, sampled once a game month (spec
// re/rules/ai-empire.md §14). Starts a skirmish the way the skirmish menu does (SetScirmishGameOptions 0x503e80, the
// menu's default settings) or the multiplayer galaxy as newgame.js does (`mp` set), hands the human empires to the
// AI as a dropped multiplayer player (PlayerKi 2, BirodalomMIzik 1, INIT_MI_KIESETT 0x50ef50), runs the clock at
// the top speed and sends, on every month change, each empire's alive flag, planet count, fleet count and money.
// Addresses are image VAs, rebased at runtime.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), SetScirmishGameOptions: va(0x503e80), InitNewGame: va(0x4e1130), ShowAnim: va(0x602bb0),
  SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300), InitMiKiesett: va(0x50ef50),
  El: va(0xbae6ac), MIzik: va(0x915f58), PlayerKi: va(0xbae560), PlayerFaj: va(0xbae574), EN: va(0x912a3c),
  Nap: va(0xbae048), Honap: va(0xbae044), Ev: va(0xbae040), Ido: va(0xbae04c), Money: va(0xbad168),
  Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8), Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8),
  AktKep: va(0xbae0fc), AktKepKell: va(0xbae250), Speed: va(0xbae25c),
  SkPlanets: va(0x9140d0), SkMoney: va(0x9140b8),
};

const cfg = { startAfterFrames: 1200, mp: null, ais: 3, humans: [] };
recv('config', (m) => { Object.assign(cfg, m.payload); });

const nf = (p, r, a) => new NativeFunction(p, r, a, { abi: 'mscdecl', traps: 'all' });
const setSkirmish = nf(A.SetScirmishGameOptions, 'void', ['int', 'int', 'int', 'int', 'int', 'int', 'int', 'int']);
const initNewGame = nf(A.InitNewGame, 'void', ['int', 'int', 'int', 'int']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const setPause = nf(A.SetIdoPause, 'void', ['int', 'int']);
const initMiKiesett = nf(A.InitMiKiesett, 'void', ['int']);
Interceptor.replace(A.ShowAnim, new NativeCallback(() => {}, 'void', ['pointer'], 'mscdecl'));

function start() {
  if (cfg.mp) {
    const mp = cfg.mp;
    const w8 = (at, v) => va(at).writeU8(v), w32 = (at, v) => va(at).writeS32(v);
    w8(0xbae2c0, 1); w8(0xbae297, 1); w32(0x912a3c, mp.en); w32(0xbb6798, mp.en);
    w32(0x912acc, 8); w32(0x912ad0, 8); w32(0x912ac8, mp.planets); w32(0xbae54c, mp.speed);
    w32(0xbae554, 1); w32(0xbae544, 0);
    for (let i = 1; i <= 18; i++) {
      w8(0xbae560 + i, mp.humans.includes(i) ? 1 : 2);
      w8(0xbae6ac + i, i <= 8 && mp.alive.includes(i) ? 1 : 0);
    }
    for (let i = 1; i <= 18; i++) w8(0xbae574 + i, { 15: 1, 16: 3, 17: 1, 18: 1 }[i] ?? i);
    initNewGame(mp.seed, 2, 0, 1);
    return mp.humans;
  }
  // The skirmish menu's defaults (mainscr_scenario.rs Skirmish::new): planets, technology, money, population 2.
  setSkirmish(1, cfg.ais, 1, 2, A.SkPlanets.add(8).readS32(), A.SkMoney.add(8).readS32(), 2, 2);
  return [1];
}

function sample() {
  const np = A.HanyBolygo.readS32(), nf_ = A.FlottaCount.readS32();
  const planets = new Array(19).fill(0), fleets = new Array(19).fill(0);
  for (let p = 1; p <= np; p++) { const o = A.Bolygok.add(0x188 * p + 0x20).readU8(); if (o < 19) planets[o]++; }
  const fl = A.Flottak.readPointer();
  for (let f = 1; f <= nf_; f++) { const o = fl.add(0x80 * f + 0x15).readU8(); if (o < 19) fleets[o]++; }
  const out = [];
  for (let e = 1; e <= 18; e++) {
    if (!A.El.add(e).readU8() && !planets[e]) continue;
    out.push([e, A.El.add(e).readU8(), A.PlayerFaj.add(e).readU8(), planets[e], fleets[e], Math.round(A.Money.add(8 * e).readDouble())]);
  }
  send({ type: 'sample', date: [A.Ev.readS32(), A.Honap.readS32(), A.Nap.readS32()], ido: A.Ido.readS32(), empires: out });
}

let frames = 0, phase = 0, at = 0, lastMonth = -1, lastIdo = -1, sameFor = 0, homeKep = -1;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (phase === 0 && frames === cfg.startAfterFrames) {
      cfg.humans = start();
      setSpeed(3, 1);
      phase = 1; at = frames;
      send({ type: 'started', humans: cfg.humans });
      return;
    }
    if (phase === 1 && frames >= at + 300) {
      for (const e of cfg.humans) { A.PlayerKi.add(e).writeU8(2); A.MIzik.add(e).writeU8(1); initMiKiesett(e); }
      homeKep = A.AktKep.readS32();
      setSpeed(3, 1);
      phase = 2;
      sample();
      return;
    }
    if (phase !== 2) return;
    const m = A.Honap.readS32();
    if (m !== lastMonth) { lastMonth = m; sample(); }
    if (frames % 300 === 0 && A.Speed.readS32() < 3) setSpeed(3, 1);
    const ido = A.Ido.readS32();
    sameFor = ido === lastIdo ? sameFor + 1 : 0;
    lastIdo = ido;
    if (sameFor === 200 || (sameFor > 200 && sameFor % 600 === 0)) {
      send({ type: 'stall', ido, kep: A.AktKep.readS32(), speed: A.Speed.readS32() });
      A.AktKepKell.writeS32(homeKep); setPause(0, 0); setSpeed(3, 1);
    }
  },
});

send({ type: 'ready', base: mod.base.toString() });
