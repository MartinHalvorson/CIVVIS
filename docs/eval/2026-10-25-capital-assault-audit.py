"""Descriptive census of the fixed eight completed material-screen games.

Use the first native state each turn, replacing city rosters every state and
plot records every tiles payload. Terrain candidates are NOT host permissions,
routes, safe sites, or proof that constructing an airstrip would win a game.
"""
import argparse
import collections
import gzip
import hashlib
import json
from pathlib import Path

TAGS = ('123652', '124842', '130400', '131248', '132534', '134257', '134959', '140632')


def records(path):
    with path.open() as stream:
        for line in stream:
            yield json.loads(line)


def distance(a, b, width):
    aq, ar = a['x'] - (a['y'] - (a['y'] & 1)) // 2, a['y']
    bq, br = b['x'] - (b['y'] - (b['y'] & 1)) // 2, b['y']
    return min(max(abs(aq + shift - bq), abs(ar - br),
                   abs(aq + shift - bq + ar - br)) for shift in (-width, 0, width))


def bases(state, plots, pid):
    result = []
    for city in state.get('cities', []):
        rate = city.get('loyalty_per_turn')
        stable = (city.get('loyalty', 100) + 2 * min(0, rate) > 0
                  if isinstance(rate, (int, float)) else None)
        result.append(dict(x=city['x'], y=city['y'], kind='city_center',
                           stable_two_ticks=stable, name=city.get('name')))
        for district in city.get('districts', []):
            if district.get('type') == 'DISTRICT_AERODROME' and district.get('complete') is True and district.get('pillaged') is False:
                result.append(dict(x=district['x'], y=district['y'], kind='aerodrome',
                                   stable_two_ticks=stable, name=city.get('name')))
    for plot in plots.values():
        if plot.get('im') == 'IMPROVEMENT_AIRSTRIP' and plot.get('p') is False and (plot.get('o') == pid or plot.get('io') == pid):
            result.append(dict(x=plot['x'], y=plot['y'], kind='airstrip', stable_two_ticks=True))
    for unit in state.get('units', []):
        if unit.get('kind') == 'UNIT_AIRCRAFT_CARRIER':
            result.append(dict(x=unit['x'], y=unit['y'], kind='carrier', stable_two_ticks=True))
    return result


def terrain_candidate(plot, pid, clear_feature=False):
    # Deliberately narrow: owned, visible, flat, empty, resource-free ground.
    return (plot.get('o') == pid and plot.get('vis') is True
            and plot.get('t') in {'TERRAIN_GRASS', 'TERRAIN_PLAINS', 'TERRAIN_DESERT', 'TERRAIN_TUNDRA', 'TERRAIN_SNOW'}
            and plot.get('w') is False and plot.get('i') is False
            and (not plot.get('f') or (clear_feature and plot['f'] in {'FEATURE_FOREST', 'FEATURE_JUNGLE', 'FEATURE_MARSH'}))
            and not any(plot.get(k) for k in ('r', 'd', 'im', 'wo'))
            and not plot.get('flooded') and not plot.get('submerged'))


