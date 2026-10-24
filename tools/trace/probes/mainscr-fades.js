// Main menu fades probe (re/screens/mainscr.md §9.3, §13.2), appended to screen.js by run_screen.py --probe: after
// every `FRISSIT_MainScr2` (exe 0x4f65f0, the 10 ms tick) that changed one of them, logs the UI counter (0xB772A4),
// `ScenarioValasztasNyitottsag` (0xBBB2AC), `ScenarioKepNyitottsag` and its target (0xBBB048 / 0xBBB04C),
// `SelectedScenario` (0xBBB350), `Mode_Scenario_Valasztas` (0xBBB3F7), `MainKorokTavolsag` and its target
// (0xBBB050 / 0xBBB0BC), `MainScrKilepesValtozo` (0xBBB360) and `EnterNameMode` (0xBBB364).
(function () {
  const f32 = (x) => va(x).readFloat();
  let last = '';
  Interceptor.attach(va(0x4f65f0), {
    onLeave() {
      const r = { o: f32(0xbbb2ac), k: f32(0xbbb048), kc: f32(0xbbb04c), sel: va(0xbbb350).readS32(), scn: va(0xbbb3f7).readU8(),
        tav: f32(0xbbb050), cel: f32(0xbbb0bc), exit: f32(0xbbb360), name: va(0xbbb364).readU8() };
      const s = JSON.stringify(r);
      if (s !== last) {
        last = s;
        send(Object.assign({ k: 'fades', ui: va(0xb772a4).readS32() }, r));
      }
    },
  });
})();
