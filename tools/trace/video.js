// Video capture (re/screens/video.md): ig2.exe's title video read back frame-exactly. `SDL_GetTicks` (exe 0x689e80)
// is replaced by a clock that moves 10 ms a `SwapBuffers`, so the video's time at a swap is 10 ms × the swaps since
// `videoentity::play` (exe 0x70f870 stores its start in entity +0x2a8). Grabs at the given video times (ms) are read
// back with glReadPixels at the swap. Logs `OpenBink`/`PlayBink`/`CloseBink`; a key or a mouse button can be pressed
// and released at given video times (posted to the window: SDL turns them into events for the skip filter).
// `cfg.open` (the message clips, re/screens/video.md §5): the title is skipped (`VoltGTLogo`) and, at the given
// call of `Fake_FRISSITES` (exe 0x4de720, the game thread's frame), `OpenBink(name, fullscreen, x, y, force)` and
// `PlayBink(0, loop, 0)` are called as `StartAnim`/`MessagesAnimFrissit` do.
'use strict';

const base = Process.getModuleByName('ig2.exe').base;
const va = (x) => base.add(x - 0x400000);

let cfg = null;
let finished = false, delays = 0, vt = 0, vtOn = false, start = null, swaps = 0, grabs = [], closed = false;
const realTicks = new NativeFunction(va(0x689e80), 'uint32', [], { abi: 'mscdecl' });
const now = () => (vtOn ? vt >>> 0 : realTicks());
const elapsed = () => (start === null ? null : ((vt - va(0xfd1534).readU32() - start) | 0));

recv('config', (m) => {
  cfg = m.payload;
  grabs = cfg.grabs.slice().sort((a, b) => a - b);
  Interceptor.replace(va(0x689e80), new NativeCallback(() => now(), 'uint32', [], 'mscdecl'));
  // The frame limiter waits on the clock: while it is virtual, a delay moves it instead of sleeping long.
  // (SDL is linked into the exe: its `SDL_Delay` ends in kernel32 `Sleep`.)
  const sleep = Process.getModuleByName('kernel32.dll').getExportByName('Sleep');
  const realSleep = new NativeFunction(sleep, 'void', ['uint32'], { abi: 'stdcall' });
  Interceptor.replace(sleep, new NativeCallback((ms) => { if (vtOn && ms > 0) { vt += ms; delays += ms; realSleep(1); } else realSleep(ms); }, 'void', ['uint32'], 'stdcall'));
  if (cfg.open) { va(0xbbb22f).writeU8(1); hookOpen(); }
  hook();
});

function hookOpen() {
  const o = cfg.open;
  const openBink = new NativeFunction(va(0x7325b0), 'uint8', ['pointer', 'uint8', 'int', 'int', 'uint8'], { abi: 'mscdecl' });
  const playBink = new NativeFunction(va(0x732660), 'void', ['uint8', 'uint8', 'uint8'], { abi: 'mscdecl' });
  const name = Memory.allocAnsiString(o.name);
  let frames = 0, opened = false;
  Interceptor.attach(va(0x4de720), {
    onEnter() {
      if (opened || ++frames < o.after) return;
      opened = true;
      const ok = openBink(name, o.fullscreen, o.x, o.y, o.force);
      if (ok) playBink(0, o.loop, 0);
      send({ k: 'staged', name: o.name, ok, frames });
    },
  });
}

function hook() {
  Interceptor.attach(va(0x7325b0), {
    onEnter(a) { this.n = a[0].readCString(); this.args = [a[1].toInt32() & 0xff, a[2].toInt32(), a[3].toInt32(), a[4].toInt32() & 0xff]; },
    onLeave(r) { send({ k: 'open', name: this.n, fullscreen: this.args[0], x: this.args[1], y: this.args[2], force: this.args[3], ok: r.toInt32() & 0xff, vt }); },
  });
  Interceptor.attach(va(0x732660), {
    onEnter(a) { send({ k: 'play', blocking: a[0].toInt32() & 0xff, loop: a[1].toInt32() & 0xff, vt, elapsed: elapsed() }); },
  });
  Interceptor.attach(va(0x732540), {
    onEnter() { closed = true; send({ k: 'close', vt, elapsed: elapsed(), swaps }); },
  });
  // `videoentity::play` (thiscall: ecx = the entity): the start time, and the virtual clock from here on.
  Interceptor.attach(va(0x70f870), {
    onEnter() { this.e = this.context.ecx; if (!vtOn) { vt = realTicks(); vtOn = !cfg.real; } },
    onLeave() { start = this.e.add(0x2a8).readU32(); send({ k: 'start', start, base: va(0xfd1534).readU32(), vt }); },
  });
  hookSwap();
}

