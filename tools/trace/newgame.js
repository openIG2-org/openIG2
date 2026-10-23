// Frida agent: the new-game start state on the GOG ig2.exe (spec re/rules/new-game.md). Starts a campaign the way the
// race menu does (CampaignGameStart 0x4f5e90 → InitNewGame 0x4e1130, as story.js --campaign), and around
// CampaignGameStart takes the whole .data section (image VAs 0x8de000.., 0x7924e4 bytes, BSS included) before and
// after, then calls the exe's own `LoadSave(slot, name, 2)` (0x543c80) so the save holds the state at tick 0.
// Logged: InitNewGame's arguments, every `srand` seed while it runs (RandomInit, NEWGAME_Diplomacy's
// srand(SDL_GetTicks())), the config/registry reads InitStory makes (overridable: storyline, storymap, registry).
// `scenario` set: starts that scenario the way the scenario menu does instead (Betolt_Scenario 0x4f5ba0 → InitNewGame
// mode 1 + LoadScenario(file, 0)), dumped and saved around it the same way. `mp` set: the multiplayer galaxy as the
// host's FRISSIT_Start (exe 0x5bc590) starts it, with the lobby (Start_Init 0x5bfbf0's defaults) emulated by writes:
// MultiPlayerMode 1, ProgramPlayers2 = ProgramPlayers = 8, the humans, races, live AI empires and planet count, then
// InitNewGame(seed, 2, 0, 1) (spec new-game.md §0; the lobby's own state writes are listed there). The intro video (st_ShowAnim) is skipped. Addresses are image VAs, rebased at runtime. Output holds game state
// derived from the user's install: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), CampaignGameStart: va(0x4f5e90), InitNewGame: va(0x4e1130), LoadSave: va(0x543c80),
  ReadRegistry: va(0x51b3b0), SaveRegistry: va(0x51b860), GetParam: va(0x53e4c0), ShowAnim: va(0x602bb0),
  DebugSeed: va(0xb7725c), FajStart: va(0xbbb17c), FajSzetnyilas: va(0xbbb16c), FajRace: va(0xbbb178), FajNehezseg: va(0xbbb180),
  Ido: va(0xbae04c),
};
const DATA = 0x8de000, DATA_LEN = 0x7924e4;

const cfg = { startAfterFrames: 1200, campaign: 1, difficulty: 2, seed: 4242, slot: 97, registry: 'none', storyline: null, storymap: null, scenario: null, mp: null };
recv('config', (m) => { Object.assign(cfg, m.payload); });

const nf = (p, r, a) => new NativeFunction(p, r, a, { abi: 'mscdecl', traps: 'all' });
const loadSave = nf(A.LoadSave, 'void', ['int', 'pointer', 'int']);
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const initNewGame = nf(A.InitNewGame, 'void', ['int', 'int', 'int', 'int']);

function dump(tag) {
  send({ type: 'dump', tag }, va(DATA).readByteArray(DATA_LEN));
}

let inInit = false;
const log = { init: null, srand: [], probes: [] };
Interceptor.attach(A.InitNewGame, {
  onEnter(args) { inInit = true; log.init = [0, 1, 2, 3].map((i) => args[i].toInt32()); },
  onLeave() { inInit = false; },
});
for (const m of Process.enumerateModules()) {
  const e = m.findExportByName('srand');
  if (e && /^msvcr/i.test(m.name)) {
    Interceptor.attach(e, { onEnter(args) { if (inInit) log.srand.push(args[0].toUInt32()); } });
  }
}
const keep = [];
Interceptor.attach(A.GetParam, {
  onEnter(args) { this.key = args[0].readAnsiString(); },
  onLeave(r) {
    if (!inInit) return;
    const o = cfg[this.key];
    if ((this.key === 'storyline' || this.key === 'storymap') && o !== null && o !== undefined) {
      const s = Memory.allocAnsiString(String(o)); keep.push(s); r.replace(s);
    }
    log.probes.push(['config', this.key, r.isNull() ? null : r.readAnsiString()]);
  },
});
Interceptor.attach(A.ReadRegistry, {
  onEnter(args) { this.key = args[0].readAnsiString(); this.out = args[1]; },
  onLeave(r) {
    if (!inInit) return;
    if (cfg.registry === 'none') r.replace(ptr(0));
    else if (cfg.registry !== null) { this.out.writeS32(cfg.registry); r.replace(ptr(1)); }
    log.probes.push(['registry', this.key, r.toInt32() & 0xff, this.out.readS32()]);
  },
});
Interceptor.replace(A.SaveRegistry, new NativeCallback(() => {}, 'void', ['pointer', 'int'], 'mscdecl'));
Interceptor.replace(A.ShowAnim, new NativeCallback(() => {}, 'void', ['pointer'], 'mscdecl'));

