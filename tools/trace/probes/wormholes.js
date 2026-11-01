// One known wormhole pair staged on the starmap (re/screens/starmap.md §3.5), appended to screen.js by run_screen.py
// --probe (run_screendiff.py starmap --exe-arg=--probe --exe-arg=…/wormholes.js). Saves rarely hold a revealed pair,
// so on the first `Fake_FRISSITES` on the starmap this writes `OsszHanyWormhole` 0xbb61a0 = 1 and record 1 of
// `Wormhole` (0xbb60f8, 0x1c bytes): the ends 60 / 40 units either side of `AktPlanet`'s star (`Bolygok`
// 0xb85ea8 + 0x188·p, x/y at +0x18/+0x1c), both revealed (+8, +0x14), no fleet (+0xa, +0x16 = −1), tint
// (0x40, 0xc0, 0xff) at +0x18. The exe's save of the grabbed moment carries it to the client.
(function () {
  let staged = false;
  Interceptor.attach(A.Fake_FRISSITES, {
    onEnter() {
      if (staged || A.AktKep.readS32() !== 2) return;
      staged = true;
      const p = va(0xb85ea8 + 0x188 * Math.max(1, A.AktPlanet.readS32()));
      const x = p.add(0x18).readFloat(), y = p.add(0x1c).readFloat();
      const w = va(0xbb60f8 + 0x1c);
      w.writeFloat(x - 60); w.add(4).writeFloat(y - 40); w.add(8).writeU8(1); w.add(0xa).writeS16(-1);
      w.add(0xc).writeFloat(x + 60); w.add(0x10).writeFloat(y + 40); w.add(0x14).writeU8(1); w.add(0x16).writeS16(-1);
      w.add(0x18).writeU8(0x40); w.add(0x19).writeU8(0xc0); w.add(0x1a).writeU8(0xff);
      va(0xbb61a0).writeS32(1);
      send({ k: 'wormhole', frames, x, y });
    },
  });
})();
