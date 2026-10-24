"""PS2 VU micro-mode disassembler (upper + lower), for reading EAGL microprograms.
Example: the skinned program is 430 instructions at .data+0x5360 of /XF/XF000007 (.data at file offset 0x40):
    python tools/vudis.py XF000007 0x53a0 430
usage: vudis.py file data_offset_of_code count"""
import struct, sys

BC = 'xyzw'


def DEST(c):
    return ''.join(ch for ch, b in zip('xyzw', (24, 23, 22, 21)) if c >> b & 1)


def F(r):
    return f'vf{r:02d}'


def I(r):
    return f'vi{r:02d}'


def simm(v, bits):
    return v - (1 << bits) if v >> (bits - 1) & 1 else v


def upper(c):
    flags = ''.join(n for n, b in (('[I]', 31), ('[E]', 30), ('[M]', 29), ('[D]', 28), ('[T]', 27)) if c >> b & 1)
    d, ft, fs, fd = DEST(c), (c >> 16) & 31, (c >> 11) & 31, (c >> 6) & 31
    op = c & 0x3f
    if op < 0x1c:
        grp = ['ADD', 'SUB', 'MADD', 'MSUB', 'MAX', 'MINI', 'MUL'][op >> 2]
        s = f'{grp}{BC[op & 3]}.{d} {F(fd)}, {F(fs)}, {F(ft)}{BC[op & 3]}'
    elif op < 0x20:
        s = {0x1c: f'MULq.{d} {F(fd)}, {F(fs)}, Q', 0x1d: f'MAXi.{d} {F(fd)}, {F(fs)}, I',
             0x1e: f'MULi.{d} {F(fd)}, {F(fs)}, I', 0x1f: f'MINIi.{d} {F(fd)}, {F(fs)}, I'}[op]
    elif op < 0x30:
        n = ['ADDq', 'MADDq', 'ADDi', 'MADDi', 'SUBq', 'MSUBq', 'SUBi', 'MSUBi',
             'ADD', 'MADD', 'MUL', 'MAX', 'SUB', 'MSUB', 'OPMSUB', 'MINI'][op - 0x20]
        if op < 0x28:
            s = f'{n}.{d} {F(fd)}, {F(fs)}, {n[-1].upper()}'
        else:
            s = f'{n}.{d} {F(fd)}, {F(fs)}, {F(ft)}'
    elif op >= 0x3c:
        bc = op & 3
        k = (c >> 6) & 31
        tab = ['ADDA', 'SUBA', 'MADDA', 'MSUBA', 'ITOF', 'FTOI', 'MULA']
        if k < 4 or k == 6:
            s = f'{tab[k]}{BC[bc]}.{d} ACC, {F(fs)}, {F(ft)}{BC[bc]}'
        elif k in (4, 5):
            s = f'{tab[k]}{[0, 4, 12, 15][bc]}.{d} {F(ft)}, {F(fs)}'
        else:
            names = {(0, 7): 'MULAq', (0, 8): 'ADDAq', (0, 9): 'SUBAq', (0, 10): 'ADDA', (0, 11): 'SUBA',
                     (1, 7): 'ABS', (1, 8): 'MADDAq', (1, 9): 'MSUBAq', (1, 10): 'MADDA', (1, 11): 'MSUBA',
                     (2, 7): 'MULAi', (2, 8): 'ADDAi', (2, 9): 'SUBAi', (2, 10): 'MULA', (2, 11): 'OPMULA',
                     (3, 7): 'CLIP', (3, 8): 'MADDAi', (3, 9): 'MSUBAi', (3, 11): 'NOP'}
            n = names.get((bc, k), f'?up{c:08x}')
            if n == 'NOP':
                s = 'NOP'
            elif n == 'ABS':
                s = f'ABS.{d} {F(ft)}, {F(fs)}'
            elif n == 'CLIP':
                s = f'CLIPw.xyz {F(fs)}, {F(ft)}w'
            elif n in ('ADDA', 'SUBA', 'MADDA', 'MSUBA', 'MULA', 'OPMULA'):
                s = f'{n}.{d} ACC, {F(fs)}, {F(ft)}'
            else:
                s = f'{n}.{d} ACC, {F(fs)}, {n[-1].upper()}'
    else:
        s = f'?up{c:08x}'
    return s + (' ' + flags if flags else '')


