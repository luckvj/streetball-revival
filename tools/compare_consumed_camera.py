"""Compare captured renderer-consumed angles through the shared production stage.

Every observation is retained. The scalar-to-angle stage is deliberately not
executed here; its independent comparison and failures remain separate evidence.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import subprocess

from compare_camera_projection import ROOT, SOURCE, sha256, fnv1a

TEST = 'view::camera_pose_tests::original_capture_matches_production_consumed_projection'
FIELDS = ('horizontal_degrees', 'aspect', 'zoom', 'm00', 'm11')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    parser.add_argument('--consumer', type=Path, required=True)
    parser.add_argument('--output-directory', type=Path, required=True)
    args = parser.parse_args()
    output = args.output_directory.resolve()
    if output.is_relative_to(ROOT):
        parser.error('retain native captures outside Git')
    document = json.loads(args.capture.read_text(encoding='utf8'))
    if document['metadata']['serial'] != 'SLUS-20651':
        parser.error('capture must be the original US executable')
    if not document['samples']:
        parser.error('capture contains no samples')
    output.mkdir(parents=True, exist_ok=True)
    rows = []
    for index, sample in enumerate(document['samples']):
        values = [sample['observed'][field] for field in FIELDS]
        if not all(isinstance(value, (int, float)) and math.isfinite(value) for value in values):
            raise ValueError(f'nonfinite or missing consumed-stage input at index {index}')
        if not all(value > 0 for value in values[:3]):
            raise ValueError(f'nonpositive angle/aspect/zoom at index {index}')
        words = [f'{struct.unpack("<I", struct.pack("<f", value))[0]:08x}' for value in values]
        bits = output / f'{index:04d}.bits.txt'
        report_path = output / f'{index:04d}.rust.json'
        bits.write_text(' '.join(words) + '\n', encoding='utf8')
        report_path.unlink(missing_ok=True)
        env = os.environ.copy()
        env['STREET_CONSUMED_CAMERA_CAPTURE'] = str(bits)
        env['STREET_CONSUMED_CAMERA_REPORT'] = str(report_path)
        run = subprocess.run([str(args.consumer.resolve()), TEST, '--exact', '--nocapture'],
                             cwd=ROOT, env=env, capture_output=True, text=True)
        (output / f'{index:04d}.log').write_text(run.stdout + run.stderr, encoding='utf8')
        if not report_path.exists():
            raise RuntimeError(f'consumer produced no report at index {index}: {run.stderr}')
        report = json.loads(report_path.read_text(encoding='utf8'))
        if report['input_bits'] != words or report['compiled_view_rs_fnv1a64'] != fnv1a(SOURCE.read_bytes()):
            raise ValueError(f'compiled source or consumed input disagreement at index {index}')
        rows.append({'index': index, 'input_bits': words, 'raw_observation': sample,
                     'consumer_exit_code': run.returncode, 'report': report,
                     'report_sha256': sha256(report_path), 'bits_sha256': sha256(bits)})
    summary = {'schema': 1, 'scope': 'Actual production consumed-angle projection stage only; scalar/angle correspondence is not established by this comparison',
               'capture_path': str(args.capture.resolve()), 'capture_sha256': sha256(args.capture),
               'source_path': str(SOURCE), 'source_sha256': sha256(SOURCE),
               'compiled_source_fnv1a64': fnv1a(SOURCE.read_bytes()),
               'consumer': str(args.consumer.resolve()), 'consumer_sha256': sha256(args.consumer),
               'adapter_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               'cargo_lock_sha256': sha256(ROOT / 'Cargo.lock'), 'observations': len(rows), 'excluded': 0,
               'matched': sum(row['report']['within_declared_tolerance'] for row in rows),
               'all_matrix_bits_exact': sum(row['report']['bit_exact'] for row in rows),
               'matrix_within_four_ulps': sum(metric['ulp_error'] <= 4 for row in rows for metric in row['report']['metrics'].values()),
               'rows': rows}
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf8')
    print(json.dumps({key: value for key, value in summary.items() if key != 'rows'}, indent=2))
    return int(summary['matched'] != len(rows))


if __name__ == '__main__':
    raise SystemExit(main())
