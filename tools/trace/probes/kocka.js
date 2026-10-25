// Box placement probe, appended to screen.js by run_screen.py --probe: every `Kocka(spsort)` (exe
// 0x5281f0) of one frame shortly before the grab logs the sort record's matrix `+0..+0x20` (the box's rotation·scale
// as its parent set it), its position `+0x48..+0x50` (scene units), its size `+0x7c`, flags `+0x68`, the chain head
// `+0x58` and the head's box count `+0x24`: how the exe places each child of the panels' models.
(function () {
  let frame = -1;
  Interceptor.attach(va(0x5281f0), {
    onEnter(args) {
      if (A.AktKep.readS32() !== 2 || phase !== 3 || (cfg.rop && !ropDone) || frames !== at + cfg.wait - 2) return;
      const sp = args[0], head = sp.add(0x58).readPointer();
      send({ k: 'kocka', head: head.toString(), boxes: head.isNull() ? -1 : head.add(0x24).readS32(),
        m: Array.from({ length: 9 }, (_, i) => sp.add(4 * i).readFloat()),
        at: [0x48, 0x4c, 0x50].map((o) => sp.add(o).readFloat()), size: sp.add(0x7c).readFloat(), flags: sp.add(0x68).readU32() });
    },
  });
})();