def census(directory):
    summary = json.loads((directory / 'summary.json').read_text())
    if summary.get('game_stopped') is not True:
        raise ValueError('fixed cohort member is not terminal')
    pid = summary['seat']['local_player']
    decisions = list(records(directory / 'decisions.jsonl'))
    shots = collections.Counter()
    for decision in decisions:
        for order in decision.get('orders', []):
            if order.get('kind') == 'unit' and order.get('verb') == 'AIR_ATTACK':
                shots[(decision['decision']['turn'], order['x'], order['y'])] += 1
    plots, seen_turns, rows, dispositions = {}, set(), [], []
    # IsOriginalCapital also marks city-state capitals subsequently captured
    # by a major. Classify founders from the observed native rosters; the
    # current holder's major status does not make the founder a major.
    majors, minors = {pid}, set()
    for observed in records(directory / 'events.jsonl'):
        if observed.get('kind') == 'state':
            majors.update(r['player'] for r in observed.get('rivals', []))
            minors.update(r['player'] for r in observed.get('minors', []))
    if majors & minors:
        raise ValueError('contradictory major/minor rosters')
    # An opening state can precede its first map sweep. Only dimensions are
    # read ahead; plot observations still enter strictly in event order.
    width = next(event['width'] for event in records(directory / 'events.jsonl')
                 if event.get('kind') == 'tiles')
    for index, event in enumerate(records(directory / 'events.jsonl')):
        if event.get('kind') == 'tiles':
            if event['width'] != width:
                raise ValueError('map width changed within a game')
            # Replacing the entire plot clears disappeared districts/features.
            plots.update({(p['x'], p['y']): p for p in event.get('plots', [])})
        if event.get('kind') == 'city_disposition':
            dispositions.append(dict(event_index=index, event=event))
        if event.get('kind') != 'state' or event['turn'] in seen_turns:
            continue
        seen_turns.add(event['turn'])
        if width is None:
            raise ValueError('state has no preceding map width')
        wing = [u for u in event.get('units', []) if u.get('class') == 'PROMOTION_CLASS_AIR_BOMBER']
        current_bases = bases(event, plots, pid)
        for rival in event.get('rivals', []):
            for capital in rival.get('cities', []):
                if capital.get('original_capital') is not True or capital.get('original_owner') not in majors:
                    continue
                ready = [u for u in wing if u.get('hp', 0) >= 50 and u.get('moves', 0) > 0 and u.get('attacks_remaining', 0) > 0]
                reaches = [u for u in ready if distance(u, capital, width) <= u.get('range', -1)]
                with_base = [u for u in ready if any(distance(b, capital, width) <= u.get('range', -1) for b in current_bases)]
                stable_base = [u for u in ready if any(b['stable_two_ticks'] is True and distance(b, capital, width) <= u.get('range', -1) for b in current_bases)]
                candidates = [p for p in plots.values() if terrain_candidate(p, pid)
                              and any(distance(p, capital, width) <= u.get('range', -1) for u in ready)]
                clearable = [p for p in plots.values() if terrain_candidate(p, pid, True)
                             and any(distance(p, capital, width) <= u.get('range', -1) for u in ready)]
                takers = [u for u in event.get('units', []) if u.get('hp', 0) >= 30 and u.get('class') in {
                    'PROMOTION_CLASS_MELEE', 'PROMOTION_CLASS_ANTI_CAVALRY',
                    'PROMOTION_CLASS_LIGHT_CAVALRY', 'PROMOTION_CLASS_HEAVY_CAVALRY', 'PROMOTION_CLASS_RECON'}]
                visible = plots.get((capital['x'], capital['y']), {}).get('vis')
                nearest = sorted(current_bases, key=lambda b: (distance(b, capital, width), b['x'], b['y']))[:3]
                rows.append(dict(turn=event['turn'], frame=event.get('frame'), event_index=index,
                    capital=capital, holder=rival['player'], at_war=rival.get('at_war'),
                    ready_bombers=len(ready), currently_in_range=len(reaches),
                    any_base_in_range=len(with_base), stable_base_in_range=len(stable_base),
                    nearest_bases=[dict(b, distance=distance(b, capital, width)) for b in nearest],
                    flight_known='TECH_FLIGHT' in event.get('techs', []),
                    engineers=[u for u in event.get('units', []) if u.get('kind') == 'UNIT_MILITARY_ENGINEER'],
                    terrain_candidates=candidates, clearable_terrain_candidates=clearable,
                    target_visible_last_plot=visible,
                    nearest_healthy_land_taker=min((distance(u, capital, width) for u in takers), default=None),
                    issued_air_attacks_on_capital_plot=shots[(event['turn'], capital['x'], capital['y'])],
                    gold=event.get('gold'), gold_per_turn=event.get('gold_per_turn')))
    blocked = [r for r in rows if r['at_war'] is True and r['ready_bombers'] >= 2 and r['any_base_in_range'] == 0]
    return dict(tag=directory.name, scope=__doc__, outcome=summary.get('outcome'),
        difficulty=summary['seat']['difficulty'], width=width, observed_major_ids=sorted(majors), observed_minor_ids=sorted(minors),
        files={n: dict(bytes=(directory/n).stat().st_size, sha256=hashlib.sha256((directory/n).read_bytes()).hexdigest())
               for n in ('summary.json', 'events.jsonl', 'decisions.jsonl')},
        first_state_turns=len(seen_turns), capital_turns=rows, dispositions=dispositions,
        counts=dict(war_capital_turns=sum(r['at_war'] is True for r in rows),
                    ready_wing_without_base=len(blocked),
                    blocked_turns_with_owned_terrain_candidates=sum(bool(r['terrain_candidates']) for r in blocked),
                    blocked_turns_with_clearable_owned_terrain=sum(bool(r['clearable_terrain_candidates']) for r in blocked),
                    ready_wing_currently_in_range=sum(r['at_war'] is True and r['currently_in_range'] >= 2 for r in rows),
                    ready_wing_in_range_visible_near_taker=sum(r['at_war'] is True and r['currently_in_range'] >= 2 and r['target_visible_last_plot'] is True and r['nearest_healthy_land_taker'] is not None and r['nearest_healthy_land_taker'] <= 8 for r in rows),
                    air_attacks_on_capital_plot=sum(r['issued_air_attacks_on_capital_plot'] for r in rows),
                    blocked_turns_with_engineer=sum(bool(r['engineers']) for r in blocked)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--control', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        raise ValueError('preserve prior evidence; use a new output path')
    result = {'scope': __doc__, 'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'games': [census(args.control / ('civvis-20261025T' + tag + 'Z')) for tag in TAGS]}
    args.out.write_bytes(gzip.compress(json.dumps(result, sort_keys=True).encode(), mtime=0))
    assert json.loads(gzip.decompress(args.out.read_bytes())) == result
    print(json.dumps({g['tag']: g['counts'] for g in result['games']}, indent=2))


if __name__ == '__main__':
    main()
