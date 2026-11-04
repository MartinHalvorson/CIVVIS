"""Descriptive terrain/infrastructure census; not host build or route permission.

The fixed cohorts and selection are registered in the companion protocol.
Every state supplies a fresh city roster; tile updates replace whole records.
Last-exported visibility is not an independently queried current host verdict.
"""
import argparse
import collections
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path

LEGACY_PATH = Path(__file__).with_name('2026-10-25-capital-assault-audit.py')
SPEC = importlib.util.spec_from_file_location('capital_audit', LEGACY_PATH)
legacy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(legacy)
FLAT = {'TERRAIN_GRASS', 'TERRAIN_PLAINS', 'TERRAIN_DESERT', 'TERRAIN_TUNDRA', 'TERRAIN_SNOW'}
CLEAR = {'FEATURE_FOREST', 'FEATURE_JUNGLE', 'FEATURE_MARSH'}
IMMEDIATE_FEATURES = {None, '', 'FEATURE_VOLCANIC_SOIL'}


def terrain_category(plot, pid):
    if (plot.get('vis') is not True or plot.get('t') not in FLAT
            or plot.get('w') is not False or plot.get('i') is not False
            or any(plot.get(k) for k in ('r', 'd', 'im', 'wo', 'flooded', 'submerged'))):
        return None
    feature = plot.get('f')
    if feature not in IMMEDIATE_FEATURES and feature not in CLEAR:
        return None
    owner = plot.get('o')
    category = 'owned' if owner == pid else 'neutral' if owner == -1 else 'unknown' if owner is None else None
    if category is None:
        return None
    return category + ('_needs_clearance' if feature in CLEAR else '_immediate')


def major_capitals(state, majors):
    # Use only this state's roster. A city disappearing is a real disappearance.
    return [(rival, capital) for rival in state.get('rivals', [])
            for capital in rival.get('cities', [])
            if capital.get('original_capital') is True
            and capital.get('original_owner') in majors]


def infrastructure(state):
    cities = state.get('cities', [])
    techs = state.get('techs')
    return {
        'flight_known': 'TECH_FLIGHT' in techs if isinstance(techs, list) else None,
        'military_engineering_known': 'TECH_MILITARY_ENGINEERING' in techs if isinstance(techs, list) else None,
        'engineers': [unit for unit in state.get('units', []) if unit.get('kind') == 'UNIT_MILITARY_ENGINEER'],
        'armory_cities': [{'id': city['id'], 'name': city.get('name'),
                          'pillaged': 'BUILDING_ARMORY' in city.get('pillaged_buildings', [])}
                         for city in cities if 'BUILDING_ARMORY' in city.get('buildings', [])],
        'cities_missing_building_roster': [city['id'] for city in cities if 'buildings' not in city],
        'engineer_offers': [{'city': city['id'], 'channel': channel, 'offer': offer}
                            for city in cities for channel in ('buildable', 'purchasable')
                            for offer in city.get(channel, []) if offer.get('t') == 'UNIT_MILITARY_ENGINEER'],
        'cities_missing_offer_roster': [{'city': city['id'], 'channel': channel}
                                        for city in cities for channel in ('buildable', 'purchasable') if channel not in city],
    }


