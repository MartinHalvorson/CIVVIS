"""Audit all captured reads of the protocol's explanatory terrain frames."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--census', type=Path, required=True)
    parser.add_argument('--control', type=Path, required=True)
    parser.add_argument('--tools', type=Path, required=True)
    parser.add_argument('--capture-map', type=Path,
                        help='optional JSON mapping original capture directories to restored absolute directories')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        raise ValueError('retain previous evidence')
    sys.path.insert(0, str(args.tools))
    from civ6_decision_trace import captured_input
    relocation = json.loads(args.capture_map.read_text()) if args.capture_map else {}
    if not isinstance(relocation, dict) or any(not isinstance(k, str) or not isinstance(v, str)
            or not Path(k).is_absolute() or not Path(v).is_absolute() for k, v in relocation.items()):
        raise ValueError('capture-map must map absolute directory strings')
    report = json.loads(gzip.decompress(args.census.read_bytes()))
    chosen = {}
    for cohort, games in report['cohorts'].items():
        found = next(((game, row) for game in games for row in game['blocked_capital_turns']
                      if row['sites'].get('neutral_immediate')), None)
        if found:
            game, row = found
            chosen[(game['tag'], row['turn'], row['frame'])] = row
    rio = next(game for game in report['cohorts']['emperor_material'] if game['tag'] == 'civvis-20261025T134959Z')
    row = next(row for row in rio['blocked_capital_turns'] if row['turn'] == 157 and row['capital']['name'] == 'Rio de Janeiro')
    chosen[(rio['tag'], row['turn'], row['frame'])] = row
    cases = []
    for (tag, turn, frame), row in chosen.items():
        root = args.control / tag
        decisions = [json.loads(line) for line in (root / 'decisions.jsonl').read_text().splitlines()]
        matches = [d for d in decisions if (d['decision']['turn'], d['decision']['frame']) == (turn, frame)]
        if len(matches) != 1:
            raise ValueError('ambiguous actual decision')
        decision = matches[0]
        capture = decision['input_capture']
        if not capture['complete'] or not capture['reads']:
            raise ValueError('incomplete actual capture')
        resolved = dict(capture, directory=relocation.get(capture['directory'], capture['directory']))
        states = [json.loads(line) for line in (root / 'events.jsonl').read_text().splitlines()
                  if '"state"' in line]
        actual = [s for s in states if s.get('kind') == 'state' and (s['turn'], s['frame']) == (turn, frame)]
        if len(actual) != 1:
            raise ValueError('ambiguous native state')
        reads = []
        for descriptor in capture['reads']:
            data = captured_input(resolved, descriptor['snapshot_id'])
            plots, last = {}, None
            for line in data.splitlines():
                event = json.loads(line)
                if event.get('kind') == 'tiles':
                    plots.update({(p['x'], p['y']): p for p in event['plots']})
                if event.get('kind') == 'state':
                    last = event
            if last != actual[0]:
                raise ValueError('captured final state differs from unique actual state')
            candidates = [site['plot'] for site in row['sites']['neutral_immediate']]
            comparisons = [{'census_plot': p, 'captured_plot': plots.get((p['x'], p['y'])),
                            'identical': p == plots.get((p['x'], p['y']))} for p in candidates]
            reads.append({'descriptor': descriptor, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(),
                          'last_state_matches': True, 'neutral_candidate_comparisons': comparisons})
        cases.append({'tag': tag, 'turn': turn, 'frame': frame, 'census_row': row,
                      'actual_state': actual[0], 'actual_decision': decision, 'reads': reads,
                      'resolved_capture_directory': resolved['directory']})
    result = {'scope': __doc__, 'cases': cases, 'census_sha256': hashlib.sha256(args.census.read_bytes()).hexdigest(),
              'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    args.out.write_bytes(gzip.compress(json.dumps(result, sort_keys=True).encode(), mtime=0))
    assert json.loads(gzip.decompress(args.out.read_bytes())) == result
    print(json.dumps([{'tag': c['tag'], 'turn': c['turn'], 'frame': c['frame'], 'distinct_reads': len(c['reads']),
                       'read_calls': sum(r['descriptor']['count'] for r in c['reads']),
                       'all_candidate_records_identical': all(p['identical'] for r in c['reads'] for p in r['neutral_candidate_comparisons'])}
                      for c in cases], indent=2))


if __name__ == '__main__':
    main()
