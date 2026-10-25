// Grwar8020hoz probe, appended to screen.js by run_screen.py --probe with --kep 2 --planet P --fleets 2: calls
// `Grwar8020hoz(f)` (exe 0x5c28e0; re/rules/ground-combat.md §2 step 1) on the two made fleets in turn from the next
// `Starmap_Main` (0x5b2b70) and records ('grwar') around each call `LastNewMessage` (0xDABDC8) and `UVoltUjUzenet`
// (0xDA65FF, cleared first) as `NewMessage` (0x508fe0) sets them (the agent's hooks don't fire inside its own calls),
// the fleet's owner byte (+0x15) after it and the story variables (`sValtozok` 0xF251A8).
(function () {
  const grwar = nf(va(0x5c28e0), 'void', ['int']);
  let done = false;
  const vars = () => {
    const n = va(0xf23a08 + 12).readS32();
    return Array.from({ length: Math.max(0, Math.min(n, 400)) }, (_, k) => [va(0xf251a8 + 8 * (k + 1)).readS32(), va(0xf251a8 + 8 * (k + 1) + 4).readS32()]);
  };
  Interceptor.attach(va(0x5b2b70), {
    onEnter() {
      if (done || fleets.length < 2) return;
      done = true;
      for (const f of fleets.slice(0, 2)) {
        const last = va(0xdabdc8).readS32();
        va(0xda65ff).writeU8(0);
        grwar(f);
        send({ k: 'grwar', fleet: f, last, lastAfter: va(0xdabdc8).readS32(), newMessage: va(0xda65ff).readU8(),
          owner: va(0xbb40a4).readPointer().add(0x80 * f + 0x15).readU8(), vars: vars() });
      }
    },
  });
})();
