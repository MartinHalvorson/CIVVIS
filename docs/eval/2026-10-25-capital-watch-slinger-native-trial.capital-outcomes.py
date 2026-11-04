"""Apply the existing original-capital census to every retained Slinger trial attempt."""
import argparse
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import re


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--state', type=Path, required=True)
    parser.add_argument('--control', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    state_bytes = args.state.read_bytes()
    state = json.loads(state_bytes)
    if state.get('gene') != 'capital-watch-arms-the-slinger' or state.get('limit') != 16:
        raise ValueError('wrong registered trial')
    source = Path(__file__).with_name('2026-10-25-culture-air-native-screen.capital-outcomes.py')
    spec = importlib.util.spec_from_file_location('existing_capital_census', source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    attempts, pending = [], []
    for ordinal, stem in enumerate(state['assigned'], 1):
        directories = [p for p in args.control.glob(stem + '*')
                       if p.is_dir() and re.fullmatch(re.escape(stem) + r'(?:-cont[1-9][0-9]*)?', p.name)]
        directories.sort(key=lambda p: 0 if p.name == stem else int(p.name.rsplit('-cont', 1)[1]))
        if not directories:
            pending.append({'ordinal': ordinal, 'stem': stem, 'reason': 'missing assigned attempt'})
        for directory in directories:
            summary_file = directory / 'summary.json'
            if not summary_file.exists() or json.loads(summary_file.read_text()).get('game_stopped') is not True:
                pending.append({'ordinal': ordinal, 'stem': stem, 'attempt': directory.name, 'reason': 'not terminal'})
                continue
            result = module.census(directory)
            result.update(assigned_ordinal=ordinal, assigned_stem=stem)
            summary_bytes = summary_file.read_bytes()
            summary = json.loads(summary_bytes)
            result['summary_sha256'] = hashlib.sha256(summary_bytes).hexdigest()
            result['terminal_record'] = {key: summary.get(key) for key in ('reason', 'last_turn', 'outcome')}
            attempts.append(result)
    report = {'scope': __doc__, 'classification_limits': 'Observed major/minor founder rosters only; unknown founders are retained. Current city rosters replace holdings. City-loss markers do not establish permanent loss. Occupation events can repeat.',
              'state_sha256': hashlib.sha256(state_bytes).hexdigest(),
              'source_census_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
              'assigned_stems': state['assigned'], 'cutoff': state.get('cutoff'),
              'pending': pending, 'attempts': attempts,
              'complete_registered_cohort': len(state['assigned']) == 16 and state.get('cutoff') is not None and not pending}
    if args.out.exists():
        raise ValueError('preserve previous output')
    args.out.write_bytes(gzip.compress((json.dumps(report, indent=2) + '\n').encode(), mtime=0))
    print(json.dumps({'assigned': len(state['assigned']), 'audited_attempts': len(attempts),
                      'complete': report['complete_registered_cohort'], 'pending': pending,
                      'attempts_with_observed_foreign_major_original_capital': sum(a['maximum_observed_held_foreign_major_original_capitals'] > 0 for a in attempts),
                      'observed_home_capital_loss_markers': sum(len(a['observed_original_capital_losses']) for a in attempts),
                      'attempts_with_unknown_original_capital_founders': sum(bool(a['unknown_original_capital_founders']) for a in attempts)}))


if __name__ == '__main__':
    main()
