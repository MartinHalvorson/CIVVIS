#!/usr/bin/env python3
"""Report gene-screen outcomes by preassigned victory target, including adaptive.

Usage: python3 tools/gene_report_targets.py rows.jsonl --gene victory-portfolio

The target is assigned before play. Portfolio phases and chosen objectives are
post-treatment diagnostics, never conditioning variables for the win contrast.
This report does not select genes or change the deployment ledger.
"""

from __future__ import annotations

import argparse
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path
import statistics


def _mean(values):
    return statistics.fmean(values) if values else None


def _median(values):
    return statistics.median(values) if values else None


def _contrast(rows):
    on = [r for r in rows if r["treatment"] is True]
    off = [r for r in rows if r["treatment"] is False]
    result = {"on_seats": len(on), "off_seats": len(off),
              "win_difference_pp": None, "cluster_se_pp": None,
              "interval_95_pp": None}
    if not on or not off:
        return result
    on_mean = _mean([float(r["win"]) for r in on])
    off_mean = _mean([float(r["win"]) for r in off])
    delta = on_mean - off_mean
    influence = defaultdict(float)
    for group, mean, sign in ((on, on_mean, 1.0), (off, off_mean, -1.0)):
        for row in group:
            influence[row["cluster"]] += sign * (float(row["win"]) - mean) / len(group)
    result["win_difference_pp"] = delta * 100
    result["games"] = len(influence)
    if len(influence) > 1:
        variance = sum(value * value for value in influence.values())
        se = math.sqrt(variance * len(influence) / (len(influence) - 1)) * 100
        # With constant outcomes the plug-in variance is zero even in a
        # tiny sample. Absence of observed variation is not exact certainty.
        if se > 0:
            result["cluster_se_pp"] = se
            result["interval_95_pp"] = [delta * 100 - 1.96 * se, delta * 100 + 1.96 * se]
    return result


def _diagnostics(rows):
    measured = [r for r in rows if isinstance(r.get("victory_portfolio"), dict)]
    commitment, switches, completed_errors, latest_errors = [], [], [], []
    late_cities, trace_points, dropped = [], 0, 0
    bottlenecks = defaultdict(int)
    late_allocation_shares = defaultdict(list)
    last_observed = defaultdict(list)
    for row in measured:
        portfolio = row["victory_portfolio"]
        if portfolio.get("first_commitment_turn") is not None:
            commitment.append(portfolio["first_commitment_turn"])
        if portfolio.get("primary_switches") is not None:
            switches.append(portfolio["primary_switches"])
        trace = portfolio.get("trace", [])
        trace_points += len(trace)
        dropped += portfolio.get("dropped_trace_points", 0)
        if trace:
            for key in ("science", "culture", "military", "gold", "faith",
                        "science_projects", "visitors", "diplomatic_points", "capitals"):
                if trace[-1].get(key) is not None:
                    last_observed[key].append(trace[-1][key])
            # One equally weighted final developed snapshot per seat; frequent
            # posture switches must not give a seat more statistical weight.
            developed = [point for point in trace if point.get("phase") in ("buildup", "finish")
                         and isinstance(point.get("production_allocation"), dict)]
            if developed:
                allocation = developed[-1]["production_allocation"]
                total = sum(allocation.get(key, 0) for key in ("primary", "secondary", "other", "idle"))
                if total > 0:
                    for key in ("primary", "secondary", "other", "idle", "settlers_and_builders"):
                        late_allocation_shares[key].append(allocation.get(key, 0) / total)
            if trace[-1].get("cities") is not None:
                late_cities.append(trace[-1]["cities"])
            if trace[-1].get("bottleneck"):
                bottlenecks[trace[-1]["bottleneck"]] += 1
        # Losing games censor a projected own finish. A missing finish must
        # never be filled with the opponent's victory turn.
        if row["win"]:
            predictions = [point for point in trace
                           if point.get("primary") == row.get("victory")
                           and point.get("expected_finish") is not None
                           and point["turn"] < row["turn"]]
            if predictions:
                completed_errors.append(row["turn"] - predictions[0]["expected_finish"])
                latest_errors.append(row["turn"] - predictions[-1]["expected_finish"])
    return {
        "measured_seats": len(measured),
        "missing_seats": len(rows) - len(measured),
        "commitment_turn_median": _median(commitment),
        "primary_switches_mean": _mean(switches),
        "last_observed_cities_mean": _mean(late_cities),
        "trace_points": trace_points,
        "dropped_trace_points": dropped,
        "last_bottlenecks": dict(sorted(bottlenecks.items())),
        "last_observed_means": {key: _mean(values) for key, values in sorted(last_observed.items())},
        "last_developed_allocation_shares": {key: _mean(values) for key, values in sorted(late_allocation_shares.items())},
        "allocation_scope": "observed queue production rates at each seat's last developed snapshot; not realized expenditure",
        "realized_same_lane_forecasts": len(completed_errors),
        "realized_finish_error_turns_mean": _mean(completed_errors),
        "realized_latest_finish_error_turns_mean": _mean(latest_errors),
        "realized_latest_finish_error_turns_mae": _mean([abs(error) for error in latest_errors]),
        "forecast_error_scope": "same-lane winners only; other outcomes are censored, not zero error",
    }


