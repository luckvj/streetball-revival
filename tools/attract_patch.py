"""Make the project PCSX2's next attract match a given one: court, teams and
the timer count its reseed reads. Patches code in emulated RAM only, over
PINE (the ISO, the runtime and its cards are untouched).

FUN_001db7f8 draws the attract's court (`r % 10`, at 0x1db818) and slot 0's
team (`r % 28`, 0x1db880, the game's side 1) from the shared generator before
any reseed, so they follow the boot's clock seed; slot 1's team (0x1db8b8,
side 0) is drawn after slot 0's set reseeded. FUN_001dbe30 reads the timer
count (`jal FUN_003998c0` at 0x1dbeec) for both sets' reseeds. Each draw
becomes `addiu v0, zero, value`; the count becomes `lui/ori v0`.

The timer itself is set to that count there too (a stub at 0xa0900: the
count stored at 0x4b32e4, gp - 0x7d0c, which the timer's handler counts up):
the speech reseeds `rand()` from the timer's count during the match
(0x1a8908, 0x1aa238), and the litter's `rand()` draws steer its gusts'
shared draws, so the match follows the reseed's absolute time.

A later attract of a long boot can load longer than a fresh boot's first
(the court's vents draw every loading frame): --extra N makes N more draws
of the shared generator at the game update's first call (FUN_0013d868, a
one-shot stub at 0xa0800, a flag at 0xa0ff0; that RAM is zero in every save
state), as that many more loading draws would have.

The match intros pick among the unplayed entries of (offset, used) lists in
.data (FUN_001851f0, which always draws): the court's pair at 0x3db318 +
court*16 and the thirteen generic scripts at 0x3db2b0. A later attract
finds some played; --intro A J marks every entry but the court's A and the
generic J played, so the picks are those whatever the draws.

Usage: python attract_patch.py --court K --slot0 T [--side0 T] --count C [--extra N]
                               [--intro A J] [--note FILE]
"""
import argparse
import json
import socket
import struct
import time

PORT = 28011
JAL_RNG = 0x0C000000 | (0x3A16B8 >> 2)
JAL_COUNT = 0x0C000000 | (0x3998C0 >> 2)
SITES = {"court": 0x1DB818, "slot0": 0x1DB880, "side0": 0x1DB8B8}
COUNT = 0x1DBEEC


class Pine:
    def __init__(self):
        self.s = socket.create_connection(("127.0.0.1", PORT), 5)
        self.s.settimeout(10)

    def exact(self, n):
        d = b""
        while len(d) < n:
            p = self.s.recv(n - len(d))
            if not p:
                raise ConnectionError("PINE closed")
            d += p
        return d

    def rpc(self, cmd):
        self.s.sendall(struct.pack("<I", len(cmd) + 4) + cmd)
        n = struct.unpack("<I", self.exact(4))[0]
        return self.exact(n - 4)

    def read(self, a):
        r = self.rpc(b"\x02" + struct.pack("<I", a))
        return struct.unpack_from("<I", r, 1)[0]

    def write(self, a, v):
        self.rpc(b"\x06" + struct.pack("<II", a, v))


TIMER_STUB = 0xA0900
UPDATE = 0x13D868
STUB = 0xA0800
FLAG = 0xA0FF0


def I(op, rs, rt, imm):
    return (op << 26) | (rs << 21) | (rt << 16) | (imm & 0xFFFF)


