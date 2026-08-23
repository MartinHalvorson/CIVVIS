#!/usr/bin/env python3
"""Is the value net still calibrated on the states the learner reaches?

The published mechanism for `policy_wide`'s collapse is that greedily
maximising a value net one ply at a time leaves the distribution it was
trained on, where its estimate carries no information. That is a testable
claim and much cheaper to test than a retraining loop: score the same net
against outcomes on states visited by the agent that generated its training
data, and on states visited by the agent that maximises it.

Both corpora use the same seeds, so the maps are shared and only the
policy that walked them differs.
"""
import csv
import json
import math
import sys


def load_net(path):
    net = json.load(open(path))
    return net["sizes"], net["weights"], net["biases"]


def evaluate(net, x):
    sizes, weights, biases = net
    a = list(x)
    last = len(weights) - 1
    for layer in range(last + 1):
        w, b = weights[layer], biases[layer]
        nxt = list(b)
        for i, ai in enumerate(a):
            row = w[i]
            for j in range(len(nxt)):
                nxt[j] += ai * row[j]
        if layer < last:
            a = [v if v > 0 else 0.0 for v in nxt]
        else:
            a = [1.0 / (1.0 + math.exp(-v)) for v in nxt]
    return a[0]


def score(net, path, width):
    rows = [r for r in csv.reader(open(path)) if len(r) == width + 2]
    if not rows:
        raise SystemExit(f"{path}: no rows of width {width}")
    bce = brier = 0.0
    bins = [[0, 0.0, 0] for _ in range(10)]
    wins = 0
    for r in rows:
        x = [float(v) for v in r[:width]]
        y = 1.0 if float(r[width]) > 0.5 else 0.0
        p = min(max(evaluate(net, x), 1e-7), 1 - 1e-7)
        bce -= y * math.log(p) + (1 - y) * math.log(1 - p)
        brier += (p - y) ** 2
        wins += y
        b = bins[min(9, int(p * 10))]
        b[0] += 1
        b[1] += p
        b[2] += y
    n = len(rows)
    ece = sum(c * abs(s / c - h / c) for c, s, h in bins if c) / n
    base = wins / n
    base = min(max(base, 1e-7), 1 - 1e-7)
    const = -(base * math.log(base) + (1 - base) * math.log(1 - base))
    return n, bce / n, brier / n, ece, wins / len(rows), const


def main():
    net = load_net(sys.argv[1])
    width = net[0][0]
    print(f"net input width {width}\n")
    print(f"{'corpus':<28} {'rows':>6} {'BCE':>8} {'Brier':>8} {'ECE':>8} {'win%':>7} {'const BCE':>10}")
    for label, path in [("expert-visited states", sys.argv[2]),
                        ("learner-visited states", sys.argv[3])]:
        n, bce, brier, ece, w, const = score(net, path, width)
        print(f"{label:<28} {n:>6} {bce:>8.4f} {brier:>8.4f} {ece:>8.4f} {100*w:>6.1f}% {const:>10.4f}")


main()
