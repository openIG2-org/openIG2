// Main menu input probe (re/screens/mainscr.md §6, deviations rows 285-290), appended to screen.js by run_screen.py
// --probe: on every `MainScr_Main` (exe 0x4fefc0) call and after every `FRISSIT_MainScr2` (0x4f65f0) tick that changed
// one of them, logs the UI counter (0xB772A4), `AktIkon` / `AktIkon2` (0xBAE23C / 0xBAE240), `Mouse_Lenyom`
// (0xDB465F), `Mouse_Felenged` (0xDB465C), `Mouse_Bal` (0xDB4651), `Mouse_FelengedesMajdMegvar` (0xDB4669), the
// pointer (0xDB4654 / 0xDB4658), `MainKorokTavolsag` and its target (0xBBB050 / 0xBBB0BC), `Mode_Main` (0xBBB3D1),
// `Mode_Tutorial_Valasztas` (0xBBB3F4), `Mode_Scenario_Valasztas` (0xBBB3F7), `ScenarioHighScoreMode` (0xBBB22E),
// `EnterNameMode` (0xBBB364), `IOMode` (0xBAE2C3) and `AktKep` (0xBAE0FC). `ScenarioGombraRanyomtak` (0x503bf0) and
// `MainScr_ESCPressedWhenMultiplayerselect` (0x4fe780) / `MainScr_CancelIPEnter` (0x4fe6f0) calls are logged too.
(function () {
  const f32 = (x) => va(x).readFloat();
  const u8 = (x) => va(x).readU8();
  const s32 = (x) => va(x).readS32();
  const state = () => ({ ai: s32(0xbae23c), ai2: s32(0xbae240), len: u8(0xdb465f), fel: u8(0xdb465c), bal: u8(0xdb4651),
    wait: u8(0xdb4669), x: s32(0xdb4654), y: s32(0xdb4658), tav: f32(0xbbb050), cel: f32(0xbbb0bc), main: u8(0xbbb3d1),
    tut: u8(0xbbb3f4), scn: u8(0xbbb3f7), hs: u8(0xbbb22e), name: u8(0xbbb364), io: u8(0xbae2c3), kep: s32(0xbae0fc) });
  let last = '';
  Interceptor.attach(va(0x4fefc0), {
    onEnter() { send(Object.assign({ k: 'main', ui: s32(0xb772a4) }, state())); },
  });
  Interceptor.attach(va(0x4f65f0), {
    onLeave() {
      const r = state();
      const s = JSON.stringify(r);
      if (s !== last) {
        last = s;
        send(Object.assign({ k: 'tick', ui: s32(0xb772a4) }, r));
      }
    },
  });
  for (const [a, n] of [[0x503bf0, 'scenario'], [0x4fe780, 'esc'], [0x4fe6f0, 'cancelio']]) {
    Interceptor.attach(va(a), { onEnter() { send(Object.assign({ k: n, ui: s32(0xb772a4) }, state())); } });
  }
})();
// Row 286: which call moves `Cel` (0xBBB0BC) after the scenario ring's press: `Cel` on entry and exit of
// `Mainscreen_Update3D` (0x500270), `MainScr_Main` and `FRISSIT_MainScr2`, whenever it changed across the call.
(function () {
  for (const [a, n] of [[0x500270, 'u3d'], [0x4fefc0, 'mainx'], [0x4f65f0, 'frissit']]) {
    Interceptor.attach(va(a), {
      onEnter() { this.c = va(0xbbb0bc).readFloat(); },
      onLeave() {
        const c = va(0xbbb0bc).readFloat();
        if (c !== this.c) send({ k: 'cel', fn: n, ui: va(0xb772a4).readS32(), from: this.c, to: c, tav: va(0xbbb050).readFloat() });
      },
    });
  }
})();
