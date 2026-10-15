# IG2 reverse-engineering overview (Phase 0 recon)

Date: 2026-10-15. Target: GOG "Imperium Galactica II: Alliances" (gameId 1254614904),
installed at `Imperium_Galactica_II_ORIGINAL_GOG/` beside the repository (gitignored; 688 MB, 17,575 files).

Each claim is marked **[obs]** (directly observed in a file or disassembly), **[inf]**
(inferred), or **[unk]** (unknown).

## 1. The key finding: this is not the 1999 build

The GOG release is the **Gyroscope Games remaster (2016–17, published by THQ Nordic)**:

- [obs] Splash string `REMASTERED BY GYROSCOPE. PUBLISHED BY THQNORDIC`. Build path
  `e:\projects\gyroscope\ig2\_base\sources_base\...`.
- [obs] The remaster is a port of Gyroscope's **iPad/mobile port**. `ig2_common.cfg` decodes to
  `# Imperium Galactica II. iPad port config file example`. The binary still has `IAPManager`,
  `AdManager`, `Social` (Facebook, GameCenter, Google Play), and touch/gesture → mouse conversion.
- [obs] The **original 1999 Digital Reality C game logic is intact**, with its Hungarian
  identifiers (`FRISSIT_*` = update, `*_Belep`/`*_Kilep` = enter/exit screen, `Flotta` = fleet,
  `Epulet` = building, `Kem` = spy, `Talalmany` = invention/tech, `MI` = AI).
  Gyroscope wrapped it in its own C++ engine (`sg3d`, `sgsnd`, `sgnet`, `sgmp`, `sgres`, `sgthread`).
- [obs] The original DirectDraw blitters survive as shims (`DDInit`, `DDBrushToScreen`,
  `DDFillToScreen3d`, …). Rendering goes to OpenGL through GLSL shaders in `render/`.
- [obs] **All assets were re-encoded** into GPU/mobile formats: PVR v3 textures (RGB565 or
  RGBA4444), Ogg Vorbis audio, H.264 MP4 video. Everything is wrapped in an encrypted container
  (see §4). Original 1999 asset files aren't present.
- [obs] **The install ships a Linux x86-64 build `ig2` that is NOT stripped**: 40,336 symbols,
  of which 16,365 are functions with C++-demangled signatures. It was built at -O0 (clang
  style, every local spilled to the stack), which makes it very readable. **This is the primary
  static-analysis oracle.** The Windows `ig2.exe` comes from the same sources (same build date).

Consequences for the project:
- The engine's asset loaders target the **remaster formats**, because that's what users own. A
  1999 CD-format loader is an optional later extra (ADR needed).
- Rule recovery is easier than expected. The sim is the original C code with real names.
- [unk] Did Gyroscope change any rule constants versus 1999? Their "mobile-style sidebars" and
  `HACK_SetBrushok` suggest UI-only changes, but this is unverified. It's tracked as an unknown.

## 2. Executables and libraries

| File | Kind | Notes |
|---|---|---|
| `ig2.exe` | PE32 i386, GUI, MSVC 2013 (linker 12.0, MSVCR120/MSVCP120), 2017-02-06 | Shipped Windows game. Imports OPENGL32, WSOCK32, WININET, IPHLPAPI, WINMM, libfreetype-6. |
| `ig2` | ELF64 x86-64, **unstripped**, 15 MB | Linux build that wasn't cleaned out. Needs libGL, libglut, libGLEW 1.13, libasound. |
| `libGLEW.so.1.13`, `libglut.so.3`, `libsteam_api.so` | ELF64 | Linux runtime leftovers. |
| `libfreetype-6.dll`, `zlib1.dll` (1.2.3) | PE32 | Font rendering; zlib for saves and webcache. |
| `steam_api.dll` | PE32 | Leftover Steamworks stub (the binary also has achievements and `Social`). |
| `goggame-1254614904.dll`, `GameuxInstallHelper.dll` | PE32 | GOG Galaxy and Windows Game Explorer integration. No DRM. |
| `language_setup.exe` | PE32, UPX-packed | Language picker that writes `ig2.settings`. |
| `unins000.*` | Inno Setup | GOG installer. |

There's no wrapper layer like DxWnd or dgVoodoo: the remaster is a native OpenGL program.

