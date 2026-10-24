"""Make a copy of the EE executable that stock Ghidra (MIPS:LE:64:64-32addr)
can decompile: python tools/ee_patch.py <SLUS_206.51> <out>

Ghidra has no R5900 (Emotion Engine) language, so every EE-only instruction
ends a function with "bad instruction data". This rewrites them, in the copy
only, to MIPS64 instructions with the same or nearly the same effect on the
registers the game's C code uses:

- `lq`/`sq` -> `ld`/`sd` (the low 64 bits; enough for 32-bit code).
- three-operand `mult`/`multu`/`mult1`/`multu1` -> `mul rd, rs, rt` (the low
  32 bits, which is what the compiler reads from rd).
- `mfhi1`/`mflo1`/`mthi1`/`mtlo1`/`div1`/`divu1` -> the pipeline-0 versions.
- `por`/`pand`/`pxor`/`pnor` -> `or`/`and`/`xor`/`nor`; `pcpyld`/`pcpyud`
  and other MMI with a destination -> `or rd, rs, rt` (approximate; 128-bit
  data moves only); the rest -> `nop`.
- FPU accumulator: ACC is modelled as $f19, the least used register (42
  instructions in the whole program use it, all outside these sequences in
  practice; the script counts them). `mula.s`/`adda.s`/`suba.s` write $f19;
  `madda.s`, `msuba.s`, `madd.s`, `msub.s` become MIPS IV `madd.s`/`nmsub.s`
  with $f19 as the addend, so dot products decompile exactly.
- `rsqrt.s fd, fs, ft` (fs / sqrt(ft)) -> `div.s` (approximate);
  `max.s`/`min.s` -> `mov.s fd, fs` (approximate).
- `mfsa`/`mtsa`/`mtsab`/`mtsah`, `ei`/`di` -> `nop`.

COP2 (VU0 macro) instructions are left alone; Ghidra decodes them as
generic coprocessor operations. Use tools/mipsx.py on the original file when
exact behaviour matters.
"""
import struct
import sys

NOP = 0


def special(rs, rt, rd, sa, fct):
    return (rs << 21) | (rt << 16) | (rd << 11) | (sa << 6) | fct


def mul(rs, rt, rd):
    return (0x1c << 26) | (rs << 21) | (rt << 16) | (rd << 11) | 0x02


def cop1x(fr, ft, fs, fd, fct):
    return (0x13 << 26) | (fr << 21) | (ft << 16) | (fs << 11) | (fd << 6) | fct


def cop1s(ft, fs, fd, fct):
    return (0x11 << 26) | (0x10 << 21) | (ft << 16) | (fs << 11) | (fd << 6) | fct


ACC = 19


