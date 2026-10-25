// Rain probe (deviation 384, re/screens/colony.md §2.6), appended to screen.js by run_screen.py --probe: per call of
// `ShowRain` (exe 0x4a34a0) the number of `line3d2` (exe 0x533db0) lines it draws and the first two lines' arguments
// (the raw stack words: points and colours), for 6 calls after the first 100; on the first of them every line's first point.
(function () {
  let calls = 0, cur = null;
  Interceptor.attach(va(0x4a34a0), {
    onEnter() {
      calls++;
      cur = calls > 100 && calls <= 106 ? { k: 'rain', call: calls, lines: 0, first: [], p: [] } : null;
    },
    onLeave() {
      if (cur) send(cur);
      cur = null;
    },
  });
  Interceptor.attach(va(0x533db0), {
    onEnter() {
      if (!cur) return;
      cur.lines++;
      // Every line's first point (camera space) on the first logged call.
      if (cur.call === 101) cur.p.push([4, 8, 12].map((o) => +this.context.esp.add(4 + 16 + o - 4).readFloat().toFixed(1)));
      if (cur.first.length < 2) {
        const sp = this.context.esp;
        cur.first.push(Array.from({ length: 12 }, (_, i) => { const w = sp.add(4 + 4 * i); return [+w.readFloat().toFixed(3), '0x' + w.readU32().toString(16)]; }));
      }
    },
  });
})();
