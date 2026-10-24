"""Compare original DB height scale with each guarded actor's own skeleton.

Uses the unchanged staged source/VM admission, retaining all exclusions and
every admitted scalar observation. No scale residual changes admission.
"""
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import subprocess

from compare_staged_clock import admit, bits, read_disc, scripts


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
    admitted, excluded = [], []
    for sample_index, sample in enumerate(capture['samples']):
        for actor in sample.get('staged_clock', {}).get('actors', []):
            locator = dict(sample=sample_index, actor_id=actor['actor_id'])
            case, reason = admit(sample, actor, bank, disc['/SLUS_206.51'])
            if reason is None:
                physical = next(value for value in sample['actors'] if value['id'] == actor['actor_id'])
                player_record = physical['gait']['player_record']
                if len(actor['guards_before']) != 10 or 'skeleton_scale' not in actor:
                    reason = 'missing_guarded_own_skeleton_scale'
                elif not 0 <= player_record < 240:
                    reason = 'roster_record_outside_original_database'
                else:
                    admitted.append(dict(**locator, **case, player_record=player_record,
                                         native_scale_bits=bits(actor['skeleton_scale']['body_scale'][0]),
                                         skeleton_address=actor['skeleton_scale']['address']))
            if reason:
                excluded.append(dict(**locator, reason=reason))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    adapter = args.output.with_suffix('.cases.txt')
    adapter.write_text(''.join(' '.join(str(case[key]) for key in
                                      ('sample', 'actor_id', 'player_record', 'native_scale_bits')) + '\n'
                              for case in admitted), encoding='utf-8')
    repo = Path(__file__).resolve().parents[1]
    report = dict(schema=1, capture=str(args.capture.resolve()),
                  capture_sha256=hashlib.sha256(capture_bytes).hexdigest(),
                  admission='Original staged source/VM/guard admission, then guarded own scale and original DB roster bounds; no residual filtering.',
                  roster_identity='The same physical actor/channel match supplies the captured gait player_record; root and proxy/actor identity guards are retained.',
                  admitted=admitted, excluded=excluded,
                  exclusion_counts=dict(collections.Counter(case['reason'] for case in excluded)),
                  production_source_sha256={name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
                                            for name in ('crates/street-assets/src/db.rs', 'crates/street-game/src/staged.rs',
                                                         'tools/compare_staged_clock.py', 'tools/compare_staged_height.py')},
                  limitations=['Live capture is non-atomic; roster identity is captured context rather than a separate bracketed DB field.',
                               'This validates observed normal model scaleX, not tiny-player or alternate body option application.'])
    if admitted:
        rust_report = args.output.with_suffix('.rust.json')
        rust_report.unlink(missing_ok=True)
        environment = dict(os.environ, STREET_ISO=str(args.iso.resolve()),
                           STREET_STAGED_HEIGHT_CAPTURE=str(adapter.resolve()),
                           STREET_STAGED_HEIGHT_REPORT=str(rust_report.resolve()))
        result = subprocess.run(['cargo', 'test', '-p', 'street-game',
                                 'production_database_height_scale_matches_guarded_own_skeleton_capture', '--', '--nocapture'],
                                cwd=repo, env=environment, capture_output=True, text=True)
        report.update(rust_exit_code=result.returncode, rust_output=result.stdout + result.stderr,
                      status=('compared_exact' if result.returncode == 0 else 'compared_with_mismatches')
                      if rust_report.exists() else 'production_comparison_failed')
        if rust_report.exists():
            report['production'] = json.loads(rust_report.read_text(encoding='utf-8'))
    else:
        report['status'] = 'no_admissible_own_skeleton_scale_observations'
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(status=report['status'], admitted=len(admitted), excluded=len(excluded),
                          output=str(args.output.resolve()))))


if __name__ == '__main__':
    main()
