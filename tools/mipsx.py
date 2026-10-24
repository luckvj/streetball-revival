"""PS2 EE ELF helper: map vaddr<->file offset, find code that builds an address with lui/addiu|ori,
and disassemble ranges with capstone (MIPS III, which covers almost all EE integer code)."""
import struct, sys, capstone

class Exe:
    def __init__(self, path):
        self.d = open(path, 'rb').read()
        phoff, = struct.unpack_from('<I', self.d, 0x1c)
        phnum, = struct.unpack_from('<H', self.d, 0x2c)
        self.segs = []
        for i in range(phnum):
            t, off, va, pa, fsz, msz, fl, al = struct.unpack_from('<8I', self.d, phoff + 32 * i)
            if t == 1:
                self.segs.append((va, off, fsz, msz))
        self.md = capstone.Cs(capstone.CS_ARCH_MIPS, capstone.CS_MODE_MIPS64 + capstone.CS_MODE_LITTLE_ENDIAN)
    def off(self, va):
        for v, o, fs, ms in self.segs:
            if v <= va < v + fs: return o + va - v
    def va(self, off):
        for v, o, fs, ms in self.segs:
            if o <= off < o + fs: return v + off - o
    def word(self, va):
        return struct.unpack_from('<I', self.d, self.off(va))[0]
    def refs(self, target):
        """Code addresses where lui rX,hi ; ... addiu/ori/lw etc rY,rX,lo build `target` (within 64 insns)."""
        hi = (target + 0x8000) >> 16 & 0xffff
        lo = target & 0xffff
        out = []
        for v, o, fs, ms in self.segs:
            for k in range(0, fs - 4, 4):
                w, = struct.unpack_from('<I', self.d, o + k)
                if w >> 26 == 0x0f and w & 0xffff == hi:
                    r = w >> 16 & 31
                    for j in range(1, 64):
                        if o + k + 4 * j + 4 > len(self.d): break
                        w2, = struct.unpack_from('<I', self.d, o + k + 4 * j)
                        if (w2 >> 21 & 31) == r and w2 & 0xffff == lo and w2 >> 26 in (0x09, 0x0d, 0x23, 0x2b, 0x31, 0x39, 0x20, 0x24, 0x21, 0x25, 0x28, 0x29, 0x37, 0x3f):
                            out.append(v + k + 4 * j); break
        return out
    def dis(self, va, n):
        """(address, mnemonic, operands) with the EE-only opcodes capstone gets wrong decoded by hand."""
        regs = ['zero', 'at', 'v0', 'v1', 'a0', 'a1', 'a2', 'a3', 't0', 't1', 't2', 't3', 't4', 't5', 't6', 't7',
                's0', 's1', 's2', 's3', 's4', 's5', 's6', 's7', 't8', 't9', 'k0', 'k1', 'gp', 'sp', 'fp', 'ra']
        o = self.off(va)
        for i in range(n):
            a = va + 4 * i
            w, = struct.unpack_from('<I', self.d, o + 4 * i)
            op, rs, rt, imm = w >> 26, w >> 21 & 31, w >> 16 & 31, w & 0xffff
            simm = imm - 0x10000 if imm & 0x8000 else imm
            if op in (0x1e, 0x1f):
                yield a, 'lq' if op == 0x1e else 'sq', f'${regs[rt]}, {simm:#x}(${regs[rs]})'
                continue
            if op == 0x1c:
                yield a, 'mmi', f'{w:#010x} rs=${regs[rs]} rt=${regs[rt]} rd=${regs[w >> 11 & 31]} f={w & 63:#x}/{w >> 6 & 31:#x}'
                continue
            if op == 0x12:  # COP2 (VU0 macro)
                yield a, 'cop2', f'{w:#010x}'
                continue
            ins = next(self.md.disasm(self.d[o + 4 * i:o + 4 * i + 4], a), None)
            if ins is None:
                yield a, '.word', f'{w:#010x}'
            else:
                yield a, ins.mnemonic, ins.op_str

if __name__ == '__main__':
    e = Exe(sys.argv[1])
    if sys.argv[2] == 'refs':
        for a in sys.argv[3:]:
            t = int(a, 0)
            print(hex(t), [hex(x) for x in e.refs(t)])
    else:
        for a, m, ops in e.dis(int(sys.argv[2], 0), int(sys.argv[3], 0)):
            print(f'{a:08x}  {m:8s} {ops}')
