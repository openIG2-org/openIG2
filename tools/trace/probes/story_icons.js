// Story icon probe, appended to screen.js by run_screen.py --probe with --kep 2 --rop 0x10|0x11|0x12 (and --fleets 1
// --rop-fleet 1 for the fleet icons): what `Starmap_Main` (exe 0x5b2b70) does with the radial menu's story icons
// (re/screens/starmap.md §14.5). On the call that takes the icon it stages, then records the state before ('sibefore')
// and after ('siafter'):
//   0x10: RandSeed[0] (0xBAF2D0) and the planets with an owner before; the selected fleet's order and target after.
//   0x11: a ship of design 0x1c added to the selected fleet (`HajoNew` 0x53e800) and its +0x52 bit 0x2000; the
//         fleet's ships before and after, and every `Story_Special` (0x5c9550) call.
//   0x12: `StartStory(402, true)` (0x5c4840), planet mode on the first planet of another empire 1..=8 (`AktPlanet`
//         0xB9933C, `StarmapMode` 0xB9998C = 1); StoryAdat+0x44 (0xF273F8) and the story lists after.
(function () {
  const startStory = nf(va(0x5c4840), 'int', ['int', 'int']);
  const P = va(0xb85ea8), NP = va(0x912ac8), EN = va(0x912a3c), SEED0 = va(0xbaf2d0);
  const ships = () => {
    const base = va(0xbb4100).readPointer(), n = va(0xbb4104).readS32(), out = [];
    for (let k = 1; k <= n; k++) out.push([base.add(6 * k).readS16(), base.add(6 * k + 2).readS16()]);
    return out;
  };
  const fleet = (f) => va(0xbb40a4).readPointer().add(0x80 * f);
  const lists = () => {
    const at = { planets: 0xf23a20, counters: 0xf24388, vars: 0xf251a8 }, out = {};
    Object.entries(at).forEach(([name, a], i) => {
      const n = va(0xf23a08 + 4 * [0, 1, 3][i]).readS32();
      out[name] = Array.from({ length: Math.max(0, Math.min(n, 400)) }, (_, k) => [va(a + 8 * (k + 1)).readS32(), va(a + 8 * (k + 1) + 4).readS32()]);
    });
    return out;
  };
  let mine = false, specials = [];
  Interceptor.attach(va(0x5c9550), {
    onEnter(args) {
      if (mine) specials.push(Array.from({ length: 8 }, (_, i) => args[i].toInt32()));
    },
  });
  Interceptor.attach(va(0x5b2b70), {
    onEnter() {
      if (!cfg.ropNow || ropDone || mine) return;
      mine = true;
      specials = [];
      const e = EN.readS32(), np = NP.readS32(), rec = { k: 'sibefore', icon: cfg.rop, en: e };
      if (cfg.rop === 0x12) {
        let p = 1;
        while (p <= np && !(P.add(0x188 * p + 0x20).readU8() > 0 && P.add(0x188 * p + 0x20).readU8() < 9 && P.add(0x188 * p + 0x20).readU8() !== e)) p++;
        rec.started = startStory(402, 1);
        va(0xb9998c).writeS32(1);
        A.AktPlanet.writeS32(p);
        rec.planet = p;
        rec.owner = P.add(0x188 * p + 0x20).readU8();
      } else {
        const f = va(0xb99664).readS32();
        rec.fleet = f;
        if (cfg.rop === 0x11) {
          hajoNew(f, 0x1c);
          fleet(f).add(0x52).writeU16(fleet(f).add(0x52).readU16() | 0x2000);
        }
        rec.ships = ships().map((s, k) => [k + 1, s[0], s[1]]).filter((s) => s[1] === f);
        rec.seed0 = SEED0.readU32();
        rec.owned = Array.from({ length: np }, (_, i) => P.add(0x188 * (i + 1) + 0x20).readU8());
        rec.order = [fleet(f).add(0x40).readS16(), fleet(f).add(0x42).readS16(), fleet(f).add(0x44).readFloat(), fleet(f).add(0x48).readFloat()];
      }
      send(rec);
    },
    onLeave() {
      if (!mine || !ropDone) return;
      mine = false;
      const rec = { k: 'siafter', icon: cfg.rop, specials, adat44: va(0xf273f8 + 0x44).readS32(), seed0: SEED0.readU32() };
      if (cfg.rop === 0x12) rec.lists = lists();
      else {
        const f = va(0xb99664).readS32();
        rec.ships = ships().map((s, k) => [k + 1, s[0], s[1]]).filter((s) => s[1] === f);
        rec.order = [fleet(f).add(0x40).readS16(), fleet(f).add(0x42).readS16(), fleet(f).add(0x44).readFloat(), fleet(f).add(0x48).readFloat()];
      }
      send(rec);
    },
  });
})();
