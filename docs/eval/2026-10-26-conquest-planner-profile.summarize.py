"""Summarize the full baseline and retain inclusive named-function sample counts."""
import collections
import json
from pathlib import Path
import re
import sys


def named_counts(path):
    totals = collections.Counter()
    stack = []
    active = False
    for line in path.read_text().splitlines():
        if line == 'Call graph:':
            active = True
            continue
        if line.startswith('Total number'):
            break
        if not active:
            continue
        match = re.match(r'([ +!:|]*)(\d+) (.*)', line)
        if not match:
            continue
        depth, count, symbol = len(match[1]), int(match[2]), match[3]
        while stack and stack[-1][0] >= depth:
            stack.pop()
        name_match = re.search(r'\d+([a-z][a-z_]+)  \(in probe', symbol)
        name = name_match[1] if name_match else None
        inside = name == 'plan_observed_turn' or any(n == 'plan_observed_turn' for _, n in stack)
        if inside and name and not any(n == name for _, n in stack):
            totals[name] += count
        stack.append((depth, name))
    return dict(totals.most_common())


def main():
    root, out = map(Path, sys.argv[1:])
    baseline = json.loads((root/'run-01/results.json').read_text())
    followup = json.loads((root/'followup-01/results.json').read_text())
    ordinary = [r for r in baseline if r['phase'] == 'baseline']
    assert len(ordinary) == 48
    assert sum(r.get('unavailable', False) for r in ordinary) == 6
    available = [r for r in ordinary if not r.get('unavailable')]
    assert len(available) == 42 and all(r['complete'] for r in available)
    profiles = [r for r in baseline if r['phase'] == 'profile'] + followup
    assert len(profiles) == 6 and all(r['complete'] and r['sampler_exit'] == 0 and len(r['samples']) == 20 for r in profiles)
    samples = []
    for phase, indices in [('run-01', [12,18,36,42]), ('followup-01', [43,20])]:
        for index in indices:
            directory = f'profile-{index:04d}' if phase == 'run-01' else f'{index:04d}'
            counts = named_counts(root/phase/directory/'sample.txt')
            # Independent raw-root readback anchors all six indentation parses.
            expected = {12:648,18:693,36:607,42:698,43:827,20:805}[index]
            assert counts['plan_observed_turn'] == expected
            samples.append(dict(phase=phase,index=index,counts=counts))
    summary = dict(assigned_cases=48,complete_baseline_cases=42,unavailable_cases=6,
                   total_single_pass_cpu_ticks=sum(r['samples'][0]['process_cpu_ticks'] for r in available),
                   all_baseline=ordinary,profiles=profiles,named_samples=samples,
                   limits='One-pass timings and one-second selected stack samples are exploratory. Counts are inclusive named nongeneric function nodes, skip recursive ancestors and omit unresolved/generic symbols. Columns overlap and must not be summed. No speed, strength, Native or whole-game improvement claim.')
    out.write_text(json.dumps(summary,indent=2)+'\n')
    print('42 baseline cases and 6 twenty-repeat sample processes verified')


if __name__ == '__main__':
    main()
