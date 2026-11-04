"""Conservative read-only Native opening census; local yield bounds are not commands."""
import math

YIELDS = ('food', 'production', 'gold', 'science', 'culture', 'faith')


def number(value, nonnegative=False):
    if type(value) not in (int, float) or not math.isfinite(value):
        return None
    return value if not nonnegative or value >= 0 else None


def major_standing(event):
    raw = event.get('major_production')
    pid = event.get('local_player')
    if not isinstance(raw, dict) or type(pid) is not int:
        return {'known': False, 'reason': 'missing_map_or_local_player'}
    readings = {str(k): number(v, True) for k, v in raw.items()}
    invalid = [k for k, v in readings.items() if v is None]
    expected = event.get('majors')
    expected = expected + 1 if type(expected) is int and expected >= 0 else None
    if invalid or str(pid) not in readings or (expected is not None and len(readings) != expected):
        return {'known': False, 'reason': 'incomplete_major_readings', 'readings': readings,
                'invalid': invalid, 'expected_players': expected}
    others = {k: v for k, v in readings.items() if k != str(pid)}
    if not others:
        return {'known': False, 'reason': 'no_rival_reading', 'readings': readings}
    leader = max(sorted(others), key=lambda k: others[k])
    ours, theirs = readings[str(pid)], others[leader]
    return {'known': True, 'local_player': pid, 'leader': int(leader), 'ours_queue': ours,
            'leader_queue': theirs, 'queue_ratio': ours / theirs if theirs > 0 else 1.0,
            'readings': readings, 'expected_players': expected}


def public_decomposition(state, standing):
    if not standing.get('known'):
        return {'known': False, 'reason': 'global_standing_unknown'}
    rivals = [r for r in state.get('rivals', []) if r.get('player') == standing['leader']]
    if len(rivals) != 1:
        return {'known': False, 'reason': 'global_leader_not_in_met_rival_state'}
    own, rival = state.get('public_stats'), rivals[0].get('public_stats')
    if not isinstance(own, dict) or not isinstance(rival, dict):
        return {'known': False, 'reason': 'public_stats_unknown'}
    values = [[number(s.get(k), True) for k in ('city_count', 'population', 'production')]
              for s in (own, rival)]
    if any(v is None for row in values for v in row) or any(row[0] <= 0 or row[1] <= 0 for row in values) or values[1][2] <= 0:
        return {'known': False, 'reason': 'public_stats_unreadable_or_zero_denominator', 'values': values}
    (oc, op, oy), (rc, rp, ry) = values
    factors = [oc / rc, (op / oc) / (rp / rc), (oy / op) / (ry / rp)]
    product = math.prod(factors)
    if not math.isclose(product, oy / ry, rel_tol=1e-10, abs_tol=1e-10):
        raise ValueError('city-yield factorization failed')
    return {'known': True, 'leader': standing['leader'], 'own_public': own, 'leader_public': rival,
            'city_yield_ratio': oy / ry, 'city_ratio': factors[0],
            'population_per_city_ratio': factors[1], 'production_per_population_ratio': factors[2],
            'product': product, 'separate_turn_queue_ratio': standing['queue_ratio']}


def yield_quote(value):
    if isinstance(value, list) and len(value) == 6:
        values = dict(zip(YIELDS, value))
    elif isinstance(value, dict) and all(k in value for k in YIELDS):
        values = {k: value[k] for k in YIELDS}
    else:
        return None
    return values if all(number(v, True) is not None for v in values.values()) else None


def terrain_workers(city):
    if not isinstance(city.get('worked'), list):
        return None
    center = (city.get('x'), city.get('y'))
    districts = {(d.get('x'), d.get('y')) for d in city.get('districts', [])}
    wonders = {(d.get('x'), d.get('y')) for d in city.get('wonders', [])}
    return [p for p in city['worked'] if (p.get('x'), p.get('y')) != center
            and (p.get('x'), p.get('y')) not in districts | wonders]