**Statically linked** into both binaries [obs, symbols and strings]: SDL2 (+ SDL_image,
SDL_mixer, SDL_ttf, X11 backend on Linux), FFmpeg/libavcodec (H.264, AAC, plus Bink video and
audio decoders), libvorbis/libogg, FreeType, zlib, LZ4 (legacy), xdelta3 (`xd3_*`, patching),
libtomcrypt/libtommath (AES-256, RSA, SHA1, used for IAP and remote assets), jpgd/jpge, rg_etc1,
squish (DXT), a JSON parser, and MSVC/GCC STL.

## 3. Runtime architecture (from ELF symbols)

```
main (0x441090)
 ├─ simple_settings_t::load_from_file("ig2.settings")    window, size, vsync, language, MP server
 ├─ sgmain_initialise → AssetHelper (filekey_base64), sg3d (GL), sgsnd
 ├─ game::initialise / registerScreen(controlscreen) / activateScreen
 └─ game::run  ──►  IG2_MainCallBack → IG2_gameUpdate (per frame)
       ├─ HANDLEMESSAGES, Mouse_Lekerdez (poll mouse), Multi_Send
       ├─ screen state machine: {Starmap, Colony, Research, Design, Diplomacy, Spy, Trade,
       │   Info, Messages, SpaceWar, Options, Start, MainScr, Irc, Help}
       │   × {_Belep enter, _MouseInit, _MouseInit2, _Main, _Kilep exit}
       └─ FRISSITES → Fake_FRISSITES  (the simulation and update tick)
             ├─ network: SocketHandleProc, MultiHandleClosure, MultiSendPackets, SendPing,
             │           MULTISEND_SpWarSynch, MULTISEND_Starmap
             ├─ time:    FRISSIT_Datum, IdoNapValtozott ("day changed")
             ├─ sim:     FRISSIT_Planet, FRISSIT_Viszony (relations), FRISSIT_MI (AI), FRISSIT_Spy,
             │           MI_DIPLOMACY, FRISSIT_Trade, FRISSIT_Research, FRISSIT_Story,
             │           FRISSIT_Epuletek (buildings), FRISSIT_Starmap, FRISSIT_Grwar, FRISSIT_Colony,
             │           FRISSIT_SpaceWar
             └─ per-screen visual updates: FRISSIT_*2, Frissit2_*
IG2_gameInit: CFG_Load, RandomInit, LoadStrTable, INIT_AutoBuild, DDInitProc, Init3DSystem,
              InitMainSound, speech init, InitHatterekEsTerepek (backgrounds/terrains), LoadSave
```

- [obs] AI is split into `FRISSIT_MI_{DESIGN, FEJLESZTES (research), FLOTTAK (fleets),
  GYARTAS (production), KEMKEDES (espionage), KOLONIZALAS (colonization), SZAMOLAS (calc)}`.
- [obs] Space combat is mostly C++ classes (`SpwC::*`, 226 methods). Ground combat is
  `WF`/`WF_Tank` (180 methods) plus the C functions `Grwar*` and `GroundWar*`.
- [inf] The sim tick is called from the render frame (`Fake_FRISSITES` reads `SDL_GetTicks`).
  The fixed-step versus variable-step behaviour still needs to be read out of `FRISSIT_Datum`
  and `SetIdoSpeed`.

### RNG [obs]
- `RandomInit(seed)` (0x6220e0): `srand(seed)`, then fills **21 stream states**
  (`.bss 0x1415d00`) from libc `random()`.
- `rndold(stream, lo, hi)` (0x622130): `s = s*214013 + 2531011; s = rotl32(s, 21)`;
  result `((s >> 16) & 0x7FFF) % (hi - lo + 1) + lo`.
- `rndlog(stream, lo, hi, file, line)` (0x61e170) is the logging variant, with **471 call
  sites** [inf: built for MP desync hunting]. There are also 35 raw libc `rand`/`random` calls
  [inf: cosmetic only; to verify].
- Implication: the seed expansion goes through the platform libc (`random()` on glibc,
  `rand()` on MSVCRT). Matching the original exactly needs both, but only for seeding. Our
  engine will use its own portable RNG (ADR) and keep a "classic" stream layout.

