#!/usr/bin/env python3
"""Regenerate `HEURISTIC_GENE_RANKING.md` — every screenable heuristic gene,
with a measurement, ranked by wins added per 10,000 six-player games, plus the
screenable genes still awaiting one.

    python3 tools/heuristic_gene_ranking.py --write     # rewrite the table
    python3 tools/heuristic_gene_ranking.py --check     # fail if it is stale

The table used to be written once, by hand, from one screen's rows. Now it is
derived: for each gene the **latest source** in `docs/gene_ledger.json` that
measured it supplies the on/off wins and games (so a gene added after the
whole-genome screen still appears, from its own screen), and the deployment
verdict comes from the ledger. Every source is the one screen the ledger
accepts — the war regime's four-player columns are gone, and the Pangaea
screens the current defaults stand on are marked `legacy` until the screen
re-prices each gene. Screenable genes with no result are listed separately
without a rank. Genes whose code has been removed this cycle are listed from
their last measurement, as before. Descriptions are the first sentence of each toggle's
doc comment in `src/ai/advanced/treatment_flags.rs`. Hand-written follow-ups
go in `docs/gene_ranking_notes.md` and are carried under the table.

`tools/test_heuristic_gene_ranking.py` holds the file to the sources, so the
ranking cannot quietly fall behind the ledger.
"""
from __future__ import annotations

import argparse
import json
import math
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
# The ledger tool owns the win column: it decides each gene's default from the
# same two numbers this table prints, so both must be one arithmetic.
from gene_ledger import (  # noqa: E402
    PER,
    pooled_win_diff_pp,
    pooled_win_rates,
    wins_per_10k as wins_per,
)
LEDGER_JSON = ROOT / "docs" / "gene_ledger.json"
RANKING_MD = ROOT / "HEURISTIC_GENE_RANKING.md"
NOTES_MD = ROOT / "docs" / "gene_ranking_notes.md"

#: A two-sided 5% test reaches 80% power at 1.96 + 0.84 standard errors.
POWER_80 = 2.8

#: How much of a gene's sentence the Description column carries. Widened
#: 160 → 480 on 2026-08-22 (operator: "three times as wide"): the longest
#: first sentence in the registry is 249 characters, so every description
#: now prints whole and the "…" that clipped 16 of them is gone.
DESCRIPTION_CHARS = 480


def column_se(win_se_pp: float) -> float:
    """One `wins_per` column's standard error, in the column's own units.

    A screen reports `win_se_pp`: the error on the on−off **difference**, in
    percentage points. The column is `(win_on - chance) * PER`, and a foldover
    holds the two arms symmetric about chance, so the column is **half** that
    difference and carries half its error. The two are not interchangeable,
    and quoting one against the other is not a rounding error: the ±110/10k
    band #2266 called eight removals "inside" is the difference's band, twice
    the width of the column it was read against. Derived here so the printed
    band and the printed column stay one arithmetic, the way `wins_per` and
    the ledger's default already are.
    """
    return win_se_pp * PER / 200.0


#: A seat wins 1-in-`players` by chance, so an unpaired estimate of the column
#: would carry this much error per pair. Every screen beats it, by however much
#: its foldover actually cancels; the ratio is the only honest account of why
#: two screens of the same size resolve differently.
def unpaired_constant(players: int) -> float:
    """`column_se × sqrt(pairs)` for independent Bernoulli arms — the no-pairing
    baseline a screen's own constant is measured against."""
    chance = 1.0 / players if players else 1.0 / 6.0
    return math.sqrt(2 * chance * (1 - chance)) * 100 * (PER / 200.0)


