#!/usr/bin/env python3
"""Build the gene ledger — what the screen says about every gene, and the
deployment genome that follows — from `gene_screen --analyze --json` outputs.

Operator directive 2026-08-22: **one screen**. Six majors on 74x46 continents
with nine city-states, Online speed to its own 250-turn clock, all six victory
lanes, every seat carrying its own drawn genome against the best-genome
baseline. There is no second regime to reconcile: a batch played at another
shape is a probe, and this tool refuses it as a source rather than pooling two
worlds into one column.

The defaults follow the ranking's two win columns, and a gene whose pooled
on-off difference is negative is vetoed whatever they say. A gene may default on
when **both** its last and prior readings are positive, or when their average
clears +15 with neither below -10. A gene with exactly one reading may
provisionally default on when that reading is above +20; every other gene
defaults off. The verdicts below still record what the screen proved; they no
longer decide what ships. This tool is the one place that decision is made, and
it is made from data:

    python3 tools/gene_ledger.py --write \\
        --source docs/gene_screens/<screen>.json \\
        --source docs/gene_screens/<newer-screen>.json

writes `docs/gene_ledger.json` and the generated Rust table
`src/ai/advanced/gene_ledger_table.rs`, which `AdvancedAi::apply_gene_ledger`
reads to withhold every treatment the ledger does not default on and to
enable every opt-in it does. `--check` re-derives both from the sources the
JSON ledger recorded and fails if either file has drifted; the same check is
`tools/test_gene_ledger.py`'s `GeneratedFiles`, which the `collaboration-policy`
workflow's `unittest discover` runs on every PR.

Default rule (repeated in src/ai/advanced/gene_ledger.rs, and the columns it
reads are the ones `HEURISTIC_GENE_RANKING.md` prints):

- The win column is wins added per 10,000 games at the gene's measured on-rate
  in one screen — `(win_on - 1/players) * 10,000`, against the 1-in-`players` a
  seat wins by chance. `wins_last_10k` is the latest screen that priced the
  gene, `wins_prior_10k` the screen before that.
- **on** when both columns are positive, or when their average is above +15
  and neither column is below -10.
- **on** with exactly one populated column when that reading is above +20.
- **off** otherwise, including an unmeasured gene.
- **off** whatever the columns say when `win_diff_pp` is negative (operator
  directive 2026-08-22). That is the ranking's *Diff*: the pooled on rate minus
  the pooled off rate in percentage points, over **every** screen that priced
  the gene, each weighted by its games. The win columns read the latest two
  screens only, so this veto is the one clause that lets an older screen speak:
  a gene whose two newest readings are positive but whose whole record is not
  ships off. Both arms of a screen carry the same games, so the 1-in-`players`
  chance base cancels inside each screen and the pooled figure is a
  games-weighted average of per-screen differences, comparable across shapes
  and player counts in a way a raw win rate is not.

⚠ The columns recorded before 2026-08-22 were read on 60x38 Pangaea, under a
four-player `domination,score` regime for some genes. The Pangaea readings are
kept as HISTORY — they are what the deployment genome stands on until the
standard screen re-prices each gene — and are marked `"shape": "legacy"` in
`sources`. The war-regime readings are gone: they never entered a default, and
their four-player 1-in-4 chance base made their columns incomparable with the
six-player ones printed beside them.

Verdict rules (repeated in src/ai/advanced/gene_ledger.rs). These record what
the screens proved and drive the ledger's counts and the screen's own reading;
since 2026-08-22 they no longer decide the default:

- helps      win z >= 2 and share z > -2, or share z >= 2 and win z > -2 —
             the screen's own `*` flag. Past the run's family-wise bar is
             recorded as `family_wise`, not required: with sixty-odd genes
             the family-wise bar would leave three on.
- hurts      the mirror image.
- unresolved otherwise — including a gene whose two axes disagree past
             |z| >= 2 (recorded as `conflict`) and a gene no screen measured.

The verdict is read off the newest screen that priced the gene. Later
`--source` arguments override earlier ones per gene, so a repaired gene's
re-screen replaces its pre-repair number while the rest of the pre-repair
screen stands.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
LEDGER_JSON = ROOT / "docs" / "gene_ledger.json"
LEDGER_RS = ROOT / "src" / "ai" / "advanced" / "gene_ledger_table.rs"
#: ⭐ THE SCREEN, leg by leg — the profile a `gene_screen` header must carry to
#: enter this ledger. `src/bin/gene_screen.rs` plays exactly this on its bare
#: defaults (`SCREEN_PLAYERS` and friends); the two are held together by
#: `tools/test_gene_ledger.py`, so neither can drift alone.
SCREEN = {
    "players": 6,
    "map": "continents",
    "width": 74,
    "height": 46,
    "city_states": 9,
    "speed": "online",
    "turns": 250,
    "victories": "science,culture,religious,diplomatic,domination,score",
    "all_seats": True,
    "randomize_civs": True,
    "design": "foldover",
    "baseline": "best",
    "field": "advanced",
}
#: The profile keys recorded for every source, whether or not they match.
PROFILE_KEYS = tuple(SCREEN) + ("start_seed",)
Z_BAR = 2.0
# The win column's scale, then the deployment rule's bars: the threshold for
# one provisional column, the average two columns must clear, and the floor
# below which neither of two columns may sit.
PER = 10_000
SINGLE_COLUMN_BAR = 20
AVERAGE_BAR = 15.0
COLUMN_FLOOR = -10
# The pooled on-off difference, in percentage points, below which no column
# reading can put a gene in the genome. Zero: a gene that has not won more than
# it lost over its whole record does not ship.
DIFF_FLOOR = 0.0
# The recorded precision of that difference. The decision is taken on the
# rounded figure the ledger publishes, never on a wider one, so the generated
# Rust table re-derives the same answer from the same number.
DIFF_PLACES = 6


def axis_verdict(win_z: float, share_z: float) -> str:
    """One screen's verdict from its two z scores."""
    helps = (win_z >= Z_BAR and share_z > -Z_BAR) or (share_z >= Z_BAR and win_z > -Z_BAR)
    hurts = (win_z <= -Z_BAR and share_z < Z_BAR) or (share_z <= -Z_BAR and win_z < Z_BAR)
    if helps and not hurts:
        return "helps"
    if hurts and not helps:
        return "hurts"
    return "unresolved"


