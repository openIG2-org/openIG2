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
  Interceptor.attach(va(0x72d2f0), {
    onLeave(r) { if (!r.isNull()) send({ k: 'surf', w: r.add(8).readS32(), h: r.add(12).readS32(), pitch: r.add(16).readS32() }); },
  });
})();