def lower(c, pc):
    op = c >> 25
    d, ft, fs = DEST(c), (c >> 16) & 31, (c >> 11) & 31
    it, is_ = ft, fs
    i11 = simm(c & 0x7ff, 11)

    def tgt():
        return f'0x{(pc + 1 + i11) * 8:04x}'

    if op == 0x40:
        f = c & 0x3f
        idr = (c >> 6) & 31
        if f == 0x30: return f'IADD {I(idr)}, {I(is_)}, {I(it)}'
        if f == 0x31: return f'ISUB {I(idr)}, {I(is_)}, {I(it)}'
        if f == 0x32: return f'IADDI {I(it)}, {I(is_)}, {simm(idr, 5)}'
        if f == 0x34: return f'IAND {I(idr)}, {I(is_)}, {I(it)}'
        if f == 0x35: return f'IOR {I(idr)}, {I(is_)}, {I(it)}'
        if f >= 0x3c:
            if c == 0x8000033c:
                return 'NOP'
            k = (c >> 6) & 31
            t = f & 3
            fsf, ftf = (c >> 21) & 3, (c >> 23) & 3
            tbl = {
                (0, 12): f'MOVE.{d} {F(ft)}, {F(fs)}', (0, 13): f'LQI.{d} {F(ft)}, ({I(is_)}++)',
                (0, 14): f'DIV Q, {F(fs)}{BC[fsf]}, {F(ft)}{BC[ftf]}', (0, 15): f'MTIR {I(it)}, {F(fs)}{BC[fsf]}',
                (0, 16): f'RNEXT.{d} {F(ft)}, R', (0, 25): f'MFP.{d} {F(ft)}, P', (0, 26): f'XTOP {I(it)}',
                (0, 27): f'XGKICK {I(is_)}', (0, 28): f'ESADD P, {F(fs)}', (0, 29): f'EATANxy P, {F(fs)}',
                (0, 30): f'ESQRT P, {F(fs)}{BC[fsf]}', (0, 31): f'ESIN P, {F(fs)}{BC[fsf]}',
                (1, 12): f'MR32.{d} {F(ft)}, {F(fs)}', (1, 13): f'SQI.{d} {F(fs)}, ({I(it)}++)',
                (1, 14): f'SQRT Q, {F(ft)}{BC[ftf]}', (1, 15): f'MFIR.{d} {F(ft)}, {I(is_)}',
                (1, 16): f'RGET.{d} {F(ft)}, R', (1, 26): f'XITOP {I(it)}', (1, 28): f'ERSADD P, {F(fs)}',
                (1, 29): f'EATANxz P, {F(fs)}', (1, 30): f'ERSQRT P, {F(fs)}{BC[fsf]}', (1, 31): f'EATAN P, {F(fs)}{BC[fsf]}',
                (2, 13): f'LQD.{d} {F(ft)}, (--{I(is_)})', (2, 14): f'RSQRT Q, {F(fs)}{BC[fsf]}, {F(ft)}{BC[ftf]}',
                (2, 15): f'ILWR.{d} {I(it)}, ({I(is_)})', (2, 16): f'RINIT R, {F(fs)}{BC[fsf]}',
                (2, 28): f'ELENG P, {F(fs)}', (2, 29): f'ESUM P, {F(fs)}', (2, 30): f'ERCPR P, {F(fs)}{BC[fsf]}',
                (2, 31): f'EEXP P, {F(fs)}{BC[fsf]}',
                (3, 13): f'SQD.{d} {F(fs)}, (--{I(it)})', (3, 14): 'WAITQ', (3, 15): f'ISWR.{d} {I(it)}, ({I(is_)})',
                (3, 16): f'RXOR R, {F(fs)}{BC[fsf]}', (3, 28): f'ERLENG P, {F(fs)}', (3, 30): 'WAITP',
            }
            return tbl.get((t, k), f'?lo{c:08x}')
        return f'?lo{c:08x}'
    if op == 0x00: return f'LQ.{d} {F(ft)}, {i11}({I(is_)})'
    if op == 0x01: return f'SQ.{d} {F(fs)}, {i11}({I(it)})'
    if op == 0x04: return f'ILW.{d} {I(it)}, {i11}({I(is_)})'
    if op == 0x05: return f'ISW.{d} {I(it)}, {i11}({I(is_)})'
    if op in (0x08, 0x09):
        imm = ((c >> 10) & 0x7800) | (c & 0x7ff)
        return f"{'IADDIU' if op == 8 else 'ISUBIU'} {I(it)}, {I(is_)}, {imm:#x}"
    if op == 0x10: return f'FCEQ vi01, {c & 0xffffff:#08x}'
    if op == 0x11: return f'FCSET {c & 0xffffff:#08x}'
    if op == 0x12: return f'FCAND vi01, {c & 0xffffff:#08x}'
    if op == 0x13: return f'FCOR vi01, {c & 0xffffff:#08x}'
    if op == 0x14: return f'FSEQ {I(it)}, {c & 0xfff:#x}'
    if op == 0x15: return f'FSSET {c & 0xfff:#x}'
    if op == 0x16: return f'FSAND {I(it)}, {((c >> 10) & 0x800) | (c & 0x7ff):#x}'
    if op == 0x17: return f'FSOR {I(it)}, {c & 0xfff:#x}'
    if op == 0x18: return f'FMEQ {I(it)}, {I(is_)}'
    if op == 0x1a: return f'FMAND {I(it)}, {I(is_)}'
    if op == 0x1b: return f'FMOR {I(it)}, {I(is_)}'
    if op == 0x1c: return f'FCGET {I(it)}'
    if op == 0x20: return f'B {tgt()}'
    if op == 0x21: return f'BAL {I(it)}, {tgt()}'
    if op == 0x24: return f'JR {I(is_)}'
    if op == 0x25: return f'JALR {I(it)}, {I(is_)}'
    br = {0x28: 'IBEQ', 0x29: 'IBNE', 0x2c: 'IBLTZ', 0x2d: 'IBGTZ', 0x2e: 'IBLEZ', 0x2f: 'IBGEZ'}
    if op in br:
        if op in (0x28, 0x29):
            return f'{br[op]} {I(it)}, {I(is_)}, {tgt()}'
        return f'{br[op]} {I(is_)}, {tgt()}'
    return f'?lo{c:08x}'


def dis(code, base_pc=0):
    for i in range(len(code) // 8):
        lo, up = struct.unpack_from('<II', code, i * 8)
        pc = base_pc + i
        u = upper(up)
        if up >> 31 & 1:  # I bit: the lower word is a float loaded into I
            l = f'LOI {struct.unpack("<f", struct.pack("<I", lo))[0]!r}'
        else:
            l = lower(lo, pc)
        yield pc, u, l


if __name__ == '__main__':
    d = open(sys.argv[1], 'rb').read()
    off, n = int(sys.argv[2], 0), int(sys.argv[3], 0)
    for pc, u, l in dis(d[off:off + n * 8]):
        print(f'{pc * 8:04x}  {u:44s} {l}')
