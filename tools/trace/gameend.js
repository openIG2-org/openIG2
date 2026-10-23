// Frida agent: the game end on the GOG ig2.exe (spec re/rules/game-end.md).
// After quick.scn is loaded, `mode` stages one end from the frame hook (`Fake_FRISSITES` 0x4de720):
//   win   every other empire's BirodalomEl (0xBAE6AC) := 0: the tick's MegnyeresEllenoriz (0x53f8c0) wins;
//   lose  the human dies in the day's FRISSIT_Viszony (BirodalomMeghal = exe MEGHALAS 0x53b630);
//   quiet MEGNYERES(0) (0x53f6b0, cdecl char): no videos.
// Then a second MEGNYERES(1) once the main menu is up (GameStarted is clear: nothing may happen).
// Calls made from inside a hook skip the other hooks (Frida ignores its own thread): the flags around them are logged.
// lose therefore goes through the game's own death check; quiet expects no video.
// `st_ShowAnim` (0x602bb0) is replaced by a logger (no video plays). Logged: the end calls with GameStarted
// (0xBAE297), ExitToMainScreen (0x5117f0) and what it clears, every AktKep (0xBAE0FC) / AktKepKell (0xBAE250)
// change, StartDemoMode (0x4fe3c0 = MainScr_Belep) with scenario_nevbevitelkell (0xBAD10C) and scenario_score
// (0xBAD110), and every LoadHighscores (0x543750) table on return (desc +0x3c key, +0x40 scores, +0x68 names).
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330),
  MEGHALAS: va(0x53b630), MEGNYERES: va(0x53f6b0), Megnyeres: va(0x53f8c0), Exit: va(0x5117f0),
  StartDemoMode: va(0x4fe3c0), ShowAnim: va(0x602bb0), LoadHighscores: va(0x543750), AddHighscore: va(0x543150),
  EN: va(0x912a3c), Alive: va(0xbae6ac), Faj: va(0xbae574), GameStarted: va(0xbae297), AktKep: va(0xbae0fc),
  AktKepKell: va(0xbae250), NevKell: va(0xbad10c), Score: va(0xbad110), Campaign: va(0x928278),
  MultiPlayerMode: va(0xbae2c0), KapcsolatEl: va(0xbae2c1), Desync: va(0xb771e8),
};

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, mode: 'win', after: 300, tail: 600 };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { ev.f = frames; buf.push(ev); if (buf.length >= 4) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

const nf = (a, r, args) => new NativeFunction(a, r, args, { abi: 'mscdecl', traps: 'all' });
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const megnyeres = nf(A.MEGNYERES, 'void', ['char']);
const str = (p) => { try { return p.isNull() ? null : p.readCString(); } catch (e) { return '?' + p; } };
const flags = () => ({ started: A.GameStarted.readU8(), mp: A.MultiPlayerMode.readU8(), kapcs: A.KapcsolatEl.readU8(),
  desync: A.Desync.readS32(), kep: A.AktKep.readS32(), kell: A.AktKepKell.readS32() });

let frames = 0;
Interceptor.replace(A.ShowAnim, new NativeCallback((name) => { emit({ k: 'anim', name: str(name) }); }, 'void', ['pointer'], 'mscdecl'));
Interceptor.attach(A.MEGHALAS, { onEnter(a) { emit({ k: 'meghalas', e: a[0].toInt32(), en: A.EN.readS32(), ...flags() }); } });
Interceptor.attach(A.MEGNYERES, { onEnter(a) { emit({ k: 'megnyeres', arg: a[0].toInt32() & 0xff, ...flags() }); } });
Interceptor.attach(A.Exit, {
  onEnter() { emit({ k: 'exit_enter', ...flags() }); },
  onLeave() { emit({ k: 'exit_leave', ...flags() }); },
});
Interceptor.attach(A.StartDemoMode, {
  onEnter() { emit({ k: 'mainscr_belep', nevkell: A.NevKell.readS32(), score: A.Score.readS32(), ...flags() }); },
});
Interceptor.attach(A.AddHighscore, { onEnter(a) { emit({ k: 'add_highscore', key: str(a[0]), name: str(a[1]), score: a[2].toInt32() }); } });
Interceptor.attach(A.LoadHighscores, {
  onEnter(a) { this.key = str(a[0]); this.d = a[1]; },
  onLeave() {
    const d = this.d;
    const scores = [], names = [];
    for (let i = 0; i < 10; i++) {
      scores.push(d.add(0x40 + 4 * i).readS32());
      names.push(str(d.add(0x68 + 4 * i).readPointer()));
    }
    emit({ k: 'highscores', key: this.key, desc_key: str(d.add(0x3c).readPointer()), scores, names });
  },
});

let started = false, loadedAt = 0, staged = 0, menuAt = 0, last = '';
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    const now = JSON.stringify([A.AktKep.readS32(), A.AktKepKell.readS32()]);
    if (now !== last) { last = now; emit({ k: 'screen', kep: A.AktKep.readS32(), kell: A.AktKepKell.readS32() }); }
    if (!started && frames === cfg.startAfterFrames) {
      started = true;
      betolt(Memory.allocAnsiString(cfg.scenario));
      setSpeed(1, 1);
      A.AktKepKell.writeS32(2);   // the starmap (the win check wants AktKep < 0xE)
      loadedAt = frames;
      const en = A.EN.readS32();
      emit({ k: 'loaded', en, race: A.Faj.add(en).readU8(), campaign: A.Campaign.readS32(), ...flags(),
        alive: Array.from({ length: 19 }, (_, e) => A.Alive.add(e).readU8()) });
      return;
    }
    if (!started) return;
    if (staged === 0 && frames >= loadedAt + cfg.after && A.AktKep.readS32() === 2) {
      staged = 1;
      const en = A.EN.readS32();
      emit({ k: 'stage', mode: cfg.mode, ...flags() });
      try {
        if (cfg.mode === 'win') for (let e = 1; e < 19; e++) { if (e !== en) A.Alive.add(e).writeU8(0); }
        else if (cfg.mode === 'lose') {
          // FRISSIT_Viszony's death check (diplomacy.md 3.2) kills an AI-run empire without planets: the human's
          // planets go to AI 3, the human is marked AI-run with countdown 1 (a call from here would skip the hooks).
          const bolygok = va(0xb85ea8), n = va(0x912ac8).readS32();
          for (let p = 1; p <= n; p++) if (bolygok.add(p * 0x188 + 0x20).readU8() === en) bolygok.add(p * 0x188 + 0x20).writeU8(3);
          va(0x915f58).add(en).writeU8(1); va(0xbaeb68).add(4 * en).writeS32(1);
        } else if (cfg.mode === 'quiet') megnyeres(0);
      } catch (e) { emit({ k: 'error', msg: e.toString() }); }
      emit({ k: 'staged', ...flags() });
    }
    if (staged === 1 && A.AktKep.readS32() === 0xf) { staged = 2; menuAt = frames; }
    if (staged === 2 && frames >= menuAt + 60) {
      staged = 3;
      emit({ k: 'second_before', ...flags() });
      megnyeres(1);
      emit({ k: 'second', ...flags() });
    }
    if (staged === 3 && frames >= menuAt + cfg.tail) { staged = 4; emit({ k: 'done' }); flush(); }
  },
});

send({ type: 'ready', base: mod.base.toString() });
