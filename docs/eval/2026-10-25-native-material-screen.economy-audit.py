"""Retain every assigned stem's native economy, notes, deals and arm readback.

Notes are proposals. Closed-deal records carry UI terms, not independent
settlement confirmation. Native stock/Gold observations remain alongside them;
this report does not attribute upgrades or outcome differences to the policy.
"""
from __future__ import annotations
import argparse
from collections import Counter
import datetime
import gzip
import hashlib
import json
from pathlib import Path
import sys


def rows(path):
    return [json.loads(line) for line in path.read_bytes().splitlines()] if path.exists() else []


def economy(state, major_ids=None, minor_ids=None):
    route_gold = sum((route.get('yields') or {}).get('gold', 0)
                     for route in state.get('trade_routes', []))
    capital_owners = [c.get('original_owner') for c in state.get('cities', [])
                      if c.get('original_capital') is True]
    unknown_capital_owners = [owner for owner in capital_owners
                             if owner not in (major_ids or set()) | (minor_ids or set())]
    return {key: state.get(key) for key in
            ('turn', 'frame', 'gold', 'gold_per_turn', 'science', 'culture',
             'strategic_resources', 'strategic_resource_income')} | {
                'techs': len(state.get('techs', [])),
                'cities': len(state.get('cities', [])),
                'reported_route_gold_per_turn': route_gold,
                'observed_original_capitals': [{'city': c.get('name'), 'original_owner': c.get('original_owner')}
                    for c in state.get('cities', []) if c.get('original_capital') is True],
                'unknown_original_capital_owners': unknown_capital_owners,
                'original_major_capital_owners': None if major_ids is None or unknown_capital_owners or
                    any('original_capital' not in c or 'original_owner' not in c for c in state.get('cities', []))
                    else sorted(c['original_owner'] for c in state.get('cities', [])
                                if c['original_capital'] and c['original_owner'] in major_ids)}


