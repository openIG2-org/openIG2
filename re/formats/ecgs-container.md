# `ecGs` / `neGs` / `pcGs` asset container (Gyroscope remaster)

Status: **verified**. All 17,575 files in the GOG install decode with 0 failures using
`tools/ecgs_decode.py` (run 2026-10-15).

Every proprietary file in the remaster (textures, models, scripts, audio, video, configs,
shaders, fonts) is wrapped in this container. The original file extension is kept.

## Evidence (ELF `ig2`, unstripped, sha256 3c18156c…6199)

| Symbol | Address | Role |
|---|---|---|
| `AssetHelper::RWBuffer_DetectType(rw_buffered_t*)` | 0x8c9070 | magic dispatch, header parse |
| `AssetHelper::RWBuffer_Getkey(const string&)` | 0x8c8ee0 | per-file key, lazy global key |
| `StringEncoder::file_encdec(char*, int, const u64&, u32, int)` | 0x8f4110 | XOR keystream |
| `StringEncoder::inthash(u32)` | 0x8f15e0 | Thomas Wang 32-bit hash |
| `StringEncoder::raw_encdec(const string&, u32, bool)` | 0x8f1660 | unwraps the global key |
| `string_hash_t::compute(const char*)` | 0x41c920 | filename hash |
| `compression::decompress(const void*, size_t)` | 0x8dd4f0 | wraps `LZ4_uncompress` (legacy block API) |
| `main` | 0x441090 (+0x1171) | assigns `AssetHelper::filekey_base64 = "6fCoWoWJGGw"` (literal @0xd3fd8d) |
| `StringEncoder::cryptKey` | .data 0x119e72c | `0x5F148D4B` |

The Windows `ig2.exe` (PE32, MSVC 2013) has the same code. Its magic compares are at file
offsets 0x2c9821..0x2c98b3.

## Layout

All integers are little-endian.

```
off  size  field
0    4     magic: "ecGs" (0x73476365) | "neGs" (0x7347656E) | "pcGs" (0x73476370)
--- ecGs / pcGs ---
4    4     packed_len   (encrypted for ecGs, keystream offset 0)
8    4     raw_len      (encrypted for ecGs, keystream offset 4)
12   n     LZ4 block, packed_len bytes (encrypted for ecGs, keystream offset 8)
--- neGs ---
4    n     payload, encrypted, keystream offset 0, no compression
```

A file without a known magic is read as-is.

The kinds used in practice: `neGs` for `.ogg` and `.mp4`, so that streams can seek. `ecGs` for
everything else. `pcGs` doesn't occur in the GOG install.

## Keys

```
gkey  : u64 = LE64( raw_decode( base64("6fCoWoWJGGw"), seed = 0x5F148D4B )[0..8] )
fkey  : u32 = hash( lowercase( basename(path) ) )
hash(s) : h = 0; for each byte c (as signed char): h = (h + c) * 0x1003F   (mod 2^32)
raw_decode(bytes, seed): per u32 word w_i: seed = inthash(seed); w_i -= seed
```

Because the key depends only on the basename, files can be moved between directories
(e.g. into `localization/xx/`) without re-encrypting.

## Keystream (`file_encdec`)

```
inthash(a): a = a ^ 0x3D ^ (a >> 16); a += a << 3; a ^= a >> 4; a *= 0x27D4EB2D; a ^= a >> 15

laneA = hi32(gkey) + inthash(fkey)
laneB = lo32(gkey) + inthash(fkey + 0x544836)
for word index w (stream offset / 4):
    if w % 4 == 0: h = inthash(w / 4); kA = laneA ^ h; kB = laneB ^ h
    k = (w odd) ? (kA = inthash(kA)) : (kB = inthash(kB))
    bytes[4w .. 4w+4] ^= LE32(k)
```

The stream is re-seeded every 16 bytes, so random access is possible. That's why `neGs`
media can be streamed and seeked. Encrypt and decrypt are the same operation.

## Rust implementation notes

- `ig2-formats::container` should expose a `Read + Seek` adapter for `neGs` (video, audio) and
  a one-shot decode for `ecGs`.
- LZ4: use `lz4_flex::block::decompress(src, raw_len)`. It's the same legacy raw block format.
- The keys are a deterministic recipe recovered from the binary, not something we ship as an
  asset. The engine derives them at runtime.