def extra_draws(p, n):
    """The stub's words and the entry's jump (the jump written last)."""
    SP, RA, S0, T0, T1 = 29, 31, 16, 8, 9
    disp = [p.read(UPDATE), p.read(UPDATE + 4)]
    assert disp == [0x27BDFF30, 0x7FB20040], "game update entry moved"
    assert p.read(FLAG) == 0 and p.read(STUB) == 0, "stub RAM in use"
    code = [
        I(0x0F, 0, T0, FLAG >> 16),               # lui t0, flag
        I(0x23, T0, T1, FLAG & 0xFFFF),           # lw t1, flag(t0)
        None,                                     # bne t1, zero, done
        0,
        I(0x09, 0, T1, 1),                        # addiu t1, zero, 1
        I(0x2B, T0, T1, FLAG & 0xFFFF),           # sw t1, flag(t0)
        I(0x09, SP, SP, -0x60),                   # addiu sp, sp, -0x60
        I(0x1F, SP, RA, 0x00), I(0x1F, SP, 4, 0x10), I(0x1F, SP, 5, 0x20),
        I(0x1F, SP, 6, 0x30), I(0x1F, SP, 7, 0x40), I(0x1F, SP, S0, 0x50),
        I(0x0F, 0, S0, n >> 16),                  # lui s0, n
        I(0x0D, S0, S0, n & 0xFFFF),              # ori s0, s0, n
        JAL_RNG,                                  # loop: jal FUN_003a16b8
        0,
        I(0x09, S0, S0, -1),                      # addiu s0, s0, -1
        None,                                     # bne s0, zero, loop
        0,
        I(0x1E, SP, RA, 0x00), I(0x1E, SP, 4, 0x10), I(0x1E, SP, 5, 0x20),
        I(0x1E, SP, 6, 0x30), I(0x1E, SP, 7, 0x40), I(0x1E, SP, S0, 0x50),
        I(0x09, SP, SP, 0x60),                    # addiu sp, sp, 0x60
        disp[0], disp[1],                         # done: the displaced two
        0x08000000 | ((UPDATE + 8) >> 2),         # j entry + 8
        0,
    ]
    done = code.index(disp[0], 20)
    loop = code.index(JAL_RNG)
    code[2] = I(0x05, T1, 0, done - 3)            # bne t1, zero, done
    code[18] = I(0x05, S0, 0, loop - 19)          # bne s0, zero, loop
    writes = [(STUB + 4 * i, w) for i, w in enumerate(code)]
    writes += [(UPDATE + 4, 0), (UPDATE, 0x08000000 | (STUB >> 2))]
    return writes


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--court", type=int, required=True)
    ap.add_argument("--slot0", type=int, required=True)
    ap.add_argument("--side0", type=int)
    ap.add_argument("--count", type=int, required=True)
    ap.add_argument("--extra", type=int, default=0)
    ap.add_argument("--intro", type=int, nargs=2)
    ap.add_argument("--note")
    a = ap.parse_args()
    t0 = time.time()
    p = None
    while p is None:
        try:
            p = Pine()
            p.read(0x3A16B8)
        except Exception:
            p = None
            time.sleep(1)
            if time.time() - t0 > 120:
                raise SystemExit("no PINE")
    while p.read(0x3A16B8) != 0x3C040043:
        time.sleep(0.05)
    print("ELF up after", round(time.time() - t0, 1), "s", flush=True)
    writes = []
    for key, at in SITES.items():
        value = getattr(a, key)
        if value is None:
            continue
        assert p.read(at) == JAL_RNG, f"{at:#x} is not the generator's call"
        writes.append((at, 0x24020000 | value))
    assert p.read(COUNT) == JAL_COUNT and p.read(COUNT + 4) == 0, "count call moved"
    timer = [
        0x3C020000 | a.count >> 16,          # lui v0, count
        0x34420000 | a.count & 0xFFFF,       # ori v0, v0, count
        I(0x0F, 0, 8, 0x004B),               # lui t0, 0x4b
        I(0x2B, 8, 2, 0x32E4),               # sw v0, 0x32e4(t0)
        0x03E00008,                          # jr ra
        0,
    ]
    assert all(p.read(TIMER_STUB + 4 * i) == 0 for i in range(len(timer))), "stub RAM in use"
    writes += [(TIMER_STUB + 4 * i, w) for i, w in enumerate(timer)]
    writes += [(COUNT, 0x0C000000 | (TIMER_STUB >> 2))]
    if a.extra:
        writes += extra_draws(p, a.extra)
    if a.intro:
        pair = 0x3DB318 + a.court * 16
        for k in range(2):
            if k != a.intro[0]:
                writes.append((pair + 8 * k + 4, 1))
        for j in range(13):
            if j != a.intro[1]:
                writes.append((0x3DB2B0 + 8 * j + 4, 1))
    for at, w in writes:
        p.write(at, w)
    for at, w in writes:
        assert p.read(at) == w
    print("patched", [(hex(at), hex(w)) for at, w in writes], flush=True)
    if a.note:
        with open(a.note, "w") as f:
            json.dump({"court": a.court, "slot0": a.slot0, "side0": a.side0, "count": a.count,
                   "extra": a.extra, "intro": a.intro,
                       "patched": [[at, w] for at, w in writes],
                       "how": "attract_patch.py: FUN_001db7f8's court/team draws and "
                              "FUN_001dbe30's timer count forced in emulated RAM"}, f, indent=1)


if __name__ == "__main__":
    main()
