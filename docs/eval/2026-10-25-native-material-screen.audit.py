"""Audit every issued upgrade in a terminal assigned attempt; keep ambiguity.

Uses every captured read, not a guessed event prefix. These are descriptive
native transitions; neither notes nor successful receipts alone prove upgrades.
"""
from __future__ import annotations
import argparse
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import sys

VETERAN = ('xp', 'level', 'formation', 'promotions')


def transition(before, after, subject):
    old = next((u for u in before.get('units', []) if u.get('id') == subject), None)
    if old is None or not old.get('upgrade_to'):
        return 'missing_native_quote', None, []
    if old.get('upgrade_blocked_reason'):
        return 'native_quote_blocked', old, []
    target = old['upgrade_to']
    if not all(k in old for k in (*VETERAN, 'x', 'y')):
        return 'missing_veteran_or_position', old, []
    old_ids = {u.get('id') for u in before.get('units', [])}
    same = next((u for u in after.get('units', []) if u.get('id') == subject), None)
    candidates = [u for u in after.get('units', [])
                  if u.get('kind') == target
                  and (u.get('id') == subject or u.get('id') not in old_ids)
                  and all(k in u and u[k] == old[k] for k in (*VETERAN, 'x', 'y'))]
    if same is not None:
        return ('same_id_exact_successor' if same in candidates else
                'same_id_transition_unresolved'), old, candidates
    return ('unique_exact_successor' if len(candidates) == 1 else
            'ambiguous_successor' if len(candidates) > 1 else
            'no_exact_successor'), old, candidates


def rows(path):
    return [json.loads(line) for line in path.read_bytes().splitlines()]


def last_state(data):
    result = None
    for line in data.splitlines():
        event = json.loads(line)
        if event.get('kind') == 'state':
            result = event
    if result is None:
        raise ValueError('captured read contains no native state')
    return result


def key(state):
    return state.get('turn'), state.get('frame')


def audit(directory, tools):
    sys.path.insert(0, str(tools))
    from civ6_decision_trace import captured_input
    summary = json.loads((directory / 'summary.json').read_text())
    if not summary.get('game_stopped'):
        raise ValueError('attempt is not terminal; defer the complete audit')
    events, decisions = rows(directory / 'events.jsonl'), rows(directory / 'decisions.jsonl')
    states = [(i, e) for i, e in enumerate(events) if e.get('kind') == 'state']
    counts = Counter(key(e) for _, e in states)
    result = {'run': directory.name, 'scope': __doc__, 'summary': summary,
              'files': {}, 'upgrades': []}
    for name in ('summary.json', 'events.jsonl', 'decisions.jsonl', 'why.log',
                 'brain.log', 'stdout.log', 'runtime_updates.jsonl'):
        p = directory / name
        if p.exists():
            result['files'][name] = {'bytes': p.stat().st_size,
                                    'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
    for decision in decisions:
        upgrades = [o for o in decision.get('orders', [])
                    if o.get('kind') == 'unit' and o.get('verb') == 'UPGRADE']
        if not upgrades:
            continue
        wanted = key(decision.get('decision', {}))
        capture = decision.get('input_capture', {})
        evidence, observed = [], []
        error = None
        try:
            if not capture.get('complete') or not capture.get('reads'):
                raise ValueError('missing or incomplete input capture')
            for read in capture['reads']:
                if 'error' in read:
                    raise ValueError('failed captured read')
                data = captured_input(capture, read['snapshot_id'])
                state = last_state(data)
                evidence.append({'snapshot_id': read['snapshot_id'],
                                 'count': read['count'], 'bytes': len(data),
                                 'sha256': hashlib.sha256(data).hexdigest()})
                observed.append(state)
            if any(key(s) != wanted or s != observed[0] for s in observed):
                raise ValueError('captured reads contain different decision states')
            if counts[wanted] != 1:
                raise ValueError('native decision frame is not unique')
            index, before = next((i, s) for i, s in states if key(s) == wanted)
            if before != observed[0]:
                raise ValueError('native event state differs from actual captured state')
            following = [(i, s) for i, s in states if i > index]
            if not following:
                raise ValueError('no following native state')
            next_index, after = following[0]
            if counts[key(after)] != 1:
                raise ValueError('following native state is not unique')
        except (KeyError, ValueError, OSError, StopIteration) as exc:
            error = str(exc)
        for order in upgrades:
            row = {'turn': wanted[0], 'frame': wanted[1], 'order': order,
                   'captured_reads': evidence, 'capture': capture}
            if error is not None:
                row.update(classification='observation_unresolved', error=error)
            else:
                classification, old, candidates = transition(before, after, order['subject'])
                row.update(classification=classification, before_state=before,
                           after_state=after, before_unit=old, candidates=candidates,
                           receipt_scope='all same-turn subject receipts; frame association is ambiguous',
                           receipts=[e for e in events
                                     if e.get('kind') in ('order_verified', 'order_failed')
                                     and e.get('verb') == 'UPGRADE'
                                     and e.get('turn') == wanted[0]
                                     and e.get('subject') == order['subject']])
            result['upgrades'].append(row)
    result['transition_counts'] = dict(Counter(r['classification'] for r in result['upgrades']))
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--run', required=True, type=Path)
    p.add_argument('--tools', required=True, type=Path)
    p.add_argument('--out', required=True, type=Path)
    a = p.parse_args()
    if a.out.exists():
        raise ValueError('retain existing audit; use a new output path')
    result = audit(a.run, a.tools)
    result['auditor_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    encoded = json.dumps(result, sort_keys=True).encode()
    a.out.write_bytes(gzip.compress(encoded, mtime=0))
    assert json.loads(gzip.decompress(a.out.read_bytes())) == result
    print(json.dumps({'run': result['run'], 'issued_upgrades': len(result['upgrades']),
                      'counts': result['transition_counts'],
                      'archive_sha256': hashlib.sha256(a.out.read_bytes()).hexdigest()}))


if __name__ == '__main__':
    main()
