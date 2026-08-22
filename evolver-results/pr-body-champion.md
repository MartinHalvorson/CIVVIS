## Ownership claim

- Machine ID: `martin-mbp`
- Agent/session ID: `claude-evolver`
- Task: deployment champion
- Claimed paths: `data/evolved/best.json`, `docs/EVAL.md`
- Coordinated with: TBD — re-check open PRs before marking ready; #684 is this session's earlier PR and is the measurement this rests on.
- Related issue/request: strategy-evolution loop

## What changed

**One data file.** `data/evolved/best.json` is replaced by the same gen-14
genome with **eleven of its forty genes** — `docs/GENOME.md`'s `economy` (7)
plus `expansion` (4) — reverted to `Weights::default()`:
`city_target`, `settler_min_pop`, `settler_stop_turn`, `min_city_dist`,
`builder_per_city`, `wonder_min_bld`, `faith_builder`, `d_campus`,
`d_commercial`, `d_holy`, `d_theater`. The other twenty-nine are byte-identical
to the incumbent. The file is written with **all forty genes explicit** rather
than relying on serde defaults, so a future change to `Weights::default()`
cannot silently move the shipped champion.

`data/evolved/best.json` is `include_str!`'d by `src/evolve.rs`, so this one
file updates both the on-disk artifact and the embedded fallback.

## Why

Recorded in `docs/EVAL.md` (#684). The incumbent is **+51 on the compact
promotion profile and −30 at deployment** in this repository's own 120-map
matrix, and a pre-registered partition on forty shared deployment maps
attributes that entirely to the eleven genes above: a deck-only calibration
rung scores 53.1%, the yield half alone 44.4%, the other twenty-nine alone
57.5%, the whole champion 42.5%. The candidate keeps the incumbent's compact
strength (55.6% against 56.9% on the same forty maps) while turning −53 into
+53 at deployment.

## The gate

`ai_eval advanced_evolved advanced --matrix --pairs 120 --jobs 5 --seed 67000000`,
pre-registered before the screen was read, decided under the **unmodified**
matrix rule. Result: TBD.

⚠ The gated artifact listed only the twenty-nine overrides and let the other
eleven fall back to `Weights::default()`; the file shipped here writes all
forty explicitly. They were checked gene-for-gene **and then played
against each other's evidence**: both forms run on the same five-map prefix
produce **byte-identical** evaluator output, so the gate's agent and the
shipped agent are demonstrably the same agent, not merely the same on paper. Writing them
explicitly is the point: the fallback form would silently follow any future
change to `Weights::default()`.

## Validation

- [ ] Branch started from current `origin/main`
- [ ] Ownership/overlap is coordinated above
- [ ] Latest `origin/main` merged before ready
- [ ] `git diff --check origin/main...`
- [ ] `cargo test --profile ci --locked`
- [ ] Relevant focused tests
- [ ] Soak run for engine changes, or reason it is not applicable
- [ ] No unrelated formatting, generated output, or runtime artifacts

⚠ This artifact is resolved by 38 evaluator arms, the league seeding and the
embedded fallback, so every strength number measured against `advanced_evolved`
before this change is measured against a different agent after it. The
`docs/EVAL.md` entry says so explicitly.

## Notes for integration

Squash merge only. Delete the branch after merge.
