// Race scene lighting probe (re/screens/mainscr.md §9.2, deviation 148), appended to screen.js by run_screen.py
// --probe with --script scripts/mainscr-race.json: every lit object of the frames at script ticks 1000..1001 (the
// race scene spread, before the boxes) through `fenyekszamolasa` (exe 0x52f190, as --light-probe): its vertex count,
// rotation chain (sort record +0x24), flags, lights (type, colour word, vector +0x28) and per vertex the normal and
// colour it got. Records {k: 'light'} go to --jsonl.
(function () {
  const seen = new Set();
  Interceptor.attach(va(0x52f190), {
    onEnter(args) {
      this.on = scriptT !== null && scriptT >= 1000 && scriptT <= 1001;
      if (!this.on) return;
      const key = args[4].toString() + ':' + args[2].toInt32();
      if (seen.has(key)) { this.on = false; return; }
      seen.add(key);
      this.pos = args[0]; this.vn = args[1]; this.n = args[2].toInt32(); this.sp = args[3]; this.xvo = args[4];
    },
    onLeave() {
      if (!this.on) return;
      const sp = this.sp, m = [];
      for (let i = 0; i < 9; i++) m.push(sp.add(0x24 + 4 * i).readFloat());
      const nl = sp.add(0x70).readS32(), lights = [];
      for (let i = 0; i < nl && i < 16; i++) {
        const l = sp.add(0x6c).readPointer().add(4 * i).readPointer();
        lights.push({ type: l.readS32(), color: l.add(4).readU32(), vec: [0x28, 0x2c, 0x30].map((o) => l.add(o).readFloat()) });
      }
      const verts = [];
      for (let i = 0; i < this.n && i < 800; i++) {
        const v = this.vn.add(0x14 * i);
        verts.push([v.readFloat(), v.add(4).readFloat(), v.add(8).readFloat(), v.add(0xc).readU32()]);
      }
      const pos = Array.from({ length: 48 }, (_, i) => +this.pos.add(4 * i).readFloat().toFixed(3));
      send({ k: 'light', t: scriptT, xvo: this.xvo.toString(), n: this.n, m, flags: sp.add(0x68).readU32(), lights, verts, pos, raw: Array.from({ length: 0x24 / 4 }, (_, i) => +sp.add(4 * i).readFloat().toFixed(4)) });
    },
  });
})();
