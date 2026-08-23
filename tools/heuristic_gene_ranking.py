#!/usr/bin/env python3
"""Regenerate `HEURISTIC_GENE_RANKING.md` — every screenable heuristic gene,
with a measurement, ranked by wins added per 10,000 six-player on-arm seats, plus the
screenable genes still awaiting one.

    python3 tools/heuristic_gene_ranking.py --write     # rewrite the table
    python3 tools/heuristic_gene_ranking.py --check     # fail if it is stale

The table used to be written once, by hand, from one screen's rows. Now it is
derived: for each gene the **latest source** in `docs/gene_ledger.json` that
measured it supplies the on/off wins and seat counts (so a gene added after the
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

Beside the operator's two win columns the table publishes a **precision-weighted
posterior** — `gene_ledger.pooled_posterior`, a random-effects inverse-variance
pool of every screen that priced the gene — with its 95% interval and
`P(effect > 0)`, the newest screen's **score-share** reading and verdict, what
each deployment authority would ship, the two shapes apart, the boundary genes
ranked by what one direct arm would buy, and the lane genes on the axis they can
actually pay on. **None of it decides a default**: `AUTHORITY` in
`gene_ledger.py` says `columns` and this file publishes the delta so the
operator can take the call on numbers.

    python3 tools/heuristic_gene_ranking.py --boundary   # the next round's --genes list
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
sys.path.insert(0, str(ROOT / "tools"))
import gene_registry  # noqa: E402

sys.path.insert(0, str(HERE))
# The ledger tool owns the win column: it decides each gene's default from the
# same two numbers this table prints, so both must be one arithmetic.
from gene_ledger import (  # noqa: E402
    AUTHORITIES,
    AUTHORITY,
    PER,
    POSTERIOR_SHAPES,
    POWER_80,
    Z95,
    arm_information_value,
    arm_pairs_to_resolve,
    column_se,
    deployment_default_on,
    direct_arm_constant,
    normal_cdf,
    pooled_posterior,
    pooled_win_diff_pp,
    pooled_win_rates,
    posterior_call,
    wins_per_10k as wins_per,
)
LEDGER_JSON = ROOT / "docs" / "gene_ledger.json"
RANKING_MD = ROOT / "HEURISTIC_GENE_RANKING.md"
NOTES_MD = ROOT / "docs" / "gene_ranking_notes.md"

#: How much of a gene's sentence the Description column carries. Widened
#: 160 → 480 on 2026-08-22 (operator: "three times as wide"): the longest
#: first sentence in the registry is 249 characters, so every description
#: now prints whole and the "…" that clipped 16 of them is gone.
DESCRIPTION_CHARS = 480

#: The en dash this table prints for a cell no screen can fill.
#: ⚠ A MODULE CONSTANT RATHER THAN AN ESCAPE INSIDE AN f-STRING. A
#: backslash in an f-string's expression part is a syntax error before
#: Python 3.12, so #2329's `f"{'\\u2013' if ...}"` made this whole file
#: unparseable on the fleet's macOS seats, which run the system 3.9 —
#: the ranking could not be regenerated there at all. CI's newer Python
#: accepted it, so nothing failed until a seat tried to run it.
EN_DASH = "\u2013"


# ⭐ `column_se` and `POWER_80` moved into `gene_ledger.py` (#2300 put the
# arithmetic beside the `wins_per_10k` it halves; that function lives there,
# and so now does the precision-weighted posterior that consumes both). They
# are imported above, so `ranking.column_se` still resolves and the printed
# band, the printed column and the deployment decision remain one arithmetic.


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
FLAGS_RS = ROOT / "src" / "ai" / "advanced" / "treatment_flags.rs"


def registry() -> dict[str, tuple[str, str]]:
    """Every registered gene: tag → (field, toggle name), from the gene
    registry (`src/ai/advanced/genes.rs`, read by `gene_registry.py`). The
    toggle name is not always the field name (`siege_tracks_wall` toggles
    through `enable_siege_tracks_the_wall`)."""
    return {row.tag: (row.field, row.toggle) for row in gene_registry.genes()}


def screenable_tags() -> list[str]:
    """The screen's own universe, in its order: every screenable row of the
    registry — the engine repairs, the production genes and the opt-ins, never
    a plain host-only flag."""
    return gene_registry.screenable_tags()


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
                # What the precision-weighted posterior pools, and the shape
                # it was measured at, so the pool can be taken per instrument
                # as well as whole.
                "win_delta_pp": float(gene["win_delta_pp"]),
                "win_se_pp": (None if gene.get("win_se_pp") is None
                              else float(gene["win_se_pp"])),
                "share_delta_pp": float(gene["share_delta_pp"]),
                "shape": src["shape"],
                "source": name,
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


#: A single-gene direct arm's default size, in matched seat pairs: the 1,200
#: map pairs `2026-08-22-h1` actually played. `--boundary --arm-pairs N` moves
#: it; the precision at that size is measured, not modelled (see
#: `gene_ledger.direct_arm_constant`).
ARM_PAIRS = 7200

#: The largest direct arm worth proposing, in matched seat pairs. 60,000 is
#: 10,000 map pairs at the screen's six seats — one standard batch. A gene
#: that needs more than a whole batch to resolve on its own is a gene to leave
#: to the next whole-genome screen, not to aim an arm at.
FEASIBLE_ARM_PAIRS = 60_000

#: How many boundary genes the printed `--genes` argument list carries.
BOUNDARY_SUGGESTIONS = 8

#: The lane modules whose flag fields make a gene a **lane gene** — discovered
#: from the code, never listed here. A lane gene serves one victory lane, and
#: `docs/GENE_SCREEN.md` pre-registers how it is judged.
LANE_MODULES = (ROOT / "src" / "ai" / "advanced" / "victory_lane.rs",)


def lane_tags() -> list[str]:
    """Every gene whose flag field a victory-lane module reads, in registry
    order.

    Discovered, never listed: `AGENTS.md`'s own rule is that a hand-written
    list is complete the day it is written. A gene joins this set by being
    read in `victory_lane.rs`, which is the same act that makes it a lane
    gene at all."""
    read = set()
    for path in LANE_MODULES:
        read |= set(re.findall(r"self\.([a-z0-9_]+)", path.read_text()))
    reg = registry()
    return [tag for tag in screenable_tags()
            if tag in reg and reg[tag][0] in read]


def analyses(ledger: dict) -> list[dict]:
    """Each ledger source with its loaded analysis, for the statistics that
    need the file rather than the recorded summary."""
    return [
        {
            "name": Path(src["path"]).name,
            "shape": src["shape"],
            "analysis": json.loads((ROOT / src["path"]).read_text()),
        }
        for src in ledger["sources"]
    ]


def posterior_cell(posterior: dict | None) -> str:
    """`+45 [−24, +114]` — the pooled effect and its 95% interval, in the win
    column's own units, so it reads directly against the two columns beside
    it."""
    if posterior is None:
        return "\u2013"
    return (f"{posterior['effect']:+.0f} "
            f"[{posterior['lo']:+.0f}, {posterior['hi']:+.0f}]")


def probability_cell(posterior: dict | None) -> str:
    """`P(effect > 0)`. This is where the shrinkage shows: two genes can print
    the same `+30` and land at 90% and 99.8% because their screens resolve
    ±64 and ±29."""
    if posterior is None:
        return "\u2013"
    return f"{100 * posterior['p_positive']:.1f}%"


def share_verdict(share_z: float) -> str:
    """The screen's own `*` convention, on the score-share axis alone."""
    if share_z >= 2.0:
        return "helps *"
    if share_z <= -2.0:
        return "hurts *"
    return "~"


