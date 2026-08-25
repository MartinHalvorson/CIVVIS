#!/usr/bin/env python3
"""⭐ THE ONE GENE TOOL: the registry, the ledger and the ranking.

Vocabulary: the GENE POOL is the collection of all genes, on or off — the
registry, `src/ai/advanced/genes.rs`. A GENOME is one player's set of on
genes, a subset of the pool; the deployment genome is the ledger's defaults.

    python3 tools/genes.py list                 every gene: kind, default, verdict
    python3 tools/genes.py source FILE [...]    enter a `gene_screen --analyze --json` file as a ledger source
    python3 tools/genes.py write                regenerate docs/gene_ledger.json, the verdict block in
                                                src/ai/advanced/genes.rs, and HEURISTIC_GENE_RANKING.md
    python3 tools/genes.py check                fail if any of the three is stale (the CI gate)
    python3 tools/genes.py boundary [--arm-pairs N] [--max-arm-pairs N]
                                                the genes one single-gene run would resolve
    python3 tools/genes.py table                print the ledger as a table

Three things used to be three tools — `gene_registry.py` (read `genes.rs`),
`gene_ledger.py` (turn screens into verdicts and defaults) and
`heuristic_gene_ranking.py` (render the table) — coupled by imports and held
together by tests. They are one module now (operator, 2026-08-23: *"is it
possible to combine this all into one file?"*), and the generated Rust
verdicts live in `genes.rs` itself, under the rows they judge, so a gene's
declaration and its standing are one file: `src/ai/advanced/genes.rs` for the
code, `HEURISTIC_GENE_RANKING.md` for the table, `docs/gene_ledger.json` for
the machine record of the screens behind them.

The sections below keep the three tools' own doctrine, verbatim where it still
holds.

──────────────────────────────────────────────────────────────────────────────
THE REGISTRY READER
──────────────────────────────────────────────────────────────────────────────
Read the gene registry — `src/ai/advanced/genes.rs` — without a Rust toolchain.

⭐ ONE REGISTRY, ONE READER. Every gene is declared once, as a row of `GENES`:

    Gene { tag: "war-economy", field: "war_economy", kind: Kind::OptIn,
           enable: AdvancedAi::enable_war_economy, disable: AdvancedAi::disable_war_economy },

and every Python tool that needs the list — the ledger, the ranking, the
manifest, the fires check — reads it through here, so the rule for "what is a
gene and what kind" is written once in each language. The Rust side pins its
half in `gene_screen.rs` (`tags_from_source_tables`) against the compiled
table; this side is held to the same answer by `tools/test_genes.py`.

The kinds are the registry's own (`Kind` in `genes.rs`):

| kind            | live | host-only | repair | production | opt-in | screenable |
|-----------------|------|-----------|--------|------------|--------|------------|
| `Repair(axis)`  | yes  | no        | yes    | no         | no     | yes        |
| `HostOnly`      | yes  | yes       | no     | no         | no     | no         |
| `HostOnlyOptIn` | yes  | yes       | no     | no         | yes    | yes        |
| `Production`    | no   | no        | no     | yes        | no     | yes        |
| `OptIn`         | no   | no        | no     | no         | yes    | yes        |

⚠ History reads too. A screen recorded before 2026-08-23 names a commit where
the registry was three tuple tables (`LIVE_TREATMENTS`, `PRODUCTION_TREATMENTS`,
`PRODUCTION_OPT_INS` in `treatments.rs`) plus tag lists in `src/elo.rs`
(`ENGINE_REPAIR_TREATMENTS`, …). `screenable_tags_from(read)` reads either
shape, so the ledger can still re-derive an old batch's gene set at the commit
it names.

──────────────────────────────────────────────────────────────────────────────
THE LEDGER (formerly tools/genes.py)
──────────────────────────────────────────────────────────────────────────────
Build the gene ledger — what the screen says about every gene, and the
deployment genome that follows — from `gene_screen --analyze --json` outputs.

Operator directive 2026-08-22: **one screen**. Six majors on 74x46 continents
with nine city-states, Online speed to its own 250-turn clock, all six victory
lanes, every seat carrying its own drawn genome against the best-genome
baseline. There is no second regime to reconcile: a batch played at another
shape is a probe, and this tool refuses it as a source rather than pooling two
worlds into one column.

The deployment default is explicitly operator-pinned. `OPERATOR_DEFAULT_ON`
names the screenable tags that ship; every other screenable tag is off. Win
columns, pooled *Diff*, posterior values, and verdicts remain evidence, but
none changes a default automatically. This tool records the pinned list beside
that evidence and validates it against the generated Rust table:

    python3 tools/genes.py write \\
        --source docs/gene_screens/<screen>.json \\
        --source docs/gene_screens/<newer-screen>.json

writes `docs/gene_ledger.json` and the generated Rust block in
`src/ai/advanced/genes.rs`, which `AdvancedAi::apply_gene_ledger` reads to
withhold unselected screenable treatments and enable selected opt-ins.
`--check` re-derives the measurements from recorded sources while separately
requiring the recorded pinned list to match `OPERATOR_DEFAULT_ON`; the same
check is `tools/test_genes.py`'s `GeneratedFiles` suite.

Deployment policy (mirrored in `src/ai/advanced/gene_ledger.rs`):

- A tag is on exactly when it appears in `deployment_genome`.
- The list must contain no duplicate, unknown, or unscreenable tag, and may
  select at most one version from a versioned family.
- Win columns, *Diff*, posterior, and verdict data are published evidence for
  a later explicit operator selection; they are not fallback rules.

⭐ A SOURCE PROVES IT PRICED THE CODE IT NAMES (2026-08-23). Beside the shape
guard there is now a build guard, and it is the same idiom: a source is
refused at the `--source` path, and one explicit flag records a deviation
deliberately. `gene_screen` stamps every header with the commit its binary was
built from, whether that tree was dirty, a sha256 of the executable, and a
sha256 of the **gene set compiled into it**. This tool re-derives the gene tags
from the gene registry (`src/ai/advanced/genes.rs`) at the commit the source
claims, and refuses the source
when the two disagree in either direction — a gene priced here and absent
there, or a gene present there and never compiled in here. It also refuses an
unstamped build, a dirty one, a commit this clone cannot read, an artefact
whose stamp does not describe its own header, and a source pricing a gene the
repository no longer registers. `--unverified-build "<why>"` records one
anyway, and the reason is written into the ledger beside the source it excuses.

⚠⚠ THIS HAS COST THE PROJECT THREE TIMES, which is why the fingerprint and not
the commit is the load-bearing field:

- **2026-08-22, P10.** #2266 culled ten genes; P10's binary was built 1h43m
  before that merge, so the batch was in flight and published a **+63** column
  for `holy-lane-parity` after the gene's code was gone. The reading was real,
  the gene came back (#2299) and confirmed directly at **+99, z +4.05**
  (#2307) — found by a careful reader, not by a gate.
- **#2307's own write-up** stated its source commit and its binary's SHA-256 in
  prose, because the artefact had nowhere to put them.
- **2026-08-23.** The first standard-shape screen re-priced `barbarian-hunt`
  from the legacy -1.73 pp to +0.20 pp while a sibling change was minutes from
  deleting that gene on the legacy reading, which would have made a brand-new
  screen a source pricing a gene the code no longer had.

⚠ The twenty sources recorded before 2026-08-23 carry no build block. They are
grandfathered — the games are played and the artefacts are history — and they
are **named** `pre-fingerprint` everywhere this tool prints or records them,
because a grandfather clause nobody can see is the same as no guard. A source
that carries a block is checked; the absence of one is a fact about the file's
age, not a way past the guard.

⭐ AND A SCREEN DECLARES ITS SIZE BEFORE IT PLAYS. `gene_screen` writes the
games it was launched for into the header, so the seats it played can be read
against an intention rather than against nothing. P10 "ended early at the
operator's request" at 5,858 of a planned 10,000 games: a legitimate decision
that left an artefact indistinguishable from a completed screen. A partial
source now says so in the analysis, in this tool's table, and in the ledger.

⚠ The columns recorded before 2026-08-22 were read on 60x38 Pangaea, under a
four-player `domination,score` regime for some genes. The Pangaea readings are
kept as HISTORY and marked `"shape": "legacy"` in `sources`; they remain
evidence beside the pinned deployment selection. The war-regime readings are
gone because their four-player 1-in-4 chance base made their columns
incomparable with the six-player evidence printed beside them.

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

★★★★ THE PRECISION-WEIGHTED POSTERIOR, PUBLISHED BESIDE THE PINNED GENOME
===========================================================================
The former threshold and veto rules showed why a single score statistic is not
enough to choose a deployment default: screens resolve at different precision,
their baselines and shapes vary, and a pooled record must carry uncertainty.

`pooled_posterior` answers all three with one estimator: a random-effects
(DerSimonian-Laird) inverse-variance pool of every screen's on-off difference
on the win column's own scale, each weighted by that screen's own standard
error, with the between-screen disagreement carried in `tau` and therefore in
the interval. Every gene gets `posterior_pp`, `posterior_se_pp` and, in
`HEURISTIC_GENE_RANKING.md`, a 95% interval and `P(effect > 0)`.

It is **published, not in force**. The deployment policy is explicitly
`operator-pinned`: `OPERATOR_DEFAULT_ON` is the whole selection, and the
ledger records that list beside the evidence. Rust validates the same list, so
screen statistics cannot silently rewrite what the agent plays.

The posterior remains useful evidence: it makes uncertainty and disagreement
visible when an operator considers changing the pinned list. It is not an
alternative authority or a switch for automatic promotion or demotion.

──────────────────────────────────────────────────────────────────────────────
THE RANKING (formerly tools/genes.py)
──────────────────────────────────────────────────────────────────────────────
Regenerate `HEURISTIC_GENE_RANKING.md` — every screenable heuristic gene,
with a measurement, ranked by wins added per 10,000 six-player on-arm seats, plus the
screenable genes still awaiting one.

    python3 tools/genes.py write                      # rewrite the table (and the ledger)
    python3 tools/genes.py check                      # fail if it is stale

The table used to be written once, by hand, from one screen's rows. Now it is
derived: for each gene the **latest source** in `docs/gene_ledger.json` that
measured it supplies the on/off wins and seat counts (so a gene added after the
whole-genome screen still appears, from its own screen), and the pinned
deployment state comes from the ledger. Every source is the one screen the ledger
accepts — the war regime's four-player columns are gone, and the Pangaea
screens are marked `legacy` as historical evidence. Screenable genes with no result are listed separately
without a rank. Genes whose code has been removed this cycle are listed from
their last measurement, as before. Descriptions are the first sentence of each toggle's
doc comment in `src/ai/advanced/treatment_flags.rs`. Hand-written follow-ups
go in `docs/gene_ranking_notes.md` and are carried under the table.

`tools/test_genes.py` holds the file to the sources, so the
ranking cannot quietly fall behind the ledger.

Beside the operator's two win columns the table publishes a **precision-weighted
posterior** — `gene_ledger.pooled_posterior`, a random-effects inverse-variance
pool of every screen that priced the gene — with its 95% interval and
`P(effect > 0)`, the newest screen's **score-share** reading and verdict, the
pinned deployment set, the two shapes apart, the boundary genes ranked by
what one direct arm would buy, and the lane genes on the axis they can actually
pay on. **None of it decides a default**: the ranking publishes the evidence so
the operator can take a future call on numbers, while `OPERATOR_DEFAULT_ON`
keeps the current selection stable.

    python3 tools/genes.py boundary                   # the next round's --genes list
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent

REGISTRY = "src/ai/advanced/genes.rs"
REGISTRY_PATH = ROOT / REGISTRY

# The three tables and the tag list of the pre-2026-08-23 registry, for
# commits that predate `genes.rs`.
LEGACY_TABLES = (
    ("src/elo.rs", "ENGINE_REPAIR_TREATMENTS", 0),
    ("src/ai/advanced/treatments.rs", "PRODUCTION_TREATMENTS", 1),
    ("src/ai/advanced/treatments.rs", "PRODUCTION_OPT_INS", 1),
)

ROW = re.compile(
    r'Gene\s*\{\s*tag:\s*"([a-z0-9-]+)"\s*,\s*field:\s*"([a-z0-9_]+)"\s*,\s*kind:\s*'
    r'(Kind::[A-Za-z]+(?:\(Axis::[A-Za-z]+\))?)\s*,\s*enable:\s*AdvancedAi::enable_([a-z0-9_]+)'
    r'\s*,\s*disable:\s*AdvancedAi::disable_([a-z0-9_]+)'
)


@dataclass(frozen=True)
class Gene:
    tag: str
    field: str
    kind: str
    toggle: str

    @property
    def axis(self) -> str | None:
        match = re.search(r"Axis::([A-Za-z]+)", self.kind)
        return match.group(1) if match else None

    @property
    def live(self) -> bool:
        return self.kind.startswith("Kind::Repair") or self.kind in ("Kind::HostOnly", "Kind::HostOnlyOptIn")

    @property
    def host_only(self) -> bool:
        return self.kind in ("Kind::HostOnly", "Kind::HostOnlyOptIn")

    @property
    def repair(self) -> bool:
        return self.kind.startswith("Kind::Repair")

    @property
    def production(self) -> bool:
        return self.kind == "Kind::Production"

    @property
    def opt_in(self) -> bool:
        return self.kind in ("Kind::OptIn", "Kind::HostOnlyOptIn")

    @property
    def universe_on(self) -> bool:
        return self.repair or self.production

    @property
    def screenable(self) -> bool:
        return self.universe_on or self.opt_in


def _uncommented(text: str) -> str:
    """`text` with `/* */` and `//` comments removed, so a tag mentioned in a
    comment cannot join the table."""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", "", text)


def _table_body(text: str, name: str) -> str:
    """The body of `pub const <name>: … = &[ … ];`, brackets balanced."""
    start = text.index(f"pub const {name}")
    open_at = text.index("= &[", start) + 4
    depth, index = 1, open_at
    while depth:
        if text[index] == "[":
            depth += 1
        elif text[index] == "]":
            depth -= 1
        index += 1
    return text[open_at:index - 1]


def genes_from_text(text: str) -> list[Gene]:
    """Every row of `GENES` in `text`, in registry order."""
    body = _uncommented(_table_body(text, "GENES"))
    rows = [Gene(tag, field, kind, toggle) for tag, field, kind, toggle, _ in ROW.findall(body)]
    if not rows:
        raise ValueError("genes.rs yielded no rows; the scrape broke rather than finding an empty registry")
    return rows


def genes() -> list[Gene]:
    """The registry in the working tree."""
    return genes_from_text(REGISTRY_PATH.read_text(encoding="utf-8"))


def gene(tag: str) -> Gene | None:
    return next((row for row in genes() if row.tag == tag), None)


def screenable_tags_from(read) -> list[str]:
    """The gene tags a `gene_screen` binary compiles in, in header order, from
    whatever `read(path)` supplies: the registry when the commit has one,
    else the three tables it replaced. Exactly what `gene_table()` builds —
    `gene_screen.rs`'s
    `the_gene_table_is_exactly_what_the_ledger_re_derives_from_the_tables`
    holds the Rust half of this rule against the compiled table."""
    try:
        text = read(REGISTRY)
    except LookupError:
        text = None
    if text is not None:
        return [row.tag for row in genes_from_text(text) if row.screenable]
    tags: list[str] = []
    cache: dict[str, str] = {}
    for path, name, offset in LEGACY_TABLES:
        if path not in cache:
            cache[path] = read(path)
        found = re.findall(r'"([^"\\]*)"', _uncommented(_table_body(cache[path], name)))
        tags += found[offset::2] if offset else found
    return tags


def screenable_tags() -> list[str]:
    """The screenable tags in the working tree, in header order."""
    return [row.tag for row in genes() if row.screenable]


def known_tags() -> set[str]:
    """Every tag the registry declares, screenable or not."""
    return {row.tag for row in genes()}


LEDGER_JSON = ROOT / "docs" / "gene_ledger.json"
#: The generated verdict block lives INSIDE the registry file (`REGISTRY_PATH`),
#: after `GENERATED_BEGIN`; `render_rust` renders it and `write` rewrites it.
#: ⭐ THE SCREEN, leg by leg — the profile a `gene_screen` header must carry to
#: enter this ledger. `src/bin/gene_screen.rs` plays exactly this on its bare
#: defaults (`SCREEN_PLAYERS` and friends); the two are held together by
#: `tools/test_genes.py`, so neither can drift alone.
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
    "baseline": "best",
}
#: ⭐ THE LEGS THAT SAY THE BOARD WAS FIELDLESS, and their fieldless values.
#: Deliberately NOT part of `SCREEN`: `SCREEN` is copied verbatim into
#: `docs/gene_ledger.json` and its keys are the recorded profile of every
#: source, so adding one there would rewrite nine historical records that were
#: written before the leg existed and `check` would report drift on the ledger's
#: own history.
#:
#: `gene_screen --contested` pins rival seats to actually pursue a victory lane
#: and turns on `Game::native_competitions`, and a batch played that way differs
#: from the standard screen in NO map leg at all -- same players, map, size,
#: city-states, speed, clock, lanes and civ shuffle. Without this check it would
#: read `standard` and pool with the ledger, re-pricing a hundred genes against
#: a board none of them was measured on. `profile_of` records these two only
#: when they are set, so every existing record reproduces byte for byte.
#:
#: ⚠ `contested_field`, not `field`. Every header the retired paired designs
#: wrote already carries a `field` -- the name of the agent the treated seat
#: played against, `"advanced"` -- so calling the new leg `field` rewrote nine
#: historical source records and `check` reported drift on the ledger's own
#: history. That is the gate working; the name is the fix.
FIELDLESS = {
    "contested_field": "",
    "native_competitions": False,
}
#: ⭐ PROVENANCE RECORDED WHEN SET, AND NOT A SHAPE LEG. `gene_screen
#: --victory-mask rotate:N` closes N of the five real conditions per game from
#: the game's seed, score always on; `victories` in its header is still the
#: batch-level set (all six) and every lane is live across the batch, so the
#: batch is the standard shape and pools with the ledger. The mask is written
#: onto the source so a reader can see it, exactly as `FIELDLESS` is recorded
#: only when set, and `shape_of` never reads it.
#: `difficulty` / `difficulty_rotate` are the majors' rung (`--difficulty`,
#: `--difficulty-rotate king:1,emperor:2,immortal:1`), recorded the same way:
#: every screen before 2026-08-25 played the engine's Prince default and
#: wrote nothing, and a batch that names its rung says so on the source.
#: `rivals` is the rival mix (`--rivals firaxis-mix`): one chair per game
#: plays a fixed, unmeasured opponent. Recorded the same way.
RECORDED_WHEN_SET = ("victory_mask", "difficulty", "difficulty_rotate", "rivals")
#: The profile keys recorded for every source, whether or not they match. The
#: draw `design` is recorded and NOT checked: it is how each seat's genome was
#: sampled (`independent` — every seat its own draw, the screen since
#: 2026-08-23 — or the earlier paired `foldover` / `prior`), not a leg of the
#: board, and the estimator reads every design the same way: seats with the
#: gene on against seats with it off.
PROFILE_KEYS = tuple(SCREEN) + ("design", "start_seed")
#: ⭐ THE BUILD A SOURCE WAS PLAYED BY, leg by leg — the keys
#: `src/bin/gene_screen.rs`'s `Build` writes into every header. Held together
#: with the Rust struct by `tools/test_genes.py`, so a field added on one
#: side and forgotten on the other fails a test instead of reaching the ledger.
BUILD_KEYS = ("commit", "commit_source", "dirty", "genes_sha256", "binary_sha256")
#: The same, for `Batch` — what the screen was launched to play.
BATCH_KEYS = ("target_games", "target_seats", "target_pairs", "target_comparisons",
              "seed_first", "seed_last")
#: Where the gene tags live, in the order `gene_screen`'s `gene_table()` builds
#: them: the registry, `src/ai/advanced/genes.rs`, read by `py` —
#: and, for a commit older than the registry (before 2026-08-23), the three
#: tables that preceded it.
GENE_TABLES = ((REGISTRY, "GENES", None),) + LEGACY_TABLES
#: The date the build stamp landed. A source written before it carries no
#: `build` block at all; see `build_state`.
FINGERPRINT_SINCE = "2026-08-23"
Z_BAR = 2.0
# The win column's scale. `DIFF_PLACES` is the recorded precision of the
# observational on/off difference and posterior, not a deployment threshold.
PER = 10_000
DIFF_PLACES = 6

#: ⭐ THE DEPLOYMENT GENOME IS OPERATOR-PINNED. Screens still publish their
#: win columns, pooled difference, posterior, and verdict, but none of those
#: metrics automatically changes what ships. Updating this tuple is the one
#: deliberate deployment-selection action; `python3 tools/genes.py write`
#: records it in the JSON ledger and emits it for Rust alongside the measured
#: rows.
DEPLOYMENT_POLICY = "operator-pinned"

#: The sixteen explicit 2026-08-24 promotions. They supplement the exact
#: 36-gene selection already shipped at the 38,160-seat standard cutoff.
#: The final seven are the later operator-selected promotions; they remain
#: explicit here rather than being inferred from screen statistics.
OPERATOR_PROMOTIONS_20260824 = (
    "unit-cost-efficiency",
    "unit-objective-memory",
    "camp-party",
    "slot-kind-tiebreak",
    "promote-when-wounded",
    "religion-sues-peace",
    "lane-great-people",
    "one-launch-pad",
    "civilian-rescue",
    "missionary-evades-raiders",
    "district-planning",
    "missionary-last-charge-explores",
    "settlement-gap-target",
    "religious-defence-scales",
    "lane-policy-deck",
    "science-multiplier-payoff",
)

#: The later 2026-08-25 operator-pinned additions. `science-victory-drive`
#: was pinned on before its first screen: "default this gene to true initially
#: once you write and merge it. i'll test it more later." The four following
#: tags were explicitly promoted from the displayed pooled-Diff ranking at the
#: +0.85 percentage-point cutoff; this remains a deliberate selection rather
#: than a rule that lets a later screen rewrite the deployment genome.
OPERATOR_PROMOTIONS_20260825 = (
    "science-victory-drive",
    "solvency-first-trade-slot",
    "settler-factory-coordination",
    "one-war-at-a-time",
    "religious-veto-defence",
)

#: The complete pinned deployment genome, in stable tag order. Every other
#: screenable gene defaults off unless an explicit operator update changes this
#: selection. Keep the historical promotion groups above named separately so
#: the policy change is auditable without re-deriving it from screen statistics.
OPERATOR_DEFAULT_ON = (
    "air-surge",
    "amenity-district-path",
    "apostle-promotion-by-role",
    "barbarian-bargain",
    "barbarian-scouts-are-scouts",
    "bounded-recovery",
    "buildings-before-projects",
    "camp-party",
    "civilian-rescue",
    "competition-victory-points",
    "culture-building-debt",
    "district-planning",
    "early-contact-window",
    "engine-faith-price",
    "escort-unstick",
    "founder-temple",
    "great-person-housing",
    "holy-lane-parity",
    "idle-faith-patronage",
    "inquisition-on-threat",
    "lane-culture-spending",
    "lane-great-people",
    "lane-policy-deck",
    "loyalty-rate-alarm",
    "maintenance-aware-deck",
    "missionary-evades-raiders",
    "missionary-last-charge-explores",
    "one-launch-pad",
    "one-war-at-a-time",
    "opportunistic-war",
    "peacetime-deterrence",
    "price-the-suzerainty",
    "promote-when-wounded",
    "raid-pillage-prizes",
    "recon-replacement",
    "recorded-tactical-step",
    "relief-targets-the-siege",
    "religion-sues-peace",
    "religious-defence-scales",
    "religious-units-heal-first",
    "religious-veto-defence",
    "science-multiplier-payoff",
    "science-victory-drive",
    "score-horizon",
    "settle-sooner",
    "settlement-gap-target",
    "settler-factory-coordination",
    "settler-threat-detour",
    "slot-kind-tiebreak",
    "solvency-first-trade-slot",
    "strike-opening",
    "theology-for-founders",
    "unit-cost-efficiency",
    "unit-objective-memory",
    "war-economy",
    "war-reinforcement",
    "wide-map-capacity",
)
#: Which source shapes the published posterior pools. Both today, because every
#: source is `legacy`; the moment a `standard` source lands this is the dial
#: that says whether the deployment shape is pooled with the retired one or
#: reads alone. `HEURISTIC_GENE_RANKING.md` prints all three scopes side by
#: side so the choice is made on the numbers.
POSTERIOR_SHAPES = ("standard", "legacy")
#: A two-sided 95% interval, and the standard normal's own constant.
Z95 = 1.959963984540054
#: A two-sided 5% test reaches 80% power at 1.96 + 0.84 standard errors.
#: `tools/genes.py` prints each screen's band from it.
POWER_80 = 2.8


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
    """One screen's win column: wins added per 10,000 on-arm seats at this
    measured on-rate. A seat wins 1-in-`players` by chance (1-in-6 when a fixture does
    not say), so the column is how far above or below that the gene's on arm
    landed. `tools/genes.py` imports this, so the table's
    printed column and the ledger's evidence record are one arithmetic."""
    chance = 1.0 / players if players else 1.0 / 6.0
    return round((win_rate - chance) * PER)


