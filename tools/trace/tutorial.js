// Frida agent: the tutorial interpreter on the GOG ig2.exe (spec re/rules/tutorial.md).
// Records:
//  'start' the main menu's tutorial choice is made (Mode_Tutorial_Start + TutorialStartMelyik, MainScr_Kilep
//          calls TutorialSetCampaignGameOptions + TutorialStart on its own), with the chapter;
//  'tick'  one per TutorialFrissit (exe 0x60c020) while a tutorial runs: the inputs it reads (AktKep, Counter,
//          Atlepes/Belep, the .mou index the replay step left, the counts TutorialCheckEvents reads while paused)
//          and the interpreter's state after it; the text window's grid and queue only when they changed;
//  'cmd'   one per TutorialGetNextCommand (exe 0x60c570): its argument and return value and the effects the
//          command had on the hooked calls (MoneyHozzaadLog, the speech and .mou loaders, ...);
//  'ev'    one per TutorialCheckEvent (exe 0x60bdf0) outside TutorialFrissit, with the awaited id; 'forced'
//          when the tracer fired it (an event awaited for --force-after UI ticks: the player's action);
//  'battle' the tracer cleared TutorialWaitForBattle (no battle comes in a replayed chapter).
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Frissit: va(0x60c020), Next: va(0x60c570), CheckEvent: va(0x60bdf0),
  ExistMp2: va(0x563be0), StartMp2: va(0x612430), PlayMouse: va(0x611db0), Money: va(0x53fb10),
  CountMyTanks: va(0x60b5d0), TankNew: va(0x541750),
  ModeStart: va(0xbbb22c), Melyik: va(0xbbb230),
  AktKep: va(0xbae0fc), Counter: va(0xbae05c), Atlepes: va(0xbae24d), Belep: va(0xbae24c), SelfPlaying: va(0xbae267),
  EN: va(0x912a3c), Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8),
  Active: va(0xf6915d), Paused: va(0xf66f2a), Ptr: va(0xf68094), Len: va(0xf68090), TargetTime: va(0xf68098),
  TargetCounter: va(0xf680a0), StartCounter: va(0xf6809c), Restarting: va(0xf680b6), Chapter: va(0xf680ac),
  Blink: va(0xf6813c), BlinkDir: va(0x928ea9), ActX: va(0xf673b8), ActY: va(0xf673bc), TargetX: va(0xf673b0),
  TargetY: va(0xf673b4), AddX: va(0xf673c0), AddY: va(0xf673c4), Queue: va(0xf673c8), QLen: va(0xf67f80),
  SorPtr: va(0xf6712c), Tomb: va(0xf67130), TCol: va(0xf67f84), TVil: va(0xf6712d), Typing: va(0xf69144),
  TypingMax: va(0xf69148), TypingStep: va(0xf6914c), TextShown: va(0x928ea8), NoText: va(0xf6712e),
  MoveDiv: va(0xf67114), Countdown: va(0xf680a8), Event: va(0xf66f40), WMouse: va(0xf6808b), WText: va(0xf680b4),
  WAtlepes: va(0xf680b5), WScreen: va(0xf6712f), WScreenId: va(0xf680a4), WBattle: va(0xf66f48),
  Speech: va(0xf6808a), Mouse: va(0xf66f20), MouseMax: va(0xf66f24), UiCounter: va(0xf6919c),
  OldCounter: va(0xf69198), Mp2Len: va(0xf67f88), TextLen: va(0xf69140), Screens: va(0xf66f2c),
  Buildings: va(0xf66f4c), Split: va(0xf66f4a), Production: va(0xf66f4b), Starmap: va(0xf66ff8),
  Colony: va(0xf66ff9), SellShips: va(0xf66ffd), ExitGomb: va(0xf66fff), Demolish: va(0xf66f49),
  NewSpies: va(0xf66f2b), Overrun: va(0xf680b7), MainPlanet: va(0xf66f88),
};
const cfg = { startAfterFrames: 1200, chapters: [1], forceAfter: 300, battleAfter: 600, nextAfterFrames: 400, debug: false };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
function emit(ev) { buf.push(ev); if (buf.length >= 32) flush(); }
setInterval(flush, 300);

