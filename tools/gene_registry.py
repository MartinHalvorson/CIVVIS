#!/usr/bin/env python3
"""Read the gene registry — `src/ai/advanced/genes.rs` — without a Rust toolchain.

⭐ ONE REGISTRY, ONE READER. Every gene is declared once, as a row of `GENES`:

    Gene { tag: "war-economy", field: "war_economy", kind: Kind::OptIn,
           enable: AdvancedAi::enable_war_economy, disable: AdvancedAi::disable_war_economy },

and every Python tool that needs the list — the ledger, the ranking, the
manifest, the fires check — reads it through here, so the rule for "what is a
gene and what kind" is written once in each language. The Rust side pins its
half in `gene_screen.rs` (`tags_from_source_tables`) against the compiled
table; this side is held to the same answer by `tools/test_gene_ledger.py`.

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
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
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
