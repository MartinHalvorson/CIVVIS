#!/usr/bin/env python3
"""Read-only capital outcome census for every assigned start, without replacements."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path


def records(path):
    with path.open() as source:
        for line in source:
            yield json.loads(line)


def census(directory):
    summary = json.loads((directory / 'summary.json').read_text())
    if summary.get('game_stopped') is not True:
        raise ValueError('assigned game is not terminal')
    pid = summary['seat']['local_player']
    events = directory / 'events.jsonl'
    majors, minors = {pid}, set()
    for event in records(events):
        if event.get('kind') == 'state':
            majors.update(r['player'] for r in event.get('rivals', []))
            minors.update(r['player'] for r in event.get('minors', []))
    if majors & minors:
        raise ValueError('contradictory major/minor rosters')
    held, lost, capital_lost, occupations = [], [], [], []
    last_city, unknown, maximum = {}, set(), 0
    for event in records(events):
        kind = event.get('kind')
        if kind == 'city_occupation' and event.get('ours_now') is True:
            occupations.append(event)
        if kind == 'city_lost':
            lost.append(event)
            before = last_city.get(event.get('city'), {})
            if before.get('original_capital') is True and before.get('original_owner') == pid:
                capital_lost.append(event)
        if kind != 'state':
            continue
        # Native `cities` is the current local roster; never accumulate holdings.
        local = event.get('cities', [])
        last_city.update({city['id']: city for city in local})
        current = [city for city in local if city.get('original_capital') is True
                   and city.get('original_owner') in majors - {pid}]
        maximum = max(maximum, len(current))
        if current:
            held.append(dict(turn=event['turn'], frame=event.get('frame'), cities=current))
        unknown.update(city.get('original_owner') for city in local
                       if city.get('original_capital') is True
                       and city.get('original_owner') not in majors | minors)
    return dict(stem=directory.name,
                maximum_observed_held_foreign_major_original_capitals=maximum,
                held_observations=held, unknown_original_capital_founders=list(unknown),
                major_ids=sorted(majors), minor_ids=sorted(minors),
                local_city_losses=lost, observed_original_capital_losses=capital_lost,
                native_occupation_events_ours_now=occupations,
                source_events_sha256=hashlib.sha256(events.read_bytes()).hexdigest())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--state', required=True, type=Path)
    parser.add_argument('--control', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    state = json.loads(args.state.read_text())
    games = []
    for stem in state['assigned']:
        attempts = sorted(args.control.glob(stem + '*'))
        if not attempts:
            raise ValueError('missing assigned attempt: ' + stem)
        games.extend(census(directory) for directory in attempts)
    report = dict(scope='All native local-city rosters, every frame. Major original founders classified from observed major/minor rosters. Occupation events may repeat and are not a deduplicated capture count. Native city-lost events joined to last observed same city ID.', games=games)
    args.out.write_bytes(gzip.compress((json.dumps(report, indent=2) + '\n').encode(), mtime=0))
    print(json.dumps(dict(attempts=len(games),
        held_foreign_capital_games=sum(g['maximum_observed_held_foreign_major_original_capitals'] > 0 for g in games),
        original_capital_loss_events=sum(len(g['observed_original_capital_losses']) for g in games))))


if __name__ == '__main__':
    main()
