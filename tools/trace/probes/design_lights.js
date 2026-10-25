// Design lighting probe (deviation 114, re/screens/design.md §14.4), appended to screen.js by run_screen.py --probe
// with a design script (scripts/design-pods.json marks a tech model from t 345): every `fenyekszamolasa` (exe
// 0x52f190; args: …, normals[] (stride 0x14: normal +0, colour +0xc), n, sort record, XVO) on the design screen is
// read after the call: the record's properties (`+0x68`), its lights (`+0x6c` list, `+0x70` count: type, colour,
// the stored vector) and the first vertices' normals and colours. The last record of every XVO is kept; when an
// object carries the marked tech's ambient 0x0e0e86 its record is sent with the same XVO's last unmarked one (up to
// 10 marked records); from the first marked one on, every object's light colours for 120 calls (`dall`).
(function () {
  const last = new Map();
  let sent = 0, after = -1;
  Interceptor.attach(va(0x52f190), {
    onEnter(args) {
      this.on = A.AktKep.readS32() === 5 && (sent < 10 || (after >= 0 && after < 120));
      if (!this.on) return;
      const sp = args[3], nl = Math.min(sp.add(0x70).readS32(), 15), lights = [];
      for (let i = 0; i < nl; i++) {
        const l = sp.add(0x6c).readPointer().add(4 * i).readPointer();
        lights.push([l.readS32(), '0x' + l.add(4).readU32().toString(16), [0x28, 0x2c, 0x30].map((o) => +l.add(o).readFloat().toFixed(3))]);
      }
      this.rec = { k: 'dlight', xvo: args[4].toString(), props: '0x' + sp.add(0x68).readU32().toString(16), n: args[2].toInt32(), lights };
      this.vn = args[1];
    },
    onLeave() {
      if (!this.on) return;
      const v = [];
      for (let i = 0; i < Math.min(this.rec.n, 8); i++) {
        const p = this.vn.add(0x14 * i);
        v.push([+p.readFloat().toFixed(3), +p.add(4).readFloat().toFixed(3), +p.add(8).readFloat().toFixed(3), '0x' + p.add(0xc).readU32().toString(16)]);
      }
      this.rec.verts = v;
      const marked = this.rec.lights.some((l) => l[1] === '0xe0e86');
      if (after >= 0 && after < 120) {
        after++;
        send({ k: 'dall', xvo: this.rec.xvo, n: this.rec.n, props: this.rec.props, lights: this.rec.lights.map((l) => l[1]), v0: this.rec.verts.slice(0, 2) });
      }
      if (marked && sent < 10) {
        sent++;
        if (after < 0) after = 0;
        send({ ...this.rec, marked: true, before: last.get(this.rec.xvo) || null });
      } else if (!marked) {
        last.set(this.rec.xvo, this.rec);
      }
    },
  });
})();
