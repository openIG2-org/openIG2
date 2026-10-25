// Colony panel pass light probe (re/screens/colony.md §11.10, the FX17b follow-up), appended to screen.js by
// run_screen.py --probe with --jsonl OUT: on the colony screen's second `Update3D` (`ColonyMasodikUpdate3d` 0x998247
// set) every `fenyekszamolasa` (exe 0x52f190) call as screen.js --light-probe records it (the node's lights, its
// rotation chain `m`, the sort record `raw`, flags, opacity and the file normals with the colour the call wrote),
// each object (XVOStruct) once, after `skip` calls of the pass so the screen has settled; and the main view's first
// lit object (pass 1) for comparison.
(function () {
  const skip = 400;
  let calls = 0, mainSent = false;
  const seen = new Set();
  Interceptor.attach(va(0x52f190), {
    onEnter(args) {
      const pass2 = va(0x998247).readU8() !== 0;
      this.on = false;
      if (pass2) {
        calls++;
        const key = args[4].toString();
        this.on = calls > skip && !seen.has(key) && seen.size < 16;
        if (this.on) seen.add(key);
      } else if (calls > skip && !mainSent) {
        this.on = mainSent = true;
      }
      if (!this.on) return;
      this.pass = pass2 ? 2 : 1;
      this.vn = args[1]; this.n = args[2].toInt32(); this.sp = args[3]; this.xvo = args[4]; this.op = args[5].toInt32();
    },
    onLeave() {
      if (!this.on) return;
      const sp = this.sp, m = [];
      for (let i = 0; i < 9; i++) m.push(sp.add(0x24 + 4 * i).readFloat());
      const flags = sp.add(0x68).readU32(), nl = sp.add(0x70).readS32(), lights = [];
      for (let i = 0; i < nl && i < 16; i++) {
        const l = sp.add(0x6c).readPointer().add(4 * i).readPointer();
        lights.push({ type: l.readS32(), color: l.add(4).readU32(), dir: [0x18, 0x1c, 0x20].map((o) => l.add(o).readFloat()),
          vec: [0x28, 0x2c, 0x30].map((o) => l.add(o).readFloat()), spec: l.add(0x44).readU8() });
      }
      const verts = [];
      for (let i = 0; i < this.n && i < 3000; i++) {
        const v = this.vn.add(0x14 * i);
        verts.push([v.readFloat(), v.add(4).readFloat(), v.add(8).readFloat(), v.add(0xc).readU32()]);
      }
      const raw = Array.from({ length: 0x68 / 4 }, (_, i) => sp.add(4 * i).readFloat());
      send({ k: 'light', pass: this.pass, xvo: this.xvo.toString(), n: this.n, m, raw, flags, opacity: this.op, lights, verts });
    },
  });
})();
