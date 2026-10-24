"""List or extract the game's small archives (magic `c0 fb`): the locale files,
/FE/*.BIN (except XAFELOAD, a BIG archive), /CREDITS, /MCARD.

    python tools/c0fb.py <file>            list entries and their first bytes
    python tools/c0fb.py <file> <outdir>   extract entries as <index>[_<name>]

Layout: `c0 fb`, big-endian u16 directory length (bytes after this field),
big-endian u16 entry count, then per entry a big-endian u24 offset, u24 size
and a NUL-terminated name (empty in most /FE files).
"""
import os
import struct
import sys


def entries(d):
    assert d[:2] == b'\xc0\xfb', 'not a c0fb archive'
    count, = struct.unpack_from('>H', d, 4)
    off = 6
    out = []
    for _ in range(count):
        o = int.from_bytes(d[off:off + 3], 'big')
        s = int.from_bytes(d[off + 3:off + 6], 'big')
        off += 6
        e = d.index(b'\0', off)
        out.append((o, s, d[off:e].decode('latin-1')))
        off = e + 1
    return out


def main():
    d = open(sys.argv[1], 'rb').read()
    es = entries(d)
    if len(sys.argv) > 2:
        os.makedirs(sys.argv[2], exist_ok=True)
        for i, (o, s, name) in enumerate(es):
            fn = '%03d' % i + ('_' + name if name else '')
            with open(os.path.join(sys.argv[2], fn), 'wb') as f:
                f.write(d[o:o + s])
    else:
        for i, (o, s, name) in enumerate(es):
            head = d[o:o + 16]
            tag = head[:4].decode('latin-1') if head[:4].isalpha() else head[:8].hex()
            print('%3d %8x %8x %-24s %s' % (i, o, s, name, tag))


if __name__ == '__main__':
    main()
