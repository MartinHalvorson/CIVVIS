# Current evaluation status

<!-- GENERATED FILE: python3 tools/eval_manifest.py --write -->

This page is generated from `src/elo.rs` and `docs/civ6_ladder.json`.
The append-only experiment evidence remains in `docs/EVAL.md`; this
page is the current inventory and live-bridge snapshot.

## Registry

| inventory | count |
|---|---:|
| Built-in agents | 8 |
| Evaluator-only agents | 199 |
| Live-bridge treatments | 74 |
| Firaxis-only treatments | 23 |
| Native engine-repair treatments | 51 |
| Withholdable live treatments | 51 |

## Live ladder

- Attempts recorded: **313**
- Configured attempts: **309**
- Terminal outcomes: **205**
- Configured wins: **3**
- Latest ledger entry: **2026-08-18T03:16:49Z**

Regenerate with `python3 tools/eval_manifest.py --write`; CI runs
`--check` so registry or ledger changes cannot silently leave this
snapshot stale.
