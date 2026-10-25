// Building height probe (deviation 383, re/screens/colony.md §3.1), appended to screen.js by run_screen.py --probe:
// every `getbottomvertex` (exe 0x4a89a0, cdecl: float a, float b, int planet; result in st0) with its arguments, the
// dumped vertex list (`*0xDE0284`, count 0xDE0288, 0x18 bytes a vertex: x, y, z first) as its height range and the
// vertices within 0.5 % of the bottom, and the result read from st0 on the way out.
(function () {
  Interceptor.attach(va(0x4a89a0), {
    onEnter(args) {
      const sp = this.context.esp;
      this.a = sp.add(4).readFloat();
      this.b = sp.add(8).readFloat();
      const list = va(0xde0284).readPointer(), n = va(0xde0288).readS32();
      const v = [];
      for (let i = 0; i < n && !list.isNull(); i++) v.push([0, 4, 8].map((o) => list.add(0x18 * i + o).readFloat()));
      let lo = 0, hi = 0;
      for (const p of v) { lo = Math.min(lo, p[0]); hi = Math.max(hi, p[0]); }
      this.info = { n, lo, hi, bottom: v.filter((p) => (p[0] - lo) / (hi - lo) > 0.995).slice(0, 40) };
    },
    onLeave() {
      // The x87 result: read through a one-instruction stub would be cleaner; the caller stores it, so log the slot
      // later instead (`slot` records of probes/scaffold.js) and keep the inputs here.
      send({ k: 'bottom', a: this.a, b: this.b, ...this.info });
    },
  });
})();
