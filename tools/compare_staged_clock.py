"""Compare production staged clocks with guarded original PINE observations.

Admission depends only on source identity, native VM domain and capture guards.
Every admitted residual and every exclusion is retained in the JSON report.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess

from iso_extract import SECTOR, walk


def bits(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]


def read_disc(path):
    with open(path, 'rb') as stream:
        stream.seek(16 * SECTOR)
        root = stream.read(SECTOR)[156:190]
        files = {}
        walk(stream, struct.unpack_from('<I', root, 2)[0],
             struct.unpack_from('<I', root, 10)[0], '', files)
        output = {}
        for name in ('/SCRIPTS/SCRIPTS.BIN', '/SLUS_206.51'):
            lba, size = files[name]
            stream.seek(lba * SECTOR)
            output[name] = stream.read(size)
        return output


def scripts(data):
    result = []
    offset = 0
    while offset < len(data):
        header = list(struct.unpack_from('<5I', data, offset))
        assert header[0] == 0x11627373
        at = offset + 20
        records = []
        actor_index = 0
        for index in range(header[4]):
            category, frame, duration = struct.unpack_from('<3I', data, at)
            size = (52, 36, 48, 40, 12)[category]
            payload = data[at + 12:at + 12 + size]
            record = dict(index=index, category=category, frame=frame, duration=duration)
            if category == 0:
                record.update(payload_words=list(struct.unpack('<13I', payload)),
                              flags=list(payload[36:52]), slot=struct.unpack_from('<H', payload, 12)[0],
                              row=struct.unpack_from('<H', payload, 18)[0], actor_index=actor_index)
                actor_index += 1
            records.append(record)
            at += 12 + size
        result.append(dict(index=len(result), header=header, records=records))
        offset += header[3]
    return result


def elf_word(data, address):
    phoff = struct.unpack_from('<I', data, 0x1c)[0]
    for index in range(struct.unpack_from('<H', data, 0x2c)[0]):
        kind, offset, virtual, _, size, _, _, _ = struct.unpack_from('<8I', data, phoff + index * 32)
        if kind == 1 and virtual <= address < virtual + size:
            return struct.unpack_from('<I', data, offset + address - virtual)[0]
    raise ValueError('address outside original ELF')


def mirror_row(data, row):
    if elf_word(data, 0x3de8a0 + row * 28 + 24):
        return row - 1
    if row + 1 < 1144 and elf_word(data, 0x3de8a0 + (row + 1) * 28 + 24):
        return row + 1
    return row


def admit(sample, actor, bank, elf):
    packet = sample['staged_clock']
    if packet['state'] != 3 or packet['pending'] != 0:
        return None, 'outside_forward_staged_state3'
    if not sample.get('root_pointers_stable', False) or sample['globals']['fps'] != 60:
        return None, 'unstable_roots_or_unsupported_fps'
    if not packet['guards_stable'] or packet['guards_before'] != [packet['state'], packet['next_tick']]:
        return None, 'unstable_or_nonmatching_manager_guard'
    if not actor['guards_stable'] or not actor['proxy_matches_actor'] or not actor['proxy_matches_manager']:
        return None, 'unstable_proxy_or_identity_guard'
    channel = actor['channel']
    guard = actor['guards_before']
    expected_guard = [actor['last_tick'], bits(channel['frame']), channel['counter']]
    if len(guard) in (9, 10):
        live_guard = actor.get('payload_words', [])
        previous = actor.get('previous_proxy_position', [])
        if len(live_guard) != 13 or len(previous) != 3 or 'actor_height' not in actor:
            return None, 'missing_expanded_motion_guard_payload'
        expected_guard.extend([live_guard[0], live_guard[2], bits(previous[0]), bits(previous[2]),
                               bits(actor['actor_height'])])
        scale = actor.get('skeleton_scale')
        expected_guard.append(int(scale['address'], 16) if scale else guard[8])
        if len(guard) == 10:
            if not scale or len(scale.get('body_scale', [])) != 4:
                return None, 'missing_expanded_skeleton_guard_payload'
            expected_guard.append(bits(scale['body_scale'][0]))
    elif len(guard) != 3:
        return None, 'unsupported_actor_guard_schema'
    if guard != expected_guard:
        return None, 'nonmatching_channel_guard_payload'
    if actor['initial_pending'] != 0 or actor['last_tick'] + 1 != packet['next_tick']:
        return None, 'unprocessed_or_nonmatching_proxy_tick'
    live = actor.get('payload_words', [])
    if len(live) != 13 or not packet.get('actor_records'):
        return None, 'missing_live_or_source_actor_payload'
    slot = live[3] & 0xffff
    sources = [record for record in packet['actor_records']
               if len(record.get('payload_words', [])) == 13
               and record['payload_words'][3] & 0xffff == slot and record['frame'] <= actor['last_tick']]
    if not sources:
        return None, 'no_applicable_source_record'
    source = max(sources, key=lambda record: (record['frame'], record['index']))
    candidates = []
    for script in bank:
        if script['header'] != packet.get('header_words') or source['index'] >= len(script['records']):
            continue
        record = script['records'][source['index']]
        if (record['category'] == 0 and record['frame'] == source['frame']
                and record['duration'] == source['duration'] and record['payload_words'] == source['payload_words']):
            candidates.append((script, record))
    if len(candidates) != 1:
        return None, 'source_not_uniquely_identified_on_original_disc'
    script, record = candidates[0]
    initial = not any(previous['category'] == 0 and previous['slot'] == slot
                      for previous in script['records'][:record['index']])
    flags = record['flags']
    live_flags = list(struct.pack('<13I', *live)[36:52])
    if flags[1] == 0 or flags[9] != 0 or (live[3] >> 16) == 0 or live[3] >> 16 > 32767 or live[8] & 0xffff > 32767:
        return None, 'unsupported_primary_actor_or_signed_word_domain'
    if any(live_flags[index] != flags[index] for index in (1, 6, 7, 10)) or live[3] >> 16 != record['payload_words'][3] >> 16 or live[8] & 0xffff != record['payload_words'][8] & 0xffff:
        return None, 'live_clock_selectors_differ_from_immutable_source'
    if actor['record_duration'] != record['duration']:
        return None, 'proxy_record_duration_differs_from_source'
    elapsed = actor['last_tick'] - record['frame']
    if elapsed > 10000:
        return None, 'outside_bounded_production_clock_domain'
    direct = initial or flags[7] != 0
    looping = flags[6] != 0
    if not direct and elapsed < 6 or channel['flags'] & 0x200000:
        return None, 'previous_pose_blend_outside_clock_comparison'
    native_row = live[4] >> 16
    if native_row not in (record['row'], mirror_row(elf, record['row'])) or channel['animation'] != native_row:
        return None, 'source_proxy_channel_row_mismatch'
    if channel['target_animation'] != 0xffffffff and not (looping and channel['target_animation'] == native_row):
        return None, 'linked_or_other_target_row_outside_comparison'
    physical = next((value for value in sample['actors'] if value['id'] == actor['actor_id']), None)
    if not physical or physical['channel0']['address'] != channel['address'] or physical['channel0']['animation'] != native_row:
        return None, 'physical_actor_channel_identity_mismatch'
    pc = physical['channel0']['script_pc']
    expected_pc = (0x456c44 if direct else 0x456c70) if looping else (0x456c55 if direct else 0x456c86)
    stopped_pc = 0x456c5b if direct else 0x456c8c
    stopped = bool(channel['flags'] & 1)
    if pc != (stopped_pc if stopped else expected_pc):
        return None, 'vm_pc_outside_matching_staged_body'
    if bool(channel['flags'] & 0x2000) != looping or not looping and not stopped and not channel['flags'] & 8:
        return None, 'channel_mode_outside_staged_vm_domain'
    expected_duration = 0xffffffff if looping else record['duration'] - (0 if initial else 6)
    if channel['duration'] != expected_duration or bits(channel['step']) != bits(channel['configured_step']):
        return None, 'channel_duration_or_step_ownership_mismatch'
    return dict(script=script['index'], actor_track=record['actor_index'], source_record=record['index'],
                elapsed=elapsed, native_frame_bits=bits(channel['frame']), native_step_bits=bits(channel['step']),
                native_row=native_row, source_frame=record['frame'], source_duration=record['duration'],
                source_speed_word=record['payload_words'][3] >> 16,
                source_end_fraction=record['payload_words'][8] & 0xffff, source_flags=flags,
                native_counter=channel['counter'], native_skip_counter=channel['skip_counter'],
                native_key=channel['key'], native_channel_flags=channel['flags'], native_script_pc=pc,
                native_seek_countdown=actor['seek_countdown'],
                looping=looping, initial=initial, direct=direct, stopped=stopped), None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', required=True, type=Path)
    parser.add_argument('--iso', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    capture_bytes = args.capture.read_bytes()
    capture = json.loads(capture_bytes)
    disc = read_disc(args.iso)
    bank = scripts(disc['/SCRIPTS/SCRIPTS.BIN'])
    cases, excluded = [], []
    history = []
    for sample_index, sample in enumerate(capture['samples']):
        packet = sample.get('staged_clock')
        if not packet:
            excluded.append(dict(sample=sample_index, reason='missing_staged_packet'))
            continue
        history.append(dict(sample=sample_index, state=packet['state'], pending=packet['pending'],
                            next_tick=packet['next_tick'], guards_stable=packet['guards_stable']))
        for actor in packet['actors']:
            case, reason = admit(sample, actor, bank, disc['/SLUS_206.51'])
            locator = dict(sample=sample_index, actor_id=actor['actor_id'])
            if reason:
                excluded.append(dict(**locator, reason=reason))
            else:
                physical = next(value for value in sample['actors'] if value['id'] == actor['actor_id'])
                cases.append(dict(**locator, **case, player_record=physical['gait']['player_record']))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    adapter = args.output.with_suffix('.cases.txt')
    adapter.write_text(''.join(' '.join(str(case[key]) for key in ('sample', 'actor_id', 'script', 'actor_track', 'elapsed', 'native_frame_bits', 'native_step_bits', 'player_record', 'native_row', 'native_counter')) + '\n' for case in cases), encoding='utf-8')
    report = dict(schema=1, component='production_staged_actor_clock', capture=str(args.capture.resolve()),
                  capture_sha256=hashlib.sha256(capture_bytes).hexdigest(), source_coverage=capture['coverage'],
                  original_disc_sha256={name: hashlib.sha256(data).hexdigest() for name, data in disc.items()},
                  admission='Source/VM/guard rules fixed before residuals; no numerical residual filtering.',
                  limitations=['Unpaused non-atomic reads cannot prove completed scheduler ticks.',
                               'Only state3 post-blend single-primary-channel matching staged VM bodies admitted.',
                               'Resumed state3 can retain extra actor updates from state7 unfreezing.',
                               'Runtime gait can overwrite the source-derived configured step.',
                               'Stateful source evaluation uses original DB height from captured roster identity and observed mirrored-row identity; applied body options remain unverified.',
                               'This checks channel phase and configured step, not pose/held-ball parity.'],
                  admitted=cases, excluded=excluded, manager_history=history,
                  adapter=str(adapter.resolve()))
    repo = Path(__file__).resolve().parents[1]
    report['production_source_sha256'] = {
        name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
        for name in ('crates/street-game/src/staged.rs', 'crates/street-assets/src/timeline.rs',
                     'crates/street-assets/src/db.rs', 'crates/street-sim/src/gamebreaker.rs',
                     'crates/street-sim/src/street.rs', 'crates/street-sim/src/snapshot.rs',
                     'crates/street-sim/src/validation.rs', 'tools/compare_staged_clock.py')}
    groups = {}
    for case in cases:
        key = (case['actor_id'], case['script'], case['source_record'], case['elapsed'])
        groups.setdefault(key, []).append(case)
    report['capture_clock_ambiguities'] = [
        dict(actor_id=key[0], script=key[1], source_record=key[2], elapsed=key[3],
             samples=[case['sample'] for case in group],
             observed_frame_bits=sorted(set(case['native_frame_bits'] for case in group)),
             observed_counters=sorted(set(case['native_counter'] for case in group)))
        for key, group in groups.items()
        if len(set((case['native_frame_bits'], case['native_counter']) for case in group)) > 1]
    report['unique_admitted_actor_source_ticks'] = len(groups)
    if cases:
        rust_report = args.output.with_suffix('.rust.json')
        rust_report.unlink(missing_ok=True)
        environment = dict(os.environ, STREET_ISO=str(args.iso.resolve()),
                           STREET_STAGED_CLOCK_CAPTURE=str(adapter.resolve()),
                           STREET_STAGED_CLOCK_REPORT=str(rust_report.resolve()))
        result = subprocess.run(['cargo', 'test', '-p', 'street-game',
                                 'production_native_staged_clock_matches_original_capture', '--', '--nocapture'],
                                cwd=Path(__file__).resolve().parents[1], env=environment, capture_output=True, text=True)
        report.update(rust_exit_code=result.returncode, rust_output=result.stdout + result.stderr,
                      status=('compared_exact' if result.returncode == 0 else 'compared_with_mismatches')
                      if rust_report.exists() else 'production_comparison_failed')
        if rust_report.exists():
            report['production'] = json.loads(rust_report.read_text())
    else:
        report['status'] = 'no_admissible_original_staged_clock_observations'
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(status=report['status'], admitted=len(cases), excluded=len(excluded), output=str(args.output.resolve()))))


if __name__ == '__main__':
    main()
