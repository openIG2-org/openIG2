// Battle rows probe (re/screens/options.md §7), appended to screen.js by run_screen.py --probe (--boot --kep 15 --run):
// at the first `Mainscreen_Update3D` (exe 0x500270) on the main menu, every combination of the four battle rows'
// values (rows k = 1..4 at 0x9160a0 + 0x22c·k + 0x204) goes through the exe's own `SaveOptions` (0x51b640), and the
// battle flags 0xB77230..0xB7723C are read back ('battlerows' records); then the rows and flags are put back.
(function () {
  let done = false;
  const row = (k) => va(0x9160a0 + 0x22c * k + 0x204);
  const flags = () => [0, 4, 8, 12].map((o) => va(0xb77230 + o).readS32());
  Interceptor.attach(va(0x500270), {
    onEnter() {
      if (done || va(0xbae0fc).readS32() !== 15) return;
      done = true;
      const save = new NativeFunction(va(0x51b640), 'void', []);
      const rows0 = [1, 2, 3, 4].map((k) => row(k).readS32());
      const flags0 = flags();
      send({ k: 'battlerows', rows: rows0, flags: flags0, initial: true });
      for (let m = 0; m < 16; m++) {
        const rows = [0, 1, 2, 3].map((b) => (m >> b) & 1);
        rows.forEach((v, i) => row(i + 1).writeS32(v));
        save();
        send({ k: 'battlerows', rows, flags: flags() });
      }
      rows0.forEach((v, i) => row(i + 1).writeS32(v));
      save();
      flags0.forEach((v, i) => va(0xb77230 + 4 * i).writeS32(v));
    },
  });
})();