def pooled_win_rates(history: list[dict]) -> tuple[float, float]:
    """The on-arm-seat-weighted on and off win rates across every screen that priced
    the gene — `HEURISTIC_GENE_RANKING.md`'s two *Total* columns. Each entry
    carries `win_on`/`win_off` and the seat observations behind each arm.
    `tools/genes.py` imports this, so the printed totals and
    the ledger's published *Diff* are one arithmetic."""
    on_seats = sum(m["n_on"] for m in history)
    off_seats = sum(m["n_off"] for m in history)
    on = sum(m["win_on"] * m["n_on"] for m in history) / on_seats
    off = sum(m["win_off"] * m["n_off"] for m in history) / off_seats
    return on, off


def pooled_win_diff_pp(history: list[dict]) -> float:
    """The ranking's *Diff*: the pooled on rate minus the pooled off rate, in
    percentage points, rounded to what the ledger records. This is the **whole**
    on-off difference, twice the scale of a win column beside it."""
    on, off = pooled_win_rates(history)
    return round(100 * (on - off), DIFF_PLACES)


# ---------------------------------------------------------------------------
# The precision-weighted posterior.
#
# ★★★★ A POOLED ESTIMATE IS EVIDENCE, NOT A DEPLOYMENT RULE. What follows
# prices a gene the way the evidence arrives: each screen's own estimate,
# weighted by its own precision, with disagreement between screens carried in
# the interval instead of assumed away. The result informs an explicit
# operator selection; it never selects one by itself.
# ---------------------------------------------------------------------------


