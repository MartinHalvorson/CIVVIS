"""Restore exact captured input bytes to a new directory; never touch a game."""
import argparse
import copy
import gzip
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
here = Path(__file__).resolve().parent
packet = json.loads(gzip.decompress((here / 'qualified-opening.json.gz').read_bytes()))
captures = packet['portable_captures']
archive = (here / 'native-events.jsonl.gz').read_bytes()
manifest = captures['capture_prefix_manifest']
assert hashlib.sha256(archive).hexdigest() == manifest['source_events_archive_sha256']
raw = gzip.decompress(archive)
assert hashlib.sha256(raw).hexdigest() == manifest['source_events_raw_sha256']
data = raw[:captures['archive_prefix_bytes']]
assert hashlib.sha256(data).hexdigest() == captures['archive_prefix_sha256']
assert manifest['all_exact_prefixes'] and len(manifest['unique_snapshots']) == 278
args.out.mkdir(parents=True, exist_ok=False)
directory = (args.out / 'captured-reads').resolve()
directory.mkdir()
original = captures['original_capture']
(directory / original['archive']).write_bytes(data)
(directory / original['index']).write_text(''.join(json.dumps(row, sort_keys=True) + '\n'
    for row in captures['snapshot_index']))
records = copy.deepcopy(packet['native_decisions'])
for record in records:
    # Only the destination changes. Snapshot identities, parent relationships,
    # every captured byte, read order and count, and all planning facts survive.
    assert record['input_capture']['directory'] == original['directory']
    record['input_capture']['directory'] = str(directory)
(args.out / 'native-decisions.jsonl').write_text(''.join(json.dumps(row, sort_keys=True) + '\n'
    for row in records))
(args.out / 'reconstructed-context.json').write_text(json.dumps(packet['original_context'], indent=2) + '\n')
(args.out / 'restoration.json').write_text(json.dumps({
    'scope': 'Exact captured bytes restored. Only capture-directory metadata is relocated. '
             'The context retains reconstructed historical argv/cwd and the frozen executable requirement.',
    'captured_archive_bytes': len(data), 'captured_archive_sha256': captures['archive_prefix_sha256'],
    'snapshot_count': len(captures['snapshot_index']), 'requests': len(records),
}, indent=2) + '\n')
print(f'Restored {len(records)} requests and {len(captures["snapshot_index"])} exact snapshots.')
