#!/usr/bin/env python3
"""Build the gene ledger — what the screens say about every gene, and the
deployment genome that follows — from `gene_screen --analyze --json` outputs.

Operator directive 2026-08-20: the defaults for the genes reflect our best
genome; only genes that provably help are on, unhelpful genes default off.
This tool is the one place that decision is made, and it is made from data:

    python3 tools/gene_ledger.py --write \\
        --source docs/gene_screens/<native>.json --regime native \\
        --source docs/gene_screens/<war>.json --regime war \\
        --source docs/gene_screens/<war-repaired>.json --regime war

writes `docs/gene_ledger.json` and the generated Rust table
`src/ai/advanced/gene_ledger_table.rs`, which `AdvancedAi::apply_gene_ledger`
reads to withhold every treatment the ledger does not find helpful and to
enable every opt-in it does. `--check` re-derives both from the sources the
JSON ledger recorded and fails if either file has drifted; the same check is
`tools/test_gene_ledger.py`'s `GeneratedFiles`, which the `collaboration-policy`
workflow's `unittest discover` runs on every PR.

Verdict rules (repeated in src/ai/advanced/gene_ledger.rs):

- helps      in a regime: win z >= 2 and share z > -2, or share z >= 2 and
             win z > -2 — the screen's own `*` flag. Past the run's
             family-wise bar is recorded as `family_wise`, not required:
             with sixty-odd genes the family-wise bar would leave three on.
- hurts      the mirror image.
- unresolved otherwise — including a gene whose two axes disagree past
             |z| >= 2 (recorded as `conflict`) and a gene no screen measured.

The native (all six lanes) regime governs the verdict when it resolves; a
gene unresolved natively takes the war regime's verdict if that resolves (a
regime where it provably helps and none where it provably hurts — or the
reverse); otherwise it is unresolved. Later `--source` arguments override
earlier ones per gene and regime, so a repaired gene's re-screen replaces its
pre-repair number while the rest of the pre-repair screen stands.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
LEDGER_JSON = ROOT / "docs" / "gene_ledger.json"
LEDGER_RS = ROOT / "src" / "ai" / "advanced" / "gene_ledger_table.rs"
REGIMES = ("native", "war")
Z_BAR = 2.0


def axis_verdict(win_z: float, share_z: float) -> str:
    """One regime's verdict from its two z scores."""
    helps = (win_z >= Z_BAR and share_z > -Z_BAR) or (share_z >= Z_BAR and win_z > -Z_BAR)
    hurts = (win_z <= -Z_BAR and share_z < Z_BAR) or (share_z <= -Z_BAR and win_z < Z_BAR)
    if helps and not hurts:
        return "helps"
    if hurts and not helps:
        return "hurts"
    return "unresolved"


def axes_conflict(win_z: float, share_z: float) -> bool:
    return (win_z >= Z_BAR and share_z <= -Z_BAR) or (share_z >= Z_BAR and win_z <= -Z_BAR)


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
        tranches.append({
            "position": str(tranche["position"]),
            "pairs": int(tranche["pairs"]),
            "win_delta_pp": round(float(tranche["win_delta_pp"]), 3),
            "win_z": round(float(tranche["win_z"]), 3),
        })
    if tranches:
        measure["win_tranches"] = tranches
    return measure


