"""Retain guarded native loop-rate observations and compare the production helper.

All admission rules use original source identity, native input guards and the
demonstrated producer domain. No step residual changes the denominator.
"""
import argparse
import collections
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import subprocess

from compare_staged_clock import admit, bits, elf_word, read_disc, scripts


def original_rows(elf):
    registers = [None] * 32
    registers[0] = 0
    floats = [None] * 32
    rows = []
    for address in range(0x32c124, 0x32f0c0, 4):
        word = elf_word(elf, address)
        op, rs, rt = word >> 26, word >> 21 & 31, word >> 16 & 31
        fs, fd = word >> 11 & 31, word >> 6 & 31
        imm = word & 0xffff
        if op == 15:
            registers[rt] = imm << 16
        elif op in (9, 13):
            source = registers[rs]
            signed = imm - 65536 if imm & 0x8000 else imm
            registers[rt] = None if source is None else (source | imm if op == 13 else (source + signed) & 0xffffffff)
        elif op == 0 and word & 63 in (0x21, 0x2d) and rt == 0:
            registers[fs] = registers[rs]
        elif op == 0x11 and rs == 4:
            floats[fs] = registers[rt]
        elif op == 0x11 and rs == 16 and word & 63 == 6:
            floats[fd] = floats[fs]
        elif op == 0x31:
            floats[rt] = None
        elif op == 0x11 and rs == 16 and word & 63 in (0, 1, 2, 3):
            floats[fd] = None
        elif op == 3 and (word & 0x3ffffff) << 2 == 0x191410:
            assert registers[6] is not None and registers[7] is not None and floats[12] is not None
            rows.append(dict(index=len(rows), row_a=registers[6], row_b=registers[7],
                             root_distance_bits=floats[12], constructor_call=address))
    assert len(rows) == 108
    return rows


