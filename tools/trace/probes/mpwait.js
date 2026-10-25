// The MP wait overlay staged in a single-player game (re/screens/input.md §9, deviation 296), appended to screen.js by
// run_screen.py --probe (run_screendiff.py mpwait). ig2.exe can't be driven into a real lockstep stall here (two
// instances and the multiplayer menus), so this puts the exe's own state for one into place and lets its own code draw:
// `MultiSendPackets` (exe 0x5126b0, which would clear `WaitingForVector`) does nothing; 30 frames into the starmap, and
// before every `Fake_FRISSITES` after that: `WaitingForVector` 0xdb9748 = 1, `DEBUGX` 0xbae587 = 0, `SuperVisor`
// 0x912a3a = 1 (host), `ProgramPlayers` 0x912acc = 3, the stall flag 0xdb94a4 = 1, slots 2 and 3 late (`bae560[i]` = 1,
// `db96e0[i]` = 0 below the turn 0xdb949c = 100), their names (`bb6730[i]`) and slot texts (0xbae593 + 11·(i − 1)),
// `MultiPlayerReconnectingMode` 0xdb94e8 = 1 (the title). `MOUSE_DrawOver` then opens the overlay and runs its loop
// (`Fake_FRISSITES` and the swaps go on, so the grab comes). The capture's clock stands still, so once it is open
// `acttime` 0xaba978 is put 5000 ms back (`SDL_GetTicks` exe 0x689e80, unsigned compare): its items are armed.
(function () {
  const NAMES = ['Alice', 'Bob'];
  const TEXTS = ['Slot two', 'Slot three'];
  const names = NAMES.map((n) => Memory.allocAnsiString(n));
  let staged = false, from = null;
  Interceptor.replace(va(0x5126b0), new NativeCallback(() => {}, 'void', [], 'mscdecl'));
  const sdlTicks = new NativeFunction(va(0x689e80), 'uint32', [], { abi: 'mscdecl' });
  const stage = () => {
    if (va(0xaba970).readS32() > 0) va(0xaba978).writeU32((sdlTicks() - 5000) >>> 0);
    va(0xdb9748).writeS32(1);
    va(0xbae587).writeU8(0);
    va(0x912a3a).writeU8(1);
    va(0x912acc).writeS32(3);
    va(0xdb94a4).writeS32(1);
    va(0xdb949c).writeS32(100);
    va(0xdb94e8).writeU8(1);
    for (let i = 1; i <= 8; i++) va(0xbae560 + i).writeU8(i === 2 || i === 3 ? 1 : 0);
    [2, 3].forEach((i, k) => {
      va(0xdb96e0 + 4 * i).writeS32(0);
      va(0xbb6730 + 4 * i).writePointer(names[k]);
      va(0xbae593 + 11 * (i - 1)).writeAnsiString(TEXTS[k]);
    });
  };
  Interceptor.attach(A.Fake_FRISSITES, {
    onEnter() {
      if (A.AktKep.readS32() !== 2 && !staged) return;
      if (from === null) from = frames;
      if (!staged && frames - from < 30) return;
      if (!staged) send({ k: 'mpwait', frames, open: va(0xaba970).readS32() });
      staged = true;
      stage();
    },
  });
  // The overlay's state at each grab's frame, for the comparison (mode, hovered item, acttime).
  Interceptor.attach(va(0x4c8f80), {
    onEnter() { if (staged && frames % 100 === 0) send({ k: 'mpwaitDraw', frames, mode: va(0xaba970).readS32(), sel: va(0xaba974).readS32(), at: va(0xaba978).readU32() }); },
  });
})();