def column_estimate(win_delta_pp: float) -> float:
    """One screen's on-off difference, on the win column's own scale.

    A foldover holds the two arms symmetric about chance, so the column
    `(win_on - chance) * PER` is exactly **half** the on-off difference. This
    is that half, unrounded, and it is the quantity `win_se_pp` measures the
    error of - so `column_estimate / column_se` reproduces the screen's own
    `win_z` exactly.

    ⚠ The two coincide only while the arms are symmetric, which every
    all-seats foldover source in the ledger is. The one exception is the
    single-seat four-player probe `2026-08-20-s2`, whose treated seat sits
    860 wins/10k below chance in BOTH arms: there the printed column is a
    statement about the seat and the difference is the statement about the
    gene. The posterior reads the difference, because that is what the
    screen's standard error belongs to."""
    return win_delta_pp * PER / 200.0


def column_se(win_se_pp: float) -> float:
    """One `wins_per_10k` column's standard error, in the column's own units.

    A screen reports `win_se_pp`: the error on the on-off **difference**, in
    percentage points. The column is half that difference and carries half
    its error. The two are not interchangeable, and quoting one against the
    other is not a rounding error: the +/-110/10k band #2266 called eight
    removals "inside" is the difference's band, twice the width of the column
    it was read against (#2300). It lives here beside the `wins_per_10k` it
    halves, so the printed band, the printed column and the evidence record
    stay one arithmetic;
    `tools/genes.py` imports it."""
    return win_se_pp * PER / 200.0


def normal_cdf(x: float) -> float:
    """The standard normal CDF, from the C library's `erf`."""
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def normal_pdf(x: float) -> float:
    return math.exp(-0.5 * x * x) / math.sqrt(2.0 * math.pi)


def screen_readings(history: list[dict],
                    shapes: tuple[str, ...] | None = None) -> list[tuple[float, float]]:
    """Every screen's `(estimate, standard error)` in column units, for the
    entries that carry both. A source that recorded no `win_se_pp` cannot be
    weighted and is skipped rather than given a made-up precision. `shapes`
    restricts the pool to sources played at those shapes."""
    readings = []
    for measurement in history:
        if shapes is not None and measurement.get("shape") not in shapes:
            continue
        delta, se = measurement.get("win_delta_pp"), measurement.get("win_se_pp")
        if delta is None or se is None:
            continue
        delta, se = float(delta), float(se)
        if not (math.isfinite(delta) and math.isfinite(se)) or se <= 0:
            continue
        readings.append((column_estimate(delta), column_se(se)))
    return readings


def pooled_posterior(history: list[dict],
                     shapes: tuple[str, ...] | None = None) -> dict | None:
    """Pool every screen that priced a gene, weighted by its own precision.

    **Random effects, DerSimonian-Laird.** With `k` readings `yᵢ` carrying
    standard errors `sᵢ`:

        wᵢ  = 1 / sᵢ²                         inverse-variance weights
        ȳ   = Σ wᵢ yᵢ / Σ wᵢ                  the fixed-effect mean
        Q   = Σ wᵢ (yᵢ − ȳ)²                  the heterogeneity statistic
        C   = Σ wᵢ − Σ wᵢ² / Σ wᵢ
        τ²  = max(0, (Q − (k − 1)) / C)       between-screen variance
        wᵢ* = 1 / (sᵢ² + τ²)                  precision, heterogeneity included
        m   = Σ wᵢ* yᵢ / Σ wᵢ*                the pooled effect
        se  = sqrt(1 / Σ wᵢ*)

    `τ²` is the whole point of the random-effects form and the reason a fixed
    effect would be dishonest here. The screens differ in baseline, in build
    and in shape, so they are not `k` draws from one number; when they
    disagree by more than their errors allow, `Q` is large, `τ²` absorbs it
    and the interval widens. A fixed-effect pool would instead report a
    narrow interval around the average of two irreconcilable instruments.

    With one reading the pool IS that reading (`τ² = 0`, `Q = 0`) and all the
    work is done by the interval: a +30 from a screen resolving +/-64 and a
    +30 from one resolving +/-29 print the same point and utterly different
    `p_positive`.

    `shapes` pools only the sources played at those shapes. That is not a
    nicety: pooling a `standard` reading with a `legacy` one is exactly the
    case `τ²` is built to expose, and when the two instruments disagree by
    more than their errors allow the honest published answer is the two
    shapes apart, not one average of two worlds.

    Returns `None` when no reading carries an error. Units are the win
    column's: wins added per 10,000 on-arm seats."""
    readings = screen_readings(history, shapes)
    if not readings:
        return None
    k = len(readings)
    weights = [1.0 / (se * se) for _, se in readings]
    total = sum(weights)
    fixed = sum(w * y for w, (y, _) in zip(weights, readings)) / total
    q = sum(w * (y - fixed) ** 2 for w, (y, _) in zip(weights, readings))
    c = total - sum(w * w for w in weights) / total
    tau2 = max(0.0, (q - (k - 1)) / c) if c > 0 else 0.0
    adjusted = [1.0 / (se * se + tau2) for _, se in readings]
    total_adjusted = sum(adjusted)
    effect = sum(w * y for w, (y, _) in zip(adjusted, readings)) / total_adjusted
    se = math.sqrt(1.0 / total_adjusted)
    return {
        "screens": k,
        "effect": round(effect, DIFF_PLACES),
        "se": round(se, DIFF_PLACES),
        "lo": round(effect - Z95 * se, DIFF_PLACES),
        "hi": round(effect + Z95 * se, DIFF_PLACES),
        "p_positive": round(normal_cdf(effect / se), DIFF_PLACES),
        "tau": round(math.sqrt(tau2), DIFF_PLACES),
        "q": round(q, DIFF_PLACES),
        "fixed_effect": round(fixed, DIFF_PLACES),
    }


def posterior_call(effect: float | None, se: float | None) -> str:
    """`on` when the 95% interval lies wholly above zero, `off` when wholly
    below, `unresolved` when it straddles. This is an evidence reading for the
    ranking and boundary planner; it does not select deployment defaults."""
    if effect is None or se is None or se <= 0:
        return "unresolved"
    if effect - Z95 * se > 0:
        return "on"
    if effect + Z95 * se < 0:
        return "off"
    return "unresolved"


def normalize_deployment_genome(deployment_genome: tuple[str, ...] | list[str],
                                allowed_tags: set[str] | None = None) -> tuple[str, ...]:
    """Validate and canonically order an explicit deployment selection.

    The list is intentionally independent of screen observations. It may only
    name screenable registry tags, and it may name at most one member of a
    versioned family; the latter is a safety invariant, not a score-based
    chooser.
    """
    selected = tuple(deployment_genome)
    if len(set(selected)) != len(selected):
        raise SystemExit("operator deployment genome names a tag more than once")
    eligible = set(screenable_tags()) if allowed_tags is None else set(allowed_tags)
    unknown = sorted(set(selected) - eligible)
    if unknown:
        raise SystemExit("operator deployment genome names unknown or unscreenable genes: "
                         + ", ".join(unknown))
    for family in families_of(sorted(eligible)):
        chosen = sorted(set(family) & set(selected))
        if len(chosen) > 1:
            raise SystemExit("operator deployment genome selects multiple versions of one family: "
                             + ", ".join(chosen))
    return tuple(sorted(selected))


def operator_default_on(tag: str, deployment_genome: tuple[str, ...]) -> bool:
    """Whether `tag` is in the explicit, operator-pinned deployment genome."""
    return tag in deployment_genome


def direct_arm_constant(sources: list[dict]) -> tuple[float, str] | None:
    """`column_se x sqrt(seat pairs)` for a **single-gene direct arm**, taken
    from the widest arm the repository has actually run at the screen's player
    count, with the file it came from.

    Discovered, never assumed. A direct arm's precision is a fact about the
    gene it flips - `s7`'s `idle-faith-patronage` rarely fires and cancels
    3.32x, `h1`'s `holy-lane-parity` changes nearly every game and cancels
    1.28x (#2302) - so a single number cannot be right for every gene. The
    **widest** measured arm is the conservative end, and it is what
    `--boundary` sizes the next round from: a rarely-firing gene will do
    better than this estimate, never worse.

    `sources` are the ledger's recorded source entries, each with the loaded
    analysis under `"analysis"`."""
    best = None
    for source in sources:
        data = source["analysis"]
        genes = data.get("genes", [])
        if len(genes) != 1:
            continue
        if int(data.get("profile", {}).get("players") or 0) != SCREEN["players"]:
            continue
        se = genes[0].get("win_se_pp")
        pairs = seat_pairs(source_seats(data))
        if se is None or pairs <= 0:
            continue
        constant = column_se(float(se)) * math.sqrt(pairs)
        if best is None or constant > best[0]:
            best = (constant, source["name"])
    return best


def arm_pairs_to_resolve(effect: float, se: float, constant: float) -> int | None:
    """Seat pairs a single-gene direct arm needs before the combined interval
    excludes zero, **if the arm reads the gene's current pooled effect**.

    Combining a reading of precision `1/a²` with the posterior's `1/se²` gives
    `se'² = 1 / (1/se² + 1/a²)`, and the interval clears zero when
    `|m| > Z95 * se'`. Solving for the arm's variance and substituting
    `a = constant / sqrt(N)`:

        N > constant² * ((Z95 / |m|)² − 1 / se²)

    `0` means the posterior already resolves it and `None` a gene whose
    effect is exactly zero, which no finite arm resolves. The figure is a
    planning number and not a promise: it assumes the arm reproduces the
    current mean, which is the assumption an operator is making anyway when
    they decide a gene is worth another run."""
    if effect == 0 or se <= 0:
        return None
    remaining = (Z95 / abs(effect)) ** 2 - 1.0 / (se * se)
    if remaining <= 0:
        return 0
    return math.ceil(constant * constant * remaining)


