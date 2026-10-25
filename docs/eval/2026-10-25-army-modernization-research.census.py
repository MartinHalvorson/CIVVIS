"""Post-hoc census of every fixed native material-trial game's research and army.

Research quotes include unresearched nodes, not a proof of current selectability.
Modeled successor technologies are labeled and not substituted for native quotes.
"""
from __future__ import annotations
import argparse, collections, datetime, gzip, hashlib, json
from pathlib import Path

TRACKED = ('UNIT_ARCHER', 'UNIT_HORSEMAN', 'UNIT_CATAPULT')
MILESTONES = (60, 75, 90, 100, 120, 150)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rows(path):
    return [json.loads(line) for line in path.read_bytes().splitlines()]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--observer', type=Path, required=True)
    parser.add_argument('--runs', type=Path, required=True)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    assert not args.out.exists()
    assigned = json.loads((args.observer / 'state.json').read_text())['assigned']
    assert len(assigned) == 8
    units_path = args.repo / 'data/units.json'
    rules = json.loads(units_path.read_text())
    successors = {}
    for kind in TRACKED:
        name = kind.removeprefix('UNIT_').lower()
        entry = rules.get(name)
        nxt = entry.get('upgrade_to') if entry else None
        successor = rules.get(nxt) if nxt else None
        successors[kind] = {'model_unit': name, 'model_successor': nxt,
                            'model_successor_tech': successor.get('tech') if successor else None}
    output = {'scope': __doc__, 'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'collector_sha256': sha(Path(__file__)), 'unit_rules_sha256': sha(units_path),
              'model_successors': successors, 'cases': []}
    for tag in assigned:
        directory = args.runs / tag
        events = rows(directory / 'events.jsonl')
        states = [x for x in events if x.get('kind') == 'state']
        first = {}
        for state in states:
            first.setdefault(state['turn'], state)
        summary = json.loads((directory / 'summary.json').read_text())
        decisions = rows(directory / 'decisions.jsonl')
        selections = [{'source_line': i + 1, 'turn': d['decision']['turn'],
                       'frame': d['decision'].get('frame', 0), 'action': action,
                       'execution_status': d.get('execution_status')}
                      for i, d in enumerate(decisions)
                      for action in d['decision'].get('native_actions', [])
                      if action['type'] == 'research']
        transitions = []
        previous = object()
        for state in states:
            research = state.get('research')
            if research != previous:
                transitions.append({'turn': state['turn'], 'frame': state.get('frame', 0),
                                    'research': research, 'research_progress': state.get('research_progress')})
                previous = research

        def snapshot(state):
            quotes = {q['t']: q for q in state.get('research_quotes', [])}
            techs = set(state.get('techs', []))
            old = {}
            for kind in TRACKED:
                held = [u for u in state.get('units', []) if u['kind'] == kind]
                model_tech = successors[kind]['model_successor_tech']
                native_tech = 'TECH_' + model_tech.upper() if model_tech else None
                old[kind] = {'count': len(held), 'model_successor_tech': native_tech,
                             'known': native_tech in techs if native_tech else None,
                             'native_research_quote': quotes.get(native_tech),
                             'native_unit_quotes': [{k: u.get(k) for k in
                                ('id', 'hp', 'moves', 'xp', 'level', 'x', 'y', 'upgrade_to',
                                 'upgrade_cost', 'upgrade_resource', 'upgrade_resource_cost',
                                 'upgrade_blocked_reason')} for u in held]}
            return {'turn': state['turn'], 'frame': state.get('frame', 0),
                    'research': state.get('research'), 'science': state.get('science'),
                    'gold': state.get('gold'), 'gold_per_turn': state.get('gold_per_turn'),
                    'major_wars': [r['player'] for r in state.get('rivals', []) if r.get('at_war')],
                    'known_techs': sorted(techs), 'research_quotes': state.get('research_quotes', []),
                    'units': dict(collections.Counter(u['kind'] for u in state.get('units', []))),
                    'tracked_army': old}

        history = [snapshot(s) for _, s in sorted(first.items())]
        case = {'tag': tag, 'summary': summary, 'source_files': {n: sha(directory / n) for n in
                ('events.jsonl', 'decisions.jsonl', 'summary.json')}, 'research_selections': selections,
                'native_research_transitions': transitions,
                'research_order_receipts': [e for e in events if e.get('kind') in ('order_verified', 'order_failed') and e.get('order_kind') == 'research'],
                'first_native_upgrade_quote': {kind: next(({'turn': s['turn'], 'units': [u for u in s.get('units', []) if u['kind'] == kind and u.get('upgrade_to')]} for s in states if any(u['kind'] == kind and u.get('upgrade_to') for u in s.get('units', []))), None) for kind in TRACKED},
                'first_state_each_turn': history,
                'milestones': {str(t): snapshot(first[t]) if t in first else None for t in MILESTONES},
                'terminal': snapshot(states[-1]) if states else None,
                'first_observed_known': {tech: next((s['turn'] for s in states if tech in s.get('techs', [])), None)
                    for tech in ('TECH_MACHINERY', 'TECH_CASTLES', 'TECH_ENGINEERING',
                                 'TECH_MILITARY_ENGINEERING', 'TECH_METAL_CASTING', 'TECH_ADVANCED_FLIGHT')}}
        output['cases'].append(case)
        print(tag, case['first_observed_known'], flush=True)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(args.out, 'wt') as stream:
        json.dump(output, stream, sort_keys=True)
        stream.write('\n')
    print('wrote', args.out, 'sha256', sha(args.out), flush=True)


if __name__ == '__main__':
    main()