let saved = false;
Interceptor.attach(A.CampaignGameStart, {
  onEnter() { if (!saved && A.FajStart.readU8()) { this.mine = true; dump('pre'); } },
  onLeave() {
    if (!this.mine || saved || log.init === null) return;
    saved = true;
    dump('post');
    loadSave(cfg.slot, Memory.allocAnsiString('NewGame'), 2);
    send({ type: 'save', slot: cfg.slot, log, ido: A.Ido.readS32() });
    send({ type: 'done' });
  },
});

let frames = 0;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (frames !== cfg.startAfterFrames) return;
    if (cfg.mp) {
      saved = true;
      const mp = cfg.mp;
      dump('pre');
      const w8 = (at, v) => va(at).writeU8(v), w32 = (at, v) => va(at).writeS32(v);
      w8(0xbae2c0, 1); w8(0xbae297, 1); w32(0x912a3c, mp.en); w32(0xbb6798, mp.en);
      w32(0x912acc, 8); w32(0x912ad0, 8); w32(0x912ac8, mp.planets); w32(0xbae54c, mp.speed);
      w32(0xbae554, 1); w32(0xbae544, 0);
      for (let i = 1; i <= 18; i++) {
        w8(0xbae560 + i, mp.humans.includes(i) ? 1 : 2);
        w8(0xbae6ac + i, i <= 8 && mp.alive.includes(i) ? 1 : 0);
      }
      // Start_Init's races (each empire its own, 15..=18 → 1, 3, 1, 1), then the lobby's picks.
      for (let i = 1; i <= 18; i++) w8(0xbae574 + i, { 15: 1, 16: 3, 17: 1, 18: 1 }[i] ?? i);
      for (const [i, r] of Object.entries(mp.races || {})) w8(0xbae574 + Number(i), r);
      initNewGame(mp.seed, 2, 0, 1);
      dump('post');
      loadSave(cfg.slot, Memory.allocAnsiString('NewGame'), 2);
      send({ type: 'save', slot: cfg.slot, log, ido: A.Ido.readS32() });
      send({ type: 'done' });
      return;
    }
    if (cfg.scenario) {
      saved = true;
      dump('pre');
      betolt(Memory.allocAnsiString(cfg.scenario));
      dump('post');
      loadSave(cfg.slot, Memory.allocAnsiString('NewGame'), 2);
      send({ type: 'save', slot: cfg.slot, log, ido: A.Ido.readS32() });
      send({ type: 'done' });
      return;
    }
    // The race menu's choice: campaign N (race index N - 1), the difficulty selector (0 → 1, 2 → 2, 1 → 4); the
    // main screen's update (MainScr_Kilep) calls CampaignGameStart once these are set.
    A.FajStart.writeU8(1); A.FajSzetnyilas.writeFloat(100.0); A.FajRace.writeS32(cfg.campaign - 1);
    A.FajNehezseg.writeS32({ 1: 0, 2: 2, 4: 1 }[cfg.difficulty] ?? 2); A.DebugSeed.writeS32(cfg.seed);
    send({ type: 'armed', frames });
  },
});

send({ type: 'ready', base: mod.base.toString() });
