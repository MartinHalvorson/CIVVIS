# Current evaluation status

<!-- GENERATED FILE: python3 tools/eval_manifest.py --write -->

This page is generated from `src/elo.rs` and `docs/civ6_ladder.json`.
The append-only experiment evidence remains in `docs/EVAL.md`; this
page is the current inventory and live-bridge snapshot.

## Registry

| inventory | count |
|---|---:|
| Built-in agents | 8 |
| Evaluator-only agents | 197 |
| Live-bridge treatments | 74 |
| Firaxis-only treatments | 23 |
| Native engine-repair treatments | 51 |
| Withholdable live treatments | 51 |

## Live ladder

- Attempts recorded: **307**
- Configured attempts: **303**
- Terminal outcomes: **199**
- Configured wins: **2**
- Latest ledger entry: **2026-08-17T05:43:41Z**

Regenerate with `python3 tools/eval_manifest.py --write`; CI runs
`--check` so registry or ledger changes cannot silently leave this
snapshot stale.
