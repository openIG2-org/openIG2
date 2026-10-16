# `.ani`: SpidyAni v2 palettised animations

Status: **verified**. All 14 files in `anim/fejek/` (talking heads; 13 × 100 frames + `ures.ani` × 3,
all 128×128) decode completely (`ig2_formats::ani`, golden test `every_ani_decodes_all_frames`),
and the viewer's contact sheets show clean frames for both stored and LZSS-packed files.

The 1999 game also used `.ani` for building and unit animations (`HADIGYAR.ani`, `KISLAKOHAZ.ani`,
…). The remaster replaced those with `anim/mpeg4/*.mp4`, and only the heads remain as `.ani`.

## Evidence (ELF `ig2`, source file `../sources_base/anim.cpp`)

| Symbol | Address | Role |
|---|---|---|
| `InitAnim(char*, bool, bool, float, bool)` | 0x45d1f0 | stores options, async `loadremotefile` → lambda |
| `InitAnim(...)::$_0::operator()` | 0x45ece0 | opens the file, checks magic and version, parses the header, first `Pal ` |
| `LoadFrame()` | 0x45e460 | streaming reads: chunk sizes |
| `OneFrame()` | 0x45da70 | per-frame decode: palette chunk, data chunk, pixel opcodes (inline asm, 32-bit path) |
| `Decode(int)` | — | LZSS decompressor (inline asm) |
| `AnimMakePaletteTable(uchar*, bool)` | — | palette → screen pixel format (`ConvertPixel2`/`3`) |

## Header (19 bytes, little-endian)
```
0   char[8] "SpidyAni"
8   char    '2'            // version; anything else → "ERROR: Bad version of animation!"
9   u16     flags          // 0x02 "kell paletta" (set in no shipped file), 0x04 = 256 colours (else 128),
                           // 0x20 = frames may be LZSS-packed
11  u16     frames         // AnimFazisok
13  u16     width          // AnimX
15  u16     height         // AnimY
17  u16     blocks         // AnimHanyBlokk: 1 everywhere; stored (ELF 0x45f086), never read
```

## Chunks (repeat until EOF)
```
"Pal "  u8 rgb[256][3]                    // 772 bytes; 8-bit components; may precede any frame
"Data"  u16 raw_len  u16 packed_len  u16 ? (0x0080 in all files)
        unpacked file (flags & 0x20 == 0):  raw_len bytes of opcodes at +10; chunk = 10 + raw_len
        packed file, packed_len == 0xFFFF:  raw_len bytes of opcodes at +10; chunk = 12 + raw_len
        packed file, otherwise:             packed_len bytes of LZSS at +12;   chunk = 12 + packed_len
```
`LoadFrame` and `OneFrame` read only bytes +4..+7 of the `Data` header; bytes 8–9 are never read
[inf: the encoder's colour count, 0x80 = 128, matching flags without 0x04].

### LZSS (`Decode`)
Okumura LZSS: 4096-byte ring, zero-filled, write index starting at 0xFEE. Flag byte LSB-first;
1 = literal byte; 0 = two bytes `lo, hi`: position `lo | (hi & 0xF0) << 4`, length `(hi & 0x0F) + 3`.

### Pixel opcodes (`OneFrame`, 128-colour path)
The frame buffer persists between frames: frames are deltas.

| Byte | Meaning |
|---|---|
| `0x00–0x7F` | one pixel, palette index = byte |
| `0x80 u16 n` | skip n pixels (unchanged) |
| `0x81–0xBF` | skip `b & 0x3F` pixels |
| `0xC0 u16 n u8 c` | n pixels of colour c |
| `0xC1–0xFF u8 c` | `b & 0x3F` pixels of colour c |

Decoding stops when the remaining-pixel counter hits zero *or would go negative*, so a skip may run
past the end ("skip the rest"; `politikus.ani` does this).

The 256-colour path asserts on 32-bit screens (`OneFrame`, anim.cpp:0x311) and no shipped file uses
it. Pixels never written are transparent in our decoder (the game starts from an uninitialised buffer).

## Resolved (2026-10-16)
- Header `blocks` (`AnimHanyBlokk` 0x11ccaf0): the only access in the ELF is the store in the
  `InitAnim` lambda; nothing reads it.
- `Data` bytes 8–9: `LoadFrame` (0x45e460) streams `10 + raw_len` / `12 + packed_len` bytes using only
  the two lengths at +4/+6, and `OneFrame` (0x45da70) decodes from +10 or `Decode(packed_len)`; neither
  touches +8. Readers can ignore both fields.

## Open
- Playback rate: `InitAnim` takes `MaxFps` per call; the diplomacy and spy screens pass their own values (Phase 2).
