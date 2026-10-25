// AddToScene probe (re/screens/starmap.md §19.3), appended to screen.js by run_screen.py --probe:
// every `AddToScene(xvb, parent, s, id)` (exe 0x451720) with s in 0.02..0.2 logs the XVB's flag byte, its box
// `+4..+0xc` (the half-extents after `BoxUjraszamol`) and the words up to `+0x60` (`+0x4c` the root's scale).
(function () {
  Interceptor.attach(va(0x451720), {
    onEnter(args) {
      const x = args[0];
      const s = new Float32Array(new Uint32Array([args[2].toUInt32()]).buffer)[0];
      if (!(s > 0.02 && s < 0.2)) return;
      const words = [];
      for (let o = 0; o < 0x60; o += 4) words.push(x.add(o).readFloat());
      send({ k: 'addtoscene', s, xvb: x.toString(), flag: x.readU8(), box: [x.add(4).readFloat(), x.add(8).readFloat(), x.add(0xc).readFloat()], words });
    },
  });
})();