def manager_height_is_zero(actor):
    # 0032f2e8 calls 0032bae8 before 0032c0a0: the signed proxy Y is
    # copied to the actor before the producer's exact-zero check. The
    # later actor update can clamp it to zero before our snapshot.
    return struct.unpack('<f', struct.pack('<I', actor['payload_words'][1]))[0] == 0.0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', required=True, type=Path)
    parser.add_argument('--iso', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    raw_capture = args.capture.read_bytes()
    capture = json.loads(raw_capture)
    disc = read_disc(args.iso)
    bank = scripts(disc['/SCRIPTS/SCRIPTS.BIN'])
    rows = original_rows(disc['/SLUS_206.51'])
    before = capture['metadata']['staged_motion_table_before']
    after = capture['metadata']['staged_motion_table_after']
    table_valid = (before['initialization_flag'] == after['initialization_flag'] == 1
                   and len(before['words']) == len(after['words']) == 1080
                   and before['words'] == after['words']
                   and all(before['words'][i * 10 + 1:i * 10 + 3] == [row['row_a'], row['row_b']]
                           and before['words'][i * 10 + 6] == row['root_distance_bits']
                           for i, row in enumerate(rows)))
    admitted, excluded = [], []
    for sample_index, sample in enumerate(capture['samples']):
        for actor in sample.get('staged_clock', {}).get('actors', []):
            locator = dict(sample=sample_index, actor_id=actor['actor_id'])
            case, reason = admit(sample, actor, bank, disc['/SLUS_206.51'])
            if reason is None:
                if not table_valid:
                    reason = 'table_not_stable_initialized_original_constructor_rows'
                elif not case['looping']:
                    reason = 'finite_actor_has_no_loop_motion_writer'
                elif len(actor['guards_before']) != 10 or 'skeleton_scale' not in actor:
                    reason = 'missing_guarded_own_actor_scale'
                elif actor['actor_height'] != 0.0 or not 0 < actor['last_tick'] < 0x80000000:
                    reason = 'outside_grounded_positive_signed_tick_domain'
                elif not manager_height_is_zero(actor):
                    reason = 'manager_height_nonzero_before_actor_clamp'
                else:
                    row = next((row for row in rows if case['native_row'] in (row['row_a'], row['row_b'])), None)
                    current = [struct.unpack('<f', struct.pack('<I', word))[0] for word in actor['payload_words'][:3]]
                    previous = actor['previous_proxy_position']
                    scale = actor['skeleton_scale']['body_scale'][0]
                    if row is None:
                        reason = 'unmatched_row_preserves_prior_rate'
                    elif not all(math.isfinite(value) and abs(value) <= 1000000 for value in current + previous) or not math.isfinite(scale) or scale <= 0:
                        reason = 'outside_finite_positive_motion_input_domain'
                    elif current[0] == previous[0] and current[2] == previous[2]:
                        reason = 'zero_motion_preserves_prior_rate'
                    else:
                        admitted.append(dict(**locator, **case, root_distance_bits=row['root_distance_bits'],
                                             table_index=row['index'], current_position_bits=[bits(v) for v in current],
                                             previous_position_bits=[bits(v) for v in previous],
                                             actor_height_bits=bits(actor['actor_height']), scale_bits=bits(scale),
                                             last_tick=actor['last_tick']))
            if reason:
                excluded.append(dict(**locator, reason=reason))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    adapter = args.output.with_suffix('.cases.txt')
    adapter.write_text(''.join(' '.join(str(v) for v in (
        case['sample'], case['actor_id'], case['script'], case['actor_track'], case['elapsed'],
        case['root_distance_bits'], *case['current_position_bits'], *case['previous_position_bits'],
        case['actor_height_bits'], case['scale_bits'], case['last_tick'], case['native_step_bits'])) + '\n'
        for case in admitted), encoding='utf-8')
    table_adapter = args.output.with_suffix('.table.txt')
    table_adapter.write_text(''.join(' '.join(str(before['words'][i * 10 + k]) for k in (1, 2, 6)) + '\n' for i in range(108)), encoding='utf-8')
    report = dict(schema=1, capture=str(args.capture.resolve()), capture_sha256=hashlib.sha256(raw_capture).hexdigest(),
                  coverage=capture['coverage'], table_valid=table_valid, table_before=before, table_after=after,
                  original_rows=rows, admitted=admitted, excluded=excluded,
                  exclusion_counts=dict(collections.Counter(case['reason'] for case in excluded)),
                  limitations=['Live guarded reads do not prove a completed scheduler update.',
                               'The initial batch retained five false producer mismatches caused by later-clamped physical heights; the signed manager-height domain follows 0032f2e8/0032bae8/0032c0a0 and is independent of residuals.',
                               'The first actor update clearing a blend follows a manager update that still saw the blend flag.',
                               'This compares the production rate producer from native inputs, not source-derived motion or full pose parity.'])
    repo = Path(__file__).resolve().parents[1]
    report['production_source_sha256'] = {
        name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
        for name in ('crates/street-game/src/staged.rs', 'crates/street-assets/src/timeline.rs',
                     'crates/street-sim/src/gamebreaker.rs', 'tools/compare_staged_motion.py',
                     'tools/compare_staged_clock.py')}
    if admitted:
        rust_report = args.output.with_suffix('.rust.json')
        rust_report.unlink(missing_ok=True)
        environment = dict(os.environ, STREET_ISO=str(args.iso.resolve()),
                           STREET_STAGED_MOTION_CAPTURE=str(adapter.resolve()),
                           STREET_STAGED_MOTION_TABLE=str(table_adapter.resolve()),
                           STREET_STAGED_MOTION_REPORT=str(rust_report.resolve()))
        result = subprocess.run(['cargo', 'test', '-p', 'street-game',
                                 'production_native_staged_motion_matches_original_capture', '--', '--nocapture'],
                                cwd=Path(__file__).resolve().parents[1], env=environment, capture_output=True, text=True)
        report.update(rust_exit_code=result.returncode, rust_output=result.stdout + result.stderr,
                      status=('compared_exact' if result.returncode == 0 else 'compared_with_mismatches')
                      if rust_report.exists() else 'production_comparison_failed')
        if rust_report.exists():
            report['production'] = json.loads(rust_report.read_text())
    else:
        report['status'] = 'no_admissible_producer_observations'
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(status=report['status'], admitted=len(admitted), excluded=len(excluded), output=str(args.output.resolve()))))


if __name__ == '__main__':
    main()