def share_cell(history: list[dict]) -> str:
    """The newest screen's score-share contrast and its verdict.

    Published beside the win columns because the deployment rule reads the win
    axis **only**, and at the standing 250-turn Online clock a science or
    congress gene cannot pay on that axis at all — science and diplomatic
    victories land at median t283 and t285, so they are 1–2% of endings. A
    lane gene's evidence is here or nowhere. `docs/GENE_SCREEN.md` carries the
    pre-registered rule for reading it."""
    newest = history[-1]
    z = newest["share_z"]
    return f"{newest['share_delta_pp']:+.2f} (z {z:+.2f}) {share_verdict(z)}"


def posterior_of(history: list[dict]) -> dict | None:
    """The published pool: every screen at a shape `POSTERIOR_SHAPES` admits."""
    return pooled_posterior(history, POSTERIOR_SHAPES)


def authority_table(ledger: dict, measured: dict[str, list[dict]]) -> list[dict]:
    """Every measured gene's shipped default beside what each authority would
    ship, with the posterior that decides it."""
    # The screen's own universe only. A host-only flag can carry a ledger row
    # from a retired native stand-in (`step-and-reassess`) and the ledger never
    # governs it, so it is not ranked and must not appear in a decision table
    # either.
    screenable = set(screenable_tags())
    rows = []
    for gene in ledger["genes"]:
        history = measured.get(gene["tag"])
        if not history or gene["tag"] not in screenable:
            continue
        posterior = posterior_of(history)
        effect = None if posterior is None else posterior["effect"]
        se = None if posterior is None else posterior["se"]
        row = {
            "tag": gene["tag"],
            "shipped": bool(gene["default_on"]),
            "posterior": posterior,
            "call": posterior_call(effect, se),
        }
        for candidate in AUTHORITIES:
            # Namespaced: one of the authorities is called `posterior`, and
            # so is the estimate it reads.
            row[f"would/{candidate}"] = deployment_default_on(
                candidate, gene["wins_last_10k"], gene["wins_prior_10k"],
                gene["win_diff_pp"], effect, se)
        rows.append(row)
    return rows


