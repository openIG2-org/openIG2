// Environment-mapping probe (re/screens/render.md §2.5), appended to screen.js by run_screen.py
// --probe: `vagashatarkirak` (exe 0x5395d0, ELF 0x606c90; args: sort record, nfaces, VertType[] stride 0x24, pont[],
// normals[] stride 0x14 (nx, ny, nz, diffuse, specular), uv[], FaceType[] stride 0x28 (u16 vertex a, b, c at +0/+2/+4,
// normal a, b, c at +6/+8/+0xa), XVO) on the main scene's pass (`ColonyMasodikUpdate3d` 0x998247 clear): the record's
// matrix and point before the call scales them, its properties, the first faces with their vertices and normals, and
// the corners of the first `gl_draw_tlv` (exe 0x4ac580, D3DTLVERTEX stride 0x28: x, y, z, rhw, …, tu +0x18, tv +0x1c)
// it issues. Skips the first 400 calls (the scene settling), then logs 8.
(function () {
  let calls = 0, sent = 0, cur = null;
  Interceptor.attach(va(0x5395d0), {
    onEnter(args) {
      if (va(0x998247).readU8() !== 0 || sent >= 8 || ++calls < 400) return;
      const sp = args[0], nf = Math.min(args[1].toInt32(), 80), vt = args[2], nt = args[4], ft = args[6];
      const faces = [], verts = new Map(), normals = new Map();
      for (let f = 0; f < nf; f++) {
        const w = [0, 2, 4, 6, 8, 10].map((o) => ft.add(0x28 * f + o).readU16());
        faces.push(w);
        for (const i of w.slice(0, 3)) if (!verts.has(i)) verts.set(i, [0, 4, 8].map((k) => vt.add(0x24 * i + k).readFloat()));
        for (const i of w.slice(3)) if (!normals.has(i)) normals.set(i, [0, 4, 8].map((k) => nt.add(0x14 * i + k).readFloat()));
      }
      cur = { k: 'env', props: sp.add(0x68).readU32(), nf: args[1].toInt32(), m: Array.from({ length: 9 }, (_, i) => sp.add(4 * i).readFloat()),
        at: [0x48, 0x4c, 0x50].map((o) => sp.add(o).readFloat()), clip: [va(0xdff8e8).readFloat(), va(0xdff8ec).readFloat()],
        faces, verts: Array.from(verts.entries()), normals: Array.from(normals.entries()), tlv: null };
    },
    onLeave() {
      if (cur) { send(cur); sent++; }
      cur = null;
    },
  });
  Interceptor.attach(va(0x4ac580), {
    onEnter(args) {
      if (!cur || cur.tlv) return;
      const p = args[1], n = Math.min(args[2].toInt32(), 240);
      cur.tlv = Array.from({ length: n }, (_, i) => [0, 4, 0x18, 0x1c].map((o) => p.add(0x28 * i + o).readFloat()));
    },
  });
})();
