// Design screen probe (re/screens/design.md §5.1, re/deviations.md 92/95), appended to screen.js by run_screen.py
// --probe: every `Design3DUpdate` (exe 0x4ad8e0) and `Design_Main` (exe 0x4b2190) call (at most 2500)
// with `AddZoomDes` (0xa3f10c), `AddZoomDirDes` (0xa3f6e4), `AktAlap`/`OldAlap` (0xa3f038/0xa3f03c),
// `NemkellTexturaCsere` (0xa3f6ec), `AktTerv` (0xb9aa64), `AktTervMutato` (0xb9aaec), `DesignTervezesMod`
// (0xa3f0b4), `AktIkon2` (0xbae240), `Mouse_Lenyom`/`Felenged` (0xdb465f/0xdb465c), the scratch's pod counts 1..4 (0xb9aac6..) and the UI counter
// (0xb772a4).
(function () {
  let n = 0;
  const rec = (k) => {
    const ui = va(0xb772a4).readS32();
    if (va(0xbae0fc).readS32() !== 5 || n > 2500) return;
    n++;
    send({ k, ui, az: va(0xa3f10c).readFloat(), dir: va(0xa3f6e4).readS32(), akt: va(0xa3f038).readS32(), old: va(0xa3f03c).readS32(),
      ntx: va(0xa3f6ec).readU8(), terv: va(0xb9aa64).readS32(), mut: va(0xb9aaec).readS32(), mod: va(0xa3f0b4).readS32(),
      ik2: va(0xbae240).readS32(), le: va(0xdb465f).readU8(), fe: va(0xdb465c).readU8(),
      pods: [1, 2, 3, 4].map((k) => va(0xb9aac5 + k).readU8()) });
  };
  Interceptor.attach(va(0x4ad8e0), { onEnter() { rec('d3d'); } });
  Interceptor.attach(va(0x4b2190), { onEnter() { rec('dmain'); } });
})();
