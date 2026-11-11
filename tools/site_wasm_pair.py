#!/usr/bin/env python3
"""Compare qualified browser engines on full Online games, in both orders.

The manual wasm-paired-cost workflow runs this on an isolated Mac runner.
Each arm uses a fresh Node process and the same normal roster/settings/seed.
Every normalized frame and reasoning payload must match, and the final save
must match as raw JSON bytes. Performance includes engine/state/reasoning work,
not download, module startup, or browser painting. A runner is not every laptop:
record its runtime and report paired ratios, never a universal seconds claim.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import re
import statistics
import subprocess
import sys
import time
from pathlib import Path

import site_wasm_symbols

SEEDS = [('default', 7), ('six', 7311001), ('default', 8),
         ('six', 7311002), ('default', 9), ('six', 7311003)]
DRIVER = Path(__file__).with_name('site_wasm_game.mjs')


def qualified(root: Path, expected_source: str) -> tuple[Path, dict]:
    if not re.fullmatch(r'[0-9a-f]{40}', expected_source):
        raise ValueError('expected source must be an exact commit SHA')
    receipt = json.loads((root / 'qualification-info.json').read_text())
    module = root / 'qualification/civvis.wasm'
    _, manifest = site_wasm_symbols.load(root / 'wasm-symbols', module)
    if receipt['source_commit'] != expected_source or manifest['source_commit'] != expected_source:
        raise ValueError('qualified source differs from the requested source')
    if receipt['engine'] != manifest:
        raise ValueError('qualification receipt differs from the bound manifest')
    if not 0 < module.stat().st_size <= 25 * 1024 * 1024:
        raise ValueError('engine exceeds the production size budget')
    if module.read_bytes()[:8] != b'\0asm\1\0\0\0':
        raise ValueError('engine is not a WebAssembly 1 module')
    return module, receipt


def plan(seeds: list[tuple[str, int]]) -> list[tuple[str, int, int, tuple[str, str]]]:
    result = []
    for repeat in range(2):
        counts = {'default': 0, 'six': 0}
        for roster, seed in seeds:
            candidate_first = (counts[roster] % 2 == 0) != (roster == 'six')
            counts[roster] += 1
            order = ('candidate', 'baseline') if candidate_first else ('baseline', 'candidate')
            result.append((roster, seed, repeat, order if repeat == 0 else order[::-1]))
    return result


def validate_run(row: dict, arm: str, roster: str, seed: int, module_sha: str) -> None:
    if (row.get('harnessVersion') != 7 or row.get('finished') is not True
            or row.get('seed') != seed or row.get('roster') != roster
            or row.get('moduleSha256') != module_sha):
        raise ValueError(f'incomplete or misidentified {arm} game')
    for key in ['traceHash', 'saveHash']:
        if not re.fullmatch(r'[0-9a-f]{64}', row.get(key, '')):
            raise ValueError('missing exact-state hash')
    for key in ['cpuMs', 'wallMs']:
        if not isinstance(row.get(key), (int, float)) or not math.isfinite(row[key]) or row[key] <= 0:
            raise ValueError('invalid full-game timing')
    if not 1 <= row.get('turn', 0) <= 251 or row.get('frames', 0) < 1:
        raise ValueError('invalid terminal game clock')


def same_game(baseline: dict, candidate: dict) -> None:
    for key in ['seed', 'roster', 'settings', 'frames', 'turn', 'finished',
                'winner', 'victoryType', 'traceHash', 'saveHash']:
        if baseline.get(key) != candidate.get(key):
            raise ValueError(f'game parity mismatch: {key}')


def paired_summary(pairs: list[dict], seeds: list[tuple[str, int]]) -> dict:
    combined = []
    for roster, seed in seeds:
        both = [p for p in pairs if p['roster'] == roster and p['seed'] == seed]
        if len(both) != 2:
            continue
        if ({p['repeat'] for p in both} != {0, 1}
                or {p['first_arm'] for p in both} != {'baseline', 'candidate'}):
            raise ValueError('both-order summary requires opposite execution orders')
        item = {'roster': roster, 'seed': seed, 'both_orders': True,
                'same_every_frame': True, 'same_raw_save': True}
        for metric in ['cpu', 'wall']:
            item[metric + '_pct'] = 100 * (math.sqrt(
                both[0][metric + '_ratio'] * both[1][metric + '_ratio']) - 1)
        combined.append(item)
    return {
        'complete': len(pairs) == 2 * len(seeds) and len(combined) == len(seeds),
        'pairs': pairs,
        'both_order_geometric_pairs': combined,
        'by_roster': {roster: {
            'median_cpu_pct': statistics.median(p['cpu_pct'] for p in combined if p['roster'] == roster),
            'median_wall_pct': statistics.median(p['wall_pct'] for p in combined if p['roster'] == roster),
        } for roster in {p['roster'] for p in combined}},
        'scope': 'Full engine/state/reasoning games; excludes download, module startup and browser painting. Negative percent means faster on this runner only.',
    }


def write_json(path: Path, data: dict) -> None:
    path.write_text(json.dumps(data, indent=2) + '\n')


def compare(args: argparse.Namespace) -> None:
    modules, receipts = {}, {}
    for arm in ['baseline', 'candidate']:
        modules[arm], receipts[arm] = qualified(getattr(args, arm), getattr(args, arm + '_source'))
    for key in ['rustc', 'binaryen']:
        if receipts['baseline'][key] != receipts['candidate'][key]:
            raise ValueError('qualified toolchains differ: ' + key)
    args.out.mkdir()  # Never overwrite another comparison's evidence.
    write_json(args.out / 'context.json', {
        'receipts': receipts,
        'node': subprocess.check_output(['node', '--version'], text=True).strip(),
        'v8': subprocess.check_output(['node', '-p', 'process.versions.v8'], text=True).strip(),
        'platform': platform.platform(), 'machine': platform.machine(), 'logical_cpus': os.cpu_count(),
        'driver_sha256': hashlib.sha256(DRIVER.read_bytes()).hexdigest(),
        'self_test': args.self_test,
    })
    seeds = SEEDS[:1] if args.self_test else SEEDS
    pairs, first = [], {}
    for roster, seed, repeat, order in plan(seeds):
        runs = {}
        for arm in order:
            stem = args.out / f'{roster}-{seed}-{repeat}-{arm}'
            print(f'Playing full {roster} seed {seed} repeat {repeat} {arm}', flush=True)
            started, load = time.time(), os.getloadavg()
            with stem.with_suffix('.log').open('w') as log:
                subprocess.run(['node', str(DRIVER), str(modules[arm]), str(seed), roster,
                                str(stem.with_suffix('.save.json'))], stdout=log,
                               stderr=subprocess.STDOUT, check=True, timeout=900)
            row = json.loads(stem.with_suffix('.log').read_text().splitlines()[-1])
            validate_run(row, arm, roster, seed, receipts[arm]['engine']['wasm_sha256'])
            row.update(arm=arm, repeat=repeat, started_at_unix=started,
                       load_start=load, load_end=os.getloadavg())
            previous = first.get((roster, seed, arm))
            if previous is not None:
                same_game(previous, row)
            else:
                first[(roster, seed, arm)] = row
            runs[arm] = row
            with (args.out / 'runs.jsonl').open('a') as log:
                log.write(json.dumps(row) + '\n')
            print(json.dumps(row), flush=True)
        same_game(runs['baseline'], runs['candidate'])
        result = {'roster': roster, 'seed': seed, 'repeat': repeat, 'first_arm': order[0],
                  'same_every_frame': True, 'same_raw_save': True,
                  'terminal_turn': runs['baseline']['turn']}
        for metric in ['cpu', 'wall']:
            result[metric + '_ratio'] = runs['candidate'][metric + 'Ms'] / runs['baseline'][metric + 'Ms']
        pairs.append(result)
        write_json(args.out / 'summary.json', paired_summary(pairs, seeds))
        print('PAIR ' + json.dumps(result), flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    for arm in ['baseline', 'candidate']:
        parser.add_argument('--' + arm, type=Path, required=True)
        parser.add_argument('--' + arm + '-source', required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--self-test', action='store_true',
                        help='One default seed in both orders, always full games; workflow self-test only')
    args = parser.parse_args()
    compare(args)


if __name__ == '__main__':
    main()
