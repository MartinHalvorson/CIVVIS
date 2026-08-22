#!/usr/bin/env python3
"""Regenerate `HEURISTIC_GENE_RANKING.md` — every screenable heuristic gene,
with a native measurement, ranked by wins added per 10,000 six-player games,
plus the screenable genes still awaiting that measurement.

    python3 tools/heuristic_gene_ranking.py --write     # rewrite the table
    python3 tools/heuristic_gene_ranking.py --check     # fail if it is stale

The table used to be written once, by hand, from one screen's rows. Now it is
derived: for each gene the **latest native source** in `docs/gene_ledger.json`
that measured it supplies the on/off wins and games (so a gene added after the
whole-genome screen still appears, from its own screen), the **latest war
source** supplies the war column, and the deployment verdict comes from the
ledger. Screenable genes without a native result are listed separately without
a rank. Genes whose code has been removed this cycle are listed from their last
measurement, as before. Descriptions are the first sentence of each toggle's
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
from gene_ledger import wins_per_10k as wins_per  # noqa: E402
LEDGER_JSON = ROOT / "docs" / "gene_ledger.json"
RANKING_MD = ROOT / "HEURISTIC_GENE_RANKING.md"
NOTES_MD = ROOT / "docs" / "gene_ranking_notes.md"
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
    if len(sentence) > 160:
        sentence = sentence[:157].rstrip() + "…"
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


def load_sources(
    ledger: dict,
) -> tuple[dict[str, list[dict]], dict[str, dict], dict[str, str], dict[str, str]]:
    """Per-gene native measurement history in source order (the ledger records
    sources oldest-first, so a gene's last entry is its newest reading), the
    latest war measurement, and the source file each came from."""
    native: dict[str, list[dict]] = {}
    war: dict[str, dict] = {}
    native_src: dict[str, str] = {}
    war_src: dict[str, str] = {}
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
            if src["regime"] == "native":
                native.setdefault(gene["tag"], []).append(g)
                native_src[gene["tag"]] = name
            else:
                war[gene["tag"]] = g
                war_src[gene["tag"]] = name
    return native, war, native_src, war_src


def fmt_int(n: float) -> str:
    return f"{int(round(n)):,}"


def cost_cell(history: list[dict], value: str, uncertainty: str) -> str:
    """The newest usable native cost reading, with one-standard-error
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
    native, war, native_src, war_src = load_sources(ledger)
    tags = screenable_tags()
    desc = descriptions()
    verdict = {g["tag"]: g for g in ledger["genes"]}
    reg = registry()

    rows = []
    unmeasured = []
    for tag in tags:
        history = native.get(tag)
        if not history:
            unmeasured.append(tag)
            continue
        rows.append((wins_per(history[-1]["win_on"], history[-1]["players"]), tag, history))
    rows.sort(key=lambda r: (-r[0], r[1]))

    removed = sorted(
        (tag for tag in set(native) | set(war) if tag not in reg),
    )
    latest = {tag: history[-1] for tag, history in native.items()}

    lines = [
        "# The heuristic gene ranking",
        "",
        "Every screenable heuristic gene on the Advanced controller, ranked most beneficial "
        "to least by **± Wins Last 10k** — wins added per 10,000 six-player games at the "
        "gene's measured on-rate in its **latest** native screen. *± Wins 10k Prior* is the "
        "same figure from the screen before that (\u2013 when the gene has only one native "
        "reading); movement between the two columns is the gene's trend across cycles. "
        "*Default* is the deployment ledger's call (`docs/gene_ledger.json`), and since "
        "2026-08-22 that call is read off these two win columns: a gene defaults **on** "
        "when both are positive, or when their average clears +15 with neither below "
        "\u221210; with exactly one populated column it defaults **on** when that reading "
        "is above +20. It defaults **off** otherwise. The *Total* "
        "columns pool every native screen that measured the gene, weighted by games. Every "
        "screen is a foldover against the best-genome baseline with shuffled civs and every "
        "major seat carrying its own genome (errors clustered by game pair), so a gene's "
        "on/off readings cover the same maps. `docs/GENE_SCREEN.md` documents the "
        "instrument; the paired contrasts, intervals and family-wise verdicts stay in "
        "`docs/gene_ledger.json`. Screenable genes awaiting their first native "
        "measurement are listed separately below without a rank.",
        "",
        "**Reading the table.** A six-player seat wins 1-in-6 by chance (1-in-4 in a "
        "four-player screen), so the expected count is 1,667 wins per 10,000 games and the "
        "win columns say how far above or below that a seat carrying the gene lands; the "
        "whole-genome screen resolves about ±110 wins per 10,000 at 80% power and a "
        "single-gene 6,000-seat-pair screen about ±130 — differences inside that band are "
        "noise, not nulls. Screens differ in baseline as repairs land, so the *Prior* "
        "column reads as history, not a strict A/B against *Last*.",
        "",
        "**Cost.** Positive is slower; negative is faster. *cost (compute)* is the "
        "on/off percent change in wall seconds per completed turn, while *cost (time)* "
        "is the percent change in whole-game wall seconds and therefore includes games "
        "that end earlier or later. Each cell is the newest native estimate ± one standard "
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
        "",
        "| Rank | Gene | Description | Default | ± Wins Last 10k | ± Wins 10k Prior | Total (on) Win rate | Total (off) Win rate | Total Games (on+off) | cost (compute) | cost (time) |",
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
        on_rate = sum(m["win_on"] * m["n_on"] for m in history) / on_games
        off_rate = sum(m["win_off"] * m["n_off"] for m in history) / off_games
        lines.append(
            f"| {rank} | `{tag}` | {desc.get(tag, '')} | {default} | {wins:+d} | {prior} | "
            f"{100 * on_rate:.2f}% | {100 * off_rate:.2f}% | {fmt_int(on_games + off_games)} | "
            f"{cost_cell(history, 'compute_cost_pct', 'compute_cost_se_pct')} | "
            f"{cost_cell(history, 'time_cost_pct', 'time_cost_se_pct')} |"
        )

    if unmeasured:
        lines += [
            "",
            "## Awaiting native measurement",
            "",
            "These screenable genes have no native on/off result, so they receive no rank or "
            "promotion from this table. Their deployment state remains explicit while a native "
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
            "| Gene | Wins ±10k (last tracked measurement) | Regime | Win rate (on) | Win rate (off) | Source |",
            "|---|---:|---|---:|---:|---|",
        ]
        def last(tag):
            if tag in latest:
                return latest[tag], "native", native_src[tag]
            return war[tag], "war", war_src[tag]
        for tag in sorted(removed, key=lambda t: wins_per(last(t)[0]["win_on"], last(t)[0]["players"]), reverse=True):
            m, regime, src = last(tag)
            lines.append(
                f"| `{tag}` | {wins_per(m['win_on'], m['players']):+d} | {regime} | {100 * m['win_on']:.2f}% | "
                f"{100 * m['win_off']:.2f}% | `{src}` |"
            )

    # Hand-written follow-ups live in `docs/gene_ranking_notes.md` and are
    # carried under the table, so a reading written against one screen is
    # not lost when the table regenerates.
    if NOTES_MD.exists():
        notes = [line for line in NOTES_MD.read_text().splitlines() if not line.startswith("<!--")]
        body = "\n".join(notes).strip()
        if body:
            lines += ["", "## Follow-ups", "", body]
    sources = ", ".join(f"`{Path(s['path']).name}` ({s['regime']}, {s['complete_pairs']:,} pairs)" for s in ledger["sources"])
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
