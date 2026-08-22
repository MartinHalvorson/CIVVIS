# Victory-lane gene screens, 2026-08-22

Raw `gene_screen` rows behind CIVVIS PRs #2274 and #2283
(`docs/VICTORY_GENES.md` §8). Kept here rather than in the repo because they
are megabytes of rows; `docs/gene_screens/*.json` holds the analysed summaries.

| file | command |
|---|---|
| `lanes.jsonl` | **TWO sections.** Section 1 = seed 62000000 (§8.1); section 2 = seed 66000000 (§8.4), appended. ⚠ They CANNOT be merged — `lane-congress-favor` was added between the runs, shifting every gene's bit position, and `--analyze` refuses. Split at the second `kind: header` line. |
| `ballot-halves.jsonl` | `--genes lane-congress-ballot,lane-congress-favor,congress-banks-decided --victories diplomatic,score --pairs 900 --all-seats --randomize-civs --players 6 --start-seed 65000000` (§8.5) |
| `diplo.jsonl` | `--genes lane-congress-ballot,congress-banks-decided,congress-counter-votes,envoy-infrastructure --victories diplomatic,score --start-seed 64000000` (§8.2) |
| `spacerace600.jsonl` | `--genes lane-space-race --turns 600 --start-seed 63000000` (§8.3) |

All four were stopped before their `--pairs` target: the box is shared with
eight other agent sessions. Continue any of them with `--append` and a
disjoint `--start-seed`, **against a build whose genome order matches the
file's header**.

Analyse with the binary from a CIVVIS checkout:

    target/ci/gene_screen --analyze ~/civvis-gene-screens/ballot-halves.jsonl