def boundary_table(ledger: dict, measured: dict[str, list[dict]],
                   arm_pairs: int = ARM_PAIRS) -> tuple[list[dict], tuple[float, str] | None]:
    """The genes whose interval straddles the decision line, ranked by what one
    single-gene direct arm would buy.

    ⚠ THE EFFICIENT PLAN IS TWO STAGE, and this is its second stage. The
    whole-genome foldover is the efficient way to RANK — p10 priced 75 genes at
    ±51 each on 17,574 seat pairs, and the same budget split into 75 single-gene
    screens would give ±146 each, about 8× worse per gene. A single-gene arm
    resolves far tighter per pair once it is aimed (`s7`: ±29 on 6,000 pairs, a
    3.32× pairing gain against p10's 1.09%). So the screen ranks and the direct
    arms resolve the boundary; a partial or blocked foldover is neither, and
    `docs/GENE_SCREEN.md` records the arithmetic."""
    arm = direct_arm_constant(analyses(ledger))
    rows = []
    for row in authority_table(ledger, measured):
        if row["call"] != "unresolved" or row["posterior"] is None:
            continue
        effect, se = row["posterior"]["effect"], row["posterior"]["se"]
        entry = dict(row)
        if arm is not None:
            constant = arm[0]
            entry["needs"] = arm_pairs_to_resolve(effect, se, constant)
            entry["buys"] = arm_information_value(
                effect, se, constant / math.sqrt(arm_pairs), row["shipped"])
        else:  # pragma: no cover - the ledger has always had a single-gene arm
            entry["needs"], entry["buys"] = None, 0.0
        rows.append(entry)
    rows.sort(key=lambda r: (-r["buys"], r["tag"]))
    return rows, arm


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


AUTHORITY_BLURB = {
    "columns": "the operator's threshold rule, exactly as it ships: both win columns "
               "positive, or their average above +15 with neither below \u221210, or one "
               "column above +20 \u2014 and off whatever they say when the pooled *Diff* "
               "is negative",
    "posterior-veto": "the same columns, with an error bar on the veto: it fires only when "
                      "the posterior's 95% interval lies **wholly below zero**, instead of "
                      "on the bare sign of a difference that carries no error at all",
    "posterior": "the pooled estimate decides wherever its interval excludes zero, and "
                 "`posterior-veto` decides where it straddles",
}


