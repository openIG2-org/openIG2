// Research tech-model probe (FX17b), appended to screen.js by run_screen.py --probe (with --script): on
// the script ticks listed in GRABS, every object the engine lights and draws (`fenyekszamolasa` exe 0x52f190, cdecl
// (pos, vertexnormal[], n, spsorttype, XVOStruct, opacity), called from `Kocka` only for the objects it draws) with its
// vertex count, flags (sp +0x68), opacity byte, rotation chain (sp +0x24), its lights (sp +0x6c/+0x70) and, for the
// wireframes (flag 0x40), the colours it gave the first vertices (vertexnormal 0x14 B, colour +0xc), plus
// `reskeringeto` (exe 0xDDF6BC). Records 'resobj' go to stdout.
(function () {
  const GRABS = new Set([250, 400, 500, 560]);
  Interceptor.attach(va(0x52f190), {
    onEnter(args) {
      this.on = A.AktKep.readS32() === 4 && scriptT !== null && GRABS.has(scriptT);
      if (!this.on) return;
      this.vn = args[1]; this.n = args[2].toInt32(); this.sp = args[3]; this.op = args[5].toInt32();
    },
    onLeave() {
      if (!this.on) return;
      const sp = this.sp, m = [];
      for (let i = 0; i < 9; i++) m.push(+sp.add(0x24 + 4 * i).readFloat().toFixed(4));
      const flags = sp.add(0x68).readU32(), nl = sp.add(0x70).readS32(), lights = [];
      for (let i = 0; i < nl && i < 16; i++) {
        const l = sp.add(0x6c).readPointer().add(4 * i).readPointer();
        lights.push([l.readS32(), l.add(4).readU32().toString(16), [0x28, 0x2c, 0x30].map((o) => +l.add(o).readFloat().toFixed(3))]);
      }
      const cols = [];
      if (flags & 0x40) for (let i = 0; i < this.n && i < 12; i++) cols.push(this.vn.add(0x14 * i + 0xc).readU32().toString(16));
      send({ k: 'resobj', t: scriptT, n: this.n, flags, op: this.op, m, lights, cols, ker: va(0xddf6bc).readFloat() });
    },
  });
})();
