// Fleet slot past the end probe, appended to screen.js by run_screen.py --probe with --kep 2 --run: dumps the record
// `Flottak[OsszHanyFlotta + 1]` (the slot story 856's raider is sent after, re/rules/campaign/elem.md 856) and
// `MAXHANYFLOTTA` (0xBB40AC), then orders the first fleet of an empire (owner 1..=8) to attack that slot with
// `FlottaMozgat(f, 2, 2, n + 1, 0)` (exe 0x53d270) and records ('pastend') its order, kind and target on every
// following `FRISSIT_Starmap` (0x594ae0) until the order changes (re/rules/fleet-movement.md §4.1).
(function () {
  const mozgat = nf(va(0x53d270), 'void', ['int', 'int', 'int', 'float', 'float']);
  const rec = (f) => {
    const r = va(0xbb40a4).readPointer().add(0x80 * f);
    return { owner: r.add(0x15).readU8(), state: r.add(0x18).readU8(), x: r.add(0x30).readFloat(), y: r.add(0x34).readFloat(),
      order: r.add(0x40).readS16(), kind: r.add(0x42).readS16(), tx: r.add(0x44).readFloat(), ty: r.add(0x48).readFloat(),
      flags: r.add(0x52).readU16(), visible: Array.from({ length: 18 }, (_, k) => r.add(0x1a + k).readU8()) };
  };
  let fleet = 0, frames = 0, last = null;
  Interceptor.attach(va(0x594ae0), {
    onEnter() {
      const n = va(0xbb40a8).readS32();
      if (fleet === 0) {
        const en = va(0x912a3c).readU8();
        for (let f = 1; f <= n && fleet === 0; f++) {
          const o = rec(f).owner;
          if (o >= 1 && o <= 8) fleet = f;
        }
        send({ k: 'pastend', n, cap: va(0xbb40ac).readS32(), en, fleet, slot: rec(n + 1) });
        if (fleet === 0) { fleet = -1; return; }
        mozgat(fleet, 2, 2, n + 1, 0);
        send({ k: 'pastend', after_order: rec(fleet) });
        return;
      }
      if (fleet < 0 || frames > 400) return;
      frames++;
      const r = rec(fleet);
      const key = [r.order, r.kind, r.tx, r.ty].join();
      if (key !== last) { last = key; send({ k: 'pastend', frame: frames, n, fleet: r, slot: rec(n + 1) }); }
    },
  });
})();
