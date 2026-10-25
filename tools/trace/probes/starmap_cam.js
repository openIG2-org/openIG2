// Starmap camera probe (re/screens/starmap.md §2.4, §10.6, re/deviations.md 226), appended to screen.js by
// run_screen.py --probe: the camera's raw f32 bits before and after every `StarmapKeyboardDoModifies` (exe 0x5ae880)
// and `Starmap_Move` (exe 0x5bb230, cdecl (f32 dx, f32 dy)) call on the starmap (at most 3000), with the held flags
// (`KeyboardZoomKi`/`Be` 0xe75ae4/5, `PitchFel`/`Le` 0xe75ae6/7, `RotateBal`/`Jobb` 0xe75af0/1) and, after
// `Starmap_Main` (exe 0x5b2b70), `TargetXStart`/`YStart` (0xf16f1c/0xf16f20) with the pointer (`Mouse_x/y`).
// Camera: `XStart` 0xb9a968, `YStart` 0xb9a96c, `Rotate` 0xb9a970, `Pitch` 0xb9a974, `Zoom` 0xb9a978.
(function () {
  let n = 0;
  const hex = (a) => va(a).readU32().toString(16).padStart(8, '0');
  const cam = () => [0xb9a968, 0xb9a96c, 0xb9a970, 0xb9a974, 0xb9a978].map(hex);
  const flags = () => [0xe75ae4, 0xe75ae5, 0xe75ae6, 0xe75ae7, 0xe75af0, 0xe75af1].map((a) => va(a).readU8());
  const on = () => va(0xbae0fc).readS32() === 2 && n < 3000;
  Interceptor.attach(va(0x5ae880), {
    onEnter() { this.on = on(); if (this.on) this.before = cam(); },
    onLeave() { if (this.on) { n++; send({ k: 'mod', ui: va(0xb772a4).readS32(), flags: flags(), before: this.before, after: cam() }); } },
  });
  Interceptor.attach(va(0x5bb230), {
    onEnter(args) { this.on = on(); if (this.on) { this.before = cam(); this.d = [this.context.esp.add(4).readU32(), this.context.esp.add(8).readU32()].map((v) => v.toString(16).padStart(8, '0')); } },
    onLeave() { if (this.on) { n++; send({ k: 'move', ui: va(0xb772a4).readS32(), d: this.d, before: this.before, after: cam() }); } },
  });
  let lastT = null;
  Interceptor.attach(va(0x5b2b70), {
    onLeave() {
      if (!on()) return;
      const t = [hex(0xf16f1c), hex(0xf16f20)].join();
      if (t !== lastT) { lastT = t; n++; send({ k: 'target', ui: va(0xb772a4).readS32(), mouse: [va(0xdb4654).readS32(), va(0xdb4658).readS32()], t: [hex(0xf16f1c), hex(0xf16f20)] }); }
    },
  });
})();
