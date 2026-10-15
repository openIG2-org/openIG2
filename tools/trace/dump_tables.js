// Frida agent: dump rules tables from live ig2.exe memory, before and after a scenario load.
'use strict';
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - 0x400000);
let cfg = null;
recv('config', (m) => { cfg = m.payload; });

function dumpAll(phase) {
  for (const t of cfg.tables) {
    const bytes = va(t.va).readByteArray(t.size);
    send({ type: 'dump', phase, name: t.name }, bytes);
  }
}

let frames = 0, done = false;
Interceptor.attach(va(0x4de720), { // Fake_FRISSITES: once per frame, main thread
  onEnter() {
    if (done || !cfg) return;
    frames++;
    if (frames === cfg.startAfterFrames) {
      dumpAll('menu');
      const betolt = new NativeFunction(va(0x4f5ba0), 'void', ['pointer'], 'mscdecl');
      betolt(Memory.allocAnsiString(cfg.scenario));
      dumpAll('scenario');
      done = true;
      send({ type: 'done' });
    }
  },
});
send({ type: 'ready' });