def arm_information_value(effect: float, se: float, arm_se: float,
                          deployed: bool) -> float:
    """What one direct arm buys, in wins per 10,000 on-arm seats.

    The expected value of sample information, against **the gene's shipped
    state** rather than against the posterior's own preference. Ship-on is
    worth `θ` and ship-off is worth `0`, so with the arm's reading `x` the
    best post-arm choice is worth `max(E[θ|x], 0)`; the posterior mean after
    the arm is itself normal, `m' ~ N(m, σ²)` with `σ = se² / sqrt(se² + a²)`,
    and for a normal that expectation is closed form:

        EVSI = m·Φ(m/σ) + σ·φ(m/σ) − (m if the gene ships on else 0)

    Reading it against the shipped state is what makes the number answer the
    operator's question. A gene the posterior likes and the genome already
    plays has little to buy - only the chance the operator changes the pinned
    selection. A gene the posterior likes that is pinned **off** has the whole
    effect to buy, and those are exactly the rows `--boundary` puts at the top."""
    variance = se * se
    sigma = variance / math.sqrt(variance + arm_se * arm_se)
    if sigma <= 0:
        return 0.0
    standardised = effect / sigma
    best = effect * normal_cdf(standardised) + sigma * normal_pdf(standardised)
    return best - (effect if deployed else 0.0)


def default_on_summary(ledger: dict) -> str:
    """The operator-pinned deployment policy, short enough for the ranking's
    heading. Measurements remain visible in the table but are not a rewrite
    trigger for this selection."""
    genome = ledger["rules"]["deployment_genome"]
    promotions = ledger["rules"]["operator_promotions"]
    if ledger["rules"]["deployment_policy"] != DEPLOYMENT_POLICY:
        raise ValueError("the ranking only renders the operator-pinned deployment policy")
    return (
        f"**Deployment default:** operator-pinned ({len(genome)} genes): retains the prior "
        f"36 selections and explicitly promotes {', '.join(f'`{tag}`' for tag in promotions)}. "
        "Screen columns, *Diff*, and posterior values are evidence only; new batches do not "
        "automatically change defaults."
    )


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
    # A contested board announces itself; a fieldless one says nothing, which
    # is exactly what its absence has always meant. Recording the leg only when
    # it is set keeps every record written before the leg existed byte-stable.
    for key, fieldless in FIELDLESS.items():
        value = raw.get(key)
        if value is not None and value != fieldless:
            profile[key] = value
    for key in RECORDED_WHEN_SET:
        if raw.get(key):
            profile[key] = raw[key]
    return profile


def shape_of(profile: dict) -> str:
    """`standard` when every leg of the screen matches, else `legacy`.

    A legacy source is history, not a second regime: the Pangaea screens that
    the deployment genome currently stands on. New ones are refused at the
    write path (`--legacy-shape` to record one deliberately), so the ledger
    cannot quietly acquire a second shape.

    ⚠ `FIELDLESS` is checked beside `SCREEN` and is the only thing standing
    between the ledger and a contested-field batch, which matches every map leg
    the screen has."""
    if any(profile.get(k, v) != v for k, v in FIELDLESS.items()):
        return "legacy"
    return "standard" if all(profile.get(k) == v for k, v in SCREEN.items()) else "legacy"


def shape_gap(profile: dict) -> str:
    """The legs that differ from the screen, for the refusal message."""
    return ", ".join(
        f"{key}={profile.get(key, fieldless)!r} (screen: {fieldless!r})"
        for key, fieldless in {**SCREEN, **FIELDLESS}.items()
        if profile.get(key, fieldless) != fieldless
    )


def gene_tags_from_sources(read) -> list[str]:
    """The gene tags a `gene_screen` binary compiles in, in header order, from
    the registry `read(path)` supplies — `screenable_tags_from`,
    which also reads the three tables a pre-2026-08-23 commit had instead.
    `gene_screen.rs`'s
    `the_gene_table_is_exactly_what_the_ledger_re_derives_from_the_tables`
    holds this rule against the compiled table itself."""
    return screenable_tags_from(read)


def gene_tags_at(commit: str) -> list[str] | None:
    """The gene tags at `commit`, or `None` when this clone cannot reach it.

    ⚠ `None` is not a pass. A commit the clone has never fetched is a claim
    nobody here can check, and `build_gap` refuses it as such — a shallow
    checkout must fetch the revision, not shrug at it."""
    def read(path: str) -> str:
        shown = subprocess.run(
            ["git", "-C", str(ROOT), "show", f"{commit}:{path}"],
            capture_output=True, text=True, check=False)
        if shown.returncode != 0:
            raise LookupError(path)
        return shown.stdout
    try:
        return gene_tags_from_sources(read)
    except (LookupError, ValueError, IndexError):
        return None


def gene_tags_now() -> list[str]:
    """The gene tags in the working tree — the code a ledger written here
    would ship."""
    return gene_tags_from_sources(lambda path: (ROOT / path).read_text())


def gene_set_fingerprint(tags) -> str:
    """The gene set, hashed: sha256 over the tags in order, one per line, each
    newline-terminated. `gene_screen.rs`'s `gene_set_fingerprint` builds the
    same string from the table it compiled in."""
    return hashlib.sha256("".join(f"{tag}\n" for tag in tags).encode()).hexdigest()


def source_seats(data: dict) -> int:
    """The seat observations one analysis rests on.

    ⭐ THE UNIT IS THE SEAT: one major seat in one game, carrying a genome and
    an outcome. A screen written since 2026-08-23 says `seats` outright. The
    paired designs before it counted `complete_pairs` — matched comparisons of
    one on-seat against one off-seat — and each of those is two seats."""
    if data.get("seats") is not None:
        return int(data["seats"])
    return 2 * int(data.get("complete_pairs", 0))


def gene_seats(gene: dict) -> int:
    """The seats behind one gene's row, by the same rule as `source_seats`."""
    if gene.get("seats") is not None:
        return int(gene["seats"])
    return 2 * int(gene["pairs"])


def seat_pairs(seats: int) -> int:
    """Seats as the matched-comparison currency the ranking's bands and the
    direct-arm sizing are still stated in: two seats make one on/off
    comparison. Kept until the ranking speaks in seats too."""
    return seats // 2


def build_of(data: dict) -> dict:
    """The build block a source recorded, with every key the stamp names.

    An empty dict means the file predates the stamp; see `build_state`."""
    raw = (data.get("profile") or {}).get("build") or {}
    return {key: raw.get(key) for key in BUILD_KEYS} if raw else {}


def batch_of(data: dict) -> dict:
    """What the source pre-registered, and what it actually played, in seats.

    A screen since 2026-08-23 declares `target_seats`; the paired designs
    declared `target_comparisons`, two seats each."""
    raw = data.get("batch") or {}
    if raw.get("target_seats") is not None:
        target = int(raw["target_seats"])
    elif raw.get("target_comparisons") is not None:
        target = 2 * int(raw["target_comparisons"])
    else:
        target = None
    complete = source_seats(data)
    return {
        "target_seats": target,
        "complete_seats": complete,
        "partial": None if not target else complete < target,
    }


def build_state(data: dict) -> str:
    """`pre-fingerprint` for a source written before the build stamp existed,
    else `stamped`.

    ⚠ GRANDFATHERED IS NOT SILENT. The twenty sources recorded before
    2026-08-23 carry no build block and cannot acquire one — the games are
    played — so they are kept as history and *named* `pre-fingerprint`
    everywhere the ledger prints or records them. What must never happen is a
    new screen entering unmarked: `gene_screen` always writes the block now, so
    the absence of one is a fact about the file's age, and a stamped source
    that fails its check is refused rather than downgraded to history."""
    return "pre-fingerprint" if not build_of(data) else "stamped"


def build_gap(data: dict, name: str, tags_at=None, tags_now=None) -> str:
    """Why this source cannot be trusted to have played the code it names, or
    `""` when it can.

    ⚠⚠ THE GENE SET IS THE LOAD-BEARING CHECK, and it is checked in BOTH
    directions. A source that prices a gene absent at the commit it claims is
    pricing code that commit does not have; a source missing a gene that
    commit does have is what an unmeasured gene quietly looks like. Both have
    happened here: P10 published a `holy-lane-parity` column after the cull
    that deleted it (#2266, #2299, #2307), and on 2026-08-23 a sibling change
    was minutes from deleting `barbarian-hunt` while the first standard-shape
    screen was re-pricing it."""
    tags_at = tags_at or gene_tags_at
    tags_now = tags_now or gene_tags_now
    build = build_of(data)
    if not build:
        return ""
    if not build.get("genes_sha256"):
        return (f"{name} carries a build block with no gene-set fingerprint, so nothing "
                "about the code it played can be checked")
    priced = [gene["tag"] for gene in data.get("genes", [])]
    compiled = list((data.get("profile") or {}).get("genes") or [])
    if gene_set_fingerprint(compiled) != build["genes_sha256"]:
        return (f"{name}'s recorded gene-set fingerprint does not describe its own header: "
                f"{len(compiled)} gene tags hash to {gene_set_fingerprint(compiled)[:12]}, "
                f"the file claims {build['genes_sha256'][:12]}. The artefact has been edited.")
    if build.get("dirty"):
        return (f"{name} was played by a binary built from a DIRTY tree at "
                f"{str(build.get('commit'))[:12]}: the code that played the games is not "
                "recoverable from any revision")
    commit = build.get("commit") or ""
    if not commit:
        return (f"{name} names no commit ({build.get('commit_source')}), so the code it "
                "played cannot be identified. Rebuild, or launch with CIVVIS_COMMIT set.")
    at_commit = tags_at(commit)
    if at_commit is None:
        return (f"{name} claims commit {commit[:12]}, which this clone cannot read. "
                f"Fetch that revision (`git fetch origin {commit}`) before recording it.")
    if gene_set_fingerprint(at_commit) != build["genes_sha256"]:
        extra = [tag for tag in compiled if tag not in set(at_commit)]
        missing = [tag for tag in at_commit if tag not in set(compiled)]
        detail = "; ".join(filter(None, [
            f"priced here but absent at {commit[:12]}: " + ", ".join(extra) if extra else "",
            f"present at {commit[:12]} but never compiled in: " + ", ".join(missing)
            if missing else "",
            "same tags in a different order" if not extra and not missing else "",
        ]))
        return (f"{name} was NOT played by the code at the commit it names "
                f"({commit[:12]}) — {detail}")
    gone = [tag for tag in priced if tag not in set(tags_now())]
    if gone:
        return (f"{name} prices {len(gone)} gene(s) this repository no longer registers: "
                + ", ".join(sorted(gone))
                + ". The screen is real; the code it measured is gone. Restore the genes or "
                  "record the source deliberately.")
    return ""


def load_source(path: Path) -> dict:
    data = json.loads(path.read_text())
    if data.get("kind") != "gene_screen_analysis":
        raise SystemExit(f"{path}: not a gene_screen --analyze --json output")
    return data


REPORTING_BATCH_LABELS = ("Last Batch", "Prior Batch", "Third Batch")


def source_record(path: Path, data: dict) -> dict:
    """The immutable metadata one analysis contributes to a record.

    Deployment sources and reporting-only batches have the same provenance
    contract.  They differ only in whether their rows are allowed to change
    the deployment ledger, so keep their identity, seat count and build stamp
    in one byte-stable shape.
    """
    profile = profile_of(data)
    entry = {
        "path": str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
        "shape": shape_of(profile),
        "seats": source_seats(data),
        "complete_pairs": seat_pairs(source_seats(data)),
        "family_wise_z": round(float(data.get("family_wise_z", 0.0)), 3),
        "profile": profile,
    }
    if data.get("games") is not None:
        entry["games"] = int(data["games"])
    build = build_of(data)
    if build:
        entry["build"] = build
    batch = batch_of(data)
    if batch["target_seats"] is not None:
        entry["batch"] = batch
    return entry


def reporting_batches_from_ledger(ledger: dict) -> list[Path]:
    """The newest-first, fixed display batches the ranking shows.

    A batch normally stays report-only while its result is reviewed.  After an
    explicit promotion it may also be an authoritative ``sources`` entry; it
    remains here so the table keeps the same three chronological columns.
    Provenance is always re-read by ``check`` so a table cannot quietly point
    at a different artifact.
    """
    raw = ledger.get("reporting_batches", [])
    if not isinstance(raw, list):
        raise SystemExit("gene ledger reporting_batches must be a list")
    if len(raw) > len(REPORTING_BATCH_LABELS):
        raise SystemExit(
            f"gene ledger has {len(raw)} reporting batches; the ranking has "
            f"only {len(REPORTING_BATCH_LABELS)} batch columns"
        )
    paths = []
    for entry in raw:
        if not isinstance(entry, dict) or not entry.get("path"):
            raise SystemExit("every reporting batch must name its analysis path")
        path = (ROOT / str(entry["path"])).resolve()
        if path in paths:
            raise SystemExit(f"reporting batch appears twice: {path}")
        paths.append(path)
    return paths


