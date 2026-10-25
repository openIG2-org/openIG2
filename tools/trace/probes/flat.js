// Flat-light probe (re/screens/colony.md §3.1), appended to screen.js by run_screen.py --probe: every
// `kirak` (exe 0x531460) or `kirak2` (0x530b20) (args: sort record, nfaces, VertType[], pont[], normals[] stride 0x14 (colour +0xc,
// specular +0x10), …) whose record properties (`+0x68`) have 0x100 and not 1 (the flat-lit trees, the Park): the
// properties and the first vertices' colour and specular words, 30 records after the first 500 calls.
(function () {
  let calls = 0, sent = 0;
  for (const [fn, addr] of [['kirak', 0x531460], ['kirak2', 0x530b20]]) Interceptor.attach(va(addr), {
    onEnter(args) {
      if (++calls < 500 || sent >= 30) return;
      const sp = args[0], props = sp.add(0x68).readU32();
      if ((props & 0x100) === 0 || (props & 1) !== 0) return;
      sent++;
      const nt = args[4];
      send({ k: 'flat', fn, props: '0x' + props.toString(16), colours: Array.from({ length: 6 }, (_, i) => ['0x' + nt.add(0x14 * i + 0xc).readU32().toString(16), '0x' + nt.add(0x14 * i + 0x10).readU32().toString(16)]) });
    },
  });
})();
