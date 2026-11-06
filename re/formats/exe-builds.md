# Other `ig2.exe` builds

Every rule table, string and constant the engine reads from `ig2.exe` is catalogued at its address in the GOG build
(sha256 `2baa530b…0442`, PE `TimeDateStamp` 0x5898a3c4, 5,864,960 bytes). Other stores ship other builds of the same
program with the same data at other addresses. `ig2_formats::exe_builds` rebuilds such an exe in the GOG layout
when it is read (`Install::read_raw("ig2.exe")` and the install check), so every reader works unchanged.

## Known builds

| Build | Store | `TimeDateStamp` | Size | sha256 |
|---|---|---|---|---|
| GOG 2017-02-06 | GOG | 0x5898a3c4 | 5,864,960 | `2baa530be6bc86000b9d63e7c47a6131f0d0f4a6cc8d3f1897f5db0427950442` |
| Steam 2018-04-18 | Steam (`steamapps/common/Imperium Galactica II`) | 0x5ad7b7b1 | 5,766,144 | `deef42c9b04a11b7a1eb593948610fb3bb3027a4c1c0f24d514657439d34e39e` |

The Steam install's data files (`dataraw/`, `anim/`, `localization/`, `sounds/`, `fonts/`, `gfx/`, `render/`, …) are
byte-identical to GOG's; it adds `.filedb.json`, `.list.dir` files and `_Modding/`, and lacks the GOG extras
(`_locale/`, the manual, the GOG Galaxy files). Only `ig2.exe` differs. (Steam's folder also has an `ig2.exe.bak`,
`TimeDateStamp` 0x58ba9589, 5,765,632 bytes, sha256 `54d06d78…400b`: not mapped.)

## The Steam 2018 build against GOG

Same section order and names. Section VAs (image base 0x400000 in both):

| Section | GOG VA, raw size | Steam VA, raw size |
|---|---|---|
| `.text` | 0x401000, 0x433e00 | 0x401000, 0x41fe00 |
| `.rdata` | 0x835000, 0xa8600 | 0x821000, 0xa5a00 |
| `.data` | 0x8de000, 0x55200 (virtual 0x7924e4) | 0x8c7000, 0x54e00 (virtual 0x7920a4) |
| `.rodata` | 0xc71000, 0xa00 | 0xc5a000, 0xa00 |
| `_RDATA` | 0xc72000, 0xa00 | 0xc5b000, 0xa00 |

`tools/exe_builds/align.py` masks every relocated dword (the base-relocation directory's type-3 entries) in both
images, finds each unique 16-byte window of the GOG `.rdata`/`.data`/`.rodata`/`_RDATA` in the other build, extends
the anchored runs byte by byte, keeps the longest runs first (a shorter run keeps only the Steam bytes no longer
run took) and writes them as (GOG VA, Steam VA, length) segments (`engine/crates/ig2-formats/src/exe_builds/steam2018.rs`).
Result:

- `.data` 347,820 of 348,672 bytes mapped (18 segments), `.rdata` 639,726 of 689,664 (124), `.rodata` and `_RDATA`
  whole.
- 0 non-pointer bytes differ inside the mapped places: the rule values are the same as GOG's.
- Of the Steam pointers in mapped places, 5,641 map onto the GOG pointer at the same place, 5,198 point into code
  (left as Steam's value) and 8 disagree. All 8 and the unmapped `.rdata` gaps lie in the MSVC C++ runtime
  metadata (RTTI `.?AV…` type descriptors, `FuncInfo`/unwind maps of `0xffffffff` records, 0x8c0000…0x8d6000 and
  the import area at 0x835000): no reader uses them.

### Code the engine decodes

Four readers decode instructions, not data: the speech init functions `InitColonySpeech` 0x47f400 (0x37ee bytes),
`InitSpaceWarSpeech` 0x55e8b0 (0x2cfb) and `StarmapInitSpeech` 0x5ad820 (0x1055) (`ig2_rules::speech`), and `Spy_New`
0x58f830 (0x4c0, the recruit names, `layout::SPY_NEW_CODE`). The aligner finds each in the Steam `.text` by voting
over 24-byte windows with relocated dwords and every `E8` byte's next four bytes masked: Steam 0x47f400, 0x55e740,
0x5ad670, 0x58f640. Inside them the builds differ only in relocated dwords and call `rel32`s (the callee moved: 981
calls, recorded as (GOG operand VA, GOG callee VA); an operand is the earliest `E8` whose target lies in `.text` in
both builds, since an operand may itself start with `E8`). Their relocated operands name 18 uninitialised globals
(`.data` past its raw size, e.g. the speakers `ColonySpeaker` 0xa3ea3c ↔ Steam 0xa275fc, `StarmapSpeaker` 0xf17d88 ↔
0xf00950, `SpaceWarSpeaker` 0xe72e84 ↔ 0xe5ba4c), recorded as (GOG VA, Steam VA) pairs; BSS shifts by 0x17438…0x17440
between them, so no single offset fits. Final table: 116 segments (1,021,978 bytes), 981 calls, 18 globals; 7,272
pointers agree, 8 disagree (the RTTI ones above).

## The rebuilt image

`to_gog_layout(exe)` (known build by `TimeDateStamp` and size, else `None`): the GOG file length, the other build's
first 0x400 header bytes with the GOG section table written in, `openIG2 GOG layout of <build>` over the DOS stub at
0x40 (`converted_from` reads it back; `ig2_rules::check_build` accepts it), each segment's bytes copied to its GOG
place, then every relocated dword of the other build that lies in a segment rewritten to the GOG address of its
target when the target is mapped (a segment, or one of the globals), then each call operand set to `callee − (operand
+ 4)`. The rest of `.text`, `.rsrc`, `.reloc` and the gaps stay zero.

Coverage: the 23,182 distinct `PeImage::read`/`read_loaded` ranges of the workspace's test suite on the GOG install
(logged once) all lie inside the segments or in BSS (zero in both layouts).

## Port status

Ported: `ig2_formats::exe_builds` (Steam 2018), the Steam library search in `ig2_rules::install` (Steam's default
folders and `steamapps/libraryfolders.vdf`). Not mapped: the Steam `ig2.exe.bak` build; the 1999 CD release (other
formats altogether). Multiplayer's rules fingerprint hashes the exe as read, so a Steam and a GOG player see different
rules and cannot join each other (`re/deviations.md`).
