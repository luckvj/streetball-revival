"""Minimal ISO9660 extractor: python tools/iso_extract.py <iso> <outdir> <path-prefix>...

In Git Bash set MSYS_NO_PATHCONV=1, or prefixes like /FE become Windows paths."""
import os
import struct
import sys

SECTOR = 2048


def walk(f, lba, size, prefix, out):
    f.seek(lba * SECTOR)
    data = f.read(size)
    off = 0
    while off < len(data):
        ln = data[off]
        if ln == 0:
            off = (off // SECTOR + 1) * SECTOR
            continue
        rec = data[off:off + ln]
        elba = struct.unpack_from('<I', rec, 2)[0]
        esize = struct.unpack_from('<I', rec, 10)[0]
        flags = rec[25]
        nlen = rec[32]
        name = rec[33:33 + nlen]
        off += ln
        if name in (b'\x00', b'\x01'):
            continue
        name = name.decode('latin-1').split(';')[0].rstrip('.')
        path = prefix + '/' + name
        if flags & 2:
            walk(f, elba, esize, path, out)
        else:
            out[path.upper()] = (elba, esize)


def main():
    iso, outdir, *prefixes = sys.argv[1:]
    with open(iso, 'rb') as f:
        f.seek(16 * SECTOR)
        pvd = f.read(SECTOR)
        root = pvd[156:190]
        lba, size = struct.unpack_from('<I', root, 2)[0], struct.unpack_from('<I', root, 10)[0]
        files = {}
        walk(f, lba, size, '', files)
        for p, (l, s) in sorted(files.items()):
            if any(p.startswith(x.upper()) for x in prefixes):
                f.seek(l * SECTOR)
                d = f.read(s)
                dst = os.path.join(outdir, p.lstrip('/').replace('/', os.sep))
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                with open(dst, 'wb') as o:
                    o.write(d)
                print(p, s)


if __name__ == '__main__':
    main()
