// run_video.py --probe: the credits text quad (ig2screen::render exe 0x7370f0 -> FUN_007115e0) and its texture.
(function () {
  const ret = [va(0x7372e0), va(0x737390)];
  let n = 0;
  Interceptor.attach(va(0x7115e0), {
    onEnter(a) {
      if (n > 40) return;
      const f2 = (p) => [p.readFloat(), p.add(4).readFloat()];
      const tex = a[0].toInt32();
      send({ k: 'quad', n: n++, tex, pos: f2(a[1]), size: f2(a[2]), uv: f2(a[3]), uvs: f2(a[4]), rot: a[5].toInt32(), col: a[7].readU32().toString(16), from: this.returnAddress.sub(base).add(0x400000).toString(16) });
    },
  });
  // `RenderText` (exe 0x6e9130): its size arguments and the screen height `[0xfd1720]` they come from (H >> 5);
  // the GDI fallback (exe 0x6f0e90) when `GetFont` (0x6e84b0) finds no font for the empty name.
  let rt = 0;
  Interceptor.attach(va(0x6e9130), {
    onEnter(a) {
      if (rt++ > 4) return;
      send({ k: 'rendertext', H: va(0xfd1720).readS32(), W: va(0xfd171c).readS32(), args: [2, 3, 4, 5].map((i) => a[i].toInt32()), from: this.returnAddress.sub(base).add(0x400000).toString(16) });
    },
  });
  let gdi = 0;
  Interceptor.attach(va(0x6f0e90), {
    onEnter(a) { if (gdi++ < 4) send({ k: 'gdi', args: [1, 2, 3, 4].map((i) => a[i].toInt32()) }); },
  });
  Interceptor.attach(va(0x72d2f0), {
    onLeave(r) { if (!r.isNull()) send({ k: 'surf', w: r.add(8).readS32(), h: r.add(12).readS32(), pitch: r.add(16).readS32() }); },
  });
})();