def analyze(paths, gene=None):
    rows, sources, seen = [], [], set()
    for raw_path in paths:
        path = Path(raw_path)
        digest = hashlib.sha256()
        genes, headers = [], []
        source_seats = 0
        with path.open("rb") as source:
            for line_number, raw in enumerate(source, 1):
                digest.update(raw)
                if not raw.strip():
                    continue
                try:
                    row = json.loads(raw)
                except (UnicodeDecodeError, json.JSONDecodeError) as exc:
                    raise ValueError(f"{path}:{line_number}: invalid JSON") from exc
                if row.get("kind") == "header":
                    genes = row.get("genes", [])
                    # Retain the full design, intended sample and build stamp;
                    # only omit bulky per-gene vectors from this summary.
                    headers.append({key: value for key, value in row.items()
                                    if key not in {"genes", "screened", "prior", "families"}})
                    continue
                if row.get("kind") != "game":
                    continue
                if not isinstance(row.get("win"), bool):
                    raise ValueError(f"{path}:{line_number}: missing Boolean outcome")
                if gene and gene not in genes:
                    raise ValueError(f"{path}:{line_number}: {gene!r} is not in the active header")
                identity = (row["seed"], row.get("arm", 0), row["seat"])
                if identity in seen:
                    raise ValueError(f"duplicate seat observation: {identity}")
                seen.add(identity)
                target = row.get("player_target") or "unknown"
                if target == "religion": target = "religious"
                if target == "diplomacy": target = "diplomatic"
                row["target_group"] = target
                row["cluster"] = (row["seed"], row.get("arm", 0))
                row["treatment"] = None
                if gene:
                    index = genes.index(gene)
                    genome = row.get("genome", "")
                    if index >= len(genome) or genome[index] not in "01":
                        raise ValueError(f"{path}:{line_number}: missing gene assignment")
                    row["treatment"] = genome[index] == "1"
                rows.append(row)
                source_seats += 1
        sources.append({"path": str(path), "sha256": digest.hexdigest(),
                        "measured_seats": source_seats, "headers": headers})
    if not rows:
        raise ValueError("no measured game seats")
    groups = defaultdict(list)
    for row in rows:
        groups[row["target_group"]].append(row)
    targets = {}
    for target, group in sorted(groups.items()):
        endings = defaultdict(int)
        for row in group:
            if row["win"]: endings[row.get("victory", "unknown")] += 1
        entry = {"seats": len(group), "games": len({r["cluster"] for r in group}),
                 "wins": sum(r["win"] for r in group),
                 "win_rate": _mean([float(r["win"]) for r in group]),
                 "winning_conditions": dict(sorted(endings.items())),
                 "diagnostics": _diagnostics(group)}
        if gene:
            entry["gene_contrast"] = _contrast(group)
            entry["diagnostics_by_treatment"] = {
                name: _diagnostics([r for r in group if r["treatment"] is value])
                for name, value in (("on", True), ("off", False))
            }
        targets[target] = entry
    return {"schema": "victory-target-report/v1", "gene": gene,
            "sources": sources, "seats": len(rows), "targets": targets,
            "interpretation": "Targets are assigned before play. Choices and phases are descriptive post-treatment telemetry. SEs cluster shared winners by game; sparse cells do not establish a benefit."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rows", nargs="+", type=Path)
    parser.add_argument("--gene")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    try:
        result = analyze(args.rows, args.gene)
    except (OSError, ValueError, KeyError) as exc:
        parser.error(str(exc))
    text = json.dumps(result, indent=2, allow_nan=False) + "\n"
    if args.out: args.out.write_text(text)
    else: print(text, end="")


if __name__ == "__main__":
    main()
