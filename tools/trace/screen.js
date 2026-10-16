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

let cfg = null;
recv('config', (m) => { cfg = m.payload; });

let frames = 0, phase = 0, at = 0, shot = 0;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    if (!cfg) return;
    frames++;
    // Keep the pointer in the middle of the 640 × 480 screen (`Mouse_Akt_x/y` 0x916008/0x91600C, `Mouse_x/y`
    // 0xDB4654/0xDB4658), so the colony screen's edge scrolling (`Colony_Autoscroll`) stays still.
    // --cam: pin `ColonyXStart/YStart/Zoom/Pitch/Rotate` (0xBA90DC, 0xBA9514, 0xBA9E8C, 0xBA9B64, 0xBA983C).
    if (phase >= 3 && cfg.cam && A.AktKep.readS32() === 3) {
      [0xba90dc, 0xba9514, 0xba9e8c, 0xba9b64, 0xba983c].forEach((x, i) => va(x).writeFloat(cfg.cam[i]));
    }
    if (phase >= 2) {
      va(0x916008).writeS32(320); va(0x91600c).writeS32(240);
      va(0xdb4654).writeS32(320); va(0xdb4658).writeS32(240);
    }
    if (phase === 0 && frames === 1200) {  // the menu needs ~1200 frames (replay.js)
      betolt(Memory.allocAnsiString('scenario/quick.scn'));
      at = frames; phase = 1;
      send({ k: 'scenario' });
    } else if (phase === 1 && frames >= at + 300) {
      loadSave(cfg.slot, Memory.allocAnsiString('screen'), 1);
      setSpeed(1, 1);
      setPause(cfg.run ? 0 : 1, 0);
      send({ k: 'loaded', kep: A.AktKep.readS32(), ido2: A.Ido2.readS32() });
      at = frames; phase = 2;
    } else if (phase === 2 && frames >= at + 60 && (A.AktKep.readS32() === 2 || frames >= at + 1500)) {
      // `LoadSave` goes through the loading screen (15) first; ask for ours once the starmap is up.
      if (cfg.kep >= 0) {
        A.AktKepKell.writeS32(cfg.kep);
        A.AktPlanetKell.writeS32(cfg.planet);
      }
      at = frames; phase = 3;
    } else if (phase === 3 && frames >= at + cfg.wait) {
      const f = (x) => va(x).readFloat();
      const p = A.AktPlanet.readS32();
      const wf = 0x9c3244 + p * 0x8ec;
      // `ColonyXStart/YStart/Zoom/Pitch/Rotate`, the camera points (`forgaspont` 0xDE0130, `kamerapont` 0xDE0148,
      // (h, x, y)), the battlefield's saved camera (`WF+0x1a8..`) and its centring flag `WF+0x184`.
      const cam = { x: f(0xba90dc), y: f(0xba9514), zoom: f(0xba9e8c), pitch: f(0xba9b64), rotate: f(0xba983c),
        pivot: [f(0xde0130), f(0xde0134), f(0xde0138)], eye: [f(0xde0148), f(0xde014c), f(0xde0150)],
        saved: [0, 4, 8, 12, 16].map((o) => f(wf + 0x1a8 + o)), centre: va(wf + 0x184).readU8() };
      send({ k: 'grab', n: shot, kep: A.AktKep.readS32(), planet: p, ido2: A.Ido2.readS32(), cam });
      shot++;
      at = frames;
      if (shot >= cfg.shots) phase = 4;
    }
  },
});
