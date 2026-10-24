"""Read the six actors' posed bones out of PCSX2 save states, offline.

A save state (`.p2s`) is a zip holding the EE's RAM (`eeMemory.bin`, the
address is the offset). For each state this writes the actors' place
(+0x30), facing word (+0xa8) and motion (+0x40), their model's scales
(model = actor+0x108: +0x4..+0xc the head, +0x40 the body), their four
animation channels (actor+0xc8's table: +0x50 the record, +0x5c the frame,
+0x60, +0x64 the float frame, +0x68 the rate, +0x14 and +0x48 the flags) and
every bone's world position and rotation (+0x54's records, 0xb0 bytes
each: +0x0 the position, +0x20 the rotation, +0x40 the parent), with the
ball (+0x30, its radius +0x11c and its holder). `street-sim`'s
`tests/pose_disc.rs` poses each actor's channel-0 clip against them
(STREET_POSE_SET names the output, or `street-pose-set.json` under
STREET_EVIDENCE or the temp folder). No emulator runs; nothing is written
but the output.

Usage: python pose_capture.py <out.json> <state.p2s>...
"""
import json
import math
import struct
import sys
import zipfile

GAME_ROOT = 0x3d30bc


def finite(value):
    if isinstance(value, float) and not math.isfinite(value):
        return 0.0
    if isinstance(value, list):
        return [finite(v) for v in value]
    if isinstance(value, dict):
        return {k: finite(v) for k, v in value.items()}
    return value


def state(path):
    ee = zipfile.ZipFile(path).read('eeMemory.bin')

    def word(a):
        return struct.unpack_from('<I', ee, a & 0x1ffffff)[0]

    def signed(a):
        return struct.unpack_from('<i', ee, a & 0x1ffffff)[0]

    def floats(a, n):
        return [struct.unpack_from('<f', ee, (a + 4 * k) & 0x1ffffff)[0] for k in range(n)]

    game = word(GAME_ROOT)
    ball = word(game + 0x130)
    actors = []
    for team in (word(game + 0x184), word(game + 0x188)):
        array = word(team + 0x44)
        actors += [word(array + 4 * k) for k in range(3)]
    holder = word(ball + 0xf4)
    out = {'state': path.rsplit('.', 2)[-2], 'tick': word(game + 0x128),
           'ball': floats(ball + 0x30, 3), 'ball_radius': floats(ball + 0x11c, 1)[0],
           'ball_holder': actors.index(holder) if holder in actors else -1, 'actors': []}
    for a in actors:
        model = word(a + 0x108)
        table = word(a + 0xc8)
        channels = []
        for c in range(4):
            ch = word(table + 4 * c)
            channels.append({'clip': signed(ch + 0x50), 'frame': signed(ch + 0x5c),
                             'f60': signed(ch + 0x60), 'time': floats(ch + 0x64, 1)[0],
                             'rate': floats(ch + 0x68, 1)[0], 'flags14': word(ch + 0x14),
                             'flags48': word(ch + 0x48)})
        bones = []
        records = word(model + 0x54)
        for b in range(word(model)):
            r = records + b * 0xb0
            bones.append({'p': floats(r, 3), 'q': floats(r + 0x20, 4), 'parent': signed(r + 0x40)})
        out['actors'].append({'position': floats(a + 0x30, 3), 'heading': signed(a + 0xa8),
                              'velocity': floats(a + 0x40, 3), 'model_head': floats(model + 4, 3),
                              'scale': floats(model + 0x40, 3), 'channels': channels,
                              'bones': bones})
    return out


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    states = [state(p) for p in sys.argv[2:]]
    with open(sys.argv[1], 'w', encoding='utf-8') as f:
        json.dump(finite(states), f, allow_nan=False)
    print('states', len(states), 'actors', sum(len(s['actors']) for s in states))


if __name__ == '__main__':
    main()
