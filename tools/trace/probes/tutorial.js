// Tutorial overlay probe (re/rules/tutorial.md §2 "Drawing"), appended to screen.js by run_screen.py --probe with
// --script (the clock one UI tick a frame): at the script's first frame on --kep it calls `TutorialStart("main", N)`
// (exe 0x611f80, cdecl; the chapter is run_screen.py --tutorial N, written into this file) over the loaded
// save, then after every `TutorialFrissit` (exe 0x60c020) logs {k: 'tutwin', t, ...} with what the overlay draws:
// `TKActX/Y` 0xf673b8/bc (f32), `TKTomb` 0xf67130 (8 × 40 × (char, colour)), `TKTombSorPtr` 0xf6712c, `TKQueueLen`
// 0xf67f80, `TutorialSzinVillogtato` 0xf6813c, `TutorialActive` 0xf6915d, `TutorialPaused` 0xf66f2a, `TextShown`
// 0x928ea8, `DisableTextWindow` 0xf6712e, `Mouse_x/y` 0xdb4654/58, `Mouse_Tipus` 0xdb9414, the UI stamp 0xbae278.
(function () {
  const chapter = TUT_CHAPTER;
  let started = false;
  const start = new NativeFunction(va(0x611f80), 'void', ['pointer', 'int'], 'mscdecl');
  const name = Memory.allocAnsiString('main');
  Interceptor.attach(va(0x60c020), {
    onEnter() {
      if (started || scriptT === null) return;
      started = true;
      start(name, chapter);
      send({ k: 'tutstart', chapter, t: scriptT });
    },
    onLeave() {
      if (!started || scriptT === null) return;
      const grid = Array.from(new Uint8Array(va(0xf67130).readByteArray(0x280)));
      send({
        k: 'tutwin', t: va(0xb772a4).readS32() - scriptUi0,
        x: va(0xf673b8).readFloat(), y: va(0xf673bc).readFloat(), grid, column: va(0xf6712c).readS8(),
        queue: va(0xf67f80).readS32(), blink: va(0xf6813c).readS32(), active: va(0xf6915d).readU8(),
        paused: va(0xf66f2a).readU8(), shown: va(0x928ea8).readU8(), disabled: va(0xf6712e).readU8(),
        mouse: [va(0xdb4654).readS32(), va(0xdb4658).readS32()], tipus: va(0xdb9414).readS32(),
        stamp: va(0xbae278).readU32(), vt,
      });
    },
  });
})();
