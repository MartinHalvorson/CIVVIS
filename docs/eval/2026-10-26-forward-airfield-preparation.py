"""Describe actual support-chain offers for every registered blocked capital-turn.

An offered next stage is not a complete project, future permission, safe route,
or strength gain. Native district `p` is turns left; `n` is a placement offer.
"""
import argparse
import collections
import gzip
import hashlib
import importlib.util
import json
import math
from pathlib import Path

LEGACY = Path(__file__).with_name('2026-10-25-capital-assault-audit.py')
SPEC = importlib.util.spec_from_file_location('capital_geometry', LEGACY)
geometry = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(geometry)
CHAIN = {'DISTRICT_ENCAMPMENT', 'BUILDING_BARRACKS', 'BUILDING_STABLE', 'BUILDING_ARMORY', 'UNIT_MILITARY_ENGINEER'}
PRIORITY = [('engineer', {'UNIT_MILITARY_ENGINEER'}), ('armory', {'BUILDING_ARMORY'}),
            ('barracks_or_stable', {'BUILDING_BARRACKS', 'BUILDING_STABLE'}), ('encampment', {'DISTRICT_ENCAMPMENT'})]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def city_offer(city, sites, width):
    offers = [{'channel': channel, 'quote': quote}
              for channel in ('buildable', 'purchasable') for quote in (city.get(channel) or [])
              if quote.get('t') in CHAIN]
    stage = next((label for label, types in PRIORITY if any(q['quote']['t'] in types for q in offers)), None)
    missing = [channel for channel in ('buildable', 'purchasable') if not isinstance(city.get(channel), list)]
    if stage is None:
        stage = 'menus_unknown' if missing else 'no_offered_stage'
    production = [q['quote'] for q in offers if q['channel'] == 'buildable']
    turns = [q['p'] for q in production if type(q.get('p')) in (int, float) and math.isfinite(q['p']) and q['p'] >= 0]
    return {'city': {key: city.get(key) for key in ('id', 'name', 'x', 'y', 'pop', 'district_capacity', 'production',
                   'production_progress', 'production_cost', 'production_turns', 'producing', 'producing_hash',
                   'queue', 'loyalty', 'loyalty_per_turn')},
            'encampments': [d for d in (city.get('districts') or []) if d.get('type') == 'DISTRICT_ENCAMPMENT'],
            'military_buildings': [b for b in (city.get('buildings') or []) if b in CHAIN],
            'pillaged_military_buildings': [b for b in (city.get('pillaged_buildings') or []) if b in CHAIN],
            'offers': offers, 'missing_menus': missing, 'most_advanced_offered_stage': stage,
            'minimum_quoted_production_turns_for_any_offered_stage': min(turns, default=None),
            'nearest_candidate_distance': min((geometry.distance(city, p, width) for p in sites), default=None)}


def describe(state, row, width):
    sites = [p['plot'] for category in ('owned_immediate', 'neutral_immediate')
             for p in row['sites'].get(category, [])]
    cities = [city_offer(city, sites, width) for city in state.get('cities', [])]
    income = state.get('gold_per_turn')
    positive = income > 0 if type(income) in (float, int) and math.isfinite(income) else None
    return {'turn': row['turn'], 'frame': row['frame'], 'capital': row['capital'], 'holder': row['holder'],
            'gold': state.get('gold'), 'income': income, 'positive_income': positive,
            'unit_maintenance_total': state.get('unit_maintenance_total'),
            'flight_known': 'TECH_FLIGHT' in state['techs'] if isinstance(state.get('techs'), list) else None,
            'military_engineering_known': 'TECH_MILITARY_ENGINEERING' in state['techs'] if isinstance(state.get('techs'), list) else None,
            'immediate_neutral_site_count': len(row['sites'].get('neutral_immediate', [])),
            'immediate_owned_site_count': len(row['sites'].get('owned_immediate', [])),
            'cities': cities,
            'observed_rival_progress': [{'player': rival['player'], 'fields': {
                key: value for key, value in rival.items()
                if 'victor' in key or 'touris' in key or 'science_project' in key}}
                for rival in state.get('rivals', [])]}


def census_game(root, game):
    for name, expected in game['files'].items():
        path = root / name
        if path.stat().st_size != expected['bytes'] or sha(path) != expected['sha256']:
            raise ValueError('fixed source file changed: ' + str(path))
    needed = {(row['turn'], row['frame']) for row in game['blocked_capital_turns']}
    states = collections.defaultdict(list)
    width = None
    if needed:
        for event in geometry.records(root / 'events.jsonl'):
            if event.get('kind') == 'tiles':
                if width is not None and event['width'] != width:
                    raise ValueError('map width changed')
                width = event['width']
            if event.get('kind') == 'state' and (event['turn'], event.get('frame')) in needed:
                states[(event['turn'], event.get('frame'))].append(event)
        if width is None:
            raise ValueError('missing observed map width')
    rows = []
    for row in game['blocked_capital_turns']:
        found = states[(row['turn'], row['frame'])]
        if len(found) != 1:
            rows.append({'turn': row['turn'], 'frame': row['frame'], 'capital': row['capital'],
                         'error': 'missing_or_ambiguous_actual_state', 'state_count': len(found)})
        else:
            rows.append(describe(found[0], row, width))
    return {'tag': game['tag'], 'difficulty': game['difficulty'], 'outcome': game['outcome'],
            'files': game['files'], 'unique_frames': len(needed), 'capital_turns': rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--census', type=Path, required=True)
    parser.add_argument('--control', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        raise ValueError('retain earlier output')
    prior = json.loads(gzip.decompress(args.census.read_bytes()))
    cohorts = {name: [census_game(args.control / game['tag'], game) for game in games]
               for name, games in prior['cohorts'].items()}
    counts = {name: sum(len(game['capital_turns']) for game in games) for name, games in cohorts.items()}
    if counts != {'emperor_material': 70, 'immortal_culture_air': 184}:
        raise ValueError('registered blocked cohort changed')
    summary = {}
    for name, games in cohorts.items():
        rows = [row for game in games for row in game['capital_turns']]
        valid = [row for row in rows if 'error' not in row]
        summary[name] = {'blocked_capital_turns': len(rows), 'valid_actual_states': len(valid),
                         'positive_income_with_neutral_site': sum(row['positive_income'] is True and row['immediate_neutral_site_count'] > 0 for row in valid),
                         'positive_income_neutral_site_and_offered_stage': sum(row['positive_income'] is True and row['immediate_neutral_site_count'] > 0
                              and any(city['offers'] for city in row['cities']) for row in valid),
                         'city_stage_counts': dict(collections.Counter(city['most_advanced_offered_stage'] for row in valid for city in row['cities']))}
    result = {'scope': __doc__, 'cohorts': cohorts, 'summary': summary,
              'census_sha256': sha(args.census), 'script_sha256': sha(Path(__file__)),
              'geometry_source_sha256': sha(LEGACY),
              'witness_frames': [{'tag': game['tag'], 'turn': game['capital_turns'][0]['turn'],
                                  'frame': game['capital_turns'][0]['frame']}
                                 for games in cohorts.values() for game in games if game['capital_turns']]}
    args.out.write_bytes(gzip.compress(json.dumps(result, sort_keys=True).encode(), mtime=0))
    if json.loads(gzip.decompress(args.out.read_bytes())) != result:
        raise ValueError('output round trip changed')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