def census(directory):
    summary = json.loads((directory / 'summary.json').read_text())
    if summary.get('game_stopped') is not True:
        raise ValueError('fixed cohort member lacks a terminal receipt')
    pid = summary['seat']['local_player']
    events = directory / 'events.jsonl'
    majors, minors = {pid}, set()
    for state in legacy.records(events):
        if state.get('kind') == 'state':
            majors.update(r['player'] for r in state.get('rivals', []))
            minors.update(r['player'] for r in state.get('minors', []))
    if majors & minors:
        raise ValueError('contradictory major/minor roster')
    width = next(e['width'] for e in legacy.records(events) if e.get('kind') == 'tiles')
    plots, seen, blocked = {}, set(), []
    war_turns = 0
    for event_index, state in enumerate(legacy.records(events)):
        if state.get('kind') == 'tiles':
            if state['width'] != width or not isinstance(state.get('plots'), list):
                raise ValueError('invalid tile snapshot')
            plots.update({(p['x'], p['y']): p for p in state['plots']})
        if state.get('kind') != 'state' or state['turn'] in seen:
            continue
        seen.add(state['turn'])
        ready = [u for u in state.get('units', []) if u.get('class') == 'PROMOTION_CLASS_AIR_BOMBER'
                 and u.get('hp', 0) >= 50 and u.get('moves', 0) > 0 and u.get('attacks_remaining', 0) > 0]
        bases = legacy.bases(state, plots, pid)
        for rival, capital in major_capitals(state, majors):
            if rival.get('at_war') is not True:
                continue
            war_turns += 1
            if len(ready) < 2 or any(legacy.distance(base, capital, width) <= u.get('range', -1)
                                     for base in bases for u in ready):
                continue
            sites, unknown = collections.defaultdict(list), collections.Counter()
            for plot in plots.values():
                category = terrain_category(plot, pid)
                if category is None:
                    continue
                distance = legacy.distance(plot, capital, width)
                covered = [u['id'] for u in ready if distance <= u.get('range', -1)]
                if len(covered) < 2:
                    continue
                if category.startswith('unknown_'):
                    unknown[category] += 1
                    continue
                sites[category].append({'plot': plot, 'target_distance': distance,
                                        'ready_bombers_covered': covered,
                                        'nearest_own_city_distance': min((legacy.distance(plot, c, width)
                                                                          for c in state.get('cities', [])), default=None)})
            blocked.append({'turn': state['turn'], 'frame': state.get('frame'), 'event_index': event_index,
                            'holder': rival['player'], 'capital': capital, 'ready_bombers': ready,
                            'nearest_bases': [dict(base, target_distance=legacy.distance(base, capital, width))
                                              for base in sorted(bases, key=lambda b: legacy.distance(b, capital, width))[:3]],
                            'sites': dict(sites), 'unknown_owner_sites_excluded': dict(unknown),
                            'infrastructure': infrastructure(state)})
    categories = ('owned_immediate', 'neutral_immediate', 'owned_needs_clearance', 'neutral_needs_clearance')
    return {'tag': directory.name, 'difficulty': summary['seat']['difficulty'], 'outcome': summary.get('outcome'),
            'observed_majors': sorted(majors), 'observed_minors': sorted(minors), 'first_state_turns': len(seen),
            'counts': {'war_capital_turns': war_turns, 'blocked_capital_turns': len(blocked),
                       **{category: sum(bool(row['sites'].get(category)) for row in blocked) for category in categories},
                       'blocked_with_engineer': sum(bool(row['infrastructure']['engineers']) for row in blocked),
                       'blocked_with_armory': sum(bool(row['infrastructure']['armory_cities']) for row in blocked),
                       'blocked_with_engineer_offer': sum(bool(row['infrastructure']['engineer_offers']) for row in blocked)},
            'blocked_capital_turns': blocked,
            'files': {name: {'bytes': (directory / name).stat().st_size,
                            'sha256': hashlib.sha256((directory / name).read_bytes()).hexdigest()}
                      for name in ('summary.json', 'events.jsonl', 'decisions.jsonl')}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--control', type=Path, required=True)
    parser.add_argument('--air-state', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        raise ValueError('preserve earlier output')
    state = json.loads(args.air_state.read_text())
    if len(state['assigned']) != 16 or state['cutoff']['action'] != 'removed_owned_screen':
        raise ValueError('not the complete registered Culture-air cohort')
    cohorts = {'emperor_material': ['civvis-20261025T' + tag + 'Z' for tag in legacy.TAGS],
               'immortal_culture_air': state['assigned']}
    if len(set(sum(cohorts.values(), []))) != 24:
        raise ValueError('cohort membership overlaps')
    result = {'scope': __doc__, 'cohorts': {name: [census(args.control / tag) for tag in tags]
                                           for name, tags in cohorts.items()},
              'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'legacy_source_sha256': hashlib.sha256(LEGACY_PATH.read_bytes()).hexdigest(),
              'assignment_state_sha256': hashlib.sha256(args.air_state.read_bytes()).hexdigest()}
    old = result['cohorts']['emperor_material']
    if (sum(r['counts']['war_capital_turns'] for r in old),
            sum(r['counts']['blocked_capital_turns'] for r in old)) != (629, 70):
        raise ValueError('prior 629/70 whole-cohort control changed')
    args.out.write_bytes(gzip.compress(json.dumps(result, sort_keys=True).encode(), mtime=0))
    assert json.loads(gzip.decompress(args.out.read_bytes())) == result
    print(json.dumps({name: {row['tag']: row['counts'] for row in games}
                      for name, games in result['cohorts'].items()}, indent=2))


if __name__ == '__main__':
    main()
