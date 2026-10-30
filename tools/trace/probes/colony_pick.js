// Colony building pick probe (re/screens/colony.md §10.1), appended to screen.js by run_screen.py --probe: on the
// colony screen each frame's click pass (`Update3D` exe 0x52cd10 with a `clicktype`, from `ShowColony` 0x49f130) gets
// the next point of a grid over the virtual 640 × 480 frame instead of the pointer; at the panel pass (0x998247 set)
// the frame's `selectedepulet` (exe 0x8ec994, the slot − 1 of the building whose model the point hit, −1 none) is
// logged with the point: `pick` records, then `pickcam` with the camera (`ColonyXStart…`, 0xBA90DC…).
(function () {
  const STEP = 8, SETTLE = 60;
  const pts = [];
  for (let y = 4; y < 480; y += STEP) for (let x = 4; x < 640; x += STEP) pts.push([x, y]);
  let frames = 0, i = -1, at = null;
  const cam = () => [0xba90dc, 0xba9514, 0xba9e8c, 0xba9b64, 0xba983c].map((a) => va(a).readFloat());
  Interceptor.attach(va(0x52cd10), {
    onEnter(args) {
      if (va(0x998247).readU8() !== 0) {
        if (at !== null) send({ k: 'pick', x: at[0], y: at[1], sel: va(0x8ec994).readS32() });
        at = null;
        return;
      }
      if (args[0].isNull() || args[1].isNull() || A.AktKep.readS32() !== 3) return;
      frames++;
      if (frames < SETTLE) return;
      if (i === -1) send({ k: 'pickcam', at: 'start', cam: cam() });
      i++;
      if (i >= pts.length) {
        if (i === pts.length) send({ k: 'pickcam', at: 'end', cam: cam() });
        return;
      }
      at = pts[i];
      args[0].writeFloat(at[0]);
      args[0].add(4).writeFloat(at[1]);
    },
  });
})();