### Saves [obs]
`LoadSave(int, char*)`: `GetSavePath`, then `LoadSaveVariables(int)`, which calls
`LoadSaveVariable(void*, int)` for each global, with zlib `compress`/`uncompress`. That makes a
save a **zlib-compressed sequence of raw global-variable blobs**. The order and sizes are
defined by `LoadSaveVariables`. An importer has to mirror that order and the struct layouts
(32-bit layout on Windows versus 64-bit on Linux, so pointers and padding may differ: [unk]).
Scenario `.scn` files, by contrast, are **plain-text key/value state** (`Money`, `Ev` year,
`Honap` month, `Nap` day, `PlayerFaj` race, `BirodalomIsmeri` contact matrix …).

### Multiplayer [obs]
Two layers:
1. **Lobby**: `sgmp::peer*`, which reimplements the **GameSpy Peer SDK** API (title room, staging
   rooms, ready flags, `peerStartGame`) against a Gyroscope matchmaking server
   (`ig2.settings`: `server_address=127.0.0.1`, `server_port=2100`). There's also a 1999-era IRC
   client ("fIRC v1.01 by Fable").
2. **Game**: the original 1999 TCP layer, `FableSocket`, `MultiPlayer`, `MultiGameComposer`,
   `Multi_Send/Get/ReceiveGameData/ReceiveFullGameData`, `MULTISEND_SpWarSynch`. [inf] It's
   host-relayed with full-state resyncs (`ReceiveFullGameData`, `HandleSendFullData`). That's
   a likely source of the fragility; to be confirmed in Phase 2.

## 4. Asset inventory

**Container:** every proprietary file is wrapped as `ecGs` (encrypted + LZ4) or `neGs`
(encrypted, seekable). The format is fully solved: [formats/ecgs-container.md](formats/ecgs-container.md),
reference decoder `tools/ecgs_decode.py`, with **17,575/17,575 files decoding and 0 failures**.

| Ext | Count | Decoded MB | Content after the container | Status |
|---|---|---|---|---|
| `.ogg` | 7,420 | 243 | Ogg Vorbis (SFX, speech per language, music `mfx/`) | standard ✔ |
| `.txt` | 6,320 | 4.9 | UI and story text, message scripts (`uzenetek/`), Hungarian dev text | text, grammar TBD |
| `.pvr` | 2,081 | 145 | PVR v3: RGB565 (1,840), RGBA4444 (230), DXT1 (9), other (2) | standard ✔ |
| `.xv` | 937 | 86 | Original IG2 3D models (`Material…`/`DEFAULT` sections) | **unknown format** |
| `.mp4` | 423 | 307 | H.264/AAC cutscenes and diplomacy and building animations (`anim/mpeg4`) | standard ✔ |
| `.mou` | 87 | 2.2 | Tutorial mouse/input recordings (binary, `0xCD` padding = MSVC debug fill) | unknown |
| `.map` | 76 | 0.7 | Colony terrain and placement grids | unknown |
| `.dsc` | 64 | <0.1 | Scenario descriptions (`title`, `pic`, `difficulty`, `race`, `desc`) | text ✔ |
| `.scn` | 29 | 8.3 | Scenario and campaign state (key/value text) | text, keys TBD |
| `.eng` | 18 | 0.5 | Diplomacy dialogue tables | text |
| `.ani` | 14 | 5.4 | `SpidyAni` v2: 8-bit paletted animation (`Pal ` chunk), talking heads | **unknown format** |
| `.bin` | 10 | <0.1 | Space-war start formations (`felallas` = formation), int records | unknown |
| `.tut` | 7 | 0.4 | Tutorial scripts | text |
| `.res`, `.m`, `.b` | 8 | <0.1 | Colony resource and lookup tables (float and int arrays) | unknown |
| `.raw` | 3 | 0.9 | HSI "mhwanh" 8-bit paletted images (640×480) | known legacy ✔ |
| `.shader/.program/.material/.state` | 31 | — | Remaster GLSL and render state | text (not needed) |
| `.json` | 3 (+`texts.json`) | — | Remaster menus (C-preprocessed JSON) | text |
| `.ttf/.otf` | 2 | 0.5 | Jura Book, Inconsolata (remaster UI fonts) | standard ✔ |
| `.psd`, `.blend`, `.sgmodel` | 6 | 0.9 | Dev leftovers (Hungarian PSDs, Blender cover, sg model) | ignore |
| `webcache.zip` | 1 | 0.3 | Cached remote web assets (plain ZIP) | inspect |
| `_locale/locale.*` | 6 | — | Plain-text locale tables | text |

