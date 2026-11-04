"""Summarize the fixed census without selecting a favorable captured-read variant."""
import argparse
import gzip
import hashlib
import json
import pathlib
import statistics


def median(values):
    return statistics.median(values) if values else None


def summarize(data):
    report = {'scope': __doc__, 'turns': [], 'turn_50': [], 'ambiguous_allocation_cases': [],
              'case_count': len(data['cases']), 'read_count': sum(len(c['variants']) for c in data['cases'])}
    for target in data['selection']['primary_turns']:
        cases = [c for c in data['cases'] if c['turn'] == target]
        ratios = [c['standing']['queue_ratio'] for c in cases if c['standing']['known']]
        decompositions = [c['variants'][0]['decomposition'] for c in cases
                          if c.get('complete') and c.get('state_variants_equal') and c['variants'][0]['decomposition']['known']]
        bounds = []
        city_count, skipped = 0, 0
        for case in cases:
            if not case.get('complete') or not case.get('state_variants_equal'):
                continue
            variants = case['variants']
            city_count += len(variants[0]['cities'])
            if any(v['allocation_bounds'] != variants[0]['allocation_bounds'] for v in variants):
                skipped += len(variants[0]['cities'])
                report['ambiguous_allocation_cases'].append({'stem': case['stem'], 'turn': target,
                    'variants': [{'descriptor': v['descriptor'], 'input_sha256': v['sha256'],
                                  'current_plot_count': v['explicit_current_plot_count'],
                                  'bounds': v['allocation_bounds']} for v in variants]})
            else:
                bounds.extend(variants[0]['allocation_bounds'])
            if target == 50:
                state = variants[0]['state']
                probe = next(p for p in data['retirement_probes'] if p['stem'] == case['stem'])
                report['turn_50'].append({'stem': case['stem'], 'queue_ratio': case['standing'].get('queue_ratio'),
                    'cities': len(state['cities']), 'population': sum(c['pop'] for c in state['cities']),
                    'public_decomposition': variants[0]['decomposition'],
                    'three_below_by_52': probe['three_readable_consecutive_below_by_52']})
        report['turns'].append({'turn': target, 'known_queue_standings': len(ratios),
            'queue_ratio_median': median(ratios), 'below_one_third': sum(v < 1 / 3 for v in ratios),
            'known_same_state_decompositions': len(decompositions),
            'city_ratio_median_known_leaders': median([d['city_ratio'] for d in decompositions]),
            'population_per_city_ratio_median_known_leaders': median([d['population_per_city_ratio'] for d in decompositions]),
            'city_yield_per_population_ratio_median_known_leaders': median([d['production_per_population_ratio'] for d in decompositions]),
            'native_city_samples': city_count, 'allocation_city_samples': len(bounds), 'ambiguous_allocation_city_samples': skipped,
            'cities_with_explicit_current_unworked_quote': sum(a['eligible_unworked'] > 0 for a in bounds),
            'pareto_bound_cities': sum(bool(a['pareto']) for a in bounds),
            'food_floor_bound_cities': sum(bool(a['food_floor_tradeoffs']) for a in bounds),
            'pareto_one_worker_delta_sum_across_distinct_games': sum(a.get('maximum_one_worker_pareto_production_delta', 0) for a in bounds),
            'food_floor_one_worker_delta_sum_across_distinct_games': sum(a.get('maximum_one_worker_food_floor_tradeoffs_production_delta', 0) for a in bounds)})
    probes = data['retirement_probes']
    report['retirement_probe'] = {'complete': sum(p['complete'] for p in probes),
        'three_below_by_52': sum(p['three_readable_consecutive_below_by_52'] is True for p in probes),
        'not_three_below_by_52': sum(p['three_readable_consecutive_below_by_52'] is False for p in probes),
        'unknown': sum(p['three_readable_consecutive_below_by_52'] is None for p in probes)}
    report['turn_50_city_median'] = median([r['cities'] for r in report['turn_50']])
    report['turn_50_population_median'] = median([r['population'] for r in report['turn_50']])
    return report


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--input', type=pathlib.Path, required=True)
    p.add_argument('--out', type=pathlib.Path, required=True)
    args = p.parse_args()
    if args.out.exists():
        raise ValueError('retain previous summary; use a fresh output')
    raw = args.input.read_bytes()
    data = json.loads(gzip.decompress(raw))
    report = summarize(data)
    report['source_sha256'] = hashlib.sha256(raw).hexdigest()
    args.out.write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')
    if json.loads(args.out.read_text()) != report:
        raise ValueError('summary readback differs')
    print(json.dumps({k: v for k, v in report.items() if k not in ('turn_50', 'ambiguous_allocation_cases')}, indent=2))


if __name__ == '__main__':
    main()