def build_ledger(sources: list[tuple[Path, str]]) -> dict:
    """Merge the sources into one ledger object (the JSON file's content)."""
    measures: dict[str, dict[str, dict]] = {}
    family: dict[str, float] = {}
    recorded = []
    for path, regime in sources:
        if regime not in REGIMES:
            raise SystemExit(f"--regime {regime!r} is not one of {REGIMES}")
        data = load_source(path)
        declared = data.get("regime", "native")
        if (regime == "native") != (declared == "native"):
            raise SystemExit(
                f"{path.name} was played in the {declared!r} regime but is being "
                f"recorded as {regime!r}; the lanes decide which genes can act"
            )
        name = path.name
        family[name] = float(data.get("family_wise_z", 0.0))
        recorded.append({
            "path": str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
            "regime": regime,
            "complete_pairs": int(data.get("complete_pairs", 0)),
            "family_wise_z": round(family[name], 3),
            "profile": {
                key: data.get("profile", {}).get(key)
                for key in ("players", "width", "height", "speed", "victories",
                            "all_seats", "baseline", "field", "design", "start_seed")
            },
        })
        for gene in data.get("genes", []):
            measures.setdefault(gene["tag"], {})[regime] = measure_from(gene, name)

    genes = []
    for tag in sorted(measures):
        by_regime = measures[tag]
        per = {regime: axis_verdict(m["win_z"], m["share_z"]) for regime, m in by_regime.items()}
        deciding = None
        for regime in REGIMES:
            if per.get(regime, "unresolved") != "unresolved":
                deciding = regime
                break
        verdict = per[deciding] if deciding else "unresolved"
        conflict = any(
            axes_conflict(m["win_z"], m["share_z"]) for m in by_regime.values()
        ) or (len({v for v in per.values() if v != "unresolved"}) > 1)
        family_wise = False
        if deciding:
            m = by_regime[deciding]
            bar = family[m["source"]]
            family_wise = bar > 0 and max(abs(m["win_z"]), abs(m["share_z"])) >= bar
        genes.append({
            "tag": tag,
            "verdict": verdict,
            "default_on": verdict == "helps",
            "deciding_regime": deciding,
            "family_wise": family_wise,
            "conflict": conflict,
            "native": by_regime.get("native"),
            "war": by_regime.get("war"),
        })
    counts = {
        "helps": sum(g["verdict"] == "helps" for g in genes),
        "hurts": sum(g["verdict"] == "hurts" for g in genes),
        "unresolved": sum(g["verdict"] == "unresolved" for g in genes),
    }
    return {
        "kind": "gene_ledger",
        "rules": {
            "z_bar": Z_BAR,
            "helps": "win z >= 2 with share z > -2, or share z >= 2 with win z > -2, in the deciding regime",
            "hurts": "the mirror image",
            "deciding_regime": "native when it resolves, else war when it resolves, else unresolved",
            "default_on": "verdict == helps",
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
        lines.append(f"        family_wise: {'true' if gene['family_wise'] else 'false'},")
        lines.append(f"        native: {rust_measure(gene['native'])},")
        lines.append(f"        war: {rust_measure(gene['war'])},")
        lines.append("    },")
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def render_json(ledger: dict) -> str:
    return json.dumps(ledger, indent=2, sort_keys=False) + "\n"


def print_table(ledger: dict) -> None:
    print(f"gene ledger · {len(ledger['genes'])} genes · "
          f"helps {ledger['counts']['helps']} · hurts {ledger['counts']['hurts']} · "
          f"unresolved {ledger['counts']['unresolved']}")
    for src in ledger["sources"]:
        print(f"  source {src['regime']:<6} {src['path']}  ({src['complete_pairs']} pairs, "
              f"family-wise |z|≥{src['family_wise_z']})")
    print(f"{'gene':<30} {'verdict':<10} {'default':<7} {'by':<6} {'native win/share z':<20} war win/share z")
    order = {"helps": 0, "unresolved": 1, "hurts": 2}
    for gene in sorted(ledger["genes"], key=lambda g: (order[g["verdict"]], g["tag"])):
        def z(m):
            return "-" if m is None else f"{m['win_z']:+.2f}/{m['share_z']:+.2f}"
        flag = "**" if gene["family_wise"] else ("!" if gene["conflict"] else "")
        print(f"{gene['tag']:<30} {gene['verdict']:<10} {'on' if gene['default_on'] else 'off':<7} "
              f"{(gene['deciding_regime'] or '-'):<6} {z(gene['native']):<20} {z(gene['war'])} {flag}")


def sources_from_args(args) -> list[tuple[Path, str]]:
    if len(args.source) != len(args.regime):
        raise SystemExit("give one --regime per --source, in the same order")
    return [(Path(p).resolve(), r) for p, r in zip(args.source, args.regime)]


def sources_from_ledger(ledger: dict) -> list[tuple[Path, str]]:
    return [((ROOT / s["path"]).resolve(), s["regime"]) for s in ledger["sources"]]


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--source", action="append", default=[],
                    help="a `gene_screen --analyze --json` file (repeatable; later wins per gene+regime)")
    ap.add_argument("--regime", action="append", default=[], choices=REGIMES,
                    help="the regime the matching --source was played in")
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
