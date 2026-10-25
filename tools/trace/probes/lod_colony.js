// Level-of-detail probe, the colony variant (lod.js is the starmap's), appended to screen.js by run_screen.py --probe: every
// `Kocka(spsort)` (exe 0x5281f0) in the last 3 frames before the grab whose box heads a LOD chain (`+0x14` next node,
// `+0x10` lod) logs the sort record's size `+0x7c` (before Kocka's ×4), its matrix `+0..+0x20`, its z `+0x50`, the
// flags `+0x68`, the chain's length and `fov` (0xE033E0), once per head and size.
(function () {
  const seen = new Set();
  Interceptor.attach(va(0x5281f0), {
    onEnter(args) {
      if (A.AktKep.readS32() !== 3 || phase !== 3 || frames < at + cfg.wait - 3) return;
      const sp = args[0], head = sp.add(0x58).readPointer();
      if (head.isNull() || head.add(0x14).readPointer().isNull()) return;
      const size = sp.add(0x7c).readFloat();
      const key = head.toString() + size.toPrecision(6);
      if (seen.has(key)) return;
      seen.add(key);
      let n = 0;
      for (let h = head; !h.isNull() && n < 16; h = h.add(0x14).readPointer()) n++;
      send({ k: 'lod', head: head.toString(), size, m: Array.from({ length: 9 }, (_, i) => sp.add(4 * i).readFloat()),
        at: [0x48, 0x4c, 0x50].map((o) => sp.add(o).readFloat()), z: sp.add(0x50).readFloat(), flags: sp.add(0x68).readU32(), chain: n, lod: head.add(0x10).readFloat(),
        fov: va(0xe033e0).readFloat(), valtashatar: va(0x919660).readS32() });
    },
  });
})();
