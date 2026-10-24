"""Compare every retained original camera observation with a compiled test consumer.

Build the street-game test binary first. This executes the production scalar
helper and Bevy projection through that binary, retaining failing observations.
Consumer/source consistency is verified by the existing one-observation adapter.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import os

from compare_camera_projection import ROOT, TEST, prepare, finalize


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    parser.add_argument('--consumer', type=Path, required=True)
    parser.add_argument('--output-directory', type=Path, required=True)
    args = parser.parse_args()
    directory = args.output_directory.resolve()
    if directory.is_relative_to(ROOT):
        parser.error('retain original captures outside Git')
    document = json.loads(args.capture.read_text(encoding='utf8'))
    if document['metadata']['serial'] != 'SLUS-20651':
        parser.error('capture must be the original US executable')
    samples = document['samples']
    if not samples:
        parser.error('capture contains no samples')
    directory.mkdir(parents=True, exist_ok=True)
    rows = []
    for index, sample in enumerate(samples):
        original = directory / f'{index:04d}.original-input.json'
        original.write_text(json.dumps(sample, indent=2) + '\n', encoding='utf8')
        report = directory / f'{index:04d}.json'
        retained = prepare(original, report)
        env = os.environ.copy()
        env['STREET_CAMERA_PARITY_CAPTURE'] = str(retained['bits'])
        env['STREET_CAMERA_PARITY_REPORT'] = str(retained['rust'])
        run = subprocess.run([str(args.consumer.resolve()), TEST, '--exact', '--nocapture'],
                             cwd=ROOT, env=env, capture_output=True, text=True)
        (directory / f'{index:04d}.log').write_text(run.stdout + run.stderr, encoding='utf8')
        if not retained['rust'].exists():
            raise RuntimeError(f'consumer produced no observation for index {index}: {run.stderr}')
        passed = finalize(report)
        result = json.loads(report.read_text(encoding='utf8'))
        rows.append({'index': index, 'consumer_exit_code': run.returncode,
                     'within_declared_tolerance': passed, 'report': result,
                     'report_sha256': hashlib.sha256(report.read_bytes()).hexdigest()})
    summary = {'schema': 1, 'scope': 'Compiled production camera components; no whole-camera or hardware parity claim',
               'source_capture': str(args.capture.resolve()),
               'source_capture_sha256': hashlib.sha256(args.capture.read_bytes()).hexdigest(),
               'consumer': str(args.consumer.resolve()),
               'consumer_sha256': hashlib.sha256(args.consumer.read_bytes()).hexdigest(),
               'adapter_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               'observations': len(rows), 'excluded': 0,
               'matched': sum(row['within_declared_tolerance'] for row in rows),
               'angle_exact': sum(row['report']['comparison']['metrics']['horizontal_degrees']['ulp_error'] == 0 for row in rows),
               'matrix_within_four_ulps': sum(row['report']['comparison']['metrics'][key]['ulp_error'] <= 4
                                            for row in rows for key in ['m00', 'native_y_m11']),
               'all_three_components_bit_exact': sum(row['report']['comparison']['bit_exact'] for row in rows),
               'rows': rows}
    (directory / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf8')
    print(json.dumps({key: value for key, value in summary.items() if key != 'rows'}, indent=2))
    return int(summary['matched'] != len(rows))


if __name__ == '__main__':
    raise SystemExit(main())