Directory map: `dataraw/` (core game data), `localization/{de,es,fr,hu,it}/` (per-language
overrides of the same basenames, English being the base), `anim/` (video and heads),
`mfx/` (music), `sounds/`, `gfx/` and `render/` (remaster UI and shaders).

[obs] The binary references `.xvb` (`a3v0.xvb`, …) and 1999-style `*.ani` building names
(`HADIGYAR.ani`, `KISLAKOHAZ.ani`). In the remaster those animations live in
`anim/mpeg4/*.mp4`. [inf] There's a name-remapping table, probably in `GetAnimFile`.

## 5. Formats still unknown (Phase 1 queue, in priority order)

1. `.xv`: 3D models for ships, buildings and planets. Loader candidates: `AddToXVB`,
   `CreateXVB`, `FreeXVB*`, `GetXVBRoot`.
2. `.map`, `.res`, `.m`, `.b`: colony maps and tables (`Decode2LargeMap`, `GenerateByteHeightMap`, `FixLandscape`).
3. `.scn` key grammar and save blobs (`Betolt_Scenario`, `LoadSaveVariables`).
4. `.txt` dialects: message scripts (`ReadUzenetAnimLista`, `InfoString_Read`), help, autobuild rules (`INIT_AutoBuild`).
5. `.ani` SpidyAni v2 (`InitAnim`, `GetAnimFrame`, `AnimMakePaletteTable`).
6. `.bin` formations, `.mou` tutorial recordings, `.eng` diplomacy (`Dip_LoadFile`).
7. PVR → atlas metadata: how UI sprites are cut (`GetKepAddr`, `LoadKepToMemory`, brush tables).

## 6. Effort estimate per subsystem

Rough figures in focused agent-plus-review work-weeks. The unstripped -O0 ELF cuts static
work by roughly 3–5× compared with an anonymous binary.

| Subsystem | Size signal | RE | Rust impl | Risk |
|---|---|---|---|---|
| Container + standard formats (PVR, OGG, MP4, fonts) | solved | 0 | 0.5 | low |
| Original formats (`.xv`, `.map`, `.ani`, `.bin`, `.mou`, text dialects) | ~8 loaders | 2 | 2 | medium |
| Asset viewer (egui) | — | — | 1 | low |
| Galaxy/strategic sim (time, fleets, planets, economy) | `FRISSIT_Planet/Starmap/Colony/Datum`, 45+36 fns | 3 | 3 | medium |
| Research, design, production | ~60 fns | 1.5 | 1.5 | low |
| Diplomacy, trade, espionage | ~85 fns | 2 | 2 | medium |
| AI (`FRISSIT_MI_*`) | 14 large fns | 2.5 | 2 | high (big functions) |
| Space combat (`SpwC`, 226 methods) | C++ | 3 | 3 | high |
| Ground combat (`WF`, `WF_Tank`, `Grwar`) | C++/C | 3 | 3 | high |
| Campaign/story/tutorial scripting | 54 fns + text scripts | 2 | 2 | medium |
| Original MP protocol (study only) | ~54 fns | 1 | — | low |
| New engine: wgpu renderer, UI, DPI, audio, video | — | — | 6 | medium |
| Deterministic lockstep MP + QUIC + lobby | — | — | 5 | high |
| Saves (new format + original importer) | `LoadSaveVariables` | 1 | 1.5 | medium |
| **Total** | | **~21** | **~32** | |

## 7. Tooling notes and blockers

- REA's Windows Ghidra provider only accepts PE. The ELF is analysed with binutils
  (`nm`/`objdump`) and a headless Ghidra project at `_local\ghidra` (gitignored, since it
  contains binary bytes).
- REA `binary_overview` on `ig2.exe` timed out twice: auto-analysis of the 5.8 MB PE runs past
  the MCP request timeout and the session closes. Workaround: the pre-built headless project,
  and/or a smaller analysis profile. **Next step:** port ELF symbol names onto `ig2.exe` (string,
  xref and call-graph matching) so the REA database for the shipped exe carries the recovered
  names for Frida tracing.
- Decoded assets for local study go to `_local\extracted\` (gitignored). **Never commit them.**
- The game install (`Imperium_Galactica_II_ORIGINAL_GOG/`) may sit in the project root; it is gitignored.
