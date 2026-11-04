"""Summarize every registered passive read; revalidate comparisons from raw dumps."""
import argparse
import collections
import gzip
import importlib.util
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--queries', type=Path, required=True)
    parser.add_argument('--census', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    here = Path(__file__).parent
    spec = importlib.util.spec_from_file_location('audit', here / '2026-10-26-native-opening-model-drift.audit.py')
    audit = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(audit)
    census = json.loads(gzip.decompress(args.census.read_bytes()))
    selection = json.loads((here / '2026-10-26-native-opening-model-drift.selection.json').read_text())
    results = [json.loads((args.queries / f'{i:04d}' / 'result.json').read_text())
               for i in range(len(selection['reads']))]
    comparisons = []
    for i, result in enumerate(results):
        assert result['complete'] and result['exit'] == 0, result
        assert result['index'] == i and result['selection'] == selection['reads'][i]
        case = census['cases'][result['selection']['case_index']]
        variant = next(v for v in case['variants'] if v['descriptor']['snapshot_id'] == result['selection']['snapshot']['snapshot_id'])
        dump = json.loads((args.queries / f'{i:04d}' / 'stdout').read_text())
        repeated = audit.compare(variant['state'], dump, selection['tolerance'])
        assert repeated == result['comparison'], i
        comparisons.extend(dict(read=i, stem=result['selection']['stem'], turn=result['selection']['turn'], **row)
                           for row in repeated)
    matched = [r for r in comparisons if r['known']]
    def metrics(values):
        known = [v for v in values if v['known']]
        return dict(known=len(known), unknown=len(values)-len(known),
                    material=sum(v['material'] for v in known),
                    mean_absolute_delta=sum(abs(v['delta']) for v in known)/len(known) if known else None,
                    maximum_absolute_delta=max((abs(v['delta']) for v in known), default=None))
    exceptions = [r for r in matched if r['housing'].get('display_floor_delta') or r['amenities'].get('material')]
    variants = []
    for ci, case in enumerate(census['cases']):
        indices = [i for i,s in enumerate(selection['reads']) if s['case_index'] == ci]
        if len(indices) > 1:
            # Retain differences in full comparison objects, not only their scores.
            variants.append(dict(case_index=ci, reads=indices,
                                 comparisons_identical=all(results[i]['comparison'] == results[indices[0]]['comparison'] for i in indices)))
    summary = dict(registered_cases=len(census['cases']), registered_reads=len(results),
        complete_reads=len(results), tolerance=selection['tolerance'],
        matched_city_read_records=len(matched), unique_city_times=len({(r['stem'],r['turn'],tuple(r['position'])) for r in matched}),
        structural_findings=[r for r in comparisons if not r['known']],
        terrain_worker_mismatches=[r for r in matched if any(r['terrain_worker_difference'].values())],
        raw_model_yields={key:metrics([r['raw_model_yield_delta'][key] for r in matched]) for key in audit.YIELDS},
        corrected_display_yields={key:metrics([r['corrected_display_delta'][key] for r in matched]) for key in audit.YIELDS},
        housing=metrics([r['housing'] for r in matched]),
        housing_raw_delta_counts=dict(collections.Counter(str(r['housing']['delta']) for r in matched if r['housing']['known'])),
        housing_display_floor_differences=sum(bool(r['housing'].get('display_floor_delta')) for r in matched),
        amenities=metrics([r['amenities'] for r in matched]), exceptions=exceptions, same_frame_variants=variants,
        scope='Current snapshot model_yields before per-city correction; per-plot corrections already incorporated. No future growth or strategic strength claim.')
    args.out.write_text(json.dumps(summary, indent=2, sort_keys=True, allow_nan=False)+'\n')
    print(json.dumps({k:v for k,v in summary.items() if k not in ['exceptions','corrected_display_yields']}, indent=2))


if __name__ == '__main__':
    main()
