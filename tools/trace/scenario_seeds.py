"""Recover a scenario start's two wall-clock seeds from run_newgame.py's `.data` dump (a Frida-made call doesn't reach
the srand hook, so `--scenario` runs log none).

Usage: python -I tools/trace/scenario_seeds.py <out-dir>/<tag>   (reads <tag>-post.bin, adds "seeds" to <tag>.json)

- `RandomInit`'s seed: `SDL_GetTicks()` a few ms before the reseed: the latest tick count at or before it whose first
  MSVC `rand()` gives `RandSeed[0]` (the only one the file's `RandSeed v1..v20` leaves).
- `NEWGAME_Diplomacy`'s `srand(SDL_GetTicks())`: the 90 saved draws (cats, parrots, anniversaries, weaknesses, ruler
  ages per empire) pin the tick count; searched upwards from 0.
Spec: re/rules/new-game.md §1 steps 7 and 38.
"""
import json
import struct
import sys

DATA = 0x8de000
tag = sys.argv[1]
post = open(tag + "-post.bin", "rb").read()


def i32(va):
    return struct.unpack_from("<i", post, va - DATA)[0]


def rands(seed):
    h = seed & 0xffffffff
    while True:
        h = (h * 214013 + 2531011) & 0xffffffff
        yield (h >> 16) & 0x7fff


want0 = i32(0xBAF2D0)

want = []
for e in range(1, 19):
    want += [i32(0xA41CD8 + 4 * e), i32(0xA41D28 + 4 * e), i32(0xA41D78 + 4 * e), i32(0xA41DC8 + 4 * e), i32(0xA41E48 + 4 * e)]
mods = [(10, 0), (10, 0), (12, 1), (10, 0), (10, 30)] * 18
ticks = None
for t in range(1 << 32):
    g = rands(t)
    if all(next(g) % m + a == w for (m, a), w in zip(mods, want)):
        ticks = t
        break

# The seed is the tick count a little before the reseed; earlier draws on stream 1 (the extra planets, StoryAdat+196)
# come from RandSeed[1..] before the file overwrites them, so an alias with the same RandSeed[0] isn't enough.
seed = next(s for s in range(ticks, -1, -1) if next(rands(s)) == want0)

j = json.load(open(tag + ".json", encoding="utf-8"))
j["seeds"] = {"seed": seed, "ticks": ticks}
json.dump(j, open(tag + ".json", "w", encoding="utf-8"))
print(tag, j["seeds"])
