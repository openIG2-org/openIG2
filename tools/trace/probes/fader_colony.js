// Fader design probe for the colony screen (AktKep 3; re/screens/hud.md §1.2.1, colony.md §12.3, deviation 139), a
// copy of probes/fader.js logging every 20th UI tick. Fader design probe (re/screens/hud.md §1.2.1), appended to screen.js by run_screen.py --probe: remembers every
// `faderstruktura` `InitFaderDesignStruktura` (exe 0x4d3590) sets up; after every `Fake_FRISSITES` (exe 0x4de720)
// while `AktKep` (0xbae0fc) is `FADER_KEP` (13, Options) logs the last one's faders 0..7: the glow `+0x3e0`, its
// style `+0x458`, `s` `+0x168`, the shine `+0xf0`, its ticks left `+0x78`, the wait `+0x0`, the texture offsets
// `+0x1e0`/`+0x258`; and `+0x4d0` (the last entered), the clock `+0x4d8`, the UI counter (0xb772a4).
(function () {
  const KEP = 3;
  let base = null;
  Interceptor.attach(va(0x4d3590), {
    onEnter(args) { base = args[0]; send({ k: 'faderinit', at: base.toString(), kep: va(0xbae0fc).readS32() }); },
  });
  Interceptor.attach(va(0x4de720), {
    onLeave() {
      if (base === null || va(0xbae0fc).readS32() !== KEP || va(0xb772a4).readS32() % 20 !== 0) return;
      const f = (o, i) => base.add(o + 4 * i).readFloat();
      const faders = [];
      for (let i = 0; i < 8; i++) {
        faders.push([f(0x3e0, i), base.add(0x458 + 4 * i).readS32(), f(0x168, i), f(0xf0, i), f(0x78, i), f(0x0, i),
          f(0x1e0, i), f(0x258, i)]);
      }
      send({ k: 'fader', ui: va(0xb772a4).readS32(), last: base.add(0x4d0).readS32(), clock: base.add(0x4d8).readFloat(), faders });
    },
  });
})();