def reporting_batch_notes_from_ledger(ledger: dict) -> dict[str, str]:
    """Recorded build exceptions for report-only batches, keyed by file name."""
    return {
        Path(batch["path"]).name: batch["unverified"]
        for batch in ledger.get("reporting_batches", [])
        if batch.get("unverified")
    }


def latest_reporting_batches(entered: list[Path], recorded: list[Path]) -> list[Path]:
    """Keep the newest three fixed display batches when a new one arrives.

    ``entered`` and ``recorded`` are both newest-first.  The ranking has exactly
    three fixed report columns, so a newly entered batch must evict the oldest
    recorded one rather than leaving four inputs for a three-column renderer.
    """
    return (entered + [path for path in recorded if path not in entered])[:
        len(REPORTING_BATCH_LABELS)
    ]


def reporting_batch_records(paths: list[Path],
                            build_notes: dict[str, str] | None = None) -> list[dict]:
    """Validate and record fixed display-batch artifacts without pricing rules.

    A recorded exception follows the same explicit policy as an authoritative
    source's ``--unverified-build`` escape. It is never implicit: a new batch
    with a missing, dirty or unreadable build is refused unless its reason is
    saved beside the report in the ledger.
    """
    records = []
    for path in paths:
        data = load_source(path)
        gap = build_gap(data, path.name)
        reason = (build_notes or {}).get(path.name)
        if gap and not reason:
            raise SystemExit(
                gap + "\nA reporting batch must name the clean build it measured, or record "
                "an explicit reporting-build exception."
            )
        record = source_record(path, data)
        if reason:
            record["unverified"] = reason
        records.append(record)
    return records


def measure_from(gene: dict, source_name: str) -> dict:
    seats = gene_seats(gene)
    measure = {
        "seats": seats,
        # ⚠ `pairs` is `seats // 2`: the matched-comparison currency the
        # ranking's bands, the Rust mirror and the direct-arm sizing still
        # speak. The seat is the unit; this is its translation.
        "pairs": seat_pairs(seats),
        "n_on": int(gene.get("n_on", seat_pairs(seats))),
        "n_off": int(gene.get("n_off", seat_pairs(seats))),
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
        tranche_seats = (int(tranche["seats"]) if tranche.get("seats") is not None
                         else 2 * int(tranche["pairs"]))
        recorded = {
            "position": str(tranche["position"]),
            "seats": tranche_seats,
            "pairs": seat_pairs(tranche_seats),
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


def families_of(tags: list[str]) -> list[list[str]]:
    """⭐ VERSIONED GENES, the Python twin of `gene_screen.rs::families_of`.

    An improvement to a gene is a NEW gene, `<base>-<n>` (`war-economy-2`),
    screened beside the original; the original keeps its tag and its history
    and is version one. A tag `<base>-<n>` with `n >= 2` whose `<base>` is
    itself a gene is that gene's version `n`. Returned base first, then
    ascending versions; only families with two or more members."""
    known = set(tags)
    found: dict[str, list[tuple[int, str]]] = {}
    for tag in tags:
        base, _, version = tag.rpartition("-")
        if not base or not version.isdigit() or int(version) < 2:
            continue
        if base in known:
            found.setdefault(base, []).append((int(version), tag))
    return [[base] + [tag for _, tag in sorted(versions)]
            for base, versions in sorted(found.items())]


def tracked_wins(gene: dict) -> float:
    """A version's tracked wins: the ledger's pooled on−off win difference over
    every screen that priced it (`win_diff_pp`, the ranking's *Diff*) — the
    whole record, not the newest reading, because versions keep being priced
    screen after screen and "independently track wins" means across all of
    them. A row with no pooled figure (synthetic) falls back to its newest
    column, scaled to points."""
    diff = gene.get("win_diff_pp")
    if diff is None:
        return float(gene.get("wins_last_10k") or 0) / 100.0
    return float(diff)


def family_of(tag: str, tags: list[str]) -> list[str]:
    """`tag`'s family in version order, base first — `[]` when the gene is
    not versioned."""
    return next((family for family in families_of(tags) if tag in family), [])


def best_versions(family: list[str], verdict: dict[str, dict],
                  measured: dict[str, list[dict]]) -> list[str]:
    """⭐ A FAMILY'S VERSIONS, BEST FIRST. The version the pinned genome ships
    leads; the rest follow by tracked wins — the ledger's pooled
    on−off win difference, or the display record's for a version the ledger
    has not recorded — ties to the higher version. Only priced versions are
    listed; an unpriced version that ships still leads (it is what plays)."""
    def key(tag: str) -> tuple[bool, float, int]:
        row = verdict.get(tag, {})
        ships = bool(row.get("default_on"))
        if row.get("win_diff_pp") is not None:
            wins = tracked_wins(row)
        elif measured.get(tag):
            wins = pooled_win_diff_pp(measured[tag])
        else:
            wins = float("-inf")
        return (ships, wins, family.index(tag))
    listed = [tag for tag in family
              if measured.get(tag) or verdict.get(tag, {}).get("default_on")]
    return sorted(listed, key=key, reverse=True)


def best_version_cell(tag: str, tags: list[str], verdict: dict[str, dict],
                      measured: dict[str, list[dict]]) -> str:
    """The ranking's *Best version* column: the number of the family's best
    version (`1` = the original, `n` = `<base>-<n>`), the same on every row
    of the family; `—` for a gene that is not versioned."""
    family = family_of(tag, tags)
    if not family:
        return "—"
    best = best_versions(family, verdict, measured)
    return str(family.index(best[0]) + 1) if best else "—"


def family_rate_cells(tag: str, tags: list[str], verdict: dict[str, dict],
                      measured: dict[str, list[dict]]) -> tuple[str, str] | None:
    """The *Total (on)* / *Total (off)* cells of a versioned gene's row: the
    best two versions' pooled on and off win rates, best first, each with its
    own seats — a version's *on* is the seats that played THAT version, and
    every other seat (off, or a sibling version on) is its *off*, exactly as
    the screen prices each version on its own row. `None` when the gene is
    not versioned or no version is priced, so the row prints its own rates."""
    family = family_of(tag, tags)
    if not family:
        return None
    shown = [t for t in best_versions(family, verdict, measured) if measured.get(t)][:2]
    if not shown:
        return None
    on_cells, off_cells = [], []
    for t in shown:
        history = measured[t]
        on_rate, off_rate = pooled_win_rates(history)
        label = f"v{family.index(t) + 1}"
        on_cells.append(f"{label} {100 * on_rate:.2f}% (n={fmt_int(sum(m['n_on'] for m in history))})")
        off_cells.append(f"{label} {100 * off_rate:.2f}% (n={fmt_int(sum(m['n_off'] for m in history))})")
    return " · ".join(on_cells), " · ".join(off_cells)


def annotate_families(genes: list[dict]) -> None:
    """Attach version metadata without selecting a deployment winner.

    The pinned genome is validated before this point to contain at most one
    member of a family, so an observation can never turn an explicit selection
    off or choose a sibling instead.
    """
    by_tag = {gene["tag"]: gene for gene in genes}
    for family in families_of([gene["tag"] for gene in genes]):
        for rank, tag in enumerate(family, start=1):
            by_tag[tag]["family"] = family[0]
            by_tag[tag]["version"] = rank


def build_ledger(sources: list[Path], filter_known: bool = True,
                 build_notes: dict[str, str] | None = None,
                 deployment_genome: tuple[str, ...] | list[str] = OPERATOR_DEFAULT_ON,
                 reporting_batches: list[Path] | None = None,
                 reporting_build_notes: dict[str, str] | None = None) -> dict:
    """Merge the sources into one ledger object (the JSON file's content).
    Sources are recorded oldest-first, and a later one overrides an earlier one
    per gene. `filter_known=False` keeps every tag (synthetic tests).

    `deployment_genome` is the explicit operator selection that decides
    `default_on`; it is recorded in the ledger so `--check` and the Rust
    mirror can validate the same pinned set.

    `build_notes` maps a source's file name to the reason its build check was
    waived, and is what makes `--unverified-build` a *recorded* escape rather
    than a spoken one: the reason lands in the ledger beside the source it
    excuses, and `rebuild_from_ledger` reads it back so `--check` re-derives
    the same file. ``reporting_batches`` are separately verified screens the
    ranking displays but deliberately does not alter ``default_on``;
    their recorded build exceptions live in ``reporting_build_notes``.
    """
    measures: dict[str, dict] = {}
    # Every win column a gene has, oldest first. The tail three are the
    # ranking's scaled last, prior and third batch columns, so each screen
    # that prices a gene shifts its predecessor one column right and pushes the
    # fourth-oldest reading out of the table. They are observational ranking
    # columns; none automatically decides the pinned deployment default.
    columns: dict[str, list[int]] = {}
    # Every screen's two arms, for the pooled on-off difference and posterior.
    # Unlike the columns this keeps the whole record, not the tail.
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
        entry = source_record(path, data)
        # ⚠ Written only when the source has one. The twenty pre-2026-08-23
        # sources carry no build block and no pre-registration, so recording
        # them here would rewrite twenty entries to say nothing; their state is
        # `pre-fingerprint`, which `build_state` derives and `print_table`
        # names on every line.
        if build_notes and name in build_notes:
            entry["unverified"] = build_notes[name]
        recorded.append(entry)
        for gene in data.get("genes", []):
            if known and gene["tag"] not in known:
                dropped.add(gene["tag"])
                continue
            measures[gene["tag"]] = measure_from(gene, name)
            if "win_on" not in gene:
                raise SystemExit(
                    f"{name}: {gene['tag']} has no `win_on`; a screen without win "
                    "rates cannot supply the ranking's win column"
                )
            columns.setdefault(gene["tag"], []).append(
                wins_per_10k(float(gene["win_on"]), players)
            )
            # `win_delta_pp`/`win_se_pp` are what the posterior pools, and
            # `shape` is what lets it be pooled per instrument as well as
            # whole - the live question the moment a `standard` source lands
            # beside the `legacy` ones.
            arms.setdefault(gene["tag"], []).append({
                "win_on": float(gene["win_on"]),
                "win_off": float(gene["win_off"]),
                "n_on": int(gene.get("n_on", seat_pairs(gene_seats(gene)))),
                "n_off": int(gene.get("n_off", seat_pairs(gene_seats(gene)))),
                "win_delta_pp": float(gene["win_delta_pp"]),
                "win_se_pp": (None if gene.get("win_se_pp") is None
                              else float(gene["win_se_pp"])),
                "shape": shape_of(profile),
                "source": name,
            })
    if dropped:
        print("gene ledger: dropped rows for genes the repository no longer registers: "
              + ", ".join(sorted(dropped)), file=sys.stderr)

    allowed = set(screenable_tags())
    if not filter_known:
        # Synthetic fixtures can name their own stand-in tags while real
        # generation remains restricted to the screenable registry.
        allowed |= set(measures)
    selected = normalize_deployment_genome(deployment_genome, allowed)

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
        third = history[-3] if len(history) > 2 else None
        record = arms.get(tag, [])
        diff_pp = pooled_win_diff_pp(record) if record else None
        posterior = pooled_posterior(record, POSTERIOR_SHAPES) if record else None
        effect = posterior["effect"] if posterior else None
        posterior_se = posterior["se"] if posterior else None
        genes.append({
            "tag": tag,
            "verdict": verdict,
            "default_on": operator_default_on(tag, selected),
            "wins_last_10k": last,
            "wins_prior_10k": prior,
            # The third newest screen is published so a reader can see whether
            # the visible trend is a bounce. Like the other measurements, it
            # is evidence rather than an automatic deployment selector.
            "wins_third_10k": third,
            "win_diff_pp": diff_pp,
            # The precision-weighted pooled on-off difference on the win
            # column's scale, and its standard error. Published beside the
            # columns, published for evidence rather than deployment selection.
            "posterior_pp": effect,
            "posterior_se_pp": posterior_se,
            "posterior_screens": posterior["screens"] if posterior else None,
            "posterior_tau_pp": posterior["tau"] if posterior else None,
            "family_wise": family_wise,
            "conflict": conflict,
            "screen": measure,
        })
    annotate_families(genes)
    counts = {
        "helps": sum(g["verdict"] == "helps" for g in genes),
        "hurts": sum(g["verdict"] == "hurts" for g in genes),
        "unresolved": sum(g["verdict"] == "unresolved" for g in genes),
        # The pinned selection governs every screenable tag, including one
        # whose first measurement has not landed yet and therefore has no
        # `GeneVerdict` row. Counts describe that runtime selection, not only
        # the measured subset emitted below.
        "default_on": len(selected),
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
            "win_column": "wins added per 10,000 on-arm seats at the gene's measured on-rate in one "
                          "screen, (win_on - 1/players) * 10000; last, prior and third are the "
                          "three most recent screens that priced the gene, newest first, so a new "
                          "screen shifts last to prior and prior to third; they are published evidence "
                          "and do not automatically decide default_on",
            "win_diff": "the pooled on rate minus the pooled off rate in percentage points, "
                        "over every screen that priced the gene, each weighted by its on-arm seats "
                        "- the ranking's `Diff`, the whole on-off difference",
            "default_on": "operator-pinned: exactly the tags in deployment_genome are on; "
                          "every other screenable tag is off. Screen observations do not "
                          "automatically alter the selection",
            "posterior": "random-effects (DerSimonian-Laird) inverse-variance pool of every "
                         "screen's on-off difference on the win column's scale, each weighted "
                         "by its own standard error and the between-screen variance carried in "
                         "the interval; posterior_pp is the pooled effect and posterior_se_pp "
                         "its standard error, both in wins per 10,000 on-arm seats",
            "deployment_policy": DEPLOYMENT_POLICY,
            "deployment_genome": list(selected),
            "operator_promotions": list(
                OPERATOR_PROMOTIONS_20260824 + OPERATOR_PROMOTIONS_20260825
            ),
            "posterior_shapes": list(POSTERIOR_SHAPES),
            "deployment_policy_meaning": "the operator-pinned list decides default_on. "
                                         "`tools/genes.py` writes the list into the ledger and "
                                         "the generated Rust table; source refreshes preserve it "
                                         "until an explicit operator edit changes the list",
        },
        "sources": recorded,
        "reporting_batches": reporting_batch_records(
            reporting_batches or [], reporting_build_notes),
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


#: The markers the generated verdict block sits between, at the END of
#: `src/ai/advanced/genes.rs`. `render_rust` renders the block; the writer
#: replaces everything from the first marker on, so the hand-written rows
#: above it are never touched by the tool.
GENERATED_BEGIN = "// ═══ GENERATED BY tools/genes.py — THE VERDICTS. Do not edit below: `python3 tools/genes.py write` ═══"
GENERATED_END = "// ═══ END GENERATED ═══"


def render_rust(ledger: dict) -> str:
    """The verdict block for `genes.rs`: the explicit deployment genome and
    one `GeneVerdict` per priced gene with its observational evidence."""
    lines = [
        GENERATED_BEGIN,
        "//",
        "// Source: docs/gene_ledger.json (the same tool writes both); `genes.py check` holds them",
        "// together, and `the_default_matches_the_operator_pinned_genome` validates every",
        "// `default_on` below against `DEPLOYMENT_GENOME`.",
        "",
        "/// The explicit policy that supplies every `default_on` below.",
        f"pub(super) const DEPLOYMENT_POLICY: &str = {json.dumps(ledger['rules']['deployment_policy'])};",
        "",
        "/// The operator-pinned, screenable deployment selection.",
        "#[rustfmt::skip]",
        "pub(super) const DEPLOYMENT_GENOME: &[&str] = &[",
        *(f"    {json.dumps(tag)}," for tag in ledger["rules"]["deployment_genome"]),
        "];",
        "",
        "#[rustfmt::skip]",
        "pub(super) const VERDICTS: &[GeneVerdict] = &[",
    ]
    for gene in ledger["genes"]:
        verdict = {"helps": "Verdict::Helps", "hurts": "Verdict::Hurts",
                   "unresolved": "Verdict::Unresolved"}[gene["verdict"]]
        lines.append(
            "    GeneVerdict { "
            f"tag: {json.dumps(gene['tag'])}, verdict: {verdict}, "
            f"default_on: {'true' if gene['default_on'] else 'false'}, "
            f"wins_last_10k: {rust_opt_i32(gene['wins_last_10k'])}, "
            f"wins_prior_10k: {rust_opt_i32(gene['wins_prior_10k'])}, "
            f"win_diff_pp: {rust_opt_f(gene['win_diff_pp'])}, "
            f"posterior_pp: {rust_opt_f(gene['posterior_pp'])}, "
            f"posterior_se_pp: {rust_opt_f(gene['posterior_se_pp'])}, "
            f"family_wise: {'true' if gene['family_wise'] else 'false'}, "
            f"screen: {rust_measure(gene['screen'])} }},"
        )
    lines.append("];")
    lines.append(GENERATED_END)
    lines.append("")
    return "\n".join(lines)


def render_json(ledger: dict) -> str:
    return json.dumps(ledger, indent=2, sort_keys=False) + "\n"


def print_table(ledger: dict) -> None:
    policy = ledger["rules"]["deployment_policy"]
    print(f"gene ledger · {len(ledger['genes'])} genes · "
          f"helps {ledger['counts']['helps']} · hurts {ledger['counts']['hurts']} · "
          f"unresolved {ledger['counts']['unresolved']} · "
          f"default on {ledger['counts']['default_on']} ({policy})")
    for src in ledger["sources"]:
        build = src.get("build") or {}
        if build.get("commit"):
            stamp = build["commit"][:12] + (" DIRTY" if build.get("dirty") else "")
        elif build:
            stamp = "unstamped"
        else:
            stamp = "pre-fingerprint"
        batch = src.get("batch") or {}
        if batch.get("partial") is None:
            size = ""
        elif batch["partial"]:
            size = (f", ⚠ PARTIAL {batch['complete_seats']}"
                    f"/{batch['target_seats']} seats")
        else:
            size = ", complete"
        print(f"  source {src['shape']:<8} {stamp:<14} {src['path']}  "
              f"({src.get('seats', 2 * src['complete_pairs'])} seats{size}, "
              f"family-wise |z|≥{src['family_wise_z']})")
        if src.get("unverified"):
            print(f"           ⚠ build unverified: {src['unverified']}")
    grandfathered = sum(1 for src in ledger["sources"] if not src.get("build"))
    if grandfathered:
        print(f"  ⚠ {grandfathered} of {len(ledger['sources'])} sources predate the build "
              f"stamp ({FINGERPRINT_SINCE}) and are kept as pre-fingerprint history")
    print(f"{'gene':<30} {'verdict':<10} {'default':<7} {'last':>6} {'prior':>6} "
          f"{'third':>6} {'diff':>7} {'posterior':>18} {'P>0':>6} {'win/share z':<20} source")
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
        effect, se = gene["posterior_pp"], gene["posterior_se_pp"]
        if effect is None or se is None:
            post, prob = "–", "–"
        else:
            post = f"{effect:+.0f} ±{Z95 * se:.0f}"
            prob = f"{100 * normal_cdf(effect / se):.0f}%"
        flag = "**" if gene["family_wise"] else ("!" if gene["conflict"] else "")
        source = gene["screen"]["source"] if gene["screen"] else "-"
        print(f"{gene['tag']:<30} {gene['verdict']:<10} {'on' if gene['default_on'] else 'off':<7} "
              f"{col(gene['wins_last_10k']):>6} {col(gene['wins_prior_10k']):>6} "
              f"{col(gene['wins_third_10k']):>6} "
              f"{diff(gene['win_diff_pp']):>7} {post:>18} {prob:>6} "
              f"{z(gene['screen']):<20} {source} {flag}")


def sources_from_args(args, notes: dict[str, str] | None = None) -> list[Path]:
    """The `--source` files, oldest first, each held to the screen's shape AND
    to the code it says it played.

    ⚠ These are the whole enforcement of "one screen, played by one known
    build", and they are two guards of the same shape rather than two idioms.
    A probe played at another profile answers a different question, and pooling
    its column with the screen's would report the difference between two worlds
    as a gene's effect: `--legacy-shape` records one anyway, which is how the
    Pangaea history already in the ledger stays there. A screen played by a
    binary that is not the code it names prices something nobody can read back:
    `--unverified-build "<why>"` records one anyway, and the reason lands in
    the ledger beside the source it excuses.

    `notes` collects those reasons for `build_ledger` to record."""
    paths = [Path(p).resolve() for p in args.sources]
    escape = getattr(args, "unverified_build", None)
    for path in paths:
        data = load_source(path)
        profile = profile_of(data)
        if not args.legacy_shape and shape_of(profile) != "standard":
            raise SystemExit(
                f"{path.name} was not played at the screen's shape: {shape_gap(profile)}."
                "\nRun it at the screen (`gene_screen --games N --out rows.jsonl`, no"
                " profile flags), or pass --legacy-shape to record it as history."
            )
        gap = build_gap(data, path.name)
        if not gap:
            continue
        if not escape:
            raise SystemExit(
                gap + "\nRe-run the batch on a clean build of the code it prices, or pass"
                ' --unverified-build "<why this source is recorded anyway>".'
            )
        if notes is not None:
            notes[path.name] = escape
    return paths


def notes_from_ledger(ledger: dict) -> dict[str, str]:
    """The escape reasons the ledger already recorded, keyed by file name."""
    return {Path(src["path"]).name: src["unverified"]
            for src in ledger["sources"] if src.get("unverified")}


def deployment_genome_of(ledger: dict) -> tuple[str, ...]:
    """The explicit selection a recorded ledger was written under.

    The fallback only lets this tool migrate the final pre-pinned ledger once;
    newly generated ledgers always carry the complete explicit list.
    """
    return tuple(ledger.get("rules", {}).get("deployment_genome", OPERATOR_DEFAULT_ON))


def rebuild_from_ledger(ledger: dict) -> dict:
    """Re-derive a ledger from its recorded sources and pinned selection, carrying
    its own build exceptions back in so `--check` reproduces the record."""
    return build_ledger(sources_from_ledger(ledger),
                        build_notes=notes_from_ledger(ledger),
                        deployment_genome=deployment_genome_of(ledger),
                        reporting_batches=reporting_batches_from_ledger(ledger),
                        reporting_build_notes=reporting_batch_notes_from_ledger(ledger))


def sources_from_ledger(ledger: dict) -> list[Path]:
    return [(ROOT / s["path"]).resolve() for s in ledger["sources"]]


#: The ranking's short name for the win column.
wins_per = wins_per_10k

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


# ⭐ `column_se` and `POWER_80` live in the ledger half of this file (#2300 put the
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
            "seats": source_seats(data),
            "pairs": pairs,
            "se": median,
            "band": POWER_80 * median,
            "gain": unpaired / (median * math.sqrt(pairs)) if pairs else 0.0,
        })
    return list(reversed(out))
