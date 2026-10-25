// Construction probe (deviation 366, re/screens/colony.md §3.4), appended to screen.js by run_screen.py --probe: the
// cut height `setvagashatar` (exe 0x4542e0: meshid, float) gets every frame from `ShowColony`, and every scaffolding
// box `ModifyAllvany` (exe 0x48b390, thiscall: models, slot k, level, cx, cy) places, with the record after the
// call (+8 the height, +0xc/+0x10 the cell's `Grid2Real`). The first 40 of each.
(function () {
  let cuts = 0, boxes = 0;
  Interceptor.attach(va(0x4542e0), {
    onEnter(args) {
      if (cuts++ >= 40) return;
      // The slots under construction of the shown planet (`Epuletek[AktPlanet]` exe 0xBA9E90, 32 bytes a slot: kind +0,
      // work left +8, bottom height +0x14) for the test (the loop runs past the planet's slots: only kinds 1..60 are kept).
      const p = A.AktPlanet.readS32(), base = va(0xba9e90).add(4 * p).readPointer();
      const slots = [];
      for (let k = 1; k < 100 && !base.isNull(); k++) {
        const r = base.add(0x20 * k), kind = r.readU8();
        if (kind >= 1 && kind <= 60 && r.add(8).readU16()) slots.push({ k, kind, left: r.add(8).readU16(), bottom: r.add(0x14).readFloat() });
      }
      send({ k: 'cut', id: args[0].toInt32(), h: this.context.esp.add(8).readFloat(), slots });
    },
  });
  Interceptor.attach(va(0x48b390), {
    onEnter(args) {
      this.rec = this.context.ecx;
      this.on = boxes++ < 40;
      if (this.on) this.a = [1, 2, 3, 4].map((i) => args[i].toInt32());
    },
    onLeave() {
      if (!this.on) return;
      send({ k: 'box', slot: this.a[0], level: this.a[1], cx: this.a[2], cy: this.a[3], rec: Array.from({ length: 16 }, (_, i) => +this.rec.add(4 * i).readFloat().toFixed(3)) });
    },
  });
})();