def explicit_plots(events, target):
    plots, width = {}, None
    for event in events:
        if event.get('kind') != 'tiles' or event.get('ctx') != 'agent':
            continue
        turn = event.get('turn')
        if type(turn) is not int or turn > target:
            continue
        if type(event.get('width')) is int and event['width'] > 0:
            width = event['width']
        for plot in event.get('plots', []):
            if type(plot.get('x')) is int and type(plot.get('y')) is int:
                plots[(plot['x'], plot['y'])] = {'plot': dict(plot), 'observed_turn': turn}
    return plots, width


def distance(a, b, width):
    aq, ar = a[0] - (a[1] - (a[1] & 1)) // 2, a[1]
    bq, br = b[0] - (b[1] - (b[1] & 1)) // 2, b[1]
    return min(max(abs(aq + shift - bq), abs(ar - br), abs(aq + shift - bq + ar - br))
               for shift in (-width, 0, width))


def allocation_bounds(city, pid, turn, plots, width):
    workers = terrain_workers(city)
    result = {'scope': 'At most one raw terrain-yield exchange; not Native allocation permission or execution.',
              'city': {k: city.get(k) for k in ('id', 'name', 'x', 'y', 'pop')},
              'terrain_workers': None if workers is None else len(workers),
              'specialists': city.get('specialists'), 'pareto': [], 'food_floor_tradeoffs': [],
              'unknowns': [], 'eligible_unworked': 0}
    if workers is None or width is None:
        result['unknowns'].append('missing_worked_roster_or_width')
        return result
    worked = {(p.get('x'), p.get('y')) for p in city['worked']}
    excluded = worked | {(city.get('x'), city.get('y'))}
    excluded |= {(d.get('x'), d.get('y')) for d in city.get('districts', []) + city.get('wonders', [])}
    surplus = number(city.get('food_surplus'))
    if surplus == -1:
        surplus = None  # The exporter uses -1 for an unreadable growth accessor.
    for pos, record in plots.items():
        if pos in excluded or distance((city['x'], city['y']), pos, width) > 3:
            continue
        p = record['plot']
        if record['observed_turn'] != turn:
            continue
        if p.get('vis') is not True or p.get('o') != pid or p.get('oc') != city.get('id') or p.get('i') is not False or p.get('d') is not None or p.get('flooded') is not False or p.get('submerged') is not False:
            continue
        target = yield_quote(p.get('yl'))
        if target is None:
            result['unknowns'].append({'plot': list(pos), 'reason': 'missing_native_yield_quote'})
            continue
        result['eligible_unworked'] += 1
        for old in workers:
            before = yield_quote(old.get('yields'))
            if before is None:
                result['unknowns'].append({'worked': [old.get('x'), old.get('y')], 'reason': 'missing_worked_yields'})
                continue
            delta = {k: target[k] - before[k] for k in YIELDS}
            if delta['production'] < 1:
                continue
            pair = {'from': [old['x'], old['y']], 'to': list(pos), 'delta': delta,
                    'food_surplus_after_raw_exchange': None if surplus is None else surplus + delta['food']}
            if all(delta[k] >= 0 for k in YIELDS if k != 'production'):
                result['pareto'].append(pair)
            if surplus is not None and surplus + delta['food'] >= 2:
                result['food_floor_tradeoffs'].append(pair)
    for key in ('pareto', 'food_floor_tradeoffs'):
        result['maximum_one_worker_' + key + '_production_delta'] = max((p['delta']['production'] for p in result[key]), default=0)
    if not result['eligible_unworked']:
        result['unknowns'].append('no_eligible_explicit_same_turn_unworked_plot; not proof no opportunity exists')
    return result


def select_primary(attempts, target):
    choices = []
    for attempt in attempts:
        for index, record in enumerate(attempt['decisions']):
            decision = record.get('decision', {})
            if decision.get('turn') == target and decision.get('frame') == 0:
                choices.append((attempt['ordinal'], index, attempt['tag'], record))
    choices.sort(key=lambda row: row[:3])
    return (choices[0] if choices else None), choices


