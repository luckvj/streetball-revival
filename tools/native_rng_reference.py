"""Execute only the original seed/generator integer instructions offline.

This bounded instruction interpreter reads the supplied ELF and private word
memory. It never attaches to or writes the running game. No copied RNG formula
is used to derive expected states. It is not a complete EE emulator.
"""
import argparse
import hashlib
import json
from pathlib import Path

from mipsx import Exe

MASK = (1 << 64) - 1
BASE = 0x4309f0
RETURN = 0xfffffff0


def signed32(value):
    value &= 0xffffffff
    return (value - (1 << 32) if value & 0x80000000 else value) & MASK


def execute(exe, entry, state, seed=0):
    registers = [0] * 32
    registers[4] = signed32(seed)
    registers[31] = RETURN
    memory = {BASE + index * 4: word for index, word in enumerate(state)}
    pc, pending = entry, None
    trace = []
    for _ in range(256):
        if pc == RETURN:
            return {'state': [memory[BASE + i * 4] for i in range(6)],
                    'return': registers[2] & 0xffffffff, 'trace': trace}
        if not 0x3a16b8 <= pc <= 0x3a1808:
            raise ValueError(f'PC outside original RNG routines: {pc:x}')
        word = exe.word(pc)
        opcode, rs, rt = word >> 26, (word >> 21) & 31, (word >> 16) & 31
        rd, shift, fn = (word >> 11) & 31, (word >> 6) & 31, word & 63
        imm = word & 0xffff
        simm = imm - 0x10000 if imm & 0x8000 else imm
        next_pending = None
        if opcode == 15:
            registers[rt] = signed32(imm << 16)
        elif opcode == 9:
            registers[rt] = signed32(registers[rs] + simm)
        elif opcode == 13:
            registers[rt] = registers[rs] | imm
        elif opcode in (35, 43):
            address = (registers[rs] + simm) & 0xffffffff
            if address not in memory:
                raise ValueError(f'Unexpected word memory access: {address:x}')
            if opcode == 35:
                registers[rt] = signed32(memory[address])
            else:
                memory[address] = registers[rt] & 0xffffffff
        elif opcode == 5:
            next_pending = pc + 4 + simm * 4 if registers[rs] != registers[rt] else pc + 8
        elif opcode == 0:
            if fn == 33:
                registers[rd] = signed32(registers[rs] + registers[rt])
            elif fn == 43:
                registers[rd] = int(registers[rs] < registers[rt])
            elif fn == 45:
                registers[rd] = (registers[rs] + registers[rt]) & MASK
            elif fn == 56:
                registers[rd] = (registers[rt] << shift) & MASK
            elif fn == 60:
                registers[rd] = (registers[rt] << (shift + 32)) & MASK
            elif fn == 62:
                registers[rd] = registers[rt] >> (shift + 32)
            elif fn == 8:
                next_pending = registers[rs]
            elif word != 0:
                raise ValueError(f'Unsupported special instruction: {word:x}')
        else:
            raise ValueError(f'Unsupported opcode: {word:x}')
        registers[0] = 0
        trace.append({'pc': f'{pc:08x}', 'word': f'{word:08x}'})
        pc, pending = (pending if pending is not None else pc + 4), next_pending
    raise ValueError('RNG execution exceeded instruction limit')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--elf', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    exe = Exe(args.elf)
    rows = []
    for seed in [0, 1, 7, 42, 0xdeadbeef, 0xffffffff]:
        seeded = execute(exe, 0x3a1790, [0] * 6, seed)
        state = seeded['state']
        draws = []
        for _ in range(16):
            result = execute(exe, 0x3a16b8, state)
            draws.append(result)
            state = result['state']
        rows.append({'seed': seed, 'seed_execution': seeded, 'draws': draws})
    carries = [{'input': state, 'execution': execute(exe, 0x3a16b8, state)} for state in
               [[0] * 6, [0xffffffff] * 6, [0, 0, 0, 0, 0, 0xffffffff],
                [0x7fffffff, 0x80000000, 0xfffffffe, 1, 0xffffffff, 0xfffffffe]]]
    output = {'scope': 'Offline original integer-instruction execution, not native runtime draw ordering',
              'elf_sha256': hashlib.sha256(args.elf.read_bytes()).hexdigest(),
              'interpreter_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'routine_words': [f'{exe.word(pc):08x}' for pc in range(0x3a16b8, 0x3a180c, 4)],
              'seeded_cases': rows, 'carry_cases': carries}
    args.output.write_text(json.dumps(output, indent=2) + '\n', encoding='utf8')
    print(json.dumps({'path': str(args.output), 'seed0': rows[0]['seed_execution']['state'],
                      'first_draws_seed0': [x['return'] for x in rows[0]['draws'][:4]],
                      'carry_outputs': [x['execution']['state'] for x in carries]}, indent=2))


if __name__ == '__main__':
    main()