FLAGS_RS = ROOT / "src" / "ai" / "advanced" / "treatment_flags.rs"


def registry() -> dict[str, tuple[str, str]]:
    """Every registered gene: tag → (field, toggle name), from the gene
    registry (`src/ai/advanced/genes.rs`, read by `py`). The
    toggle name is not always the field name (`siege_tracks_wall` toggles
    through `enable_siege_tracks_the_wall`)."""
    return {row.tag: (row.field, row.toggle) for row in genes()}


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


def measurements_from_source(data: dict, name: str, shape: str) -> dict[str, dict]:
    """One source's per-gene observations, retaining real on/off seat counts."""
    rows: dict[str, dict] = {}
    source_total_seats = source_seats(data)
    for gene in data.get("genes", []):
        # Only legacy sources need this fallback. Do not evaluate it for an
        # independent batch that recorded both arms but no `pairs`.
        legacy_arm_seats = None
        if gene.get("n_on") is None or gene.get("n_off") is None:
            legacy_arm_seats = seat_pairs(gene_seats(gene))
        rows[gene["tag"]] = {
            "win_on": float(gene["win_on"]),
            "win_off": float(gene["win_off"]),
            "n_on": int(gene["n_on"] if gene.get("n_on") is not None else legacy_arm_seats),
            "n_off": int(gene["n_off"] if gene.get("n_off") is not None else legacy_arm_seats),
            "win_z": float(gene["win_z"]),
            "share_z": float(gene["share_z"]),
            "win_delta_pp": float(gene["win_delta_pp"]),
            "win_se_pp": (None if gene.get("win_se_pp") is None
                          else float(gene["win_se_pp"])),
            "share_delta_pp": float(gene["share_delta_pp"]),
            "shape": shape,
            "source": name,
            "source_seats": source_total_seats,
            "players": int(data.get("profile", {}).get("players", 0) or 0),
            "compute_cost_pct": gene.get("compute_cost_pct"),
            "compute_cost_se_pct": gene.get("compute_cost_se_pct"),
            "time_cost_pct": gene.get("time_cost_pct"),
            "time_cost_se_pct": gene.get("time_cost_se_pct"),
        }
    return rows


