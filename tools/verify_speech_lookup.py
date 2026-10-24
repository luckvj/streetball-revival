"""Verify every player speech lookup against the unpatched US disc executable.

Usage: python tools/verify_speech_lookup.py <iso>
Reads only the user's ISO; emits no disc data. Uses a deliberately narrow MIPS
interpreter with branch delay slots, failing on any unsupported instruction.
"""
import argparse
from pathlib import Path
import re
import struct

from c0fb import entries
from iso_extract import SECTOR, walk


def read_disc(path):
    with open(path, "rb") as disc:
        disc.seek(16 * SECTOR)
        root = disc.read(SECTOR)[156:190]
        files = {}
        walk(disc, struct.unpack_from("<I", root, 2)[0],
             struct.unpack_from("<I", root, 10)[0], "", files)
        result = []
        for name in ("/SLUS_206.51", "/SPEECH/PLYRHDRS.BIG"):
            sector, size = files[name]
            disc.seek(sector * SECTOR)
            result.append(disc.read(size))
        return result


class Executable:
    def __init__(self, data):
        if data[:7] != b"\x7fELF\x01\x01\x01":
            raise ValueError("expected the original ELF32 little-endian executable")
        self.data = data
        self.segments = []
        offset = struct.unpack_from("<I", data, 0x1c)[0]
        count = struct.unpack_from("<H", data, 0x2c)[0]
        for index in range(count):
            kind, file_offset, address, _, size, _, _, _ = struct.unpack_from(
                "<8I", data, offset + index * 32)
            if kind == 1:
                self.segments.append((address, file_offset, size))

    def word(self, address):
        for start, offset, size in self.segments:
            if start <= address and address + 4 <= start + size:
                return struct.unpack_from("<I", self.data, offset + address - start)[0]
        raise ValueError(f"unmapped executable address {address:#x}")

    def lookup(self, voice, key, entry=0x1af230):
        registers = [0] * 32
        registers[5], registers[6] = voice, key
        pc, pending = entry, None
        signed = lambda n: n if n < 0x80000000 else n - 0x100000000
        for _ in range(2000):
            instruction = self.word(pc)
            op = instruction >> 26
            rs, rt, rd = (instruction >> 21) & 31, (instruction >> 16) & 31, (instruction >> 11) & 31
            immediate = instruction & 0xffff
            simmediate = immediate if immediate < 0x8000 else immediate - 0x10000
            branch = None
            if op == 0:
                function = instruction & 63
                if function == 0:
                    registers[rd] = registers[rt] << ((instruction >> 6) & 31)
                elif function == 8:
                    branch = registers[rs]
                elif function in (0x21, 0x2d):
                    registers[rd] = registers[rs] + registers[rt]
                elif function == 0x2a:
                    registers[rd] = int(signed(registers[rs]) < signed(registers[rt]))
                else:
                    raise ValueError(f"unsupported SPECIAL {instruction:#x} at {pc:#x}")
            elif op in (4, 5):
                take = registers[rs] == registers[rt]
                if op == 5:
                    take = not take
                branch = pc + 4 + simmediate * 4 if take else pc + 8
            elif op == 9:
                registers[rt] = registers[rs] + simmediate
            elif op == 0xa:
                registers[rt] = int(signed(registers[rs]) < simmediate)
            elif op == 0xb:
                registers[rt] = int(registers[rs] < (simmediate & 0xffffffff))
            elif op == 0xc:
                registers[rt] = registers[rs] & immediate
            elif op == 0xd:
                registers[rt] = registers[rs] | immediate
            elif op == 0xf:
                registers[rt] = immediate << 16
            elif op == 0x23:
                registers[rt] = self.word((registers[rs] + simmediate) & 0xffffffff)
            else:
                raise ValueError(f"unsupported opcode {instruction:#x} at {pc:#x}")
            registers = [value & 0xffffffff for value in registers]
            registers[0] = 0
            if pending is not None:
                if branch is not None:
                    raise ValueError("branch in a delay slot")
                if pending == 0:
                    return registers[2]
                pc, pending = pending, None
            else:
                pc += 4
                pending = branch
        raise ValueError("lookup did not return within the instruction limit")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("iso", type=Path)
    args = parser.parse_args()
    executable, archive = read_disc(args.iso)
    executable = Executable(executable)
    source = (Path(__file__).resolve().parents[1] /
              "crates/street-audio/src/speech_lookup.rs").read_text(encoding="utf-8")
    tables = {}
    for prefix, voice, body in re.findall(r"const (SHOT_)?VOICE_(\d+):.*?= \[(.*?)\];", source, re.S):
        tables[(bool(prefix), int(voice))] = {int(start, 0): int(stream) for start, stream in
                              re.findall(r"\((0x[0-9a-f]+|\d+), (\d+)\)", body)}
    checked = 0
    voices = set()
    for offset, size, name in entries(archive):
        is_shot = name.lower().startswith("shot")
        if not is_shot and not name.lower().startswith("action"):
            continue
        header = archive[offset:offset + size]
        voice = struct.unpack_from("<H", header, 2)[0]
        voices.add((is_shot, voice))
        for ordinal in range(header[5]):
            start = struct.unpack_from(">H", header, 12 + ordinal * 4)[0]
            expected = tables[(is_shot, voice)][start]
            actual = executable.lookup(voice, start * 256, 0x1c4b20 if is_shot else 0x1af230)
            if actual != expected:
                raise ValueError(f"voice {voice} start {start:#x}: native {actual}, Rust {expected}")
            checked += 1
    if voices != set(tables) or checked != sum(map(len, tables.values())):
        raise ValueError("header and Rust table coverage differ")
    print(f"Verified {checked} PLAY selectors across {len(voices)} voice/type headers against original MIPS instructions.")


if __name__ == "__main__":
    main()