def posterior_sections(ledger: dict, measured: dict[str, list[dict]],
                       desc: dict[str, str]) -> list[str]:
    """Everything the posterior publishes: what each authority would ship, the
    genes that move, the shapes apart, the boundary set, and the lane genes.

    ⚠ NOTHING HERE DECIDES ANYTHING. `default_on` is the ledger's, under the
    authority the ledger records, and this PR does not move one of them. These
    tables exist so the operator can take the call on numbers rather than on a
    threshold nobody derived from the errors."""
    rows = authority_table(ledger, measured)
    in_force = ledger["rules"]["authority"]
    lines = [
        "",
        "## What the posterior would change",
        "",
        "A threshold in column units is not a threshold in evidence. The screens these "
        "columns come from resolve between \u00b129 and \u00b1101 at 80% power \u2014 more than "
        "three to one \u2014 so **+24 decides differently depending only on which screen "
        "priced the gene**, and #2294's single-column +20 bar sits below every band the "
        "instrument has ever printed. *Posterior (95% CI)* above is the answer to that: a "
        "random-effects (DerSimonian\u2013Laird) inverse-variance pool of every screen's "
        "on\u2212off difference on this column's own scale, each screen weighted by its own "
        "standard error, with the disagreement **between** screens carried in the interval "
        "rather than assumed away. `P(>0)` is where the shrinkage shows: two genes can "
        "print the same +30 and land at 90% and 99.8%.",
        "",
        "**It is published, not in force.** `AUTHORITY` in `tools/gene_ledger.py` is the "
        "whole switch, it says `columns`, and this table is what the other settings would "
        "ship. Two reasons it is not flipped here, neither of them arithmetic: the "
        "threshold rule is an explicit operator directive, and **every source in this "
        "ledger is the retired `legacy` 60\u00d738 Pangaea shape** \u2014 re-deciding the "
        "deployment genome now would re-decide it on the wrong instrument.",
        "",
        "| Authority | Genes on | Genes moved | What it is |",
        "|---|---:|---:|---|",
    ]
    for candidate in AUTHORITIES:
        mark = " **(in force)**" if candidate == in_force else ""
        lines.append(
            f"| `{candidate}`{mark} | {ledger['counts'][f'default_on_under_{candidate}']} | "
            f"{ledger['counts'][f'moved_by_{candidate}']} | {AUTHORITY_BLURB[candidate]} |"
        )

    moved = [r for r in rows
             if any(r[f"would/{c}"] != r["shipped"] for c in AUTHORITIES)]
    lines += ["", "### The genes that move", ""]
    if not moved:
        lines.append(
            "**None.** On today's evidence every authority ships the same genome, so "
            "adopting the posterior would cost nothing and change nothing \u2014 which is "
            "itself the reading: the two rules only diverge once a screen disagrees with "
            "the record, and no source here does."
        )
    else:
        lines += [
            "Each row is a gene whose shipped default one of the settings above would "
            "change. `on`/`off` in bold is a move.",
            "",
            "\u26a0 Read what these rows do and do not say. Every one is a **re-admission**, "
            "and not one of them has a positive point estimate: the posterior is not "
            "saying these genes help, it is saying the veto that removed them **could not "
            "tell**. The shipped rule fires on the sign of a pooled difference that "
            "carries no error at all \u2014 \u22120.78, \u22120.21 and \u22120.06 pp \u2014 and every one "
            "of those three intervals straddles zero. Where the interval straddles, the "
            "`posterior` setting inherits the columns' answer, because `default_on` has "
            "to be a pure function of the sources and the only other candidate is "
            "whatever shipped yesterday. That deferral is the reason *Where a direct arm "
            "pays* exists.",
            "",
            "| Gene | Shipped | "
            + " | ".join(f"`{c}`" for c in AUTHORITIES if c != in_force)
            + " | Posterior (95% CI) | P(>0) | Pooled *Diff* |",
            "|---|---|" + "---|" * (len(AUTHORITIES) - 1) + "---:|---:|---:|",
        ]
        by_tag = {g["tag"]: g for g in ledger["genes"]}
        for row in moved:
            cells = []
            for candidate in AUTHORITIES:
                if candidate == in_force:
                    continue
                word = "on" if row[f"would/{candidate}"] else "off"
                cells.append(f"**{word}**" if row[f"would/{candidate}"] != row["shipped"]
                             else word)
            lines.append(
                f"| `{row['tag']}` | {'on' if row['shipped'] else 'off'} | "
                + " | ".join(cells)
                + f" | {posterior_cell(row['posterior'])} | "
                f"{probability_cell(row['posterior'])} | "
                f"{by_tag[row['tag']]['win_diff_pp']:.2f}% |"
            )

    decided_on = [r for r in rows if r["call"] == "on"]
    decided_off = [r for r in rows if r["call"] == "off"]
    straddle = [r for r in rows if r["call"] == "unresolved"]
    lines += [
        "",
        "### What the posterior can decide at all",
        "",
        f"Of {len(rows)} priced genes the interval clears zero for **{len(decided_on)} "
        f"upward** and **{len(decided_off)} downward**; **{len(straddle)} sit inside the "
        "interval either way** and are the boundary set below. A straddling interval is "
        "not a null \u2014 it is the instrument saying it cannot tell, which is exactly what "
        "a fixed \u00b115 bar cannot say.",
        "",
        "| Gene | Posterior (95% CI) | P(>0) | Screens | Shipped | Posterior call |",
        "|---|---:|---:|---:|---|---|",
    ]
    for row in decided_on + decided_off:
        lines.append(
            f"| `{row['tag']}` | {posterior_cell(row['posterior'])} | "
            f"{probability_cell(row['posterior'])} | {row['posterior']['screens']} | "
            f"{'on' if row['shipped'] else 'off'} | **{row['call']}** |"
        )

    lines += shape_section(ledger, measured, rows)
    lines += boundary_section(ledger, measured)
    lines += lane_section(ledger, measured, desc)
    return lines


