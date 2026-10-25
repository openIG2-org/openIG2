// Colony panel pass probe (re/screens/colony.md §11.10), appended to screen.js by run_screen.py
// --probe: on the colony screen's second `Update3D` (exe 0x52cd10 with `ColonyMasodikUpdate3d` 0x998247 set) logs
// the projection's inputs at entry and exit: `xcenter`/`ycenter` 0x919650/4, `screenaddx/y` 0xE03744/8, the focal
// `fov2` 0xE033E0, the depth offset 0x91971C (`-(fov2 / (z + off))` in the draw routine 0x52b3af), `zbm` 0x91965C.
(function () {
  let n = 0;
  const read = () => ({ xc: va(0x919650).readFloat(), yc: va(0x919654).readFloat(), addx: va(0xe03744).readFloat(), addy: va(0xe03748).readFloat(),
    fov: va(0xe033e0).readFloat(), off: va(0x91971c).readFloat(), zbm: va(0x91965c).readFloat(), hatar: [0xb9a980, 0xb9a984, 0xb9a988, 0xb9a98c].map((a) => va(a).readS32()) });
  Interceptor.attach(va(0x52cd10), {
    onEnter() {
      this.on = va(0x998247).readU8() !== 0 && n < 6;
      if (this.on) send({ k: 'pass2', at: 'enter', ...read() });
    },
    onLeave() {
      if (this.on) { n++; send({ k: 'pass2', at: 'leave', ...read() }); }
    },
  });
})();
// The pass's lit objects (`fenyekszamolasa` exe 0x52f190, as screen.js --light-probe): per object its sort record
// before the flags (`raw`, words 0..0x19: the geometry matrices and the camera-space point) and its vertex count,
// for the first two passes.
(function () {
  let calls = 0;
  Interceptor.attach(va(0x52f190), {
    onEnter(args) {
      this.on = va(0x998247).readU8() !== 0 && calls < 12;
      if (!this.on) return;
      calls++;
      const sp = args[3];
      send({ k: 'pass2obj', n: args[2].toInt32(), xvo: args[4].toString(), raw: Array.from({ length: 0x68 / 4 }, (_, i) => +sp.add(4 * i).readFloat().toFixed(4)) });
    },
  });
})();
