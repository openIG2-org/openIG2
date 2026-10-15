"""Reference decoder for the Gyroscope IG2 remaster asset container (ecGs/neGs/pcGs).

Spec: re/formats/ecgs-container.md. Evidence: ELF `ig2` symbols
AssetHelper::RWBuffer_DetectType @0x8c9070, StringEncoder::file_encdec @0x8f4110,
StringEncoder::inthash @0x8f15e0, string_hash_t::compute @0x41c920,
compression::decompress @0x8dd4f0 (LZ4_uncompress).

Usage: python -I ecgs_decode.py <in-file> [<out-file>]
Never write decoded output into the repo.
"""
import os
import struct
import sys

M32 = 0xFFFFFFFF
MAGIC_EC = 0x73476365  # "ecGs": encrypted + LZ4
MAGIC_NE = 0x7347656E  # "neGs": encrypted only
MAGIC_PC = 0x73476370  # "pcGs": LZ4 only


def inthash(a: int) -> int:
    a = (a ^ 0x3D ^ (a >> 16)) & M32
    a = (a + (a << 3)) & M32
    a ^= a >> 4
    a = (a * 0x27D4EB2D) & M32
    a ^= a >> 15
    return a


def name_key(path: str) -> int:
    h = 0
    for b in os.path.basename(path).lower().encode("latin-1"):
        c = b - 256 if b >= 128 else b  # movsx: signed char
        h = ((h + c) * 0x1003F) & M32
    return h


def file_encdec(buf: bytearray, start: int, length: int, gkey: int, fkey: int, offset: int) -> None:
    """In-place XOR of buf[start:start+length], which sits at stream position `offset`."""
    lane_a = ((gkey >> 32) + inthash(fkey)) & M32
    lane_b = ((gkey & M32) + inthash((fkey + 0x544836) & M32)) & M32
    base = (offset // 16) * 16
    i = -(offset - base)
    w = base >> 2
    ka = kb = 0
    while i < length:
        if w & 3 == 0:
            h = inthash(w >> 2)
            ka, kb = lane_a ^ h, lane_b ^ h
        if w & 1:
            ka = inthash(ka); k = ka
        else:
            kb = inthash(kb); k = kb
        w += 1
        if i >= -3:
            for j in range(max(0, -i), min(4, length - i)):
                buf[start + i + j] ^= (k >> (8 * j)) & 0xFF
        i += 4


def lz4_block(src: bytes, out_len: int) -> bytes:
    out = bytearray()
    p = 0
    while True:
        tok = src[p]; p += 1
        lit = tok >> 4
        if lit == 15:
            while True:
                b = src[p]; p += 1; lit += b
                if b != 255:
                    break
        out += src[p:p + lit]; p += lit
        if len(out) >= out_len:
            break
        off = src[p] | (src[p + 1] << 8); p += 2
        ml = tok & 15
        if ml == 15:
            while True:
                b = src[p]; p += 1; ml += b
                if b != 255:
                    break
        ml += 4
        s = len(out) - off
        for k in range(ml):
            out.append(out[s + k])
    return bytes(out[:out_len])


CRYPT_KEY = 0x5F148D4B          # StringEncoder::cryptKey (.data @0x119e72c)
FILEKEY_B64 = "6fCoWoWJGGw"     # literal @0xd3fd8d, assigned to AssetHelper::filekey_base64 in main()


def raw_dec(data: bytes, seed: int) -> bytes:
    """StringEncoder::raw_encdec(..., decode) @0x8f1660: subtract a Wang-hash keystream per u32 word."""
    n = (len(data) - 1) // 4 + 1
    buf = bytearray(data) + bytes(n * 4 - len(data))
    for i in range(n):
        seed = inthash(seed)
        w = struct.unpack_from("<I", buf, 4 * i)[0]
        struct.pack_into("<I", buf, 4 * i, (w - seed) & M32)
    return bytes(buf[:len(data)])


def global_key() -> int:
    """AssetHelper::RWBuffer_Getkey @0x8c8ee0: first 8 bytes of the unwrapped key, little-endian."""
    import base64
    raw = base64.b64decode(FILEKEY_B64 + "=" * (-len(FILEKEY_B64) % 4))
    return struct.unpack("<Q", raw_dec(raw, CRYPT_KEY)[:8])[0]


GKEY = global_key()


def decode(path: str, data: bytes, gkey: int = GKEY) -> bytes:
    if len(data) < 4:
        return data
    magic = struct.unpack_from("<I", data)[0]
    fkey = name_key(path)
    if magic == MAGIC_NE:
        buf = bytearray(data[4:])
        file_encdec(buf, 0, len(buf), gkey, fkey, 0)
        return bytes(buf)
    if magic in (MAGIC_EC, MAGIC_PC) and len(data) >= 12:
        buf = bytearray(data)
        if magic == MAGIC_EC:
            file_encdec(buf, 4, 8, gkey, fkey, 0)
        packed, raw = struct.unpack_from("<II", buf, 4)
        if magic == MAGIC_EC:
            file_encdec(buf, 12, packed, gkey, fkey, 8)
        return lz4_block(bytes(buf[12:12 + packed]), raw)
    return data  # plain file


if __name__ == "__main__":
    src = sys.argv[1]
    out = decode(src, open(src, "rb").read())
    if len(sys.argv) > 2:
        open(sys.argv[2], "wb").write(out)
    else:
        sys.stdout.buffer.write(out[:512])
