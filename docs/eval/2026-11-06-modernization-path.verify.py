#!/usr/bin/env python3
"""Verify raw fixed-input proposals; never certify native application or strength."""
from pathlib import Path
import hashlib
import json
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
from civ6_decision_trace import captured_input
from native_city_route_replay import control_payload
import gene_fires


def verify(archive):
    artifact = json.loads((ROOT / 'docs/gene_screens/fires/modernization-prices-the-path.action-pairs.json').read_text())
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == artifact['archive']['sha256']
    assert gene_fires.captured_action_pair_fired(artifact)
    with tempfile.TemporaryDirectory() as directory:
        target = Path(directory)
        with tarfile.open(archive, 'r:*') as source:
            for member in source.getmembers():
                path = Path(member.name)
                assert member.isfile() and not path.is_absolute() and '..' not in path.parts
                destination = target / path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(source.extractfile(member).read())
        fixed = (target / 'input.events.jsonl').read_bytes()
        provenance = json.loads((target / 'source.json').read_text())
        assert hashlib.sha256(fixed).hexdigest() == provenance['fixed_input_sha256']
        assert len(fixed) == provenance['fixed_input_bytes']
        seen = {}
        for name in artifact['members']:
            pair = json.loads((target / name).read_text())
            assert pair['complete']
            for read in pair['reads']:
                assert read['bytes'] == len(fixed) and read['sha256'] == provenance['fixed_input_sha256']
                for arm in read['arms']:
                    dirname = Path(arm['archive_directory'])
                    assert not dirname.is_absolute() and '..' not in dirname.parts
                    where = target / dirname
                    raw = json.loads((where / 'stdout.json').read_text())
                    assert raw == arm['payload']
                    assert json.loads((where / 'result.json').read_text()) == arm
                    genomes = []
                    for line in (where / 'stderr.log').read_text().splitlines():
                        try:
                            value = json.loads(line)
                        except ValueError:
                            continue
                        if value.get('kind') == 'genome':
                            genomes.append(value)
                    assert genomes == [arm['genome']]
                    assert raw['turn'] == 68 and raw['decision']['frame'] == 0
                    original = Path(arm['argv'][arm['argv'].index('--mirror') + 1])
                    capture = dict(raw['input_capture'])
                    relative = Path(capture['directory']).relative_to(original)
                    assert '..' not in relative.parts
                    capture['directory'] = str(where / relative)
                    actual_reads = 0
                    for frontier in capture['reads']:
                        assert captured_input(capture, frontier['snapshot_id']) == fixed
                        actual_reads += frontier['count']
                    assert capture['complete'] and actual_reads == arm['actual_reads'] and actual_reads > 0
                    seen[str(dirname)] = (arm, control_payload(raw))
        assert len(seen) == 4
        assert seen['0-off'][1] == seen['3-off'][1]
        assert seen['1-on'][1] == seen['2-on'][1]
        assert seen['0-off'][0]['payload']['orders'] != seen['1-on'][0]['payload']['orders']
        print(json.dumps({'verified_queries': 4, 'actual_reads': {k: v[0]['actual_reads'] for k, v in seen.items()}, 'scope': provenance['scope']}, indent=2))


if __name__ == '__main__':
    verify(Path(sys.argv[1]))
