#!/usr/bin/env python3
"""Compare four complete qualified website games in both orders on one host.

The existing browser observer verifies each natural game and every paint.
This wrapper adds balanced fresh-browser ordering, recipe checks, original
save/publication-identity parity and uncorrected paired timings. Pure tests
start no browser game. Identical qualified modules are valid A/A controls.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import math
import os
import platform
import subprocess
from pathlib import Path

import site_wasm_browser_ci as browser


def recipe_matches(baseline: dict, candidate: dict) -> None:
    for key in ('publisher_commit', 'rustc', 'binaryen'):
        if baseline.get(key) is None or baseline[key] != candidate.get(key):
            raise ValueError('qualification recipes differ: ' + key)


def plan(case: str) -> list[tuple[int, str]]:
    return [(repeat, arm) for _, _, repeat, arms in browser.pair.plan([browser.profile.CASES[case]])
            for arm in arms]


def summarize(records: list[dict], case: str) -> dict:
    if [(row['repeat'], row['arm']) for row in records] != plan(case):
        raise ValueError('four complete games in both original orders are required')
    for field in ('save_sha256', 'publication_identities', 'chrome', 'chrome_executable_sha256', 'runtime'):
        if not records[0].get(field) or any(row[field] != records[0][field] for row in records):
            raise ValueError('browser games differ in ' + field)
    ratios = []
    for repeat in (0, 1):
        arms = {row['arm']: row for row in records if row['repeat'] == repeat}
        row = {'repeat': repeat, 'first_arm': next(x['arm'] for x in records if x['repeat'] == repeat)}
        for metric in ('first_paint_to_final_paint_ms', 'navigation_to_final_paint_ms'):
            values = [arms[arm][metric] for arm in ('baseline', 'candidate')]
            if any(isinstance(value, bool) or not isinstance(value, (int, float))
                   or not math.isfinite(value) or value <= 0 for value in values):
                raise ValueError('invalid complete-game timing')
            row[metric + '_ratio'] = values[1] / values[0]
            if not math.isfinite(row[metric + '_ratio']) or row[metric + '_ratio'] <= 0:
                raise ValueError('invalid paired timing ratio')
        ratios.append(row)
    if any(not math.isfinite(ratios[0][metric + '_ratio'] * ratios[1][metric + '_ratio'])
           for metric in ('first_paint_to_final_paint_ms', 'navigation_to_final_paint_ms')):
        raise ValueError('invalid combined timing ratio')
    combined = {metric + '_pct': (math.sqrt(ratios[0][metric + '_ratio'] * ratios[1][metric + '_ratio']) - 1) * 100
                for metric in ('first_paint_to_final_paint_ms', 'navigation_to_final_paint_ms')}
    return {'complete': True, 'case': case, 'games': 4,
            'same_original_save_bytes': True, 'same_every_publication_identity': True,
            'both_orders': ratios, 'both_order_geometric_change': combined,
            'scope': 'One case, one remote host, fresh browser per game, both orders. '
                     'Negative percent is faster on this runner. No samples discarded or noise corrections. '
                     'Publication identities and final original saves match; full frame contents are not hashed. '
                     'Not a laptop or universal speed milestone.'}


def host_snapshot() -> dict:
    result = {'at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'platform': platform.platform(), 'logical_cpus': os.cpu_count(),
              'load_average': list(os.getloadavg()) if hasattr(os, 'getloadavg') else None}
    if platform.system() == 'Darwin':
        info = subprocess.run(['sysctl', '-n', 'machdep.cpu.brand_string'], capture_output=True, text=True)
        result['cpu_model'] = info.stdout.strip() if info.returncode == 0 else None
    return result


def compare(baseline: Path, baseline_source: str, baseline_run: int,
            candidate: Path, candidate_source: str, candidate_run: int,
            repo: str, case: str, out: Path, chrome: Path | None = None) -> dict:
    if case not in browser.profile.CASES or min(baseline_run, candidate_run) <= 0:
        raise ValueError('invalid normal case or qualification run')
    if out.exists():
        raise FileExistsError('inspect original paired browser evidence; output already exists')
    modules = {}
    for arm, directory, source, run in [('baseline', baseline, baseline_source, baseline_run),
                                       ('candidate', candidate, candidate_source, candidate_run)]:
        module, receipt = browser.verify_module(directory, source, run, repo)
        modules[arm] = {'directory': str(directory), 'source': source, 'run': run,
                        'module_sha256': hashlib.sha256(module.read_bytes()).hexdigest(),
                        'qualification': receipt}
    recipe_matches(modules['baseline']['qualification'], modules['candidate']['qualification'])
    out.mkdir(parents=True)
    records = []
    result = {'phase': 'running', 'case': case, 'modules': modules,
              'tooling_source': os.environ.get('GITHUB_SHA'), 'host': host_snapshot(),
              'same_engine_bytes': modules['baseline']['module_sha256'] == modules['candidate']['module_sha256'],
              'identical_qualification_control': baseline_run == candidate_run and baseline_source == candidate_source,
              'records': records}
    def record():
        (out / 'receipt.json').write_text(json.dumps(result, indent=2) + '\n')
    record()
    try:
        for repeat, arm in plan(case):
            entry = modules[arm]
            game_dir = out / f'{repeat}-{arm}'
            result['active_game'] = str(game_dir)
            record()
            before = host_snapshot()
            receipt = browser.browser_game(Path(entry['directory']), entry['source'], entry['run'], repo, case, game_dir,
                                           chrome=chrome)
            if (receipt.get('phase') != 'complete' or receipt.get('source') != entry['source']
                    or receipt.get('case') != case or receipt.get('qualification_run') != entry['run']):
                raise ValueError('subgame is incomplete or names a different input')
            raw = (game_dir / 'game.save.json').read_bytes()
            if hashlib.sha256(raw).hexdigest() != receipt['raw_save']['sha256']:
                raise ValueError('subgame original save bytes changed')
            observed = json.loads((game_dir / 'observation.json').read_text())['observation']
            browser.validate_runtime(receipt.get('runtime') or {})
            if not isinstance(receipt.get('chrome'), str) or not receipt['chrome']:
                raise ValueError('missing browser runtime version')
            identities = [[frame.get(key) for key in ('seed', 'turn', 'sequence', 'finished', 'winner', 'victoryType')]
                          for frame in observed['frames']]
            records.append({'repeat': repeat, 'arm': arm, 'path': str(game_dir),
                            'save_sha256': receipt['raw_save']['sha256'],
                            'publication_identities': identities, 'host_before': before,
                            'chrome': receipt['chrome'], 'chrome_executable_sha256': receipt['chrome_executable_sha256'],
                            'runtime': receipt['runtime'],
                            'host_after': host_snapshot(), **receipt['timing']})
            record()
        # Compare bytes as well as hashes; neither path reparses/reserializes them.
        reference = (Path(records[0]['path']) / 'game.save.json').read_bytes()
        if any((Path(row['path']) / 'game.save.json').read_bytes() != reference for row in records[1:]):
            raise ValueError('paired original save bytes differ')
        summary = summarize(records, case)
        (out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        result.update(phase='complete', summary=summary)
        record()
        return result
    except BaseException as error:
        result.update(phase='failed; partial paired browser evidence retained', error=repr(error))
        record()
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arm in ('baseline', 'candidate'):
        parser.add_argument('--' + arm, type=Path, required=True)
        parser.add_argument('--' + arm + '-source', required=True)
        parser.add_argument('--' + arm + '-run', type=int, required=True)
    parser.add_argument('--repo', required=True)
    parser.add_argument('--case', choices=sorted(browser.profile.CASES), required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--chrome', type=Path, required=True,
                        help='Fixed Chrome for Testing executable; do not use an updating desktop browser')
    args = parser.parse_args()
    result = compare(args.baseline.resolve(), args.baseline_source, args.baseline_run,
                     args.candidate.resolve(), args.candidate_source, args.candidate_run,
                     args.repo, args.case, args.out.resolve(), chrome=args.chrome.resolve())
    print(json.dumps(result['summary']))


if __name__ == '__main__':
    main()