def shape_section(ledger: dict, measured: dict[str, list[dict]],
                  rows: list[dict]) -> list[str]:
    """Legacy, standard and pooled \u2014 side by side, because pooling two
    instruments that disagree is the one thing a pooled estimate must not do
    silently."""
    shapes: dict[str, dict] = {}
    for src in ledger["sources"]:
        entry = shapes.setdefault(src["shape"], {"sources": 0, "pairs": 0})
        entry["sources"] += 1
        entry["pairs"] += int(src["complete_pairs"])
    ranked = {row["tag"] for row in rows}
    for shape in shapes:
        shapes[shape]["genes"] = sum(
            1 for tag, history in measured.items()
            if tag in ranked and any(m["shape"] == shape for m in history))

    lines = [
        "",
        "## The two shapes, apart",
        "",
        "`\u03c4` (tau) is the between-screen standard deviation the random-effects pool "
        "estimates. It is the statistic that answers *\u201cis 'both columns positive' two "
        "confirmations?\u201d*: when screens agree to within their errors it is zero and the "
        "pool is the ordinary inverse-variance one; when they do not, it widens the "
        "interval instead of averaging two worlds into a confident wrong answer. "
        f"`POSTERIOR_SHAPES` in `tools/gene_ledger.py` says which shapes the published "
        f"pool admits and is currently `{', '.join(POSTERIOR_SHAPES)}`.",
        "",
        "| Shape | Sources | Seat pairs | Genes priced |",
        "|---|---:|---:|---:|",
    ]
    for shape in ("standard", "legacy"):
        entry = shapes.get(shape, {"sources": 0, "pairs": 0, "genes": 0})
        lines.append(f"| {shape} | {entry['sources']} | {fmt_int(entry['pairs'])} | "
                     f"{entry['genes']} |")

    both = []
    for row in rows:
        history = measured[row["tag"]]
        legacy = pooled_posterior(history, ("legacy",))
        standard = pooled_posterior(history, ("standard",))
        if legacy is None or standard is None:
            continue
        pooled = pooled_posterior(history)
        both.append((row["tag"], legacy, standard, pooled))
    if not both:
        lines += [
            "",
            "\u26a0 **No `standard` source is in the ledger yet**, so every figure in this "
            "file is the retired Pangaea instrument and the per-gene split below is empty. "
            "It fills the moment a screen at the deployment shape enters "
            "`docs/gene_ledger.json`, and `docs/gene_ranking_notes.md` carries what the "
            "first one already says about the genes it disagrees with.",
        ]
        return lines

    lines += [
        "",
        "Genes priced at both shapes. **A row whose two intervals do not overlap is not a "
        "gene with one number; it is two instruments disagreeing**, and the pooled column "
        "beside it should be read as a warning rather than an answer.",
        "",
        "| Gene | legacy | standard | pooled | \u03c4 | overlap |",
        "|---|---:|---:|---:|---:|---|",
    ]
    for tag, legacy, standard, pooled in both:
        overlap = legacy["lo"] <= standard["hi"] and standard["lo"] <= legacy["hi"]
        lines.append(
            f"| `{tag}` | {posterior_cell(legacy)} | {posterior_cell(standard)} | "
            f"{posterior_cell(pooled)} | {pooled['tau']:.0f} | "
            f"{'yes' if overlap else '**no**'} |"
        )
    return lines