def axes_conflict(win_z: float, share_z: float) -> bool:
    return (win_z >= Z_BAR and share_z <= -Z_BAR) or (share_z >= Z_BAR and win_z <= -Z_BAR)


def wins_per_10k(win_rate: float, players: int) -> int:
    """One screen's win column: wins added per 10,000 games at this measured
    on-rate. A seat wins 1-in-`players` by chance (1-in-6 when a fixture does
    not say), so the column is how far above or below that the gene's on arm
    landed. `tools/heuristic_gene_ranking.py` imports this, so the table's
    printed column and the ledger's decision are one arithmetic."""
    chance = 1.0 / players if players else 1.0 / 6.0
    return round((win_rate - chance) * PER)


def pooled_win_rates(history: list[dict]) -> tuple[float, float]:
    """The games-weighted on and off win rates across every screen that priced
    the gene — `HEURISTIC_GENE_RANKING.md`'s two *Total* columns. Each entry
    carries `win_on`/`win_off` and the games behind each arm.
    `tools/heuristic_gene_ranking.py` imports this, so the printed totals and
    the ledger's veto are one arithmetic."""
    on_games = sum(m["n_on"] for m in history)
    off_games = sum(m["n_off"] for m in history)
    on = sum(m["win_on"] * m["n_on"] for m in history) / on_games
    off = sum(m["win_off"] * m["n_off"] for m in history) / off_games
    return on, off


def pooled_win_diff_pp(history: list[dict]) -> float:
    """The ranking's *Diff*: the pooled on rate minus the pooled off rate, in
    percentage points, rounded to what the ledger records. This is the **whole**
    on-off difference, twice the scale of a win column beside it."""
    on, off = pooled_win_rates(history)
    return round(100 * (on - off), DIFF_PLACES)


