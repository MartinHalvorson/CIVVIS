#!/usr/bin/env python3
"""Posthoc observed warning timings, not a simulated winning counterfactual."""
import argparse
import gzip
import hashlib
import importlib.util
import json
import re
from pathlib import Path

LAND = {'PROMOTION_CLASS_' + name for name in
        ['MELEE', 'RANGED', 'ANTI_CAVALRY', 'LIGHT_CAVALRY', 'HEAVY_CAVALRY', 'SIEGE', 'ANTI_AIR']}
HELPER = Path(__file__).with_name('2026-10-25-capital-assault-audit.py')
spec = importlib.util.spec_from_file_location('capital_assault', HELPER)
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)


def records(path):
    with path.open() as stream:
        for line in stream:
            yield json.loads(line)


def census(directory):
    summary = json.loads((directory / 'summary.json').read_text())
    if summary.get('game_stopped') is not True:
        raise ValueError('nonterminal cohort member')
    pid = summary['seat']['local_player']
    events = directory / 'events.jsonl'
    width = next(e['width'] for e in records(events) if e.get('kind') == 'tiles')
    majors, minors = {pid}, set()
    for event in records(events):
        if event.get('kind') == 'state':
            majors.update(r['player'] for r in event.get('rivals', []))
            minors.update(r['player'] for r in event.get('minors', []))
    if majors & minors:
        raise ValueError('contradictory founder classifications')
    plots, last_city, turns, first_wars = {}, {}, set(), {}
    rows, losses, orders, journal = [], [], [], []
    timing = {key: None for key in ['peace_2x', 'peace_2x_exported_land8',
              'peace_2x_visible_land8', 'masonry_readback', 'wall_queue_readback',
              'walls_readback', 'first_capital_damage']}
    for index, event in enumerate(records(events)):
        kind = event.get('kind')
        if kind == 'tiles':
            if event['width'] != width:
                raise ValueError('map width changed')
            plots.update({(p['x'], p['y']): p for p in event.get('plots', [])})
        if kind == 'city_lost':
            before = last_city.get(event.get('city'), {})
            if before.get('original_capital') is True and before.get('original_owner') == pid:
                losses.append(event)
        if kind != 'state':
            continue
        local = event.get('cities', [])
        last_city.update({city['id']: city for city in local})
        capitals = [city for city in local if city.get('original_capital') is True
                    and city.get('original_owner') == pid]
        if len(capitals) > 1:
            raise ValueError('multiple own original capitals')
        for rival in event.get('rivals', []):
            if rival.get('at_war') is True:
                first_wars.setdefault(str(rival['player']), event['turn'])
        stamp = dict(turn=event['turn'], frame=event.get('frame'), event_index=index)
        if 'TECH_MASONRY' in event.get('techs', []) and timing['masonry_readback'] is None:
            timing['masonry_readback'] = stamp
        if not capitals:
            continue
        capital = capitals[0]
        for key, present in [
            ('wall_queue_readback', capital.get('producing') == 'BUILDING_WALLS'),
            ('walls_readback', 'BUILDING_WALLS' in capital.get('buildings', [])
             or capital.get('max_wall_damage', 0) > 0),
            ('first_capital_damage', capital.get('damage', 0) > 0),
        ]:
            if present and timing[key] is None:
                timing[key] = stamp
        if event['turn'] in turns:
            continue
        turns.add(event['turn'])
        threats = []
        for rival in event.get('rivals', []):
            ratio = float(rival.get('military', 0)) / max(float(event.get('military', 0)), 1)
            if rival.get('at_war') is not False or ratio < 2:
                continue
            land = [dict(unit=unit, distance=helper.distance(unit, capital, width),
                         last_plot_visibility=plots.get((unit['x'], unit['y']), {}).get('vis'))
                    for unit in rival.get('units', []) if unit.get('class') in LAND]
            nearby = [unit for unit in land if unit['distance'] <= 8]
            threats.append(dict(player=rival['player'], civ=rival.get('civ'),
                                ratio=ratio, exported_land_units=land, within8=nearby))
        if threats and timing['peace_2x'] is None:
            timing['peace_2x'] = stamp
        if any(t['within8'] for t in threats) and timing['peace_2x_exported_land8'] is None:
            timing['peace_2x_exported_land8'] = stamp
        if any(u['last_plot_visibility'] is True for t in threats for u in t['within8']) \
                and timing['peace_2x_visible_land8'] is None:
            timing['peace_2x_visible_land8'] = stamp
        rows.append(dict(**stamp, capital=capital, military=event.get('military'),
                         research=event.get('research'), research_progress=event.get('research_progress'),
                         peace_twice_power=threats))
    for decision in records(directory / 'decisions.jsonl'):
        for order in decision.get('orders', []):
            if order.get('kind') == 'research' and order.get('verb') == 'TECH_MASONRY':
                orders.append(dict(turn=decision.get('decision', {}).get('turn'), order=order))
    for line in (directory / 'why.log').read_text().splitlines():
        if line.startswith('[why]') and 'capital-walls-before-the-surprise:' in line:
            match = re.match(r'\[why\] t(\d+) ', line)
            journal.append(dict(turn=int(match[1]) if match else None, line=line))
    return dict(tag=directory.name, outcome=summary.get('outcome'),
                played_policy='capital-walls-before-the-surprise' in summary['genome_treatments']['treatments'],
                timing=timing, wars=first_wars, losses=losses, masonry_orders=orders,
                policy_journal=journal, first_state_rows=rows,
                files={name: dict(bytes=(directory / name).stat().st_size,
                       sha256=hashlib.sha256((directory / name).read_bytes()).hexdigest())
                       for name in ['summary.json', 'events.jsonl', 'decisions.jsonl', 'why.log']})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--state', required=True, type=Path)
    parser.add_argument('--control', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    state = json.loads(args.state.read_text())
    games = []
    for stem in state['assigned']:
        directories = sorted(args.control.glob(stem + '*'))
        if not directories:
            raise ValueError('missing assignment: ' + stem)
        for directory in directories:
            result = census(directory)
            games.append(result)
            print(result['tag'], 'lost', [e['turn'] for e in result['losses']],
                  'timings', {k: v['turn'] if v else None for k, v in result['timing'].items()}, flush=True)
    report = dict(scope=__doc__, games=games,
                  helper_sha256=hashlib.sha256(HELPER.read_bytes()).hexdigest(),
                  script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    args.out.write_bytes(gzip.compress((json.dumps(report, indent=2) + '\n').encode(), mtime=0))


if __name__ == '__main__':
    main()
