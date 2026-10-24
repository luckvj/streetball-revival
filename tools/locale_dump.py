"""Dump the game's locale text: python tools/locale_dump.py <XLOC_*.EUS> [out.txt]

Each line is `<file>#<index>: <text>`. The archive is `c0 fb`, a big-endian
u16 header length and u16 file count, then per file a big-endian u24 offset,
u24 size and a NUL-terminated name. Each file is a `LOCH` chunk (0x14 bytes)
then a `LOCL` chunk: 'LOCL', u32 size, u32 0, u32 count, count u32 offsets
from the start of the chunk to NUL-terminated UTF-16LE strings. Commas and
other punctuation are stored as literal `\\xNN` text.
"""
import re
import struct
import sys


def files(d):
    assert d[:2] == b'\xc0\xfb', 'not a locale archive'
    count, = struct.unpack_from('>H', d, 4)
    off = 6
    for _ in range(count):
        o = int.from_bytes(d[off:off + 3], 'big')
        s = int.from_bytes(d[off + 3:off + 6], 'big')
        off += 6
        e = d.index(b'\0', off)
        yield d[off:e].decode('latin-1'), d[o:o + s]
        off = e + 1


def strings(chunk):
    loc = chunk.index(b'LOCL')
    n, = struct.unpack_from('<I', chunk, loc + 12)
    offs = struct.unpack_from('<%dI' % n, chunk, loc + 16)
    for x in offs:
        s = e = loc + x
        while chunk[e:e + 2] != b'\0\0':
            e += 2
        yield unescape(chunk[s:e].decode('utf-16le'))


def unescape(s):
    """Punctuation is stored as literal `\\xNN` text (`\\x2c`, `\\x27`, `\\xae`)."""
    return re.sub(r'\\x([0-9a-fA-F]{2})', lambda m: chr(int(m.group(1), 16)), s)


def main():
    d = open(sys.argv[1], 'rb').read()
    out = open(sys.argv[2], 'w', encoding='utf-8') if len(sys.argv) > 2 else sys.stdout
    for name, chunk in files(d):
        for i, s in enumerate(strings(chunk)):
            out.write('%s#%d: %s\n' % (name, i, s.replace('\n', '\\n')))


if __name__ == '__main__':
    main()