def default_from_win_columns(last: int | None, prior: int | None) -> bool:
    """The win-column clause (operator directive 2026-08-22): a gene may default
    on when both native win columns are positive, or when their average clears
    +15 with neither column below -10. With exactly one populated column, its
    reading must be above +20; an unmeasured gene stays off.

    This is the clause alone. `default_from_columns` is the deployment call."""
    populated = [value for value in (last, prior) if value is not None]
    if len(populated) == 1:
        return populated[0] > SINGLE_COLUMN_BAR
    if len(populated) == 0:
        return False
    assert last is not None and prior is not None
    if last > 0 and prior > 0:
        return True
    return (last + prior) / 2 > AVERAGE_BAR and last >= COLUMN_FLOOR and prior >= COLUMN_FLOOR


def default_from_columns(last: int | None, prior: int | None,
                         diff_pp: float | None) -> bool:
    """The deployment call: the win-column clause, vetoed by a negative pooled
    on-off difference (operator directive 2026-08-22).

    The veto is one-way. A gene whose whole record is negative ships off however
    its latest two screens read; a positive record promotes nothing on its own,
    because the columns still have to clear their bars. A gene no screen has
    priced has no difference to read, and stays off on the columns."""
    if diff_pp is not None and diff_pp < DIFF_FLOOR:
        return False
    return default_from_win_columns(last, prior)


TREATMENTS_RS = ROOT / "src" / "ai" / "advanced" / "treatments.rs"
ROW = re.compile(r'\(\s*"([a-z0-9_]+)"\s*,\s*"([a-z0-9-]+)"\s*,\s*AdvancedAi::')


def profile_of(data: dict) -> dict:
    """The recorded profile of one analysis file, every key the screen names.

    Older analyses predate a flag and simply omit it; the header's own defaults
    are what that absence meant, so they are filled in here rather than read as
    a mismatch that never happened."""
    raw = data.get("profile", {})
    profile = {key: raw.get(key) for key in PROFILE_KEYS}
    if profile["victories"] in (None, ""):
        profile["victories"] = SCREEN["victories"]
    if profile["design"] is None:
        profile["design"] = "foldover"
    for key in ("all_seats", "randomize_civs"):
        if profile[key] is None:
            profile[key] = False
    return profile


def shape_of(profile: dict) -> str:
    """`standard` when every leg of the screen matches, else `legacy`.

    A legacy source is history, not a second regime: the Pangaea screens that
    the deployment genome currently stands on. New ones are refused at the
    write path (`--legacy-shape` to record one deliberately), so the ledger
    cannot quietly acquire a second shape."""
    return "standard" if all(profile.get(k) == v for k, v in SCREEN.items()) else "legacy"


def shape_gap(profile: dict) -> str:
    """The legs that differ from the screen, for the refusal message."""
    return ", ".join(
        f"{key}={profile.get(key)!r} (screen: {value!r})"
        for key, value in SCREEN.items()
        if profile.get(key) != value
    )


def known_tags() -> set[str]:
    """Every gene tag the repository registers — the `(field, tag, toggle)`
    rows of `LIVE_TREATMENTS`, `PRODUCTION_TREATMENTS` and
    `PRODUCTION_OPT_INS` in `src/ai/advanced/treatments.rs`. A screen played
    on an older build can carry a gene whose code has since been removed;
    its row must not enter the ledger (the Rust table refuses unknown tags)."""
    return {tag for _, tag in ROW.findall(TREATMENTS_RS.read_text())}


def load_source(path: Path) -> dict:
    data = json.loads(path.read_text())
    if data.get("kind") != "gene_screen_analysis":
        raise SystemExit(f"{path}: not a gene_screen --analyze --json output")
    return data


def measure_from(gene: dict, source_name: str) -> dict:
    measure = {
        "pairs": int(gene["pairs"]),
        "n_on": int(gene.get("n_on", gene["pairs"])),
        "n_off": int(gene.get("n_off", gene["pairs"])),
        "win_delta_pp": round(float(gene["win_delta_pp"]), 3),
        "win_z": round(float(gene["win_z"]), 3),
        "share_delta_pp": round(float(gene["share_delta_pp"]), 3),
        "share_z": round(float(gene["share_z"]), 3),
        "read": gene.get("read", ""),
        "source": source_name,
    }
    # Newer analyzer outputs retain three chronological, non-overlapping win
    # tranches. Keep them in the ledger JSON so a later drop decision can ask
    # whether its harm replicated, while older sources remain byte-for-byte
    # compatible and the generated Rust runtime table stays intentionally
    # focused on the pooled estimate.
    tranches = []
    for tranche in gene.get("win_tranches", []):
        recorded = {
            "position": str(tranche["position"]),
            "pairs": int(tranche["pairs"]),
            "win_delta_pp": round(float(tranche["win_delta_pp"]), 3),
            "win_z": round(float(tranche["win_z"]), 3),
        }
        # Retain the standard error when emitted by the newer analyzer.  It
        # makes the independent-window confidence check auditable from the
        # ledger, while accepting prefeature JSON fixtures and old sources.
        if "win_se_pp" in tranche:
            recorded["win_se_pp"] = round(float(tranche["win_se_pp"]), 3)
        tranches.append(recorded)
    if tranches:
        measure["win_tranches"] = tranches
    return measure