def load_sources(ledger: dict) -> tuple[dict[str, list[dict]], dict[str, str]]:
    """Authoritative per-gene history, oldest source first."""
    history: dict[str, list[dict]] = {}
    newest_src: dict[str, str] = {}
    for src in ledger["sources"]:
        data = json.loads((ROOT / src["path"]).read_text())
        name = Path(src["path"]).name
        for tag, row in measurements_from_source(data, name, src["shape"]).items():
            history.setdefault(tag, []).append(row)
            newest_src[tag] = name
    return history, newest_src


def load_reporting_batches(ledger: dict) -> list[dict]:
    """The three fixed batch columns, newest first, with their source rows."""
    batches = []
    for meta in ledger.get("reporting_batches", []):
        data = load_source(ROOT / meta["path"])
        name = Path(meta["path"]).name
        batches.append({
            "meta": meta,
            "rows": measurements_from_source(data, name, meta["shape"]),
        })
    return batches


def load_display_sources(ledger: dict) -> tuple[dict[str, list[dict]], dict[str, str]]:
    """Display history: ledger evidence plus any fixed display batch.

    Existing authoritative sources are never counted twice when they also
    occupy a fixed batch slot.  A display-only batch therefore refreshes the
    table totals and rankings while the deployment ledger stays byte-for-byte
    tied to its own sources.
    """
    history, newest_src = load_sources(ledger)
    authoritative = {str(src["path"]) for src in ledger["sources"]}
    for batch in load_reporting_batches(ledger):
        meta = batch["meta"]
        if str(meta["path"]) in authoritative:
            continue
        for tag, row in batch["rows"].items():
            history.setdefault(tag, []).append(row)
            newest_src[tag] = row["source"]
    return history, newest_src


def fmt_int(n: float) -> str:
    return f"{int(round(n)):,}"


def batch_win_cell(history: list[dict], back: int = 0) -> str:
    """One legacy chronological batch cell, scaled to 10,000 on-arm seats.

    `back=0` is the latest batch, `back=1` the prior batch, and so on. A
    source without that many readings leaves the table cell unpopulated. The
    production table uses ``load_reporting_batches`` instead, so its sample
    size appears once in each fixed column header rather than in every cell.
    """
    if len(history) <= back:
        return EN_DASH
    batch = history[-1 - back]
    return f"{wins_per(batch['win_on'], batch['players']):+d}"


def total_seat_batch_wins(row: dict) -> int:
    """On-arm excess, normalized to 10,000 *total* player seats.

    This keeps the displayed chance expectation at 1,667 wins per 10,000
    seats even when a default-on gene occupies roughly three quarters of an
    independent screen's seats.
    """
    players = int(row["players"])
    chance = 1.0 / players if players else 1.0 / 6.0
    return round((row["win_on"] - chance) * row["n_on"] * PER / row["source_seats"])


def reporting_batch_cell(batch: dict | None, tag: str) -> str:
    if batch is None or tag not in batch["rows"]:
        return EN_DASH
    return f"{total_seat_batch_wins(batch['rows'][tag]):+d}"


def reporting_batch_header(label: str, batch: dict | None) -> str:
    if batch is None:
        return f"Wins ± /10k total seats — {label} (n=not recorded)"
    return (f"Wins ± /10k total seats — {label} "
            f"(n={fmt_int(batch['meta']['seats'])} total seats)")


def diff_cell(history: list[dict]) -> str:
    """Render the whole on−off win-rate difference as a percentage.

    The figure is `gene_ledger.pooled_win_diff_pp`, published as evidence
    beside the pinned deployment state. Positive values intentionally have no
    leading plus; negative values retain their minus sign.
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

    Published beside the win columns because, at the standing 250-turn Online
    clock, a science or congress gene cannot pay on the win axis at all — science and diplomatic
    victories land at median t283 and t285, so they are 1–2% of endings. A
    lane gene's evidence is here or nowhere. `docs/GENE_SCREEN.md` carries the
    pre-registered interpretation for reading it."""
    newest = history[-1]
    z = newest["share_z"]
    return f"{newest['share_delta_pp']:+.2f} (z {z:+.2f}) {share_verdict(z)}"


def posterior_of(history: list[dict]) -> dict | None:
    """The published pool: every screen at a shape `POSTERIOR_SHAPES` admits."""
    return pooled_posterior(history, POSTERIOR_SHAPES)


def evidence_table(ledger: dict, measured: dict[str, list[dict]]) -> list[dict]:
    """Every measured, screenable gene's pinned state and posterior evidence."""
    # The screen's own universe only. A host-only flag can carry a ledger row
    # from a retired native stand-in (`step-and-reassess`) and the ledger never
    # governs it, so it is not ranked and must not appear in this evidence table
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
            "pinned": bool(gene["default_on"]),
            "posterior": posterior,
            "call": posterior_call(effect, se),
        }
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
    for row in evidence_table(ledger, measured):
        if row["call"] != "unresolved" or row["posterior"] is None:
            continue
        effect, se = row["posterior"]["effect"], row["posterior"]["se"]
        entry = dict(row)
        if arm is not None:
            constant = arm[0]
            entry["needs"] = arm_pairs_to_resolve(effect, se, constant)
            entry["buys"] = arm_information_value(
                effect, se, constant / math.sqrt(arm_pairs), row["pinned"])
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


def evidence_sections(ledger: dict, measured: dict[str, list[dict]],
                      desc: dict[str, str]) -> list[str]:
    """The evidence tables that inform future explicit operator selections.

    None of these statistics is a deployment rule. The pinned set above stays
    stable until an operator changes `OPERATOR_DEFAULT_ON` deliberately.
    """
    rows = evidence_table(ledger, measured)
    decided_on = [r for r in rows if r["call"] == "on"]
    decided_off = [r for r in rows if r["call"] == "off"]
    straddle = [r for r in rows if r["call"] == "unresolved"]
    lines = [
        "",
        "## Evidence for future operator selections",
        "",
        "The deployment genome is explicitly operator-pinned. The win columns, pooled "
        "*Diff*, posterior, and score-share readings below remain useful evidence, but a new "
        "source does not promote or demote a gene automatically. To change a default, update "
        "the pinned list with an explicit operator decision and regenerate this ledger.",
        "",
        "*Posterior (95% CI)* is a random-effects (DerSimonian\u2013Laird) inverse-variance "
        "pool of every screen's on\u2212off difference on the win column's scale. It weights each "
        "screen by its standard error and carries between-screen disagreement in the interval; "
        "`P(>0)` makes the resulting precision visible.",
        "",
        "### What the posterior resolves",
        "",
        f"Of {len(rows)} priced genes the interval clears zero for **{len(decided_on)} "
        f"upward** and **{len(decided_off)} downward**; **{len(straddle)} straddle zero**. "
        "Those are evidence states, not automatic deployment calls.",
        "",
        "| Gene | Posterior (95% CI) | P(>0) | Screens | Pinned | Evidence call |",
        "|---|---:|---:|---:|---|---|",
    ]
    for row in decided_on + decided_off:
        lines.append(
            f"| `{row['tag']}` | {posterior_cell(row['posterior'])} | "
            f"{probability_cell(row['posterior'])} | {row['posterior']['screens']} | "
            f"{'on' if row['pinned'] else 'off'} | **{row['call']}** |"
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
        entry = shapes.setdefault(src["shape"], {"sources": 0, "seats": 0})
        entry["sources"] += 1
        entry["seats"] += int(src["seats"])
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
        f"`POSTERIOR_SHAPES` in `tools/genes.py` says which shapes the published "
        f"pool admits and is currently `{', '.join(POSTERIOR_SHAPES)}`.",
        "",
        "| Shape | Sources | Player seats | Genes priced |",
        "|---|---:|---:|---:|",
    ]
    for shape in ("standard", "legacy"):
        entry = shapes.get(shape, {"sources": 0, "seats": 0, "genes": 0})
        lines.append(f"| {shape} | {entry['sources']} | {fmt_int(entry['seats'])} | "
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
        "pairs**, in wins per 10,000 on-arm seats, read against the gene's **pinned** "
        "state \u2014 so a gene the evidence likes and the genome already plays has little "
        "to buy, and a gene the evidence likes that the pinned genome holds off has the whole "
        "effect to buy. *Pairs to resolve* is how many matched seat pairs that arm needs "
        "before the combined interval clears zero, if it reads the gene's current pooled "
        f"effect. Both are sized from `{name}`, the widest single-gene arm this "
        f"repository has actually run ({constant / math.sqrt(ARM_PAIRS):.1f} per-column SE "
        f"at {fmt_int(ARM_PAIRS)} pairs) \u2014 the conservative end, since a gene that "
        "rarely fires cancels far more and resolves tighter.",
        "",
        "| Gene | Posterior (95% CI) | P(>0) | Pinned | Buys | Pairs to resolve |",
        "|---|---:|---:|---|---:|---:|",
    ]
    for row in rows:
        needs = row["needs"]
        lines.append(
            f"| `{row['tag']}` | {posterior_cell(row['posterior'])} | "
            f"{probability_cell(row['posterior'])} | "
            f"{'on' if row['pinned'] else 'off'} | {row['buys']:+.1f} | "
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
            "`python3 tools/genes.py boundary` prints this list on its "
            "own, with `--arm-pairs` and `--max-arm-pairs` to size it.",
        ]
    return lines


def lane_section(ledger: dict, measured: dict[str, list[dict]],
                 desc: dict[str, str]) -> list[str]:
    """The lane genes, judged on the axis they can actually pay on."""
    tags = lane_tags()
    selected = set(ledger["rules"]["deployment_genome"])
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
        default = "**on**" if tag in selected else "off"
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
            f"{gene.get('verdict', 'unmeasured')} |"
        )
    return lines


