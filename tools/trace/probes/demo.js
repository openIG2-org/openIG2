// Demo mode probe (re/screens/mainscr.md §9.5), appended to screen.js by run_screen.py --probe: every 200th
// `Fake_FRISSITES` (exe 0x4de720) `SelfPlaying` (0xbae267), `AktKep` (0xbae0fc), `TutorialActive` (0xf6915d),
// `EN` and the game's day counter as screen.js reads them, and the UI counter (0xb772a4).
(function () {
  let n = 0;
  Interceptor.attach(va(0x4de720), {
    onLeave() {
      if (++n % 200) return;
      send({ k: 'demo', n, self: va(0xbae267).readU8(), kep: va(0xbae0fc).readS32(), tut: va(0xf6915d).readU8(), ui: va(0xb772a4).readS32() });
    },
  });
})();