def build_ledger(sources: list[Path], filter_known: bool = True) -> dict:
    """Merge the sources into one ledger object (the JSON file's content).
    Sources are recorded oldest-first, and a later one overrides an earlier one
    per gene. `filter_known=False` keeps every tag (synthetic tests)."""
    measures: dict[str, dict] = {}
    # Every win column a gene has, oldest first: the last two are the ranking's
    # `± Wins Last 10k` and `± Wins 10k Prior`, and the deployment default is
    # read off them.
    columns: dict[str, list[int]] = {}
    # Every screen's two arms, for the pooled on-off difference that vetoes a
    # default. Unlike the columns this keeps the whole record, not the tail.
    arms: dict[str, list[dict]] = {}
    family: dict[str, float] = {}
    recorded = []
    known = known_tags() if filter_known else set()
    dropped: set[str] = set()
    for path in sources:
        data = load_source(path)
        name = path.name
        profile = profile_of(data)
        players = int(profile.get("players") or 0)
        family[name] = float(data.get("family_wise_z", 0.0))
        recorded.append({
            "path": str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
            "shape": shape_of(profile),
            "complete_pairs": int(data.get("complete_pairs", 0)),
            "family_wise_z": round(family[name], 3),
            "profile": profile,
        })
        for gene in data.get("genes", []):
            if known and gene["tag"] not in known:
                dropped.add(gene["tag"])
                continue
            measures[gene["tag"]] = measure_from(gene, name)
            if "win_on" not in gene:
                raise SystemExit(
                    f"{name}: {gene['tag']} has no `win_on`; a screen without win "
                    "rates cannot supply the win column the defaults are read from"
                )
            columns.setdefault(gene["tag"], []).append(
                wins_per_10k(float(gene["win_on"]), players)
            )
            arms.setdefault(gene["tag"], []).append({
                "win_on": float(gene["win_on"]),
                "win_off": float(gene["win_off"]),
                "n_on": int(gene.get("n_on", gene["pairs"])),
                "n_off": int(gene.get("n_off", gene["pairs"])),
            })
    if dropped:
        print("gene ledger: dropped rows for genes the repository no longer registers: "
              + ", ".join(sorted(dropped)), file=sys.stderr)

    genes = []
    for tag in sorted(measures):
        measure = measures[tag]
        verdict = axis_verdict(measure["win_z"], measure["share_z"])
        conflict = axes_conflict(measure["win_z"], measure["share_z"])
        bar = family[measure["source"]]
        family_wise = (
            verdict != "unresolved"
            and bar > 0
            and max(abs(measure["win_z"]), abs(measure["share_z"])) >= bar
        )
        history = columns.get(tag, [])
        last = history[-1] if history else None
        prior = history[-2] if len(history) > 1 else None
        record = arms.get(tag, [])
        diff_pp = pooled_win_diff_pp(record) if record else None
        genes.append({
            "tag": tag,
            "verdict": verdict,
            "default_on": default_from_columns(last, prior, diff_pp),
            "wins_last_10k": last,
            "wins_prior_10k": prior,
            "win_diff_pp": diff_pp,
            "family_wise": family_wise,
            "conflict": conflict,
            "screen": measure,
        })
    counts = {
        "helps": sum(g["verdict"] == "helps" for g in genes),
        "hurts": sum(g["verdict"] == "hurts" for g in genes),
        "unresolved": sum(g["verdict"] == "unresolved" for g in genes),
        "default_on": sum(g["default_on"] for g in genes),
    }
    return {
        "kind": "gene_ledger",
        "screen": dict(SCREEN),
        "rules": {
            "z_bar": Z_BAR,
            "helps": "win z >= 2 with share z > -2, or share z >= 2 with win z > -2",
            "hurts": "the mirror image",
            "shape": "one screen: a source whose profile is not `screen` above is "
                     "marked legacy and kept as history; new ones are refused",
            "win_column": "wins added per 10,000 games at the gene's measured on-rate in one "
                          "screen, (win_on - 1/players) * 10000; last and prior are the "
                          "two most recent screens that priced the gene",
            "win_diff": "the pooled on rate minus the pooled off rate in percentage points, "
                        "over every screen that priced the gene, each weighted by its games "
                        "- the ranking's `Diff`, the whole on-off difference",
            "default_on": f"both win columns positive, or their average above +{AVERAGE_BAR:.0f} "
                          f"with neither below {COLUMN_FLOOR}; with exactly one populated "
                          f"column, on when it is above +{SINGLE_COLUMN_BAR}; unmeasured is off; "
                          f"and off whatever the columns say when win_diff_pp is below "
                          f"{DIFF_FLOOR:.0f}",
        },
        "sources": recorded,
        "counts": counts,
        "genes": genes,
    }