def boundary_section(ledger: dict, measured: dict[str, list[dict]]) -> list[str]:
    """The straddlers, ranked by what one direct arm would buy, with the
    `--genes` list an operator can paste."""
    rows, arm = boundary_table(ledger, measured)
    lines = [
        "",
        "## Where a direct arm pays: the boundary genes",
        "",
        "**The efficient plan is two stage, and it is not a partial foldover.** The "
        "whole-genome screen is the efficient way to RANK: `p10` priced 75 genes at "
        "\u00b151 each on 17,574 seat pairs, and the same budget split into 75 single-gene "
        "screens of 234 pairs would give \u00b1145 each even at the best pairing gain this "
        "repository has measured \u2014 2.84\u00d7 wider, which is **8\u00d7 the games** for the "
        "same band. A single-gene arm resolves far tighter per pair once it is aimed "
        "(`s7`: \u00b129 on 6,000 pairs at a 3.32\u00d7 pairing gain, against `p10`'s "
        "1.09\u00d7). So the screen ranks and direct arms resolve the boundary. "
        "`docs/GENE_SCREEN.md` carries the arithmetic; do not re-derive it into a "
        "blocked or partial foldover, which is neither stage \u2014 four-gene `s6` "
        "resolves \u00b164 over 6,000 pairs where one-gene `s7` resolves \u00b129 over the "
        "same 6,000.",
        "",
    ]
    if arm is None:  # pragma: no cover - the ledger has always had one
        lines.append("No single-gene arm in the ledger to size the next one from.")
        return lines
    constant, name = arm
    lines += [
        f"*Buys* is the expected value of one direct arm of **{fmt_int(ARM_PAIRS)} seat "
        "pairs**, in wins per 10,000 on-arm seats, read against the gene's **shipped** "
        "state \u2014 so a gene the evidence likes and the genome already plays has little "
        "to buy, and a gene the evidence likes that the rule holds off has the whole "
        "effect to buy. *Pairs to resolve* is how many matched seat pairs that arm needs "
        "before the combined interval clears zero, if it reads the gene's current pooled "
        f"effect. Both are sized from `{name}`, the widest single-gene arm this "
        f"repository has actually run ({constant / math.sqrt(ARM_PAIRS):.1f} per-column SE "
        f"at {fmt_int(ARM_PAIRS)} pairs) \u2014 the conservative end, since a gene that "
        "rarely fires cancels far more and resolves tighter.",
        "",
        "| Gene | Posterior (95% CI) | P(>0) | Shipped | Buys | Pairs to resolve |",
        "|---|---:|---:|---|---:|---:|",
    ]
    for row in rows:
        needs = row["needs"]
        lines.append(
            f"| `{row['tag']}` | {posterior_cell(row['posterior'])} | "
            f"{probability_cell(row['posterior'])} | "
            f"{'on' if row['shipped'] else 'off'} | {row['buys']:+.1f} | "
            f"{EN_DASH if needs is None else fmt_int(needs)} |"
        )
    feasible = [r for r in rows
                if r["needs"] is not None and 0 < r["needs"] <= FEASIBLE_ARM_PAIRS]
    if feasible:
        lines += [
            "",
            f"The top {min(len(feasible), BOUNDARY_SUGGESTIONS)} that one batch could "
            f"actually resolve (\u2264 {fmt_int(FEASIBLE_ARM_PAIRS)} seat pairs each), as an "
            "argument list:",
            "",
            "```sh",
            "gene_screen --genes "
            + ",".join(r["tag"] for r in feasible[:BOUNDARY_SUGGESTIONS]),
            "```",
            "",
            "`python3 tools/heuristic_gene_ranking.py --boundary` prints this list on its "
            "own, with `--arm-pairs` and `--max-arm-pairs` to size it.",
        ]
    return lines


