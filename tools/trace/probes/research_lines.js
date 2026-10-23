// Research button-line probe (re/screens/research.md §5.6), appended to screen.js by run_screen.py --probe: for every
// `ResDrawVonalak` (exe 0x51f340, cdecl floats X, Y, z) logs the centre, `ResVonalkak[10]` (0xddf794),
// `ResVonalakNovelo` (0xddf674), `ResKellVonalTavolodas` (0xddf62b), the UI counter (0xb772a4), and the
// `DrawArrowAbsolute` (exe 0x593cb0: p0*, p1*, colour, width) calls it makes: p0, p1 (x, y as f32 bits), colour.
// Records 'resline' go to --jsonl.
(function () {
  let cur = null;
  const fb = (p) => { const b = Memory.alloc(4); b.writeU32(p.toUInt32()); return b.readFloat(); };
  Interceptor.attach(va(0x51f340), {
    onEnter(args) {
      const v = [];
      for (let i = 0; i < 10; i++) v.push(va(0xddf794 + 4 * i).readU32());
      cur = { k: 'resline', ui: va(0xb772a4).readS32(), x: fb(args[0]), y: fb(args[1]), v,
        novelo: va(0xddf674).readU32(), kell: va(0xddf62b).readU8(), arrows: [] };
    },
    onLeave() { if (cur) send(cur); cur = null; },
  });
  Interceptor.attach(va(0x593cb0), {
    onEnter(args) {
      if (!cur) return;
      cur.arrows.push([args[0].readU32(), args[0].add(4).readU32(), args[1].readU32(), args[1].add(4).readU32(),
        args[2].toUInt32(), args[3].toUInt32()]);
    },
  });
})();