def inspect(directory, stem, tools, upgrade_audits):
    sys.path.insert(0, str(tools))
    import civ6_ladder
    summary_path = directory / 'summary.json'
    summary = json.loads(summary_path.read_text()) if summary_path.exists() else None
    result = {'tag': directory.name, 'stem': stem, 'summary': summary,
              'status': 'terminal' if summary and summary.get('game_stopped') else 'pending',
              'files': {}, 'upgrade_audits': []}
    for path in sorted(directory.glob('*')):
        if path.is_file() and path.name in ('summary.json', 'events.jsonl', 'decisions.jsonl',
                'why.log', 'brain.log', 'civvis_notes.jsonl', 'runtime_updates.jsonl'):
            result['files'][path.name] = {'bytes': path.stat().st_size,
                                         'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
    if result['status'] != 'terminal':
        return result
    genome = civ6_ladder.decider_genome(directory / 'why.log')
    result['played_genome'] = genome
    gene = 'army-upgrade-materials'
    key = f"{stem}|{gene}|{summary['difficulty']}|{summary['victory_target']}"
    arm = 'on' if int.from_bytes(hashlib.blake2b(key.encode(), digest_size=8).digest(), 'big') % 2 else 'off'
    result['arm_key'] = key
    result['derived_arm'] = arm
    result['summary_arm_matches'] = summary.get('screen_gene') == gene and summary.get('screen_arm') == arm
    result['played_arm_matches'] = None if genome is None else ((gene in genome.get('treatments', [])) == (arm == 'on'))
    notes = rows(directory / 'civvis_notes.jsonl')
    result['all_notes'] = notes
    result['material_notes'] = [{'source_line': i + 1, **note} for i, note in enumerate(notes)
                                if 'army_material' in note.get('note', '')]
    markers = {'buy_proposal': 'army_material_buy=1', 'sale_hold_proposal': 'army_material_sale_held=',
               'buy_hold': 'army_material_buy_hold:'}
    result['marker_counts'] = {}
    for name, marker in markers.items():
        selected = [n for n in notes if marker in n.get('note', '')]
        result['marker_counts'][name] = {'notes': len(selected), 'distinct_turns': len({n['turn'] for n in selected})}
    events = rows(directory / 'events.jsonl')
    states = [s for s in events if s.get('kind') == 'state']
    seats = [e for e in events if e.get('kind') == 'seat']
    result['native_seats'] = seats
    local = seats[0].get('local_player') if seats else None
    roster = {local} if local is not None else set()
    roster.update(r['player'] for state in states for r in state.get('rivals', []) if 'player' in r)
    complete = bool(seats) and len(roster) == seats[0].get('players')
    result['observed_major_roster'] = sorted(roster)
    result['major_roster_complete'] = complete
    major_ids = roster
    minor_ids = {r['player'] for state in states for r in state.get('minors', []) if 'player' in r}
    result['observed_minor_roster'] = sorted(minor_ids)
    result['economy_timeline'] = [economy(s, major_ids, minor_ids) for s in states]
    changes = []
    previous = set()
    for entry in result['economy_timeline']:
        owners = entry['original_major_capital_owners']
        if owners is None:
            continue
        current = set(owners)
        if current != previous:
            changes.append({'turn': entry['turn'], 'frame': entry['frame'],
                            'observed_gains': sorted(current - previous),
                            'observed_losses': sorted(previous - current)})
        previous = current
    result['observed_major_capital_ownership_changes'] = changes
    result['milestones'] = {}
    for turn in [60, 100, 150]:
        matches = [s for s in states if s.get('turn') == turn]
        result['milestones'][str(turn)] = economy(matches[0], major_ids, minor_ids) if matches else None
    result['terminal_native_economy'] = economy(states[-1], major_ids, minor_ids) if states else None
    result['all_deal_events'] = [s for s in events if str(s.get('kind', '')).startswith('deal_')]
    result['closed_deal_terms'] = [s for s in events if s.get('kind') == 'deal_closed']
    reports = [s for s in events if s.get('kind') == 'orders'
               and ((s.get('seen_by') or {}).get('buy') or (s.get('seen_by') or {}).get('sell'))]
    result['trade_order_reports'] = reports
    failures = Counter()
    for report in reports:
        failures.update((report.get('refused_by') or {}).get('buy') or {})
    result['all_buy_refusal_counts'] = dict(failures)
    decisions = rows(directory / 'decisions.jsonl')
    result['resource_order_proposals'] = [
        {'turn': d.get('decision', {}).get('turn'), 'frame': d.get('decision', {}).get('frame'),
         'orders': selected}
        for d in decisions
        if (selected := [order for order in d.get('orders', [])
                         if order.get('kind') in ('buy', 'sell') and 'RESOURCE_' in order.get('verb', '')])]
    result['runtime'] = rows(directory / 'runtime_updates.jsonl')
    for path, audit in upgrade_audits:
        if audit['run'] == directory.name:
            ids = [(r['candidates'][0]['id'], r['candidates'][0]['kind'])
                   for r in audit['upgrades'] if r['classification'] in
                   ('unique_exact_successor', 'same_id_exact_successor')]
            result['upgrade_audits'].append({'file': str(path),
                'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                'issued': len(audit['upgrades']), 'classifications': audit['transition_counts'],
                'confirmed_rows': len(ids), 'distinct_successors': len(set(ids))})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--observer', type=Path, required=True)
    parser.add_argument('--runs', type=Path, required=True)
    parser.add_argument('--tools', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        raise ValueError('retain the previous audit; use a new output path')
    state = json.loads((args.observer / 'state.json').read_text())
    if len(state['assigned']) != 8 or state['cutoff'] is None:
        raise ValueError('the registered start cutoff has not completed')
    audits = [(p, json.loads(gzip.decompress(p.read_bytes())))
              for p in sorted(args.observer.glob('*-game-upgrade-audit.json.gz'))]
    cases = []
    for stem in state['assigned']:
        paths = [args.runs / stem] + sorted(args.runs.glob(stem + '-cont*'))
        cases.append({'stem': stem, 'attempts': [inspect(p, stem, args.tools, audits) for p in paths]})
    report = {'scope': __doc__, 'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'assignment_state': state, 'drivers': rows(args.observer / 'drivers.jsonl'),
              'cases': cases, 'auditor_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    args.out.write_bytes(gzip.compress(json.dumps(report, sort_keys=True).encode(), mtime=0))
    assert json.loads(gzip.decompress(args.out.read_bytes())) == report
    print(json.dumps({'stems': len(cases), 'terminal_attempts': sum(a['status'] == 'terminal'
                       for c in cases for a in c['attempts']),
                       'pending_attempts': sum(a['status'] != 'terminal' for c in cases for a in c['attempts']),
                       'output_sha256': hashlib.sha256(args.out.read_bytes()).hexdigest()}))


if __name__ == '__main__':
    main()
