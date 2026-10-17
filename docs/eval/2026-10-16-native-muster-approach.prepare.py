import argparse
import hashlib
import json
import shutil
from pathlib import Path


def digest(filename):
    return hashlib.sha256(filename.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    record = json.loads(Path(__file__).with_suffix('.json').read_text())
    registration = record['registration']
    expected = registration['source_hashes']
    observed = {name: digest(args.source / name) for name in expected}
    if observed != expected:
        raise ValueError('Original stopped-run inputs do not match the registration')
    args.output.mkdir(parents=True, exist_ok=False)
    shutil.copy2(args.source / 'summary.json', args.output / 'summary.json')
    cases = []
    prefix = bytearray()
    turns = {case['turn'] for case in registration['cases']}
    with (args.source / 'events.jsonl').open('rb') as events:
        for line in events:
            prefix.extend(line)
            event = json.loads(line)
            if event.get('kind') != 'state' or event.get('frame') != 0 or event.get('turn') not in turns:
                continue
            turn = event['turn']
            if any(case['turn'] == turn for case in cases):
                raise ValueError('Ambiguous repeated opening frame')
            destination = args.output / f't{turn}-f0' / 'events.jsonl'
            destination.parent.mkdir(exist_ok=False)
            destination.write_bytes(prefix)
            cases.append({'turn': turn, 'frame': 0, 'bytes': len(prefix), 'sha256': hashlib.sha256(prefix).hexdigest()})
    if cases != registration['cases']:
        raise ValueError('Opening prefixes do not match the registration')
    if observed != {name: digest(args.source / name) for name in expected}:
        raise ValueError('Original stopped-run inputs changed during extraction')
    (args.output / 'registration.json').write_text(json.dumps(registration, indent=2) + '\n')
    print(json.dumps({'cases': cases, 'original_files_unchanged': True}, indent=2))


if __name__ == '__main__':
    main()
