#!/usr/bin/env python3
"""Aggregate two feature corpora for DAgger-style retraining.

The trainer splits by game index so correlated snapshots from one game
cannot cross train/validation/test. Both corpora number their games from
zero, so a naive concatenation would fuse unrelated games under a shared
index and leak a game across splits — quietly inflating every reported
number. The second corpus's indices are therefore offset past the first's.
"""
import csv
import sys


def read(path):
    rows = []
    with open(path, newline="") as source:
        for raw in csv.reader(source):
            if len(raw) >= 3:
                rows.append(raw)
    return rows


def main():
    first, second, out = sys.argv[1], sys.argv[2], sys.argv[3]
    a, b = read(first), read(second)
    widths = {len(r) for r in a} | {len(r) for r in b}
    if len(widths) != 1:
        raise SystemExit(f"row widths differ across corpora: {sorted(widths)}")
    offset = max(int(float(r[-1])) for r in a) + 1
    for row in b:
        row[-1] = str(int(float(row[-1])) + offset)
    with open(out, "w", newline="") as sink:
        csv.writer(sink).writerows(a + b)
    games = len({r[-1] for r in a + b})
    print(f"{len(a)} + {len(b)} = {len(a) + len(b)} rows over {games} distinct games")
    print(f"second corpus game indices offset by {offset}")


main()