def patch(w, stats):
    op = w >> 26
    rs, rt, rd, sa, fct = w >> 21 & 31, w >> 16 & 31, w >> 11 & 31, w >> 6 & 31, w & 63
    if op == 0x1e:
        stats['lq'] += 1
        return (0x37 << 26) | (w & 0x3ffffff)
    if op == 0x1f:
        stats['sq'] += 1
        return (0x3f << 26) | (w & 0x3ffffff)
    if op == 0 and fct in (0x18, 0x19) and rd != 0:
        stats['mult3'] += 1
        return mul(rs, rt, rd)
    if op == 0 and fct in (0x28, 0x29):  # mfsa / mtsa
        stats['sa'] += 1
        return NOP
    if op == 1 and rt in (0x18, 0x19):  # mtsab / mtsah
        stats['sa'] += 1
        return NOP
    if op == 0x10 and rs == 0x10 and fct in (0x38, 0x39):  # ei / di
        stats['eidi'] += 1
        return NOP
    if op == 0x1c:
        stats['mmi'] += 1
        if fct in (0x00, 0x01, 0x18, 0x19, 0x20, 0x21):  # madd(u)(1), mult(u)1
            return mul(rs, rt, rd) if rd else NOP
        if fct in (0x10, 0x11, 0x12, 0x13):  # mfhi1 mthi1 mflo1 mtlo1
            return special(rs, rt, rd, 0, fct)
        if fct in (0x1a, 0x1b):  # div1 divu1
            return special(rs, rt, 0, 0, fct)
        if fct == 0x29 and sa in (0x12, 0x13):  # por pnor
            return special(rs, rt, rd, 0, 0x25 if sa == 0x12 else 0x27)
        if fct == 0x09 and sa in (0x12, 0x13):  # pand pxor
            return special(rs, rt, rd, 0, 0x24 if sa == 0x12 else 0x26)
        if fct == 0x09 and sa == 0x0e:  # pcpyld rd, rs, rt: low 64 bits = rt
            return special(rt, 0, rd, 0, 0x25)
        if fct == 0x29 and sa == 0x0e:  # pcpyud rd, rs, rt: low = high of rs
            return special(rs, 0, rd, 0, 0x25)
        if rd:
            stats['mmi_approx'] += 1
            return special(rs, rt, rd, 0, 0x25)
        return NOP
    if op == 0x11 and rs == 0x10:
        ft, fs, fd = rt, rd, sa
        if fct == 0x16:  # rsqrt.s
            stats['fpu_approx'] += 1
            return cop1s(ft, fs, fd, 0x03)
        if fct in (0x18, 0x19, 0x1a):  # adda.s suba.s mula.s -> ACC
            stats['acc'] += 1
            return cop1s(ft, fs, ACC, {0x18: 0x00, 0x19: 0x01, 0x1a: 0x02}[fct])
        if fct == 0x1c:  # madd.s fd = ACC + fs*ft
            stats['acc'] += 1
            return cop1x(ACC, ft, fs, fd, 0x20)
        if fct == 0x1d:  # msub.s fd = ACC - fs*ft
            stats['acc'] += 1
            return cop1x(ACC, ft, fs, fd, 0x38)
        if fct == 0x1e:  # madda.s
            stats['acc'] += 1
            return cop1x(ACC, ft, fs, ACC, 0x20)
        if fct == 0x1f:  # msuba.s
            stats['acc'] += 1
            return cop1x(ACC, ft, fs, ACC, 0x38)
        if fct in (0x28, 0x29):  # max.s min.s
            stats['fpu_approx'] += 1
            return cop1s(0, fs, fd, 0x06)
    return w


def uses_acc_register(w):
    op = w >> 26
    if op == 0x11 and (w >> 21 & 31) in (0x10, 0x14):
        return ACC in (w >> 16 & 31, w >> 11 & 31, w >> 6 & 31)
    if op == 0x11 and (w >> 21 & 31) in (0, 4):  # mfc1 / mtc1
        return (w >> 11 & 31) == ACC
    if op in (0x31, 0x39):  # lwc1 / swc1
        return (w >> 16 & 31) == ACC
    return False


def text_section(d):
    shoff, = struct.unpack_from('<I', d, 0x20)
    shnum, shstr = struct.unpack_from('<HH', d, 0x30)
    names = struct.unpack_from('<10I', d, shoff + 40 * shstr)[4]
    for i in range(shnum):
        nm, t, fl, addr, off, size = struct.unpack_from('<6I', d, shoff + 40 * i)
        if d[names + nm:d.index(b'\0', names + nm)] == b'.text':
            return off, size
    raise SystemExit('no .text section')


def main():
    src, dst = sys.argv[1:3]
    d = bytearray(open(src, 'rb').read())
    stats = {k: 0 for k in ('lq', 'sq', 'mult3', 'sa', 'eidi', 'mmi', 'mmi_approx', 'acc', 'fpu_approx')}
    clash = 0
    # Only .text: the one PT_LOAD segment also holds data, which must not change.
    off, size = text_section(d)
    for k in range(off, off + size - 3, 4):
        w, = struct.unpack_from('<I', d, k)
        clash += uses_acc_register(w)
        struct.pack_into('<I', d, k, patch(w, stats))
    open(dst, 'wb').write(d)
    print(stats, 'instructions already using $f%d:' % ACC, clash)


if __name__ == '__main__':
    main()
