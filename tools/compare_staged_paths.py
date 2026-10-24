"""Compare the production staged reference placement with guarded original reads.

Admission uses the existing original source/VM identity checks and the physical
position bookends, never a model residual. This checks sampled reference placements;
it does not compare the ball, hands, complete rendered poses or every tick.
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

from compare_staged_clock import admit, read_disc, scripts


def reference_position(actor):
    before, after = (actor.get('reference_position_' + suffix) for suffix in ('before', 'after'))
    if (not actor.get('physical_position_guards_stable') or before != after
            or not isinstance(before, list) or len(before) != 3):
        return None, 'missing_or_changed_reference_position_bookends'
    if actor.get('actor_launch_before') != 0 or actor.get('actor_launch_after') != 0:
        return None, 'native_actor_launch_outside_reference_placement'
    physical = actor.get('physical_position_before')
    if not isinstance(physical, list) or len(physical) != 3:
        return None, 'missing_physical_height_bookend'
    payload = actor.get('payload_words', [])
    if len(payload) != 13 or before != payload[:3]:
        return None, 'reference_bookend_does_not_match_live_proxy_payload'
    if physical[1] != before[1]:
        return None, 'native_reference_and_physical_height_do_not_correspond'
    if not all(isinstance(word, int) and 0 <= word <= 0xffffffff for word in before):
        return None, 'invalid_original_reference_position_word'
    if not all(math.isfinite(struct.unpack('<f', struct.pack('<I', word))[0]) for word in before):
        return None, 'nonfinite_original_reference_position'
    return before, None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', required=True, type=Path)
    parser.add_argument('--iso', required=True, type=Path)
    parser.add_argument('--team', required=True, type=int, choices=(0, 1),
                        help='shooting team at the original press, from the paired played capture')
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    raw = args.capture.read_bytes()
    capture = json.loads(raw)
    disc = read_disc(args.iso)
    bank = scripts(disc['/SCRIPTS/SCRIPTS.BIN'])
    cases, excluded = [], []
    for sample_index, sample in enumerate(capture['samples']):
        for actor in sample.get('staged_clock', {}).get('actors', []):
            locator = dict(sample=sample_index, actor_id=actor['actor_id'])
            case, reason = admit(sample, actor, bank, disc['/SLUS_206.51'])
            if reason is None:
                before, reason = reference_position(actor)
                if reason is None:
                    slot = actor['payload_words'][3] & 0xffff
                    if slot >= 6:
                        reason = 'nonplayer_source_role'
                    else:
                        cases.append(dict(**locator, **case, slot=slot, frame=actor['last_tick'],
                                          team=args.team, native_position_bits=before))
            if reason:
                excluded.append(dict(**locator, reason=reason))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    adapter = args.output.with_suffix('.cases.txt')
    adapter.write_text(''.join(' '.join(str(v) for v in
                                      (c['sample'], c['actor_id'], c['script'], c['slot'], c['frame'],
                                       c['team'], *c['native_position_bits'])) + '\n' for c in cases),
                       encoding='utf-8')
    repo = Path(__file__).resolve().parents[1]
    report = dict(schema=1, component='production_staged_reference_placement',
                  capture=str(args.capture.resolve()), capture_sha256=hashlib.sha256(raw).hexdigest(),
                  original_disc_sha256={name: hashlib.sha256(data).hexdigest() for name, data in disc.items()},
                  source_coverage=capture['coverage'], shooting_team=args.team,
                  admission='Unchanged original source/VM identity admission, then matching physical position bookends; no residual filtering.',
                  tolerance='Every admitted reference placement component within 1e-4 game units.',
                  limitations=['Unpaused bookends do not prove atomicity or absence of interior ABA changes.',
                               'Only sampled primary staged VM reference placements in the declared shooting-team scenario are compared.',
                               'Native physical actor x/z includes animation root displacement and is not this reference transform.',
                               'Native actor launches and frames whose physical/reference heights differ are excluded by original fields, not model residuals.',
                               'Source role binding is supplied by the native proxy, so this does not test the rewrite role picker.',
                               'Ball paths, hands, complete poses, freeze/replay timing and unobserved ticks are not covered.'],
                  admitted=cases, excluded=excluded,
                  exclusion_counts=dict(collections.Counter(c['reason'] for c in excluded)),
                  production_source_sha256={name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
                                            for name in ('crates/street-game/src/staged.rs',
                                                         'crates/street-sim/src/gamebreaker.rs',
                                                         'crates/street-net/src/disc/staged.rs',
                                                         'tools/compare_staged_clock.py',
                                                         'tools/compare_staged_paths.py')})
    if cases:
        rust_report = args.output.with_suffix('.rust.json')
        rust_report.unlink(missing_ok=True)
        environment = dict(os.environ, STREET_ISO=str(args.iso.resolve()),
                           STREET_STAGED_PATH_CAPTURE=str(adapter.resolve()),
                           STREET_STAGED_PATH_REPORT=str(rust_report.resolve()))
        result = subprocess.run(['cargo', 'test', '-p', 'street-game',
                                 'production_staged_reference_positions_match_original_capture', '--', '--nocapture'],
                                cwd=repo, env=environment, capture_output=True, text=True)
        report.update(rust_exit_code=result.returncode, rust_output=result.stdout + result.stderr,
                      status=('compared_within_bound' if result.returncode == 0 else 'compared_with_mismatches')
                      if rust_report.exists() else 'production_comparison_failed')
        if rust_report.exists():
            report['production'] = json.loads(rust_report.read_text(encoding='utf-8'))
    else:
        report['status'] = 'no_admissible_original_position_observations'
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(status=report['status'], admitted=len(cases), excluded=len(excluded),
                          output=str(args.output.resolve()))))


if __name__ == '__main__':
    main()
