// Info screen probe (re/screens/info.md §5), appended to screen.js by run_screen.py --probe. While `AktKep`
// is 9: every `Info_MouseInit2` (exe 0x4e9530, once a drawn frame) first gives `MainScenarioDescriptor` (0xBBD140) a
// dummy record when it is 0 (a save outside a scenario has none, and the charts and star-log buttons 14/15 are off
// without one), and logs ('f') and every `Info_Main` (exe 0x4e81c0, run by `IG2_gameUpdate` only on button frames)
// logs ('m'): the UI counter (0xB772A4), `AktIkon2` (0xBAE240), `Mouse_Megfogva` (0xDB466A), `Mouse_y` (0xDB4658),
// `BolygoFlottaPointer` (0xB82984) / `HanyBolygoFlotta` (0xB82988), `StarLogPointer` (0xB84C10) /
// `StarlogHanysorfeltoltve` (0xB84BF4), `Info_StarLogMode` (0xB79FB3 = 0 star log) and the button animation:
// `InfoGombNew` 0xB85310, `InfoGombFrameCsokkent` 0xB85311 (bytes), `InfoMelyikGombot` 0xB85314, `InfoGombFrame`
// 0xB8531C.
(function () {
  const dummy = Memory.alloc(0x1000);
  const rec = (k) => {
    if (va(0xbae0fc).readS32() !== 9) return;
    send({ k, ui: va(0xb772a4).readS32(), ikon2: va(0xbae240).readS32(), megf: va(0xdb466a).readU8(),
      my: va(0xdb4658).readS32(), ptr: va(0xb82984).readS32(), hany: va(0xb82988).readS32(),
      log: va(0xb84c10).readS32(), filled: va(0xb84bf4).readS32(), chartmode: va(0xb79fb3).readU8(),
      gnew: va(0xb85310).readU8(), csok: va(0xb85311).readU8(), mel: va(0xb85314).readS32(), frame: va(0xb8531c).readS32() });
  };
  Interceptor.attach(va(0x4e9530), {
    onEnter() {
      if (va(0xbae0fc).readS32() === 9 && va(0xbbd140).readU32() === 0) va(0xbbd140).writePointer(dummy);
      rec('f');
    },
  });
  Interceptor.attach(va(0x4e81c0), { onLeave() { rec('m'); } });
})();