def resolutions(ledger: dict) -> list[dict]:
    """What each screen can actually resolve, from its own errors.

    The median gene's column standard error times `POWER_80`, plus the *pairing
    gain* — how far the screen's own error per pair sits below the unpaired
    baseline. A foldover only cancels to the extent its two arms play a similar
    game, so the gain is a reading about the genes, not about the design:
    a gene that rarely fires leaves most pairs identical and cancels almost
    everything, while a whole-genome screen flips every gene between arms and
    cancels almost nothing.

    ⚠ Gene count is NOT the driver, though it looks like one and this docstring
    used to say it was. The repository's own screens refute it: the one-gene
    `h1` at 7,200 pairs reads a WIDER band than the four-gene `s6` at 6,000.
    What separates them is the 3.3x gain on `s7`'s rarely-firing
    `idle-faith-patronage` against 1.28x on `h1`'s `holy-lane-parity`, which
    changes nearly every game. Read the screen's own row; do not reason from
    how many genes it carried. Newest first.
    """
    out = []
    for src in ledger["sources"]:
        data = json.loads((ROOT / src["path"]).read_text())
        errors = sorted(
            column_se(float(gene["win_se_pp"]))
            for gene in data.get("genes", [])
            if gene.get("win_se_pp") is not None
        )
        if not errors:
            continue
        median = errors[len(errors) // 2]
        pairs = int(data.get("complete_pairs", 0))
        players = int(data.get("profile", {}).get("players", 0) or 0)
        unpaired = unpaired_constant(players)
        out.append({
            "name": Path(src["path"]).name,
            "shape": src["shape"],
            "genes": len(errors),
            "pairs": pairs,
            "se": median,
            "band": POWER_80 * median,
            "gain": unpaired / (median * math.sqrt(pairs)) if pairs else 0.0,
        })
    return list(reversed(out))
TREATMENTS_RS = ROOT / "src" / "ai" / "advanced" / "treatments.rs"
FLAGS_RS = ROOT / "src" / "ai" / "advanced" / "treatment_flags.rs"
ELO_RS = ROOT / "src" / "elo.rs"

ROW = re.compile(r'\(\s*"([a-z0-9_]+)"\s*,\s*"([a-z0-9-]+)"\s*,\s*AdvancedAi::(?:enable|disable)_([a-z0-9_]+)')


def registry() -> dict[str, tuple[str, str]]:
    """Every registered gene: tag → (field, toggle name), from the
    treatments registry. The toggle name is not always the field name
    (`siege_tracks_wall` toggles through `enable_siege_tracks_the_wall`)."""
    return {tag: (field, toggle) for field, tag, toggle in ROW.findall(TREATMENTS_RS.read_text())}


def screenable_tags() -> list[str]:
    """The screen's own universe, in its order: the engine repairs (from
    `elo.rs`), then the production treatments and opt-ins (from the registry
    tables), never the Firaxis-only flags."""
    text = ELO_RS.read_text()
    start = text.index("pub const ENGINE_REPAIR_TREATMENTS: &[&str] = &[")
    end = text.index("];", start)
    repairs = re.findall(r'"([a-z0-9-]+)"', text[start:end])
    reg_text = TREATMENTS_RS.read_text()
    tags = list(repairs)
    for table in ("PRODUCTION_TREATMENTS", "PRODUCTION_OPT_INS"):
        s = reg_text.index(f"pub const {table}: &[LiveTreatment] = &[")
        e = reg_text.index("];", s)
        tags += [tag for _, tag, _ in ROW.findall(reg_text[s:e])]
    seen: set[str] = set()
    return [t for t in tags if not (t in seen or seen.add(t))]


ADVANCED_RS = ROOT / "src" / "ai" / "advanced.rs"
AI_RS = ROOT / "src" / "ai.rs"


def _first_sentence(doc_lines: str) -> str:
    doc = " ".join(line.strip().lstrip("/").strip() for line in doc_lines.splitlines())
    doc = re.sub(r"\s+", " ", doc).strip()
    # The ★ banners and ⚠ marks are emphasis, not prose.
    doc = re.sub(r"^[★⚠\s]+", "", doc)
    sentence = re.split(r"(?<=[.!?])\s", doc, maxsplit=1)[0]
    sentence = re.sub(r"\[`([^`]+)`\]", r"`\1`", sentence)
    if len(sentence) > DESCRIPTION_CHARS:
        sentence = sentence[: DESCRIPTION_CHARS - 3].rstrip() + "…"
    return sentence


def descriptions() -> dict[str, str]:
    """tag → one sentence: the `enable_<field>` toggle's doc, or — when that is
    missing or only says "See …" — the flag field's own doc in `advanced.rs` /
    `ai.rs`."""
    reg = registry()
    flags = FLAGS_RS.read_text()
    fields = ADVANCED_RS.read_text() + "\n" + AI_RS.read_text()
    out: dict[str, str] = {}
    for tag, (field, toggle) in reg.items():
        candidates = []
        m = re.search(r"((?:[ \t]*///[^\n]*\n)+)[ \t]*pub fn enable_" + re.escape(toggle) + r"\(", flags)
        if m:
            candidates.append(_first_sentence(m.group(1)))
        m = re.search(r"((?:[ \t]*///[^\n]*\n)+)[ \t]*(?:pub(?:\(crate\))? )?" + re.escape(field) + r": bool,", fields)
        if m:
            candidates.append(_first_sentence(m.group(1)))
        usable = [c for c in candidates if c and not c.startswith("See ")]
        out[tag] = (usable or candidates or [""])[0]
    return out


def load_sources(ledger: dict) -> tuple[dict[str, list[dict]], dict[str, str]]:
    """Per-gene measurement history in source order (the ledger records sources
    oldest-first, so a gene's last entry is its newest reading), and the source
    file the newest one came from."""
    history: dict[str, list[dict]] = {}
    newest_src: dict[str, str] = {}
    for src in ledger["sources"]:
        data = json.loads((ROOT / src["path"]).read_text())
        name = Path(src["path"]).name
        for gene in data.get("genes", []):
            g = {
                "win_on": float(gene["win_on"]),
                "win_off": float(gene["win_off"]),
                "n_on": int(gene.get("n_on", gene["pairs"])),
                "n_off": int(gene.get("n_off", gene["pairs"])),
                "win_z": float(gene["win_z"]),
                "share_z": float(gene["share_z"]),
                "players": int(data.get("profile", {}).get("players", 0) or 0),
                "compute_cost_pct": gene.get("compute_cost_pct"),
                "compute_cost_se_pct": gene.get("compute_cost_se_pct"),
                "time_cost_pct": gene.get("time_cost_pct"),
                "time_cost_se_pct": gene.get("time_cost_se_pct"),
            }
            history.setdefault(gene["tag"], []).append(g)
            newest_src[gene["tag"]] = name
    return history, newest_src


def fmt_int(n: float) -> str:
    return f"{int(round(n)):,}"


def diff_cell(history: list[dict]) -> str:
    """Render the whole on−off win-rate difference as a percentage.

    The figure is `gene_ledger.pooled_win_diff_pp` — the same number the ledger
    vetoes a default on, so the printed column and the deployment call cannot
    drift. Positive values intentionally have no leading plus; negative values
    retain their minus sign.
    """
    return f"{pooled_win_diff_pp(history):.2f}%"


def cost_cell(history: list[dict], value: str, uncertainty: str) -> str:
    """The newest usable cost reading, with one-standard-error
    uncertainty. Old analysis JSON predates the timing estimator and therefore
    carries no cost rather than a made-up zero."""
    for measurement in reversed(history):
        point = measurement.get(value)
        se = measurement.get(uncertainty)
        if point is None or se is None:
            continue
        point, se = float(point), float(se)
        if math.isfinite(point) and math.isfinite(se):
            return f"{point:+.2f}% ±{se:.2f}%"
    return "–"


def render(ledger: dict) -> str:
    measured, newest_src = load_sources(ledger)
    tags = screenable_tags()
    desc = descriptions()
    verdict = {g["tag"]: g for g in ledger["genes"]}
    reg = registry()

    rows = []
    unmeasured = []
    for tag in tags:
        history = measured.get(tag)
        if not history:
            unmeasured.append(tag)
            continue
        rows.append((wins_per(history[-1]["win_on"], history[-1]["players"]), tag, history))
    rows.sort(key=lambda r: (-r[0], r[1]))

    removed = sorted(tag for tag in measured if tag not in reg)
    latest = {tag: history[-1] for tag, history in measured.items()}

    # Everything that explains the table, kept but moved out from in front of it:
    # the operator reads the ranking, and twenty-two lines of preamble stood
    # between the file and its first row. Carried under the table instead, so
    # nothing derived is lost and nothing derived is in the way.
    reference = [
        "Every screenable heuristic gene on the Advanced controller, ranked most beneficial "
        "to least by **± Wins Last 10k** — wins added per 10,000 six-player games at the "
        "gene's measured on-rate in its **latest** screen. *± Wins 10k Prior* is the "
        "same figure from the screen before that (\u2013 when the gene has only one "
        "reading); movement between the two columns is the gene's trend across cycles. "
        "*Default* is the deployment ledger's call (`docs/gene_ledger.json`), and since "
        "2026-08-22 that call is read off these two win columns: a gene defaults **on** "
        "when both are positive, or when their average clears +15 with neither below "
        "\u221210; with exactly one populated column it defaults **on** when that reading "
        "is above +20. It defaults **off** otherwise. The *Total* win-rate "
        "columns pool every screen that measured the gene, weighted by games, and "
        "each carries its own game count `n` — the two arms are only equal when every "
        "screen that measured the gene split them evenly. *Diff* is the on rate minus the "
        "off rate, rendered as a percentage: the **whole** on−off difference, so it stands at "
        "roughly twice the scale of the win columns beside it and must be read against a "
        "screen’s difference band rather than the halved column band below. "
        "**A negative *Diff* vetoes the default** (operator, 2026-08-22): a gene that has "
        "not won more than it lost across its whole record ships off however its two win "
        "columns read. That is the one clause that lets a screen older than the last two "
        "speak, and it is one-way — a positive *Diff* promotes nothing on its own, the "
        "columns still have to clear their bars. Three genes ship off on it alone: "
        "`war-economy`, `apostle-promotion-by-role` and `siege-commitment`, each carrying "
        "positive recent columns over a 2026-08-20 screen they have not made back. "
        "**There is one screen** (operator, 2026-08-22): six majors on 74x46 continents "
        "with nine city-states, Online speed to its own 250-turn clock, all six victory "
        "lanes, a foldover against the best-genome baseline with shuffled civs and every "
        "major seat carrying its own genome (errors clustered by game pair), so a gene's "
        "on/off readings cover the same maps. `docs/GENE_SCREEN.md` documents the "
        "instrument; the paired contrasts, intervals and family-wise verdicts stay in "
        "`docs/gene_ledger.json`. Screenable genes awaiting their first "
        "measurement are listed separately below without a rank.",
        "",
        "**Reading the table.** A six-player seat wins 1-in-6 by chance, so the expected "
        "count is 1,667 wins per 10,000 games and the "
        "win columns say how far above or below that a seat carrying the gene lands. "
        "**A column is half its screen’s on−off difference** — a foldover puts the two arms "
        "either side of chance — so the band that says whether a column is real is half the "
        "band on that difference. The two are not interchangeable: the ±110/10k figure this "
        "paragraph used to quote, and #2266 used to call eight removals noise, is the "
        "*difference*’s band and is twice too wide for the column beside it. Each screen’s "
        "own band is below, derived from its errors rather than quoted. Screens differ in "
        "baseline as repairs land, so the *Prior* column reads as history, not a strict A/B "
        "against *Last*.",
        "",
        "**⚠ Every column below is `legacy`.** The shape marked `legacy` in the screen "
        "table is the pre-2026-08-22 instrument: 60x38 Pangaea, six city-states, where 48% "
        "of games ended in a religious conversion against 28% on continents. Those readings "
        "are what the deployment genome stands on and they are kept for that reason, but a "
        "gene is only priced at the screen once a `standard` row appears beside it. The "
        "four-player `domination,score` war columns are gone: a 1-in-4 chance base made "
        "them incomparable with the six-player columns printed next to them.",
        "",
        "**What each screen resolves.** The median gene’s column standard error "
        f"times {POWER_80} — a two-sided 5% test at 80% power. Judge a column against "
        "the band of the screen named beside it, never against a single number for the "
        "instrument: these differ by more than three to one.",
        "",
        "*Pairing gain* is how far a screen’s error per pair sits below the unpaired "
        "baseline, and it is what separates them. A foldover cancels only to the extent "
        "its two arms play a similar game, so the gain reads on the **genes**, not the "
        "design — a gene that rarely fires leaves most pairs identical and cancels "
        "almost everything, while a whole-genome screen flips every gene between arms "
        "and cancels almost nothing. ⚠ Gene count is not the driver, though the rows "
        "below invite that reading — the falsifier is in them. `h1` carries **one** gene "
        "over **7,200** pairs and resolves ±68 at a 1.28× gain, *wider* than four-gene "
        "`s6` over 6,000. Its gene changes nearly every game; `s7`'s rarely fires. That, "
        "not the count, is the difference.",
        "",
        "| Screen | Shape | Genes | Seat pairs | 1 SE | ±80% power | Pairing gain |",
        "|---|---|---:|---:|---:|---:|---:|",
        *(
            f"| `{r['name']}` | {r['shape']} | {r['genes']} | {fmt_int(r['pairs'])} | "
            f"{r['se']:.1f} | ±{r['band']:.0f} | {r['gain']:.2f}× |"
            for r in resolutions(ledger)
        ),
        "",
        "**Cost.** Positive is slower; negative is faster. *cost (compute)* is the "
        "on/off percent change in wall seconds per completed turn, while *cost (time)* "
        "is the percent change in whole-game wall seconds and therefore includes games "
        "that end earlier or later. Each cell is the newest estimate ± one standard "
        "error. The screen derives both from paired log-ratios on the same maps, fits every "
        "randomized gene together with an arm-order intercept, and keeps one timing per game "
        "pair; all-seats signs are summed so the answer is the incremental cost of enabling "
        "one major's genome. This reuses the screen's existing `secs` and `turn` rows — no "
        "hot-path timers and no extra profiling games. A dash means the source analysis "
        "predates the estimator and is unknown, never zero.",
        "",
        "Regenerate with `python3 tools/heuristic_gene_ranking.py --write` after every "
        "screen enters the ledger; `tools/test_heuristic_gene_ranking.py` fails when this "
        "file is older than the ledger's sources.",
    ]

    lines = [
        "# The heuristic gene ranking",
        "",
        "| Rank | Gene | Description | Default | ± Wins Last 10k | ± Wins 10k Prior | Total (on) Win rate | Total (off) Win rate | Diff | cost (compute) | cost (time) |",
        "|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for rank, (wins, tag, history) in enumerate(rows, 1):
        v = verdict.get(tag, {})
        default = "**on**" if v.get("default_on") else "off"
        prior = (
            f"{wins_per(history[-2]['win_on'], history[-2]['players']):+d}"
            if len(history) > 1
            else "\u2013"
        )
        on_games = sum(m["n_on"] for m in history)
        off_games = sum(m["n_off"] for m in history)
        on_rate, off_rate = pooled_win_rates(history)
        lines.append(
            f"| {rank} | `{tag}` | {desc.get(tag, '')} | {default} | {wins:+d} | {prior} | "
            f"{100 * on_rate:.2f}% (n={fmt_int(on_games)}) | "
            f"{100 * off_rate:.2f}% (n={fmt_int(off_games)}) | "
            f"{diff_cell(history)} | "
            f"{cost_cell(history, 'compute_cost_pct', 'compute_cost_se_pct')} | "
            f"{cost_cell(history, 'time_cost_pct', 'time_cost_se_pct')} |"
        )

    if unmeasured:
        lines += [
            "",
            "## Awaiting measurement",
            "",
            "These screenable genes have no on/off result, so they receive no rank or "
            "promotion from this table. Their deployment state remains explicit while a "
            "screen is pending.",
            "",
            "| Gene | Default | Description |",
            "|---|---|---|",
        ]
        for tag in sorted(unmeasured):
            v = verdict.get(tag, {})
            default = "**on**" if v.get("default_on") else "off"
            verdict_word = v.get("verdict", "unmeasured")
            lines.append(
                f"| `{tag}` | {default} ({verdict_word}) | {desc.get(tag, '')} |"
            )

    if removed:
        lines += [
            "",
            "## Removed from the code",
            "",
            "Genes whose code has left the repository (operator directive: the bottom of the "
            "table leaves the code), listed from their last measurement:",
            "",
            "| Gene | Wins ±10k (last tracked measurement) | Win rate (on) | Win rate (off) | Source |",
            "|---|---:|---:|---:|---|",
        ]
        for tag in sorted(removed, key=lambda t: wins_per(latest[t]["win_on"], latest[t]["players"]), reverse=True):
            m, src = latest[tag], newest_src[tag]
            lines.append(
                f"| `{tag}` | {wins_per(m['win_on'], m['players']):+d} | {100 * m['win_on']:.2f}% | "
                f"{100 * m['win_off']:.2f}% | `{src}` |"
            )

    lines += ["", "## How to read this", ""] + reference

    # Hand-written follow-ups live in `docs/gene_ranking_notes.md` and are
    # carried under the table, so a reading written against one screen is
    # not lost when the table regenerates.
    if NOTES_MD.exists():
        notes = [line for line in NOTES_MD.read_text().splitlines() if not line.startswith("<!--")]
        body = "\n".join(notes).strip()
        if body:
            lines += ["", "## Follow-ups", "", body]
    sources = ", ".join(f"`{Path(s['path']).name}` ({s['shape']}, {s['complete_pairs']:,} pairs)" for s in ledger["sources"])
    lines += [
        "",
        f"_Generated by `tools/heuristic_gene_ranking.py` from the ledger's sources: {sources}. "
        "The paired contrasts, intervals and family-wise verdicts live in `docs/gene_ledger.json`; "
        "this table is the operator's wins-per-ten-thousand view of the same games._",
        "",
    ]
    return "\n".join(lines)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--write", action="store_true", help="rewrite HEURISTIC_GENE_RANKING.md")
    ap.add_argument("--check", action="store_true", help="fail if the table is stale")
    args = ap.parse_args(argv)
    ledger = json.loads(LEDGER_JSON.read_text())
    text = render(ledger)
    if args.check:
        if RANKING_MD.read_text() != text:
            print("heuristic gene ranking: stale — run `python3 tools/heuristic_gene_ranking.py --write`")
            return 1
        print("heuristic gene ranking: up to date")
        return 0
    if args.write:
        RANKING_MD.write_text(text)
        print(f"wrote {RANKING_MD.relative_to(ROOT)}")
        return 0
    sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