def lane_section(ledger: dict, measured: dict[str, list[dict]],
                 desc: dict[str, str]) -> list[str]:
    """The lane genes, judged on the axis they can actually pay on."""
    tags = lane_tags()
    verdicts = {g["tag"]: g for g in ledger["genes"]}
    lines = [
        "",
        "## Lane genes and the share axis",
        "",
        "At the standing 250-turn Online clock a **science or congress gene cannot pay "
        "through the win axis at all**: science and diplomatic victories land at median "
        "t283 and t285, past the clock, so they are 1\u20132% of endings and "
        "`docs/VICTORY_GENES.md` records **science 0/8** and **diplomacy 1/8** for exactly "
        "that reason. The seat a lane gene would have carried to a science victory shows "
        "up as a score win or a score loss instead. The decision axis stays WINS \u2014 "
        "`docs/GENOME.md` records what happened the one time selection ran on a correlate "
        "\u2014 so the share reading is a **pre-registered secondary**, fixed in "
        "`docs/GENE_SCREEN.md` before the next screen rather than chosen after it.",
        "",
        "The set is discovered from the code: every gene whose flag field "
        "`src/ai/advanced/victory_lane.rs` reads. A gene joins it by being a lane gene, "
        "not by being listed here.",
        "",
        "| Lane gene | Default | ± Wins / 10k seats | Share Δpp (z) | Posterior (95% CI) | Status |",
        "|---|---|---:|---|---:|---|",
    ]
    for tag in tags:
        gene = verdicts.get(tag, {})
        history = measured.get(tag)
        default = "**on**" if gene.get("default_on") else "off"
        if not history:
            lines.append(
                f"| `{tag}` | {default} | \u2013 | \u2013 | \u2013 | awaiting its first "
                "screen |"
            )
            continue
        posterior = posterior_of(history)
        lines.append(
            f"| `{tag}` | {default} | "
            f"{wins_per(history[-1]['win_on'], history[-1]['players']):+d} | "
            f"{share_cell(history)} | {posterior_cell(posterior)} | "
            f"{verdicts[tag]['verdict']} |"
        )
    return lines


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
        "to least by **± Wins / 10k seats** — wins added per 10,000 six-player on-arm seats at the "
        "gene's measured on-rate in its **latest** screen. *± Wins / 10k seats prior* is the "
        "same figure from the screen before that, and *± Wins / 10k seats third* the one "
        "before that again (\u2013 where the gene has no reading that far back): three "
        "chronological windows, newest first, so every new screen shifts a gene's readings "
        "one column right and drops the fourth-oldest off the table. Movement across the "
        "three is the gene's trend, and it is the column the two-column rule cannot see \u2014 "
        "a pair of positives that is the tail of a decline reads the same as one that is a "
        "rise until the third window is printed beside it. **The third column is published, "
        "not in force**: the rule below reads the first two and nothing else. "
        "*Default* is the deployment ledger's call (`docs/gene_ledger.json`), and since "
        "2026-08-22 that call is read off the first two win columns: a gene defaults **on** "
        "when both are positive, or when their average clears +15 with neither below "
        "\u221210; with exactly one populated column it defaults **on** when that reading "
        "is above +20. It defaults **off** otherwise. The *Total* win-rate "
        "columns pool every screen that measured the gene, weighted by on-arm seats, and "
        "each carries its own on-arm seat count `n` — the two arms are only equal when every "
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
        "count is 1,667 wins per 10,000 on-arm seats and the "
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
        "**Posterior (95% CI), P(>0), Share Δpp (z).** *Posterior* is a random-effects "
        "(DerSimonian\u2013Laird) inverse-variance pool of **every** screen that priced the "
        "gene, on this column's own scale: each screen's on\u2212off difference weighted by "
        "its own standard error, with the between-screen disagreement (\u03c4) carried in the "
        "interval instead of assumed away. It is the answer to two things the columns "
        "cannot express \u2014 that the same +24 means different things from a \u00b129 screen "
        "and a \u00b164 one, and that two positive columns from screens differing in "
        "baseline, build and shape are not two confirmations (#2283/#2284 measured that: "
        "five of seven lane genes changed sign on disjoint seeds). *P(>0)* is where the "
        "shrinkage lands. *Share Δpp (z)* is the newest screen's score-share contrast and "
        "its verdict, published beside the win columns because the deployment rule reads "
        "the win axis only and a lane gene cannot pay on that axis at 250 turns. **None "
        "of these three decides anything today**; `AUTHORITY` in `tools/gene_ledger.py` "
        "says `columns` and *What the posterior would change* above is the delta.",
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
        "| Rank | Gene | Description | Default | ± Wins / 10k seats | ± Wins / 10k seats prior | ± Wins / 10k seats third | Total (on) Win rate | Total (off) Win rate | Diff | Posterior (95% CI) | P(>0) | Share Δpp (z) | cost (compute) | cost (time) |",
        "|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|",
    ]
    for rank, (wins, tag, history) in enumerate(rows, 1):
        v = verdict.get(tag, {})
        default = "**on**" if v.get("default_on") else "off"
        # The window columns, newest first. Each new screen that prices a gene
        # shifts its predecessor one place right, so `third` is the reading
        # before the two the ledger's rule is taken on and the fourth-oldest
        # falls off the table (operator request 2026-08-23).
        def window(back: int) -> str:
            if len(history) <= back:
                return "\u2013"
            screen = history[-1 - back]
            return f"{wins_per(screen['win_on'], screen['players']):+d}"

        prior, third = window(1), window(2)
        on_seats = sum(m["n_on"] for m in history)
        off_seats = sum(m["n_off"] for m in history)
        on_rate, off_rate = pooled_win_rates(history)
        posterior = posterior_of(history)
        lines.append(
            f"| {rank} | `{tag}` | {desc.get(tag, '')} | {default} | {wins:+d} | {prior} | "
            f"{third} | "
            f"{100 * on_rate:.2f}% (n={fmt_int(on_seats)}) | "
            f"{100 * off_rate:.2f}% (n={fmt_int(off_seats)}) | "
            f"{diff_cell(history)} | "
            f"{posterior_cell(posterior)} | {probability_cell(posterior)} | "
            f"{share_cell(history)} | "
            f"{cost_cell(history, 'compute_cost_pct', 'compute_cost_se_pct')} | "
            f"{cost_cell(history, 'time_cost_pct', 'time_cost_se_pct')} |"
        )

    lines += posterior_sections(ledger, measured, desc)

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
            "| Gene | Wins ±/10k seats (last tracked measurement) | Win rate (on) | Win rate (off) | Source |",
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
        "this table is the operator's wins-per-ten-thousand-seat view of the same observations._",
        "",
    ]
    return "\n".join(lines)


