"""Verify one known retrospective Native win, never a treatment effect or rate.

The host TeamVictory callback and the run's own victory table determine the
winner/type. A pre-resolution city roster does not replace that host outcome.
"""
import argparse
import collections
import gzip
import hashlib
import json
from pathlib import Path
import sys


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def validate_host(summary, victories):
    outcome, seat = summary.get('outcome') or {}, summary.get('seat') or {}
    require(summary.get('game_stopped') is True, 'not terminal')
    require(outcome.get('kind') == 'victory' and outcome.get('won') is True, 'not a declared host win')
    require(victories == [outcome], 'raw host callback does not uniquely match the summary')
    require(outcome.get('team') is not None and outcome.get('team') == outcome.get('local_team'), 'winner is not our team')
    require(type(outcome.get('local_player')) is int and outcome['local_player'] >= 0
            and outcome['local_player'] == seat.get('local_player'), 'local seat differs')
    require(outcome.get('run') == summary.get('tag') == seat.get('run'), 'run identity differs')
    table = {row['index']: row['type'] for row in seat.get('victory_types', [])}
    require(table.get(outcome.get('victory')) == outcome.get('victory_type') == 'VICTORY_CONQUEST', 'not a host-table Conquest victory')
    require(summary.get('difficulty') == seat.get('difficulty') == 'DIFFICULTY_EMPEROR', 'Emperor readbacks differ')
    require(summary.get('ruleset') == summary.get('ruleset_requested') == seat.get('ruleset') == 'RULESET_EXPANSION_2', 'ruleset readbacks differ')
    require(summary.get('victory_target') == 'domination', 'not the assigned Domination lane')
    require(all(seat.get('victories', {}).get(key) is True for key in
                ('conquest', 'culture', 'diplomatic', 'religious', 'score', 'technology')), 'victory conditions were disabled or unknown')
    require(not summary.get('abandoned') and not summary.get('operator_retire'), 'abandoned or retired')
    return {'native_winner_matches_local_team': True, 'native_type_matches_actual_victory_table': True,
            'difficulty': seat['difficulty'], 'turn': outcome['turn'], 'type': outcome['victory_type']}


def rows(path):
    with path.open() as stream:
        for line in stream:
            yield json.loads(line)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', type=Path, required=True)
    parser.add_argument('--binary', type=Path, help='optional relocated frozen adapter')
    parser.add_argument('--capture-directory', type=Path, help='optional relocated immutable input archive')
    parser.add_argument('--tools', type=Path, default=Path(__file__).resolve().parents[2] / 'tools')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    require(not args.out.exists(), 'retain earlier output')
    summary = json.loads((args.run / 'summary.json').read_text())
    victories, last_state, counts, majors, minors = [], None, collections.Counter(), {summary['seat']['local_player']}, set()
    for event in rows(args.run / 'events.jsonl'):
        counts[event.get('kind')] += 1
        if event.get('kind') == 'victory':
            victories.append(event)
        if event.get('kind') == 'state':
            last_state = event
            majors.update(r['player'] for r in event.get('rivals', []))
            minors.update(r['player'] for r in event.get('minors', []))
    host = validate_host(summary, victories)
    starts = [r for r in rows(args.run / 'runtime_updates.jsonl') if r.get('status') == 'start']
    require(len(starts) == 1, 'not a unique frozen runtime')
    runtime = starts[0]
    binary = args.binary or Path(runtime['binary'])
    require(sha(binary) == runtime['binary_sha256'], 'frozen adapter bytes differ')
    decisions = list(rows(args.run / 'decisions.jsonl'))
    require(decisions and all(d['binary_on_disk_sha256'] == runtime['binary_sha256'] for d in decisions), 'decision binary identity differs')
    sys.path.insert(0, str(args.tools))
    from civ6_decision_trace import captured_input, validate_input_capture
    for decision in decisions:
        validate_input_capture(decision['input_capture'])
    capture = decisions[-1]['input_capture']
    resolved = dict(capture, directory=str(args.capture_directory.resolve())) if args.capture_directory else capture
    reads = []
    for descriptor in capture['reads']:
        require('error' not in descriptor, 'final captured read failed')
        data = captured_input(resolved, descriptor['snapshot_id'])
        states = [json.loads(line) for line in data.splitlines() if b'"state"' in line]
        final = [state for state in states if state.get('kind') == 'state'][-1]
        require(final == last_state, 'last captured Native state differs from actual event state')
        reads.append({'descriptor': descriptor, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'last_state_matches': True})
    result = {'scope': __doc__, 'host_verification': host, 'summary': summary, 'runtime': runtime,
              'binary_bytes_sha256': sha(binary), 'decision_count': len(decisions),
              'incomplete_capture_decisions': sum(not d['input_capture']['complete'] for d in decisions),
              'native_event_counts': dict(counts), 'last_state_before_victory': last_state,
              'last_actual_decision': decisions[-1], 'last_decision_captured_reads': reads,
              'observed_major_ids': sorted(majors), 'observed_minor_ids': sorted(minors),
              'loaded_mod_bytes_attested': False,
              'files': {name: {'bytes': (args.run / name).stat().st_size, 'sha256': sha(args.run / name)}
                        for name in ('summary.json', 'events.jsonl', 'decisions.jsonl', 'runtime_updates.jsonl', 'why.log')},
              'script_sha256': sha(Path(__file__))}
    args.out.write_bytes(gzip.compress(json.dumps(result, sort_keys=True).encode(), mtime=0))
    require(json.loads(gzip.decompress(args.out.read_bytes())) == result, 'output round trip differs')
    print(json.dumps({'host': host, 'decisions': len(decisions), 'incomplete_captures': result['incomplete_capture_decisions'],
                      'final_distinct_reads': len(reads), 'final_read_calls': sum(r['descriptor']['count'] for r in reads)}))


if __name__ == '__main__':
    main()