const hex = (p, n) => Array.from(new Uint8Array(p.readByteArray(n)), (b) => b.toString(16).padStart(2, '0')).join('');
const f32 = (p) => p.readFloat();
const countMyTanks = new NativeFunction(A.CountMyTanks, 'int', [], 'mscdecl');
const checkEvent = new NativeFunction(A.CheckEvent, 'void', ['int'], 'mscdecl');

function owned() {
  const en = A.EN.readS32(), n = A.HanyBolygo.readS32();
  let k = 0, x = 0;
  for (let p = 1; p <= n; p++) { const r = A.Bolygok.add(0x188 * p); if (r.add(0x20).readU8() === en) k++; if (r.add(0xd8 + 2 * en).readS16() === 1) x++; }
  return [k, x];
}

function state() {
  return {
    act: A.Active.readU8(), pau: A.Paused.readU8(), ptr: A.Ptr.readS32(), len: A.Len.readS32(),
    tt: A.TargetTime.readS32(), tc: A.TargetCounter.readS32(), sc: A.StartCounter.readS32(), rs: A.Restarting.readU8(),
    ch: A.Chapter.readS32(), bl: A.Blink.readS32(), bd: A.BlinkDir.readU8(),
    x: f32(A.ActX), y: f32(A.ActY), txy: [A.TargetX.readS32(), A.TargetY.readS32()], add: [f32(A.AddX), f32(A.AddY)],
    ql: A.QLen.readS32(), sp: A.SorPtr.readS8(), col: A.TCol.readS32(), vil: A.TVil.readU8(),
    ty: f32(A.Typing), tm: f32(A.TypingMax), ts: f32(A.TypingStep), shown: A.TextShown.readU8(), notext: A.NoText.readU8(),
    md: A.MoveDiv.readS32(), cd: A.Countdown.readS32(), ev: A.Event.readS32(),
    w: [A.WMouse.readU8(), A.WText.readU8(), A.WAtlepes.readU8(), A.WScreen.readU8(), A.WScreenId.readS32(), A.WBattle.readU8()],
    spb: A.Speech.readU8(), mo: A.Mouse.readS32(), mm: A.MouseMax.readS32(), uc: A.UiCounter.readS32(), oc: A.OldCounter.readS32(),
    mp2: A.Mp2Len.readS32(), tl: A.TextLen.readS32(), scr: hex(A.Screens, 20), bld: hex(A.Buildings, 0x3c),
    g: [A.Split.readU8(), A.Production.readU8(), A.Starmap.readU8(), A.Colony.readU8(), A.SellShips.readU8(),
      A.ExitGomb.readU8(), A.Demolish.readU8(), A.NewSpies.readU8(), A.Overrun.readU8()],
    mp: A.MainPlanet.readCString(),
  };
}

let cur = null, inFrissit = 0, inNext = 0, cmd = null, lastTomb = '', lastQueue = '', pausedFor = 0, battleFor = 0, tickNo = 0;
let frames = 0, chapterIx = -1, startedAt = 0, idleSince = -1, pendingForce = 0;

Interceptor.attach(A.Frissit, {
  onEnter() {
    inFrissit++;
    const live = A.Active.readU8() || A.Paused.readU8();
    this.rec = live ? {
      k: 'tick', n: tickNo++, kep: A.AktKep.readS32(), cnt: A.Counter.readS32(),
      tr: (A.Atlepes.readU8() | A.Belep.readU8()) ? 1 : 0, self: A.SelfPlaying.readU8(),
      // `KepernyokLetiltva` at entry: InitNewGame, Starmapvisszaallit, screenfrissites, Fake_FRISSITES and the
      // space battle write it between ticks.
      pre: { mo: A.Mouse.readS32(), uc: A.UiCounter.readS32(), oc: A.OldCounter.readS32(), scr: hex(A.Screens, 20) }, cmds: [],
    } : null;
    // The counts TutorialInitEvents (WAITFOREVENT) and TutorialCheckEvents (paused) read.
    if (this.rec) { const [k, x] = owned(); this.rec.own = k; this.rec.expl = x; this.rec.tanks = countMyTanks(); }
    cur = this.rec;
  },
  onLeave() {
    inFrissit--;
    const r = this.rec;
    cur = null;
    if (!r) return;
    r.post = state();
    const tomb = hex(A.Tomb, 640), q = hex(A.Queue, Math.min(Math.max(r.post.ql, 0) + 4, 3000));
    if (tomb !== lastTomb) { r.tomb = tomb; lastTomb = tomb; }
    if (q !== lastQueue) { r.q = q; lastQueue = q; }
    emit(r);
    if (r.post.pau && r.post.ev && r.post.ev !== 0x22) { if (++pausedFor >= cfg.forceAfter) { pendingForce = r.post.ev; pausedFor = 0; } } else pausedFor = 0;
    if (r.post.act && r.post.w[5]) { if (++battleFor >= cfg.battleAfter) { A.WBattle.writeU8(0); battleFor = 0; emit({ k: 'battle', after: r.n }); } } else battleFor = 0;
  },
});

