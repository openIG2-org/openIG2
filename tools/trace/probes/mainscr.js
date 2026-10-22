// Main menu probe (re/screens/mainscr.md), appended to screen.js by run_screen.py --probe: while `AktKep` is 15,
// every 50th `Mainscreen_Update3D` (exe 0x500270) logs the camera (`forgaspont` 0xDE0130, `kamerapont` 0xDE0148,
// `elforgatas` 0xDE0158 and the two angles after it), `fov`, the centre (0x919650/54), the clocks (`MainScrMozgato`
// 0xBBABE0, `MainKorokTavolsag` 0xBBB050, `GlobalTransparency` 0xBBB0CC, `MaxMainKorok` 0xBBB0C0), the rings'
// positions (`MainKorok` 0xD84370, 0x1c each, x y z at +0x10) and the ring centres `TransformPoint` (exe 0x52cc90)
// gives during that call, the meteors (0xBBABE4, speed 0xBBAC44), the ships (pos 0xBBAC60, speed 0xBBADE0), the timers and
// the UI counter (0xB772A4); also the first 12 calls.
(function () {
  const f32 = (x) => va(x).readFloat();
  let calls = 0, log = false, points = [];
  Interceptor.attach(va(0x52cc90), {
    onEnter(args) { if (log) { this.i = args[0]; this.o = args[1]; } },
    onLeave() {
      if (!log || !this.i) return;
      points.push([this.i.readFloat(), this.i.add(4).readFloat(), this.i.add(8).readFloat(), this.o.readFloat(), this.o.add(4).readFloat()]);
    },
  });
  Interceptor.attach(va(0x500270), {
    onEnter() {
      if (va(0xbae0fc).readS32() !== 15) return;
      calls++;
      log = calls <= 12 || calls % 50 === 0;
      points = [];
    },
    onLeave() {
      if (!log) return;
      log = false;
      const rings = [];
      for (let i = 0; i < 6; i++) rings.push([0x10, 0x14, 0x18].map((o) => f32(0xd84370 + 0x1c * i + o)));
      send({ k: 'mainprobe', calls, cam: [0, 4, 8].map((o) => f32(0xde0148 + o)), pivot: [0, 4, 8].map((o) => f32(0xde0130 + o)),
        angles: [0, 4, 8].map((o) => f32(0xde0158 + o)), centre: [f32(0x919650), f32(0x919654)],
        m: f32(0xbbabe0), tav: f32(0xbbb050), gt: f32(0xbbb0cc), n: va(0xbbb0c0).readS32(), ui: va(0xb772a4).readS32(), rings, points: points.slice(0, 12),
        meteors: [0, 1, 2].map((i) => [0, 4, 8].map((o) => f32(0xbbabe4 + 0x10 * i + o)).concat([f32(0xbbac44 + 4 * i)])),
        ships: [0, 1, 2, 3, 4, 5, 6, 7].map((i) => [0, 4, 8].map((o) => f32(0xbbac60 + 0x10 * i + o)).concat([0, 4, 8].map((o) => f32(0xbbade0 + 0x10 * i + o)))),
        timers: [va(0xbbb0c8).readS32(), va(0xbbb0d0).readS32()],
        // The first particles of the four galaxy clouds (`ChangeGalaxyColors`' arrays, stride 0x4c: x y z, colour +0xc, size +0x14).
        clouds: calls === 1 ? [0xbbd5d0, 0xbfe520, 0xc3f470, 0xc803c0].map((b) => [0, 1, 2].map((k) => [0, 4, 8, 0x14].map((o) => f32(b + 0x4c * k + o)))) : null });
    },
  });
})();
