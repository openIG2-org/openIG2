// Drawn-vertex probe (deviations #47), appended to screen.js by run_screen.py --probe: `kirak` (exe 0x531460, ELF
// 0x60e160) and `kirak2` (exe 0x530b20) as called from `Kocka` (spsort, nfaces, VertType[] stride 0x24, pont[], …,
// FaceType[] stride 0x28 with u16 a, b, c at +0/+2/+4, XVO), on the frames around the grab (`frame` relative to it), for the sort
// records around the fleet panel's first three cells and its big preview (camera-fixed at z −600): the record's matrix and
// position before `kirak` scales them, and every vertex the faces use (as stored in the XVO).
(function () {
  let tlvOn = false, cur = null;
  const near = (sp) => {
    const x = sp.add(0x48).readFloat() * 1024, y = sp.add(0x4c).readFloat() * 1024, z = sp.add(0x50).readFloat() * 1024;
    return Math.abs(z + 600) < 60 && [[-229, -156], [-100, -156], [-229, -86], [150, -150]].some(([cx, cy]) => Math.abs(x - cx) < 60 && Math.abs(y - cy) < 60);
  };
  for (const [name, addr] of [['kirak', 0x531460], ['kirak2', 0x530b20]]) {
    Interceptor.attach(va(addr), {
      onEnter(args) {
        if (A.AktKep.readS32() !== 2 || phase !== 3 || (cfg.rop && !ropDone) || frames < at + cfg.wait - 2 || frames > at + cfg.wait + 2) return;
        const sp = args[0];
        cur = { fn: name, m: Array.from({ length: 9 }, (_, i) => sp.add(4 * i).readFloat()), at: [0x48, 0x4c, 0x50].map((o) => sp.add(o).readFloat()), nf: args[1].toInt32() };
        if (!near(sp)) return;
        const nf = args[1].toInt32(), vt = args[2], ft = args[6];
        const used = new Map();
        for (let f = 0; f < nf && f < 4000; f++) {
          for (const o of [0, 2, 4]) {
            const i = ft.add(0x28 * f + o).readU16();
            if (!used.has(i)) used.set(i, [0, 4, 8].map((k) => vt.add(0x24 * i + k).readFloat()));
          }
        }
        tlvOn = Math.abs(sp.add(0x48).readFloat() * 1024 + 100) < 60 && Math.abs(sp.add(0x4c).readFloat() * 1024 + 156) < 60;
        send({ k: 'kirak', fn: name, frame: frames - at - cfg.wait, xvo: args[7].toString(), nf, m: Array.from({ length: 9 }, (_, i) => sp.add(4 * i).readFloat()),
          at: [0x48, 0x4c, 0x50].map((o) => sp.add(o).readFloat()), verts: Array.from(used.entries()) });
      },
      onLeave() { tlvOn = false; cur = null; },
    });
  }
  // `gl_draw_tlv` (exe 0x4ac580: (4, D3DTLVERTEX*, n, n, 0), stride 0x28: x, y, z, rhw, …) inside the second cell's
  // `kirak` calls: the screen-space vertices as drawn.
  Interceptor.attach(va(0x4ac580), {
    onEnter(args) {
      if (A.AktKep.readS32() !== 2 || phase !== 3 || frames < at + cfg.wait - 2 || frames > at + cfg.wait + 2) return;
      const p = args[1], n = Math.min(args[2].toInt32(), 3000);
      const v = Array.from({ length: n }, (_, i) => [0, 4, 8, 12].map((o) => p.add(0x28 * i + o).readFloat()));
      // Any draw landing in the second cell's box (virtual 640 × 480: x 170..270, y 70..115), whoever issued it.
      if (!tlvOn && !v.some(([x, y]) => x > 170 && x < 270 && y > 70 && y < 115)) return;
      send({ k: 'tlv', frame: frames - at - cfg.wait, own: tlvOn, rec: cur, n, v });
    },
  });
})();