Interceptor.attach(A.Next, {
  onEnter(args) { inNext++; this.p = args[0].toInt32(); cmd = { p: this.p, fx: [] }; },
  onLeave(ret) {
    inNext--;
    cmd.r = ret.toInt32();
    if (cur) cur.cmds.push(cmd);
    else emit(Object.assign({ k: 'cmd' }, cmd));
    cmd = null;
  },
});
Interceptor.attach(A.ExistMp2, { onLeave(ret) { if (cmd) cmd.fx.push(['speech?', ret.toInt32() & 0xff]); } });
Interceptor.attach(A.StartMp2, { onLeave() { if (cmd) cmd.fx.push(['mp2', A.Mp2Len.readS32()]); } });
Interceptor.attach(A.PlayMouse, { onEnter(args) { this.n = args[0].readCString(); }, onLeave() { if (cmd) cmd.fx.push(['mou', this.n, A.MouseMax.readS32()]); } });
Interceptor.attach(A.Money, {
  // MoneyHozzaadLog(e, k, double x, file, line), cdecl.
  onEnter(args) { if (cmd) cmd.fx.push(['money', args[0].toInt32(), args[1].toInt32(), this.context.esp.add(12).readDouble()]); },
});
Interceptor.attach(A.TankNew, { onEnter(args) { if (cmd) cmd.fx.push(['tank', args[0].toInt32(), args[1].toInt32(), args[2].toInt32()]); } });
Interceptor.attach(A.CheckEvent, {
  onEnter(args) {
    const id = args[0].toInt32();
    if (!inFrissit) emit({ k: 'ev', id, ev: A.Event.readS32(), pau: A.Paused.readU8(), forced: this.forced || false, before: tickNo });
  },
});

function startChapter() {
  chapterIx++;
  if (chapterIx >= cfg.chapters.length) { if (chapterIx === cfg.chapters.length) { emit({ k: 'done' }); flush(); } return; }
  A.Melyik.writeS32(cfg.chapters[chapterIx]);
  A.ModeStart.writeU8(1);
  // Leave the main menu for the starmap: `MainScr_Kilep` then starts the tutorial (re/rules/tutorial.md §7).
  va(0xbae250).writeS32(2);
  startedAt = frames;
  emit({ k: 'start', chapter: cfg.chapters[chapterIx], frame: frames });
}

// Skip the title video (`VoltGTLogo` 0xBBB22F, as screen.js does).
va(0xbbb22f).writeU8(1);

Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    // `IsActive` (0x912A38): the game runs in a background window too; `MainScr_Kilep`'s exit fade (0xBBB360,
    // flagged by 0xBBB22A) doesn't run there (screen.js).
    va(0x912a38).writeU8(1);
    if (va(0xbbb22a).readU8() !== 0) va(0xbbb360).writeFloat(0);
    if (chapterIx < 0 && frames === cfg.startAfterFrames) startChapter();
    if (pendingForce && A.Paused.readU8() && A.Event.readS32() === pendingForce) {
      const id = pendingForce;
      pendingForce = 0;
      emit({ k: 'ev', id, ev: id, pau: 1, forced: true, before: tickNo });
      inFrissit++; // the native call's own hook record is skipped: this one stands for it
      checkEvent(id);
      inFrissit--;
    }
    if (chapterIx >= 0 && frames > startedAt + 600 && !A.Active.readU8() && !A.Paused.readU8()) {
      if (idleSince < 0) idleSince = frames;
      if (frames - idleSince === cfg.nextAfterFrames) { idleSince = -1; startChapter(); }
    } else idleSince = -1;
  },
});
