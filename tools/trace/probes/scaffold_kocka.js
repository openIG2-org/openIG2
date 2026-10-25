// Scaffolding draw probe, appended to screen.js by run_screen.py --probe: on the colony screen, after the first
// 30000 calls, every `Kocka(spsort)` (exe 0x5281f0) whose effective properties (`+0x68`) carry 0x200
// (alpha test: the scaffolding layer `atlatszodolgok`, also the trees): the sort record's camera-space matrix `+0..+0x20`
// and point `+0x48..+0x50`, its size `+0x7c` and properties. Compared with the boxes `ModifyAllvany` placed.
(function () {
  let calls = 0, all = 0;
  Interceptor.attach(va(0x5281f0), {
    onEnter(args) {
      if (++all < 30000) return;
      const sp = args[0], flags = sp.add(0x68).readU32();
      if ((flags & 0x200) === 0 || calls++ > 300) return;
      send({ k: 'kocka', flags, m: Array.from({ length: 9 }, (_, i) => +sp.add(4 * i).readFloat().toFixed(5)),
        at: [0x48, 0x4c, 0x50].map((o) => +sp.add(o).readFloat().toFixed(5)), size: sp.add(0x7c).readFloat() });
    },
  });
})();
