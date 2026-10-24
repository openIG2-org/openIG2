// Probe for run_video.py --real (re/screens/video.md §4): when a fullscreen clip's sound starts against its picture clock,
// and what else sounds. soundhook exe 0x7100e0 (udata, stream, len), cdecl through Mix_HookMusic; SoundEngine::Play 0x55e2a0.
// Events: hook_first, sound_first (first non-zero S16 frame of the hook's output since the play), sync, se_play.
(function () {
  const vtime = () => (start === null ? null : ((realTicks() - va(0xfd1534).readU32() - start) | 0));
  let calls = 0, frames = 0, firstSound = null, marks = [1000, 3000, 6000, 12000, 14000, 20000, 30000];
  Interceptor.attach(va(0x7100e0), {
    onEnter(a) { this.s = a[1]; this.n = a[2].toInt32(); },
    onLeave() {
      const n = this.n >> 2;
      if (calls === 0) send({ k: 'hook_first', t: vtime(), len: this.n, ticks: realTicks() });
      calls++;
      if (firstSound === null) {
        const b = this.s.readByteArray(this.n), v = new Int16Array(b);
        for (let i = 0; i < v.length; i++) if (v[i] !== 0) { firstSound = frames + (i >> 1); break; }
        if (firstSound !== null) send({ k: 'sound_first', t: vtime(), frame: firstSound, frames_before_call: frames });
      }
      frames += n;
      const t = vtime();
      while (marks.length && t !== null && t >= marks[0]) { send({ k: 'sync', t, audio_ms: Math.round(frames * 1000 / 44100), calls }); marks.shift(); }
    },
  });
  Interceptor.attach(va(0x55e2a0), {
    onEnter(a) { send({ k: 'se_play', t: vtime(), h: this.context.ecx.toString(), arg: a[0].toInt32() }); },
  });
})();