def city_metrics(city):
    workers = terrain_workers(city)
    return {'identity': {k: city.get(k) for k in ('id', 'name', 'x', 'y')},
            'queue_production': number(city.get('production'), True),
            'city_yield_production': number((city.get('yields') or {}).get('production'), True),
            'native_fields': {k: city.get(k) for k in ('pop', 'food_surplus', 'housing', 'growth_turns',
                'overall_growth_mult', 'happiness_yield_mult', 'food_favored', 'food_disfavored',
                'food_focus_managed', 'producing_hash', 'production_cost', 'production_progress',
                'production_turns', 'buildable')},
            'terrain_workers': None if workers is None else len(workers),
            'specialist_count': len(city['specialists']) if isinstance(city.get('specialists'), list) else None}


def main():
    import argparse
    import gzip
    import hashlib
    import json
    import pathlib
    import re
    import sys

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=pathlib.Path, required=True)
    parser.add_argument('--control', type=pathlib.Path, required=True)
    parser.add_argument('--out', type=pathlib.Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    selection_path = pathlib.Path(__file__).with_name('2026-10-26-native-opening-production.selection.json')
    selection = json.loads(selection_path.read_text())
    sys.path.insert(0, str(args.repo / 'tools'))
    from civ6_decision_trace import captured_input
    from civ6_ladder import decider_genome
    import civ6_play

    def digest(data):
        return hashlib.sha256(data).hexdigest()

    def rows(path):
        data = path.read_bytes()
        values, malformed = [], []
        for index, line in enumerate(data.splitlines()):
            try:
                values.append(json.loads(line))
            except ValueError:
                malformed.append(index + 1)
        return values, {'path': str(path), 'bytes': len(data), 'sha256': digest(data), 'malformed_lines': malformed}

    result = {'scope': __doc__, 'selection': selection,
              'selection_sha256': digest(selection_path.read_bytes()),
              'analyzer_sha256': digest(pathlib.Path(__file__).read_bytes()),
              'retirement_helper_sha256': digest((args.repo / 'tools/civ6_play.py').read_bytes()),
              'cases': [], 'attempts': [], 'retirement_probes': []}
    if (civ6_play.PRODUCTION_SHARE_TURN, civ6_play.PRODUCTION_SHARE_TURNS) != (50, 3):
        raise ValueError('retirement helper constants changed from registration')
    for stem in selection['cohort']:
        pattern = re.compile(re.escape(stem) + r'(?:-cont(\d+))?$')
        paths = sorted((p for p in args.control.iterdir() if p.is_dir() and pattern.fullmatch(p.name)),
                       key=lambda p: int(pattern.fullmatch(p.name).group(1) or 0))
        attempts = []
        for path in paths:
            events, event_identity = rows(path / 'events.jsonl')
            decisions, decision_identity = rows(path / 'decisions.jsonl')
            runtime, runtime_identity = rows(path / 'runtime_updates.jsonl')
            attempt = {'tag': path.name, 'ordinal': int(pattern.fullmatch(path.name).group(1) or 0),
                       'events': events, 'decisions': decisions, 'played_genome': decider_genome(path / 'why.log'),
                       'runtime': runtime, 'source_files': [event_identity, decision_identity, runtime_identity]}
            attempts.append(attempt)
            result['attempts'].append({k: v for k, v in attempt.items() if k not in ('events', 'decisions')})
        if not attempts:
            result['attempts'].append({'tag': stem, 'missing_directory': True})
        for target in selection['primary_turns']:
            chosen, alternatives = select_primary(attempts, target)
            case = {'stem': stem, 'turn': target, 'selected': None, 'variants': [],
                    'alternatives': [{'attempt': a[2], 'decision_index': a[1], 'receipt_sha256': a[3].get('sha256')}
                                     for a in alternatives]}
            result['cases'].append(case)
            if chosen is None:
                case['error'] = 'missing_exact_frame_zero_decision'
                continue
            ordinal, index, tag, decision = chosen
            attempt = next(a for a in attempts if a['tag'] == tag)
            case['selected'] = {'attempt': tag, 'ordinal': ordinal, 'decision_index': index, 'decision': decision}
            turn_rows = [e for e in attempt['events'] if e.get('kind') == 'turn' and e.get('ctx') == 'agent' and e.get('turn') == target]
            case['turn_record'] = turn_rows[0] if turn_rows else None
            case['other_turn_record_count'] = max(0, len(turn_rows) - 1)
            standing = major_standing(turn_rows[0]) if turn_rows else {'known': False, 'reason': 'missing_turn_record'}
            case['standing'] = standing
            try:
                capture = decision['input_capture']
                if not capture.get('complete') or not capture.get('reads') or any('error' in read for read in capture['reads']):
                    raise ValueError('incomplete_or_failed_capture')
                seen = set()
                for descriptor in capture['reads']:
                    sid = descriptor['snapshot_id']
                    if sid in seen:
                        continue
                    seen.add(sid)
                    data = captured_input(capture, sid)
                    events = [json.loads(line) for line in data.splitlines() if line]
                    states = [e for e in events if e.get('kind') == 'state' and e.get('ctx') == 'agent']
                    if not states or (states[-1].get('turn'), states[-1].get('frame')) != (target, 0):
                        raise ValueError('captured_read_does_not_end_at_selected_frame')
                    state = states[-1]
                    pid = standing.get('local_player')
                    if pid is None:
                        seats = [e.get('local_player') for e in events if e.get('kind') == 'seat' and type(e.get('local_player')) is int]
                        pid = seats[-1] if seats else None
                    plots, width = explicit_plots(events, target)
                    variant = {'descriptor': descriptor, 'bytes': len(data), 'sha256': digest(data), 'state': state,
                               'decomposition': public_decomposition(state, standing),
                               'cities': [city_metrics(c) for c in state.get('cities', [])],
                               'allocation_bounds': [allocation_bounds(c, pid, target, plots, width) for c in state.get('cities', [])],
                               'explicit_current_plot_count': sum(v['observed_turn'] == target for v in plots.values())}
                    case['variants'].append(variant)
                case['complete'] = bool(case['variants'])
                state_hashes = {digest(json.dumps(v['state'], sort_keys=True).encode()) for v in case['variants']}
                case['state_variants_equal'] = len(state_hashes) == 1
            except Exception as error:
                case['complete'] = False
                case['error'] = type(error).__name__ + ': ' + str(error)
        probe = {'stem': stem, 'readings': [], 'complete': True}
        streak = {}
        for target in selection['retirement_probe_turns']:
            candidates = [(a['ordinal'], i, a['tag'], e) for a in attempts for i, e in enumerate(a['events'])
                          if e.get('kind') == 'turn' and e.get('ctx') == 'agent' and e.get('turn') == target]
            candidates.sort(key=lambda x: x[:3])
            reading = civ6_play.production_share_reading(candidates[0][3], 1 / 3) if candidates else None
            row = {'turn': target, 'attempt': candidates[0][2] if candidates else None, 'reading': reading}
            if reading is None or reading.get('unreadable'):
                probe['complete'] = False
            else:
                row['streak'] = civ6_play.production_share_run(streak, reading)
            probe['readings'].append(row)
        probe['three_readable_consecutive_below_by_52'] = (streak.get('production_share_run', 0) >= 3) if probe['complete'] else None
        result['retirement_probes'].append(probe)
        print(stem, 'selected', sum(c.get('complete', False) for c in result['cases'] if c['stem'] == stem), flush=True)
    result['summary'] = {'registered_samples': len(selection['cohort']) * len(selection['primary_turns']),
                         'cases': len(result['cases']), 'complete': sum(c.get('complete', False) for c in result['cases']),
                         'errors': [{'stem': c['stem'], 'turn': c['turn'], 'error': c.get('error')} for c in result['cases'] if not c.get('complete')]}
    encoded = gzip.compress((json.dumps(result, sort_keys=True, allow_nan=False) + '\n').encode(), mtime=0)
    (args.out / 'census.json.gz').write_bytes(encoded)
    if json.loads(gzip.decompress(encoded)) != result:
        raise ValueError('output readback differs')
    (args.out / 'summary.json').write_text(json.dumps(result['summary'], indent=2) + '\n')
    print(result['summary'], flush=True)


if __name__ == '__main__':
    main()
