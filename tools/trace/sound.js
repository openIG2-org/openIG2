// Sound-id tracer (re/screens/sound.md): every sound ig2.exe starts, with the UI frame, the drawn frame and the input
// event before it, so a screen's input script yields its (action → sound) table. Appended to screen.js by
// run_screen.py --sounds OUT.jsonl (it uses screen.js's `va`, `A`, `cfg`, `frames` and the script clock `scriptT`);
// the records {k: 'sound' | 'input'} go to OUT.jsonl.
//
// - `PlaySound(Hang*, bool raw)` exe 0x55cd20 (ELF 0x671070): every effect; `raw` 0 prefixes "sounds/". The `Hang`
//   record (0x11c B): name +0, voice handles +0x100/+0x108/+0x10c, loop +0x104, volume % +0x110, load on demand
//   +0x114. The tables come from sound_tables.py (`<Screen>_Hangok`, in cfg.soundTables): table + index = the id.
// - `PlaySoundGlobal(id)` exe 0x55d0b0 = `PlaySound(&Global_Hangok[id], 0)` (Global_Hangok exe 0x919a60).
// - `PlayWav(name, loop)` 0x563e60, `PlayMp2` 0x563df0, `Speech::Play` 0x563d50, `PlayMessagesHang(name, bool)`
//   0x55c9b0: voices and message audio (names as given).
// - Input: `MainCallBackEventHandler` exe 0x4e2db0 (the SDL2 event filter): 0x300/0x301 key down/up (keysym +0x10:
//   scancode, +0x14 sym), 0x401/0x402 button down/up (button +0xc, window x/y +0x10/+0x14); screen.js's injected
//   events report through `sndInput` (Frida doesn't run our hooks inside our own NativeFunction calls).
// `SOUND` (exe 0x919930) clear means PlaySound returns at once; the record says so (`on`).
const SND = { PlaySound: 0x55cd20, PlaySoundGlobal: 0x55d0b0, PlayWav: 0x563e60, PlayMp2: 0x563df0, SpeechPlay: 0x563d50,
  PlayMessagesHang: 0x55c9b0, Handler: 0x4e2db0, UiFrame: 0xb772a4, SoundOn: 0x919930, MouseX: 0xdb4654, MouseY: 0xdb4658 };
const SND_REC = 0x11c;
let sndLast = null, sndGlobal = null, sndCount = 0;

function sndNow() {
  return { ui: va(SND.UiFrame).readS32(), frame: frames, t: typeof scriptT === 'number' ? scriptT : null, kep: A.AktKep.readS32() };
}

// A sound is put down to the last press, key or move before it (a release only reports itself).
function sndInput(ev) {
  const rec = Object.assign(sndNow(), ev);
  if (ev.ev !== 'up' && ev.ev !== 'keyup') sndLast = rec;
  send({ k: 'input', ...rec });
}

function sndId(p) {
  const tabs = (cfg && cfg.soundTables) || {};
  for (const name in tabs) {
    const t = tabs[name];
    const off = p.sub(va(t.va)).toInt32();
    if (off >= 0 && off < t.n * SND_REC && off % SND_REC === 0) return [name, off / SND_REC];
  }
  return [null, null];
}

function sndRecord(fn, extra) {
  const now = sndNow();
  const rec = { k: 'sound', n: sndCount++, fn, ...now, ...extra, on: va(SND.SoundOn).readU8() };
  if (sndLast) rec.input = sndLast, rec.dui = now.ui - sndLast.ui;
  send(rec);
}

const sndStr = (p) => { try { return p.isNull() ? null : p.readCString(); } catch (e) { return null; } };

function hookSounds() {
  Interceptor.attach(va(SND.PlaySoundGlobal), {
    onEnter(args) { sndGlobal = args[0].toInt32(); },
    onLeave() { sndGlobal = null; },
  });
  Interceptor.attach(va(SND.PlaySound), {
    onEnter(args) {
      const h = args[0];
      const [table, idx] = sndId(h);
      sndRecord(sndGlobal !== null ? 'PlaySoundGlobal' : 'PlaySound', { table, idx, global: sndGlobal, name: sndStr(h), raw: args[1].toInt32() & 0xff,
        loop: h.add(0x104).readS32(), vol: h.add(0x110).readS32() });
    },
  });
  Interceptor.attach(va(SND.PlayWav), { onEnter(args) { sndRecord('PlayWav', { name: sndStr(args[0]), loop: args[1].toInt32() }); } });
  Interceptor.attach(va(SND.PlayMp2), { onEnter(args) { sndRecord('PlayMp2', { name: sndStr(args[0]), arg1: args[1].toInt32() }); } });
  // `Speech::Play`: its arguments aren't pinned yet; the first two stack words as given.
  Interceptor.attach(va(SND.SpeechPlay), { onEnter(args) { sndRecord('Speech::Play', { name: sndStr(args[0]), arg1: args[1].toInt32(), ecx: this.context.ecx.toString() }); } });
  Interceptor.attach(va(SND.PlayMessagesHang), { onEnter(args) { sndRecord('PlayMessagesHang', { name: sndStr(args[0]), arg1: args[1].toInt32() & 0xff }); } });
  Interceptor.attach(va(SND.Handler), {
    onEnter(args) {
      const e = args[0], type = e.readU32();
      if (type === 0x300 || type === 0x301) {
        sndInput({ src: 'sdl', ev: type === 0x300 ? 'keydown' : 'keyup', scancode: e.add(0x10).readS32(), sym: e.add(0x14).readS32() });
      } else if (type === 0x401 || type === 0x402) {
        sndInput({ src: 'sdl', ev: type === 0x401 ? 'down' : 'up', button: e.add(0xc).readU8(), win: [e.add(0x10).readS32(), e.add(0x14).readS32()],
          at: [va(SND.MouseX).readS32(), va(SND.MouseY).readS32()] });
      }
    },
  });
}