def rust_f(value: float) -> str:
    text = repr(float(value))
    if "e" in text or "E" in text:
        text = f"{float(value):.6f}"
    if "." not in text:
        text += ".0"
    return text


def rust_opt_i32(value: int | None) -> str:
    return "None" if value is None else f"Some({value})"


def rust_opt_f(value: float | None) -> str:
    return "None" if value is None else f"Some({rust_f(value)})"


def rust_measure(m: dict | None) -> str:
    if m is None:
        return "None"
    return (
        "Some(Measure { pairs: %d, win_delta_pp: %s, win_z: %s, share_delta_pp: %s, "
        "share_z: %s, source: %s })"
        % (
            m["pairs"], rust_f(m["win_delta_pp"]), rust_f(m["win_z"]),
            rust_f(m["share_delta_pp"]), rust_f(m["share_z"]), json.dumps(m["source"]),
        )
    )


def render_rust(ledger: dict) -> str:
    lines = [
        "// GENERATED by tools/gene_ledger.py — do not edit by hand.",
        "// Source: docs/gene_ledger.json (the same tool writes both); `--check` holds them together.",
        "#![allow(",
        "    unused_imports,",
        "    clippy::excessive_precision,",
        "    clippy::unreadable_literal",
        ")]",
        "use super::{GeneVerdict, Measure, Verdict};",
        "",
        "#[rustfmt::skip]",
        "pub(super) const ROWS: &[GeneVerdict] = &[",
    ]
    for gene in ledger["genes"]:
        verdict = {"helps": "Verdict::Helps", "hurts": "Verdict::Hurts",
                   "unresolved": "Verdict::Unresolved"}[gene["verdict"]]
        lines.append("    GeneVerdict {")
        lines.append(f"        tag: {json.dumps(gene['tag'])},")
        lines.append(f"        verdict: {verdict},")
        lines.append(f"        default_on: {'true' if gene['default_on'] else 'false'},")
        lines.append(f"        wins_last_10k: {rust_opt_i32(gene['wins_last_10k'])},")
        lines.append(f"        wins_prior_10k: {rust_opt_i32(gene['wins_prior_10k'])},")
        lines.append(f"        win_diff_pp: {rust_opt_f(gene['win_diff_pp'])},")
        lines.append(f"        family_wise: {'true' if gene['family_wise'] else 'false'},")
        lines.append(f"        screen: {rust_measure(gene['screen'])},")
        lines.append("    },")
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def render_json(ledger: dict) -> str:
    return json.dumps(ledger, indent=2, sort_keys=False) + "\n"


