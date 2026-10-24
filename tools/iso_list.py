"""List every file on a PS2 disc image (ISO9660): python tools/iso_list.py <iso>
Prints path, size and LBA."""
import struct, sys
SECTOR = 2048
def walk(f, lba, size, prefix, out):
    f.seek(lba * SECTOR); data = f.read(size); off = 0
    while off < len(data):
        ln = data[off]
        if ln == 0:
            off = (off // SECTOR + 1) * SECTOR; continue
        rec = data[off:off + ln]
        elba = struct.unpack_from('<I', rec, 2)[0]; esize = struct.unpack_from('<I', rec, 10)[0]
        flags = rec[25]; nlen = rec[32]; name = rec[33:33 + nlen]; off += ln
        if name in (b'\x00', b'\x01'): continue
        name = name.decode('latin-1').split(';')[0].rstrip('.')
        path = prefix + '/' + name
        if flags & 2: walk(f, elba, esize, path, out)
        else: out[path.upper()] = (elba, esize)
f = open(sys.argv[1], 'rb'); f.seek(16 * SECTOR); pvd = f.read(SECTOR); root = pvd[156:190]
files = {}; walk(f, struct.unpack_from('<I', root, 2)[0], struct.unpack_from('<I', root, 10)[0], '', files)
for p, (l, s) in sorted(files.items()): print(p, s, l)