def render(ledger: dict) -> str:
    authoritative_measured, _ = load_sources(ledger)
    measured, newest_src = load_display_sources(ledger)
    reporting = load_reporting_batches(ledger)
    reporting_slots = reporting + [None] * (len(REPORTING_BATCH_LABELS) - len(reporting))
    tags = screenable_tags()
    desc = descriptions()
    verdict = {g["tag"]: g for g in ledger["genes"]}
    selected = set(ledger["rules"]["deployment_genome"])
    # A reporting batch can already display a screenable gene before an
    # authoritative source has supplied its GeneVerdict. Preserve its explicit
    # pinned state in the display and family helpers instead of falling back to
    # an accidental "off" merely because that row is still unmeasured.
    for tag in selected:
        verdict.setdefault(tag, {"default_on": True})
    reg = registry()

    rows = []
    unmeasured = []
    for tag in tags:
        history = measured.get(tag)
        if not history:
            unmeasured.append(tag)
            continue
        # Keep the visual ranking tied to the displayed pooled difference,
        # rather than to one screen's projected win total.  This is the same
        # quantity `diff_cell` prints below.
        rows.append((pooled_win_diff_pp(history), tag, history))
    rows.sort(key=lambda r: (-r[0], r[1]))

    removed = sorted(tag for tag in measured if tag not in reg)
    latest = {tag: history[-1] for tag, history in measured.items()}

    # Everything that explains the table, kept but moved out from in front of it:
    # the operator reads the ranking, and twenty-two lines of preamble stood
    # between the file and its first row. Carried under the table instead, so
    # nothing derived is lost and nothing derived is in the way.
    reference = [
        "Every screenable heuristic gene on the Advanced controller, ranked by the displayed "
        "pooled *Diff* from highest to lowest (alphabetically by tag on a tie). Each batch "
        "header carries its actual player-seat "
        "count once; cells show the enabled arm's excess projected to 10,000 **total** player "
        "seats, where a six-player chance expectation is 1,667 wins. A dash means that batch "
        "did not screen the gene. The *Total* win-rate columns pool the displayed observations "
        "and retain their real per-gene on/off seat counts in every row. *Diff* is that display "
        "total's on rate minus off rate, in percentage points. *Default* is the explicit "
        "operator-pinned deployment selection: sources and display batches update evidence, "
        "not defaults. Screenable genes "
        "awaiting every displayed measurement are listed separately below without a rank.",
        "",
        "**Versioned genes.** An improvement to a gene is a new gene `<base>-<n>` "
        "(`docs/GENE_SCREEN.md`, *Versioning a gene*), priced on its own row: a version's "
        "*on* is the seats that played that version, and every other seat — off, or a "
        "sibling version on — is its *off*. *Best version* names the family's best version "
        "(`1` is the original) on every row of the family: the pinned version, else the "
        "priced version with the highest tracked wins. A versioned row's *Total (on)* and "
        "*Total (off)* cells show the best two versions' rates side by side, best first, "
        "each with its own `n`; `—` marks a gene with no versions.",
        "",
        "**Reading the table.** A six-player seat wins 1-in-6 by chance, so the expected "
        "count is 1,667 wins per 10,000 total seats. The batch cells are the enabled arm's "
        "excess over that chance rate, scaled from actual completed seats; they do not invent "
        "games or seats. The independent latest batch can have unequal on/off arms, which is "
        "why the pooled *Total (on)* and *Total (off)* cells retain their own `n` on every row.",
        "",
        "**Batch provenance.** The newest displayed batch is the completed current-standard "
        "6-major Continents screen (74×46, nine city-states, Online speed through turn 250, "
        "all six victory lanes, shuffled civilizations and best-genome baseline). Its "
        "completed seats update the published evidence only; the operator-pinned default "
        "does not move until explicitly edited. Older displayed batches remain visible for "
        "trend context.",
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
        "over **14,400 player seats** and resolves ±68 at a 1.28× gain, *wider* than "
        "four-gene `s6` over 12,000 seats. Its gene changes nearly every game; `s7`'s rarely fires. That, "
        "not the count, is the difference.",
        "",
        "| Screen | Shape | Genes | Player seats | 1 SE | ±80% power | Pairing gain |",
        "|---|---|---:|---:|---:|---:|---:|",
        *(
            f"| `{r['name']}` | {r['shape']} | {r['genes']} | {fmt_int(r['seats'])} | "
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
        "its verdict, published beside the win columns because a lane gene can fail to pay "
        "on the win axis at 250 turns. **None of these three automatically decides a "
        "default**; they are evidence for a later explicit operator selection.",
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
        "Regenerate with `python3 tools/genes.py write` after every "
        "screen enters the ledger; `tools/test_genes.py` fails when this "
        "file is older than the ledger's sources.",
    ]

    lines = [
        "# The heuristic gene ranking",
        "",
        default_on_summary(ledger),
        "",
        "| Rank | Gene | Description | Best version | Default | "
        + " | ".join(
            reporting_batch_header(label, batch)
            for label, batch in zip(REPORTING_BATCH_LABELS, reporting_slots)
        )
        + " | Total (on) Win rate | Total (off) Win rate | Diff | Posterior (95% CI) | "
        "P(>0) | Share Δpp (z) | cost (compute) | cost (time) |",
        "|---:|---|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|",
    ]
    for rank, (_diff, tag, history) in enumerate(rows, 1):
        v = verdict.get(tag, {})
        default = "**on**" if v.get("default_on") else "off"
        # Fixed report batches put `n` in the headers once, while each total
        # arm keeps its own real seat count below.
        last, prior, third = (
            reporting_batch_cell(batch, tag) for batch in reporting_slots
        )
        on_seats = sum(m["n_on"] for m in history)
        off_seats = sum(m["n_off"] for m in history)
        on_rate, off_rate = pooled_win_rates(history)
        # A versioned gene's row shows the best two versions' rates side by
        # side (operator, 2026-08-23); every other row shows its own.
        on_cell, off_cell = family_rate_cells(tag, tags, verdict, measured) or (
            f"{100 * on_rate:.2f}% (n={fmt_int(on_seats)})",
            f"{100 * off_rate:.2f}% (n={fmt_int(off_seats)})",
        )
        posterior = posterior_of(history)
        lines.append(
            f"| {rank} | `{tag}` | {desc.get(tag, '')} | "
            f"{best_version_cell(tag, tags, verdict, measured)} | {default} | {last} | {prior} | "
            f"{third} | "
            f"{on_cell} | "
            f"{off_cell} | "
            f"{diff_cell(history)} | "
            f"{posterior_cell(posterior)} | {probability_cell(posterior)} | "
            f"{share_cell(history)} | "
            f"{cost_cell(history, 'compute_cost_pct', 'compute_cost_se_pct')} | "
            f"{cost_cell(history, 'time_cost_pct', 'time_cost_se_pct')} |"
        )

    # Evidence analysis stays tied to authoritative ledger sources. A display
    # batch cannot silently change a runtime default.
    lines += evidence_sections(ledger, authoritative_measured, desc)

    if unmeasured:
        lines += [
            "",
            "## Awaiting measurement",
            "",
            "These screenable genes have no on/off result, so they receive no rank or "
            "promotion from this table. Their deployment state remains explicit while a "
            "screen is pending.",
            "",
            "| Gene | Default | Description | Best version |",
            "|---|---|---|---:|",
        ]
        for tag in sorted(unmeasured):
            v = verdict.get(tag, {})
            default = "**on**" if v.get("default_on") else "off"
            verdict_word = v.get("verdict", "unmeasured")
            lines.append(
                f"| `{tag}` | {default} ({verdict_word}) | {desc.get(tag, '')} | "
                f"{best_version_cell(tag, tags, verdict, measured)} |"
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
    sources = ", ".join(
        f"`{Path(s['path']).name}` ({s['shape']}, {s['seats']:,} seats)"
        for s in ledger["sources"]
    )
    reporting_sources = ", ".join(
        f"`{Path(s['path']).name}` ({s['seats']:,} seats)"
        for s in ledger.get("reporting_batches", [])
    )
    lines += [
        "",
        f"_Generated by `tools/genes.py` from the ledger's sources: {sources}. "
        + (f"The fixed display batches are: {reporting_sources}. " if reporting_sources else "")
        + "The deployment verdicts live in `docs/gene_ledger.json`; the table's batch cells "
        "are the operator's wins-per-ten-thousand-total-seat reporting view._",
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
    print(f"{'gene':<32} {'posterior':>20} {'P>0':>7} {'pinned':>6} {'buys':>7} "
          f"{'pairs to resolve':>17}")
    for row in rows:
        needs = row["needs"]
        print(f"{row['tag']:<32} {posterior_cell(row['posterior']):>20} "
              f"{probability_cell(row['posterior']):>7} "
              f"{'on' if row['pinned'] else 'off':>6} {row['buys']:>+7.1f} "
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


# ─────────────────────────────────────────────────────────────────────────────
# THE ONE COMMAND LINE
# ─────────────────────────────────────────────────────────────────────────────

def registry_with_block(block: str) -> str:
    """`genes.rs` with its generated verdict block replaced by `block`. The
    hand-written rows above the marker are untouched; a registry without a
    block yet gets one appended."""
    text = REGISTRY_PATH.read_text(encoding="utf-8")
    if GENERATED_BEGIN in text:
        head = text[: text.index(GENERATED_BEGIN)]
    else:
        head = text.rstrip("\n") + "\n\n"
    return head + block


def rust_block_of(text: str) -> str:
    """The generated block of a `genes.rs` text, or `""` when it has none."""
    if GENERATED_BEGIN not in text:
        return ""
    return text[text.index(GENERATED_BEGIN):]


def _add_source_args(ap: argparse.ArgumentParser) -> None:
    ap.add_argument("sources", nargs="*", help="gene_screen --analyze --json outputs to enter")
    ap.add_argument(
        "--reporting-batch", action="append", default=[], metavar="FILE",
        help=("newest-first report-only batch for the ranking's three display columns; "
              "does not change deployment defaults"),
    )
    ap.add_argument(
        "--reporting-unverified-build", metavar="REASON", default=None,
        help=("record why every newly named reporting batch cannot have its build "
              "re-verified; requires --reporting-batch"),
    )
    ap.add_argument("--legacy-shape", action="store_true",
                    help="record a source played away from the screen's shape as history")
    ap.add_argument("--unverified-build", metavar="REASON", default=None,
                    help="record a source whose build cannot be verified, with the reason")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    sub = ap.add_subparsers(dest="command", required=True)
    sub.add_parser("list", help="every gene: kind, default, verdict")
    source = sub.add_parser("source", help="enter screens as ledger sources and regenerate")
    _add_source_args(source)
    write = sub.add_parser("write", help="regenerate the ledger, the verdict block and the ranking")
    _add_source_args(write)
    check = sub.add_parser("check", help="fail if any generated file is stale")
    boundary = sub.add_parser("boundary", help="the genes one single-gene run would resolve")
    boundary.add_argument("--arm-pairs", type=int, default=ARM_PAIRS)
    boundary.add_argument("--max-arm-pairs", type=int, default=FEASIBLE_ARM_PAIRS)
    sub.add_parser("table", help="print the ledger as a table")
    args = ap.parse_args(argv)

    if args.command == "list":
        ledger = json.loads(LEDGER_JSON.read_text()) if LEDGER_JSON.exists() else {}
        ledger_rows = {g["tag"]: g for g in ledger.get("genes", [])}
        selected = set(ledger.get("rules", {}).get(
            "deployment_genome", OPERATOR_DEFAULT_ON))
        for row in genes():
            verdict = ledger_rows.get(row.tag, {})
            print(f"{row.tag:<32} {row.kind:<26} {'on ' if row.tag in selected else 'off'}  "
                  f"{verdict.get('verdict', 'unmeasured')}")
        return 0
    if args.command == "table":
        print_table(json.loads(LEDGER_JSON.read_text()))
        return 0
    if args.command == "boundary":
        ledger = json.loads(LEDGER_JSON.read_text())
        print_boundary(ledger, args.arm_pairs, args.max_arm_pairs)
        return 0

    if args.command == "check":
        recorded = json.loads(LEDGER_JSON.read_text())
        ledger = rebuild_from_ledger(recorded)
        drift = []
        if deployment_genome_of(recorded) != normalize_deployment_genome(OPERATOR_DEFAULT_ON):
            drift.append("operator deployment genome")
        if render_json(ledger) != LEDGER_JSON.read_text():
            drift.append(str(LEDGER_JSON.relative_to(ROOT)))
        if rust_block_of(REGISTRY_PATH.read_text(encoding="utf-8")) != render_rust(ledger):
            drift.append(f"{REGISTRY} (generated block)")
        if render(ledger) != RANKING_MD.read_text():
            drift.append(str(RANKING_MD.relative_to(ROOT)))
        if drift:
            print("genes: out of date — " + ", ".join(drift) + "; run `python3 tools/genes.py write`")
            return 1
        print("genes: ledger, verdict block and ranking are current")
        return 0

    # source / write
    current = json.loads(LEDGER_JSON.read_text()) if LEDGER_JSON.exists() else None
    recorded_reporting = reporting_batches_from_ledger(current) if current else []
    recorded_reporting_notes = reporting_batch_notes_from_ledger(current) if current else {}
    entered_reporting = [Path(path).resolve() for path in args.reporting_batch]
    if args.reporting_unverified_build and not entered_reporting:
        raise SystemExit("--reporting-unverified-build requires --reporting-batch FILE")
    reporting = latest_reporting_batches(entered_reporting, recorded_reporting)
    reporting_notes = dict(recorded_reporting_notes)
    if args.reporting_unverified_build:
        for path in entered_reporting:
            reporting_notes[path.name] = args.reporting_unverified_build
    if getattr(args, "sources", None):
        notes: dict[str, str] = {}
        # New sources are appended to the ones the ledger already records.
        recorded = sources_from_ledger(current) if current else []
        recorded_notes = notes_from_ledger(current) if current else {}
        entered = sources_from_args(args, notes)
        paths = recorded + [p for p in entered if p not in recorded]
        ledger = build_ledger(paths, build_notes={**recorded_notes, **notes},
                              reporting_batches=reporting,
                              reporting_build_notes=reporting_notes)
    else:
        if current is None:
            raise SystemExit("no existing ledger; provide at least one source")
        ledger = build_ledger(sources_from_ledger(current),
                              build_notes=notes_from_ledger(current),
                              reporting_batches=reporting,
                              reporting_build_notes=reporting_notes)
    LEDGER_JSON.write_text(render_json(ledger))
    REGISTRY_PATH.write_text(registry_with_block(render_rust(ledger)), encoding="utf-8")
    RANKING_MD.write_text(render(ledger))
    print_table(ledger)
    print(f"wrote {LEDGER_JSON.relative_to(ROOT)}, the verdict block in {REGISTRY} and {RANKING_MD.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