def print_table(ledger: dict) -> None:
    print(f"gene ledger · {len(ledger['genes'])} genes · "
          f"helps {ledger['counts']['helps']} · hurts {ledger['counts']['hurts']} · "
          f"unresolved {ledger['counts']['unresolved']} · "
          f"default on {ledger['counts']['default_on']}")
    for src in ledger["sources"]:
        print(f"  source {src['shape']:<8} {src['path']}  ({src['complete_pairs']} pairs, "
              f"family-wise |z|≥{src['family_wise_z']})")
    print(f"{'gene':<30} {'verdict':<10} {'default':<7} {'last':>6} {'prior':>6} "
          f"{'diff':>7} {'win/share z':<20} source")
    # Best default first, then the deciding column, so the rule reads down the page.
    for gene in sorted(ledger["genes"],
                       key=lambda g: (not g["default_on"],
                                      -(g["wins_last_10k"] if g["wins_last_10k"] is not None else -10**6),
                                      g["tag"])):
        def z(m):
            return "-" if m is None else f"{m['win_z']:+.2f}/{m['share_z']:+.2f}"
        def col(v):
            return "–" if v is None else f"{v:+d}"
        def diff(v):
            return "–" if v is None else f"{v:+.2f}"
        flag = "**" if gene["family_wise"] else ("!" if gene["conflict"] else "")
        source = gene["screen"]["source"] if gene["screen"] else "-"
        print(f"{gene['tag']:<30} {gene['verdict']:<10} {'on' if gene['default_on'] else 'off':<7} "
              f"{col(gene['wins_last_10k']):>6} {col(gene['wins_prior_10k']):>6} "
              f"{diff(gene['win_diff_pp']):>7} "
              f"{z(gene['screen']):<20} {source} {flag}")


def sources_from_args(args) -> list[Path]:
    """The `--source` files, oldest first, each held to the screen's shape.

    ⚠ This is the whole enforcement of "one screen": a probe played at another
    profile answers a different question, and pooling its column with the
    screen's would report the difference between two worlds as a gene's
    effect. `--legacy-shape` records one anyway, which is how the Pangaea
    history already in the ledger stays there."""
    paths = [Path(p).resolve() for p in args.source]
    if args.legacy_shape:
        return paths
    for path in paths:
        profile = profile_of(load_source(path))
        if shape_of(profile) != "standard":
            raise SystemExit(
                f"{path.name} was not played at the screen's shape: {shape_gap(profile)}."
                "\nRun it at the screen (`gene_screen --pairs N --out rows.jsonl`, no"
                " profile flags), or pass --legacy-shape to record it as history."
            )
    return paths


def sources_from_ledger(ledger: dict) -> list[Path]:
    return [(ROOT / s["path"]).resolve() for s in ledger["sources"]]


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--source", action="append", default=[],
                    help="a `gene_screen --analyze --json` file (repeatable, oldest first; "
                         "later wins per gene)")
    ap.add_argument("--legacy-shape", action="store_true",
                    help="record a source that was not played at the screen's shape, as history")
    ap.add_argument("--write", action="store_true",
                    help="write docs/gene_ledger.json and src/ai/advanced/gene_ledger_table.rs")
    ap.add_argument("--check", action="store_true",
                    help="re-derive both files from the sources the ledger records and fail on drift")
    args = ap.parse_args(argv)

    if args.check:
        current = json.loads(LEDGER_JSON.read_text())
        ledger = build_ledger(sources_from_ledger(current))
        drift = []
        if render_json(ledger) != LEDGER_JSON.read_text():
            drift.append(str(LEDGER_JSON.relative_to(ROOT)))
        if render_rust(ledger) != LEDGER_RS.read_text():
            drift.append(str(LEDGER_RS.relative_to(ROOT)))
        if drift:
            print("gene ledger: out of date — " + ", ".join(drift)
                  + "; run `python3 tools/gene_ledger.py --write` with the recorded sources")
            return 1
        print("gene ledger: up to date")
        return 0

    if args.source:
        ledger = build_ledger(sources_from_args(args))
    elif LEDGER_JSON.exists():
        ledger = build_ledger(sources_from_ledger(json.loads(LEDGER_JSON.read_text())))
    else:
        raise SystemExit("no --source given and no docs/gene_ledger.json to read sources from")

    print_table(ledger)
    if args.write:
        LEDGER_JSON.write_text(render_json(ledger))
        LEDGER_RS.write_text(render_rust(ledger))
        print(f"wrote {LEDGER_JSON.relative_to(ROOT)} and {LEDGER_RS.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
