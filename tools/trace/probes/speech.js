// Speech probe (re/screens/audio.md §5), appended to screen.js by run_screen.py --probe; use on the starmap (--kep 2)
// with --sounds, which logs every `Speech::Play`/`PlayMessagesHang`/`PlaySound`. Counting the UI ticks
// (`Fake_FRISSITES` exe 0x4de720) while `AktKep` (0xBAE0FC) is 2, it calls the exe's own functions:
// - `SpeakingDirector::Play(&StarmapSpeaker 0xF17D88, line, kind)` (exe 0x563cd0, thiscall) at ticks 50 (1, 1), 51
//   (1, 1) again (on cooldown: silent), 52 (2, 0), 53 (3, 2), then (1, 1) every 10 ticks from 60 to 200: the first
//   one off cooldown shows the refresh;
// - `PlayMessagesHang(name, 0)` (exe 0x55c9b0) at 250 for the human's `EN` (0x912A3C) and at 1700 with `EN` set to 1
//   for the call (`0011_01.ogg`, a file of folder 1), logging the duck target (0xE4166C), its level (0xE41678), the
//   music volume (0x919928) and each tick `MessagesHangJatszikMeg` (exe 0x55c890) until it turns false;
// - `PlayHalkAlienHang` (exe 0x509120) at 1300 with `AnimFile` (0xD909C0) = "messages/pirate/probe" and `EN` 1, at
//   1310 with `UzenetFaj` (0xD90DDC) = 17, at 1320 with "misc/probe" and `UzenetekRandomKitol[UAktUzenet]` = 5;
//   the globals are put back after each call; the `Mes_Hangok` record whose handle changed is the one played. The
//   probe runs in a replacement of `Fake_FRISSITES`: sound.js's hooks don't see these calls, so it reads state.
(function () {
  const play = new NativeFunction(va(0x563cd0), 'void', ['pointer', 'int', 'int'], 'thiscall');
  const mesHang = new NativeFunction(va(0x55c9b0), 'void', ['pointer', 'int']);
  const jatszik = new NativeFunction(va(0x55c890), 'uint8', []);
  const alien = new NativeFunction(va(0x509120), 'void', []);
  const director = va(0xf17d88);
  const EN = va(0x912a3c), ANIM = va(0xd909c0), FAJ = va(0xd90ddc), KITOL = va(0xdabc10), UAKT = va(0xdabdc0);
  let n = 0, polling = null;
  const ticks = new NativeFunction(va(0x689e80), 'uint32', []);
  const duck = () => ({ ms: ticks(), t0: va(0x91a800).readU32(), handle: va(0x91a7e8).readS32(), ui: va(0xb772a4).readS32(), duck: va(0xe4166c).readS32(), duckReal: va(0xe41678).readFloat(), m: va(0x919928).readS32() });
  const withEn = (en, f) => { const old = EN.readS32(); EN.writeS32(en); try { f(); } finally { EN.writeS32(old); } };
  const speak = (line, kind) => { send({ k: 'speechprobe', n, call: 'Play', line, kind, en: EN.readS32() }); play(director, line, kind); };
  const hang = (name, en) => {
    const s = Memory.allocUtf8String(name);
    const f = () => mesHang(s, 0);
    if (en) withEn(en, f); else f();
    polling = { name, from: n };
    send({ k: 'speechprobe', n, call: 'PlayMessagesHang', name, en: en || EN.readS32(), ...duck() });
  };
  const halk = (anim, faj, kitol) => {
    const oldAnim = ANIM.readByteArray(64), oldFaj = FAJ.readS32(), k = KITOL.add(4 * UAKT.readS32()), oldK = k.readS32();
    ANIM.writeUtf8String(anim); FAJ.writeS32(faj); k.writeS32(kitol);
    send({ k: 'speechprobe', n, call: 'PlayHalkAlienHang', anim, faj, kitol });
    // `Mes_Hangok[i]+0x100` (exe 0x914950 + 0x11c i): the handle `PlaySound` gives a record; the one that changes.
    const handles = () => [...Array(19).keys()].map((i) => va(0x914950 + 0x11c * i + 0x100).readS32());
    const before = handles();
    try { withEn(1, () => alien()); } finally { ANIM.writeByteArray(oldAnim); FAJ.writeS32(oldFaj); k.writeS32(oldK); }
    const after = handles();
    send({ k: 'speechprobe', n, call: 'alienPlayed', mes: after.map((h, i) => (h !== before[i] ? i : -1)).filter((i) => i >= 0), after: after.slice(3) });
  };
  // The set (speaker 1, kind 1)'s cooldown: `SpeakingDirector` -> `Speaker` {+0 sets, +4 id, +8 next} -> `Speeches`
  // {+0 count, +4 list, +8 cooldown, +0xc kind, +0x10 next}.
  const cooldown = (speaker, kind) => {
    for (let sp = director.readPointer(); !sp.isNull(); sp = sp.add(8).readPointer()) {
      if (sp.add(4).readS32() !== speaker) continue;
      for (let s = sp.readPointer(); !s.isNull(); s = s.add(0x10).readPointer()) if (s.add(0xc).readS32() === kind) return s.add(8).readS32();
    }
    return null;
  };
  // A replacement, not a listener: the calls made from a listener's callback don't reach sound.js's hooks.
  const fake = new NativeFunction(va(0x4de720), 'void', []);
  Interceptor.replace(va(0x4de720), new NativeCallback(() => {
    fake();
    tick();
  }, 'void', []));
  function tick() {
    {
      if (va(0xbae0fc).readS32() !== 2) return;
      if (n >= 49 && n <= 210) send({ k: 'speechprobe', n, call: 'cooldown', ui: va(0xb772a4).readS32(), cd: cooldown(1, 1) });
      n++;
      if (n === 50 || n === 51) speak(1, 1);
      if (n === 52) speak(2, 0);
      if (n === 53) speak(3, 2);
      if (n >= 60 && n <= 200 && n % 10 === 0) speak(1, 1);
      if (n === 250) hang('0011_01.ogg', 0);
      if (n === 1700) hang('0011_01.ogg', 1);
      if (polling) {
        const on = jatszik() & 1;
        if ((n - polling.from) % 50 === 0 || !on) send({ k: 'speechprobe', n, call: 'poll', name: polling.name, on, ...duck() });
        if (!on) polling = null;
      }
      if (n === 1300) halk('messages/pirate/probe', 0, 0);
      if (n === 1310) halk('messages/trader/probe', 17, 0);
      if (n === 1320) halk('misc/probe', 0, 5);
    }
  }
})();
