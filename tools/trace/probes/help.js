// Help screen probe (re/screens/help.md §6), appended to screen.js by run_screen.py --probe: every `FRISSIT_Help2`
// (exe 0x4d5150) and `GFX_Help` (exe 0x4d5540) call with `helpframe` (0xb49e7c), the flags `hlpELINDULT/BECSUKAS/
// KINYITVA` (0xb49e78/79/7b), `TopicSel` (0x90af94), `TopicActSorPtr` (0xb76a34) and the UI counter (0xb772a4).
(function () {
  let n = 0;
  const rec = (k) => {
    if (va(0xbae0fc).readS32() !== 1 || n > 3000) return;
    n++;
    send({ k, ui: va(0xb772a4).readS32(), frame: va(0xb49e7c).readFloat(), el: va(0xb49e78).readU8(), be: va(0xb49e79).readU8(),
      ki: va(0xb49e7b).readU8(), topic: va(0x90af94).readS32(), top: va(0xb76a34).readS32(), kep: va(0xbae0fc).readS32() });
  };
  Interceptor.attach(va(0x4d5150), { onLeave() { rec('hfr'); } });
  Interceptor.attach(va(0x4d5540), { onEnter() { rec('hgfx'); } });
})();
// `Help_Main` (exe 0x4d7510) while a button is down or just released: `AktIkon` (0xbae23c), `AktIkon2` (0xbae240),
// `Mouse_x/y` (0xdb4654/58), `Mouse_Bal` (0xdb4651), `Mouse_Lenyom` (0xdb465f), `Mouse_Megfogva` (0xdb466a),
// `HelpCsuszkaMode` (0xb49e7a), `TopicActSorPtr`, `SubTopicSel/Pos` (0x90afa4/a8).
(function () {
  let m = 0;
  Interceptor.attach(va(0x4d7510), {
    onEnter() {
      const bal = va(0xdb4651).readU8(), ik = va(0xbae23c).readS32();
      if ((!bal && ik === 0) || m > 400) return;
      m++;
      send({ k: 'hmain', ui: va(0xb772a4).readS32(), ik, ik2: va(0xbae240).readS32(), x: va(0xdb4654).readS32(), y: va(0xdb4658).readS32(), bal,
        le: va(0xdb465f).readU8(), meg: va(0xdb466a).readU8(), cs: va(0xb49e7a).readU8(), top: va(0xb76a34).readS32(),
        ss: va(0x90afa4).readS32(), sp: va(0x90afa8).readS32() });
    },
  });
})();
