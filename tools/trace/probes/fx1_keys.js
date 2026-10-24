// FX1 keyboard probe (re/screens/input.md §1), appended to screen.js by run_screen.py --probe: after every
// `DoKeyboard` (exe 0x4db1a0) that leaves a screen change asked for, and every 50th UI tick, the state its F-key
// gates read: `AktKep` 0xbae0fc, `AktKepKell` 0xbae250, `Belep`/`Atlepes` 0xbae24c/0xbae24d, `KepernyokLetiltva`
// 0xf66f2c[20], `DEBUGX` 0xbae587, the UI counter 0xb772a4.
(function () {
  const st = () => ({ ui: va(0xb772a4).readS32(), kep: va(0xbae0fc).readS32(), kell: va(0xbae250).readS32(),
    belep: va(0xbae24c).readU8(), atlep: va(0xbae24d).readU8(), debugx: va(0xbae587).readU8(),
    locks: Array.from(new Uint8Array(va(0xf66f2c).readByteArray(20))).join('') });
  Interceptor.attach(va(0x4db1a0), {
    onEnter() { this.before = va(0xbae250).readS32(); },
    onLeave() { const s = st(); if (s.kell !== this.before) send(Object.assign({ k: 'fx1kell' }, s)); },
  });
  Interceptor.attach(va(0x4de720), {
    onLeave() { const s = st(); if (s.ui % 50 === 0) send(Object.assign({ k: 'fx1state' }, s)); },
  });
})();
