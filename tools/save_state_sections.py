"""Cut the game's save out of PCSX2 save states, offline.

A save state (`.p2s`) is a zip holding the EE's RAM (`eeMemory.bin`, the
address is the offset). The game data the memory card stores lives in one
object at 0x4bbf10 (`FUN_001e0bc0`, built by `FUN_001dadb0`), each section a
flat block the save writes as it is:

| Section | Object | Bytes at | Length |
|---|---|---|---|
| the game record | +0x0 (`FUN_001d3bd0(obj, obj + 0x18, 0x60)`) | +0x18 | 0x60 |
| the options | +0xfc (`FUN_001d6e90`: obj + 0x30) | +0x12c | 0x18 |
| the high scores | +0x144 (`FUN_001d3d08`: obj + 0x18) | +0x15c | 0xa64 |
| the users | +0x1c3c (`FUN_001ef028`: obj + 0x38, 0xbc x 32) | +0x1c74 | 0x1780 |
| the careers | +0x33f4 (`FUN_001eb190`: obj + 0x38, 0xc88 x 10) | +0x342c | 0x7d50 |
| the teams | +0xb57c (`FUN_001edd40`: obj + 0x38, 0x70 x 10) | +0xb5b4 | 0x460 |

Each output is the decoded game file (`street_assets::save::Ps2Save`): six
sections, each a little-endian u32 length and its bytes, 42,020 bytes. The
memory card's own wrapping (CRC, LZARI) is not involved. `street-game`'s
`original_save` test `the_originals_own_save_round_trips_through_the_native_one`
imports them (STREET_ORIGINAL_SECTIONS, paths joined by ';'). No emulator
runs; nothing is written but the outputs.

Usage: python save_state_sections.py <out-prefix> <state.p2s>...
(writes <out-prefix>-<n>.bin per state, in the order given)
"""
import struct
import sys
import zipfile

GAME_DATA = 0x4bbf10
SECTIONS = [
    (0x18, 0x60),
    (0x12c, 0x18),
    (0x15c, 0xa64),
    (0x1c3c + 0x38, 0xbc * 32),
    (0x33f4 + 0x38, 0xc88 * 10),
    (0xb57c + 0x38, 0x70 * 10),
]


def ram(path):
    with zipfile.ZipFile(path) as z:
        name = next(n for n in z.namelist() if n.lower().endswith('eememory.bin'))
        return z.read(name)


def main():
    prefix, states = sys.argv[1], sys.argv[2:]
    for n, path in enumerate(states, 1):
        memory = ram(path)
        out = bytearray()
        for offset, length in SECTIONS:
            start = GAME_DATA + offset
            out += struct.pack('<I', length) + memory[start:start + length]
        assert len(out) == 42020
        with open(f'{prefix}-{n}.bin', 'wb') as f:
            f.write(out)
        print(f'{prefix}-{n}.bin', path)


if __name__ == '__main__':
    main()
