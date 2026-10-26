"""Passive, exact-prefix model audit. Corrected displays are calibration, not fidelity."""
import math

YIELDS = ('food', 'production', 'gold', 'science', 'culture', 'faith')


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def delta(host, model, tolerance=.05):
    if not finite(host) or not finite(model):
        return {'known': False, 'host': host if finite(host) else None,
                'model': model if finite(model) else None}
    difference = model - host
    return {'known': True, 'host': host, 'model': model, 'delta': difference,
            'material': abs(difference) > tolerance}


def compare(state, dump, tolerance=.05):
    def indexed(rows):
        out = {}
        for row in rows:
            key = (row.get('x'), row.get('y'))
            out.setdefault(key, []).append(row)
        return out
    hosts, models = indexed(state.get('cities', [])), indexed(dump.get('cities', []))
    result = []
    for pos in sorted(set(hosts) | set(models)):
        h, m = hosts.get(pos, []), models.get(pos, [])
        row = {'position': list(pos), 'host_count': len(h), 'model_count': len(m)}
        result.append(row)
        if len(h) != 1 or len(m) != 1:
            row['known'] = False
            row['reason'] = 'missing_or_duplicate_city_position'
            continue
        host, model = h[0], m[0]
        row.update(known=True, name=host.get('name'), population=host.get('pop'))
        native = host.get('yields') or {}
        raw = model.get('model_yields') or {}
        corrected = model.get('yields') or {}
        row['raw_model_yield_delta'] = {key: delta(native.get(key), raw.get(key), tolerance) for key in YIELDS}
        row['corrected_display_delta'] = {key: delta(native.get(key), corrected.get(key), tolerance) for key in YIELDS}
        housing = host.get('housing')
        if not finite(housing) or housing < 0:
            row['housing'] = {'known': False, 'host': housing, 'model': model.get('model_housing')}
        else:
            record = delta(housing, model.get('model_housing'), tolerance)
            record['display_floor_delta'] = (math.floor(model['model_housing'] + 1e-9) - housing
                if finite(model.get('model_housing')) and math.isclose(housing, round(housing), abs_tol=1e-9) else None)
            row['housing'] = record
        amenity = host.get('amenities')
        row['amenities'] = ({'known': False, 'host': amenity, 'model': model.get('model_amenities'),
                             'reason': 'ambiguous_native_minus_one_accessor'} if amenity == -1
                            else delta(amenity, model.get('model_amenities'), tolerance))
        sources = model.get('model_housing_sources') or {}
        row['housing_sources'] = {key: delta(host.get('housing_from_' + key), sources.get(key), tolerance)
                                  for key in ('water', 'buildings', 'districts', 'improvements', 'civics',
                                              'great_people', 'great_works', 'starting_era')
                                  if finite(host.get('housing_from_' + key)) and host['housing_from_' + key] >= 0}
        row['native_context'] = {key: host.get(key) for key in ('food_surplus', 'overall_growth_mult',
            'happiness_growth_mult', 'housing_growth_mult', 'happiness_yield_mult', 'amenities_needed',
            'religion', 'producing', 'production', 'yield_sources')}
        districts = {(d.get('x'), d.get('y')) for d in host.get('districts', [])}
        native_workers = {(p.get('x'), p.get('y')) for p in host.get('worked', [])} - {pos} - districts
        model_workers = {(p.get('x'), p.get('y')) for p in model.get('worked', [])} - {pos}
        row['terrain_worker_difference'] = {'native_only': [list(p) for p in sorted(native_workers - model_workers)],
                                            'model_only': [list(p) for p in sorted(model_workers - native_workers)]}
    return result


def main():
    import argparse
    import gzip
    import hashlib
    import json
    import pathlib
    import subprocess
    import sys
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=pathlib.Path, required=True)
    parser.add_argument('--census', type=pathlib.Path, required=True)
    parser.add_argument('--out', type=pathlib.Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    selection_path = pathlib.Path(__file__).with_name('2026-10-26-native-opening-model-drift.selection.json')
    selection = json.loads(selection_path.read_text())
    raw = args.census.read_bytes()
    sha = lambda data: hashlib.sha256(data).hexdigest()
    if sha(raw) != selection['census_artifact']['member_sha256']:
        raise ValueError('census source identity differs')
    census = json.loads(gzip.decompress(raw))
    binary = pathlib.Path(selection['binary']['path'])
    if sha(binary.read_bytes()) != selection['binary']['sha256']:
        raise ValueError('actual frozen adapter bytes differ')
    sys.path.insert(0, str(args.repo / 'tools'))
    from civ6_decision_trace import captured_input
    registration = {'scope': __doc__, 'selection_sha256': sha(selection_path.read_bytes()),
                    'audit_sha256': sha(pathlib.Path(__file__).read_bytes()), 'binary': selection['binary']}
    (args.out / 'registration.json').write_text(json.dumps(registration, indent=2) + '\n')
    results = []
    for index, spec in enumerate(selection['reads']):
        directory = args.out / f'{index:04d}'
        directory.mkdir()
        case = census['cases'][spec['case_index']]
        variant = next(v for v in case['variants'] if v['descriptor']['snapshot_id'] == spec['snapshot']['snapshot_id'])
        result = {'index': index, 'selection': spec, 'complete': False}
        try:
            data = captured_input(case['selected']['decision']['input_capture'], spec['snapshot']['snapshot_id'])
            if len(data) != spec['input_bytes'] or sha(data) != spec['input_sha256']:
                raise ValueError('actual prefix identity differs')
            (directory / 'events.jsonl').write_bytes(data)
            # Passive diagnostic only. This directory contains no orders database or worker.
            command = [str(binary), '--mirror', str(directory), '--dump-mirror', '--turn', str(spec['turn']),
                       '--victory', 'civvis', '--players', '4', '--max-turns', '650']
            done = subprocess.run(command, capture_output=True, timeout=300)
            (directory / 'stdout').write_bytes(done.stdout)
            (directory / 'stderr').write_bytes(done.stderr)
            result.update(argv=command, exit=done.returncode)
            if done.returncode != 0:
                raise ValueError('passive diagnostic returned nonzero')
            dump = json.loads(done.stdout)
            if dump.get('turn') != spec['turn']:
                raise ValueError('diagnostic turn differs')
            result['comparison'] = compare(variant['state'], dump, selection['tolerance'])
            result['complete'] = True
            # The full prefix is already committed in the original cohort. Keep its exact identity.
            (directory / 'events.jsonl').unlink()
        except Exception as error:
            result['error'] = type(error).__name__ + ': ' + str(error)
        (directory / 'result.json').write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')
        results.append(result)
        print(index, spec['stem'], spec['turn'], result['complete'], result.get('error'), flush=True)
    summary = {'registered_reads': len(selection['reads']), 'complete_reads': sum(r['complete'] for r in results),
               'errors': [{'index': r['index'], 'error': r.get('error')} for r in results if not r['complete']]}
    (args.out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(summary, flush=True)


if __name__ == '__main__':
    main()
