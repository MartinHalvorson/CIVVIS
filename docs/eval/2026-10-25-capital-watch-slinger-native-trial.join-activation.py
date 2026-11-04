"""Join one candidate journal to its actual decision and conservative Native successor audit."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import sys


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--run', type=Path, required=True)
    p.add_argument('--audit', type=Path, required=True)
    p.add_argument('--tools', type=Path, required=True)
    p.add_argument('--turn', type=int, required=True)
    p.add_argument('--out', type=Path, required=True)
    args = p.parse_args()
    sys.path.insert(0, str(args.tools))
    from civ6_decision_trace import captured_input
    journal = [line for line in (args.run / 'why.log').read_text().splitlines()
               if line.startswith(f'[why] t{args.turn} ') and 'capital-watch-arms-the-slinger:' in line]
    if len(journal) != 1:
        raise ValueError('candidate journal is not unique')
    audit_bytes = args.audit.read_bytes()
    audit = json.loads(gzip.decompress(audit_bytes))
    choices = [u for u in audit['upgrades'] if u['turn'] == args.turn
               and u.get('before_unit', {}).get('kind') == 'UNIT_SLINGER']
    if len(choices) != 1 or choices[0]['classification'] != 'unique_exact_successor':
        raise ValueError('unique exact Slinger successor is not established')
    upgrade = choices[0]
    decisions = [json.loads(line) for line in (args.run / 'decisions.jsonl').read_text().splitlines()]
    selected = [d for d in decisions if (d['decision']['turn'], d['decision']['frame']) == (args.turn, upgrade['frame'])]
    if len(selected) != 1:
        raise ValueError('actual decision frame is not unique')
    actual = selected[0]
    subject = upgrade['before_unit']['id']
    if sum(o.get('verb') == 'UPGRADE' and o.get('subject') == subject for o in actual['orders']) != 1:
        raise ValueError('actual decision does not issue this unique upgrade')
    binary = Path(actual['binary'])
    if hashlib.sha256(binary.read_bytes()).hexdigest() != actual['binary_on_disk_sha256']:
        raise ValueError('actual binary bytes changed')
    inputs = []
    for read in actual['input_capture']['reads']:
        data = captured_input(actual['input_capture'], read['snapshot_id'])
        states = [json.loads(line) for line in data.splitlines() if json.loads(line).get('kind') == 'state']
        if not states or states[-1] != upgrade['before_state']:
            raise ValueError('captured read does not end at the audited actual state')
        inputs.append({'snapshot_id': read['snapshot_id'], 'calls': read['count'], 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
    summary = json.loads((args.run / 'summary.json').read_text())
    packet = {'scope': __doc__ + ' This establishes the executed candidate branch, not an incremental strength or survival effect.',
              'run': args.run.name, 'journal': journal[0], 'actual_decision': actual,
              'upgrade_audit': upgrade, 'audit_source_sha256': hashlib.sha256(audit_bytes).hexdigest(),
              'captured_input_controls': inputs,
              'terminal_record': {k: summary.get(k) for k in ('reason', 'last_turn', 'outcome')},
              'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    if args.out.exists():
        raise ValueError('preserve previous output')
    encoded = gzip.compress((json.dumps(packet, sort_keys=True, indent=2) + '\n').encode(), mtime=0)
    args.out.write_bytes(encoded)
    if json.loads(gzip.decompress(args.out.read_bytes())) != packet:
        raise ValueError('packet readback differs')
    print(json.dumps({'run': args.run.name, 'turn': args.turn, 'frame': upgrade['frame'],
                      'old_unit': subject, 'new_unit': upgrade['candidates'][0]['id'],
                      'inputs': inputs, 'packet_bytes': len(encoded), 'packet_sha256': hashlib.sha256(encoded).hexdigest()}))


if __name__ == '__main__':
    main()