def print_boundary(ledger: dict, arm_pairs: int, max_arm_pairs: int) -> None:
    """The boundary set on stdout, ending in a `--genes` list to paste.

    This is the second stage of the two-stage plan: the whole-genome screen
    ranks (75 genes at ±51 each for the budget that would give ±146 apiece
    split up), and these arms resolve what the ranking left straddling zero."""
    measured, _ = load_sources(ledger)
    rows, arm = boundary_table(ledger, measured, arm_pairs)
    if arm is None:  # pragma: no cover - the ledger has always had one
        print("no single-gene arm in the ledger to size the next one from")
        return
    constant, name = arm
    print(f"boundary genes · {len(rows)} intervals straddle zero · one direct arm of "
          f"{arm_pairs:,} seat pairs resolves ±{POWER_80 * constant / math.sqrt(arm_pairs):.0f} "
          f"(sized from {name})")
    print(f"{'gene':<32} {'posterior':>20} {'P>0':>7} {'ships':>6} {'buys':>7} "
          f"{'pairs to resolve':>17}")
    for row in rows:
        needs = row["needs"]
        print(f"{row['tag']:<32} {posterior_cell(row['posterior']):>20} "
              f"{probability_cell(row['posterior']):>7} "
              f"{'on' if row['shipped'] else 'off':>6} {row['buys']:>+7.1f} "
              f"{'–' if needs is None else format(needs, ',d'):>17}")
    feasible = [r for r in rows
                if r["needs"] is not None and 0 < r["needs"] <= max_arm_pairs]
    print()
    if not feasible:
        print(f"nothing resolves inside {max_arm_pairs:,} seat pairs; the next "
              "whole-genome screen is the cheaper instrument for all of these")
        return
    print(f"# the {min(len(feasible), BOUNDARY_SUGGESTIONS)} best that one arm of "
          f"≤{max_arm_pairs:,} seat pairs resolves:")
    print("--genes " + ",".join(r["tag"] for r in feasible[:BOUNDARY_SUGGESTIONS]))


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--write", action="store_true", help="rewrite HEURISTIC_GENE_RANKING.md")
    ap.add_argument("--check", action="store_true", help="fail if the table is stale")
    ap.add_argument("--boundary", action="store_true",
                    help="list the genes whose posterior interval straddles the decision "
                         "line, ranked by what one single-gene direct arm would buy, and "
                         "print them as a `gene_screen --genes` argument list")
    ap.add_argument("--arm-pairs", type=int, default=ARM_PAIRS,
                    help=f"matched seat pairs in the direct arm being priced "
                         f"(default {ARM_PAIRS:,}, the size `2026-08-22-h1` played)")
    ap.add_argument("--max-arm-pairs", type=int, default=FEASIBLE_ARM_PAIRS,
                    help=f"largest arm worth proposing (default {FEASIBLE_ARM_PAIRS:,}, "
                         "one standard batch)")
    args = ap.parse_args(argv)
    ledger = json.loads(LEDGER_JSON.read_text())
    if args.boundary:
        print_boundary(ledger, args.arm_pairs, args.max_arm_pairs)
        return 0
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