function hookSwap() {
  const gl = Process.getModuleByName('opengl32.dll'), gdi = Process.getModuleByName('gdi32.dll'), user = Process.getModuleByName('user32.dll');
  const f = (m, n, r, a) => new NativeFunction(m.getExportByName(n), r, a, { abi: 'stdcall' });
  const readPixels = f(gl, 'glReadPixels', 'void', ['int', 'int', 'int', 'int', 'uint', 'uint', 'pointer']);
  const getInt = f(gl, 'glGetIntegerv', 'void', ['uint', 'pointer']);
  const readBuffer = f(gl, 'glReadBuffer', 'void', ['uint']);
  const pixelStore = f(gl, 'glPixelStorei', 'void', ['uint', 'int']);
  const getProc = f(gl, 'wglGetProcAddress', 'pointer', ['pointer']);
  const winOfDc = f(user, 'WindowFromDC', 'pointer', ['pointer']);
  const clientRect = f(user, 'GetClientRect', 'int', ['pointer', 'pointer']);
  const post = f(user, 'PostMessageW', 'int', ['pointer', 'uint', 'pointer', 'pointer']);
  let bindFb = null, bindBuf = null;
  const tmp = Memory.alloc(16), rect = Memory.alloc(16);
  const get = (e) => { getInt(e, tmp); return tmp.readS32(); };
  const input = (cfg.input || []).slice().sort((a, b) => a.t - b.t);
  // gdi32's `SwapBuffers` forwards to gdi32full's, which the game reaches.
  let swap = null;
  try { swap = Process.getModuleByName('gdi32full.dll').getExportByName('SwapBuffers'); } catch (e) { swap = gdi.getExportByName('SwapBuffers'); }
  Interceptor.attach(swap, {
    onEnter(args) {
      if (cfg.real && start !== null) vt = realTicks();
      if (!vtOn && !(cfg.real && start !== null)) return;
      swaps++;
      const t = elapsed();
      const hwnd = winOfDc(args[0]);
      clientRect(hwnd, rect);
      const w = rect.add(8).readS32(), h = rect.add(12).readS32();
      while (t !== null && input.length && input[0].t <= t) {
        const e = input.shift();
        const lp = ptr(((h >> 1) << 16) | (w >> 1));
        // WM_KEYDOWN/UP (VK_SPACE), WM_LBUTTONDOWN/UP.
        const [msg, wp] = e.key ? [e.down ? 0x100 : 0x101, ptr(0x20)] : [e.down ? 0x201 : 0x202, ptr(e.down ? 1 : 0)];
        post(hwnd, msg, wp, e.key ? ptr(e.down ? 0x00390001 : 0xc0390001) : lp);
        send({ k: 'input', t, swaps, key: !!e.key, down: !!e.down });
      }
      if (t !== null && grabs.length && t >= grabs[0]) {
        const want = grabs.shift();
        if (!bindFb) {
          const p = getProc(Memory.allocAnsiString('glBindFramebuffer')), q = getProc(Memory.allocAnsiString('glBindBuffer'));
          bindFb = p.isNull() ? () => {} : new NativeFunction(p, 'void', ['uint', 'uint'], { abi: 'stdcall' });
          bindBuf = q.isNull() ? () => {} : new NativeFunction(q, 'void', ['uint', 'uint'], { abi: 'stdcall' });
        }
        const fb = get(0x8caa), pack = get(0x88ed), rb = get(0x0c02), align = get(0x0d05);
        bindFb(0x8ca8, 0); bindBuf(0x88eb, 0); readBuffer(0x0405); pixelStore(0x0d05, 1);
        const buf = Memory.alloc(w * h * 4);
        readPixels(0, 0, w, h, 0x1908, 0x1401, buf);
        bindFb(0x8ca8, fb); bindBuf(0x88eb, pack); readBuffer(rb); pixelStore(0x0d05, align);
        send({ k: 'frame', t: want, elapsed: t, swaps, w, h }, buf.readByteArray(w * h * 4));
      }
      if (!finished && grabs.length === 0 && input.length === 0 && (closed || !(cfg.input || []).length)) { finished = true; send({ k: 'done', swaps, closed }); }
      if (cfg.step) vt += cfg.step;
    },
  });
}
