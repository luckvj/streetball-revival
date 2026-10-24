"""Bounded read-only original camera timeline; retain every attempt and raw bookend.

No pause, restore, guest write, patch or input is issued. A dense timeline can
show publication ordering; matching bookends are not proof of an atomic read.
"""
import argparse
import hashlib
import json
from pathlib import Path
import time

from capture_camera_projection import capture, ROOT
from pine_probe import Pine


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--samples', type=int, default=10000)
    parser.add_argument('--interval', type=float, default=0.0005)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--provenance', required=True)
    args = parser.parse_args()
    if not 1 <= args.samples <= 20000 or not 0 <= args.interval <= 1:
        parser.error('samples must be 1..20000 and interval 0..1 seconds')
    output = args.output.resolve()
    if output.is_relative_to(ROOT):
        parser.error('retain native captures outside Git')
    output.parent.mkdir(parents=True, exist_ok=True)
    sources = []
    for path in [Path(__file__), ROOT / 'tools/capture_camera_projection.py', ROOT / 'tools/pine_probe.py']:
        raw = path.read_bytes()
        retained = output.parent / (output.stem + '-' + path.name)
        retained.write_bytes(raw)
        sources.append({'path': str(path.resolve()), 'retained': str(retained),
                        'sha256': hashlib.sha256(raw).hexdigest()})
    pine = Pine(28011)
    attempts = []
    try:
        metadata = {'emulator': pine.string(8), 'serial': pine.string(12),
                    'title': pine.string(11), 'uuid': pine.string(13)}
        if metadata['serial'] != 'SLUS-20651':
            raise ValueError('wrong executable for fixed native field map')
        for index in range(args.samples):
            if index:
                time.sleep(args.interval)
            try:
                sample = capture(pine, args.provenance)
                attempts.append({'index': index, 'sample': sample})
            except ValueError as error:
                attempts.append({'index': index, 'time_ns': time.time_ns(), 'capture_error': str(error)})
    finally:
        pine.socket.close()
    result = {'schema': 1, 'metadata': metadata, 'provenance': args.provenance,
              'limitations': 'Live ordered reads, not an atomic or paused EE snapshot; every attempt retained.',
              'sources': sources, 'attempts': attempts,
              'successful_captures': sum('sample' in attempt for attempt in attempts),
              'capture_errors': sum('capture_error' in attempt for attempt in attempts)}
    output.write_text(json.dumps(result, indent=2, allow_nan=False) + '\n', encoding='utf8')
    print(json.dumps({'output': str(output), 'attempts': len(attempts),
                      'successful_captures': result['successful_captures'],
                      'capture_errors': result['capture_errors']}))


if __name__ == '__main__':
    main()
