# Earlier production from worked-tile research unlocks

The production ramp needs reachable tile improvements before another generic
Builder quota. In the recorded King game `civvis-20261004T033533Z`, Construction
is still unknown at turn 100. At turn 75 the empire has eight cities and one
Builder, while several cities work unimproved forests. Researching Construction
makes Lumber Mills legal on those forests. The existing improvement definition
is `data/improvements.json:431`: Construction, +2 Production, Forest terrain.

The retained change gives named victory controllers a bounded production
research detour. It compares legal improvements on currently worked, owned,
unimproved tiles before and after granting an unknown improvement technology on
a disposable world. It counts additional local production, including researched
bonuses, rather than resource access, future citizens, or adjacency promises.

The detour requires two cities, at least four additional tile production, and
a complete missing-prerequisite cost within forty Standard turns of current
science (twenty Online), bounded by the remaining game clock. It closes after
Standard turn 160. Recovery and a threatened city suppress it. Opening defense,
appointed breakthroughs, barbarian modernization, committed launch research,
and fuel for the standing army retain their earlier research priority. The
detour precedes discretionary wartime modernization and ordinary lane goals.

An already legal target can cross the ordinary era window. Otherwise the
selector chooses a legal missing prerequisite, stopping at known parents;
directly granted technologies must not require researching their ancestors
again. Host remaining-cost quotes are used without another speed discount.

The forecast opens no production queues and makes no changes to the parent
world. Existing Builder supply, production scoring, and work routing remain
unchanged. Tile gains are available opportunities: city yield modifiers,
travel, safety, and actually supplying workers still determine the realized
production. Adaptive genomes retain their previous behavior.

## Paired evidence

The final candidate is `f0c79ad09`. Subsequent source changes are formatting
only; merging current main adds camera/report changes without changing the
Rust simulation, data, deployment inputs, or gene ledger used by the controls.
The preserved control runtime matches the task base. Binary hashes, source
hashes, setup, seeds, and artifact hashes are in the adjacent manifest.

There are twelve fresh Emperor pairs (61004500–61004511) and eight fresh Deity
pairs (61004600–61004607). The focal controller targets Domination with the gene
ledger and no difficulty handicap. Rivals use `AdvancedAi::fleet` with their
difficulty handicap. Maps are four-major, six-city-state, 60×38 Pangaea, Online,
Gran Colombia focal, randomized rival civilizations, capped at 150 turns. These
are CIVVIS simulations, not actuated Civilization VI games or Firaxis AI wins.

The observer reads once at each turn start. Accumulated production is the sum
of those rates, not a ledger of item-specific modified hammers. Each later row
contains only seeds reaching that checkpoint in both arms; early victories
censor games, so later rows describe progressively smaller cohorts.

| Difficulty | Turn | Matched pairs | Production, control → candidate | Rate change | Accumulated change | Science change | Culture change |
|---|---:|---:|---:|---:|---:|---:|---:|
| Emperor | 50 | 12 | 26.81 → 26.94 | +0.5% | +0.0% | +2.4% | 0.0% |
| Emperor | 75 | 12 | 43.24 → 47.28 | +9.4% | +0.9% | −4.0% | −1.4% |
| Emperor | 100 | 11 | 69.34 → 69.00 | −0.5% | +0.5% | +2.7% | +0.3% |
| Emperor | 125 | 8 | 86.66 → 96.23 | +11.0% | +3.3% | +20.8% | +7.5% |
| Emperor | 150 | 7 | 107.81 → 115.84 | +7.4% | +5.8% | +8.5% | −2.3% |
| Deity | 50 | 8 | 28.61 → 28.61 | 0.0% | 0.0% | 0.0% | 0.0% |
| Deity | 75 | 8 | 45.47 → 47.35 | +4.1% | +1.4% | +19.2% | +6.2% |
| Deity | 100 | 5 | 64.94 → 77.78 | +19.8% | +4.2% | +12.4% | +12.5% |
| Deity | 125 | 4 | 80.80 → 96.53 | +19.5% | +10.4% | +22.7% | +9.0% |
| Deity | 150 | 4 | 96.13 → 121.33 | +26.2% | +12.2% | +23.5% | +20.8% |

Both arms have zero focal wins. All Emperor focal seats survive; one Deity
candidate is eliminated while all Deity controls survive. This small sample
supports an earlier production opportunity, with an Emperor research cost at
turn 75, but cannot establish a win-rate or survival improvement.

Production remains behind the strongest rival. At turn 75 the mean per-game
production ratio moves from 0.351 to 0.381 on Emperor and 0.232 to 0.243 on
Deity. At turn 100 it moves from 0.209 to 0.259 in the five matched Deity pairs.
This is progress toward the operator's goal, not demonstrated AI parity.

## Rejected workforce approaches

Earlier checkpoints and their raw tables remain in this PR's history and the
adjacent artifacts. None of their Builder premiums or reservations is retained.

| Candidate | Source checkpoint | Screen | Finding |
|---|---|---|---|
| Idle Builder reservation, two local jobs | `7ff8dbd70` | Four Emperor pairs | No change through turn 100; only a late effect |
| Researched yields, one job, parallel queues | `7a051149f` | Twelve fresh Emperor pairs and four Deity pairs | No change through turn 75; Deity late production loss |
| Building-price Builder production premium | `48930a12b` | Twelve fresh Emperor pairs and eight Deity pairs | No change through turn 75 on either difficulty |
| Research detour plus workforce changes | `0b5bb4537` | Twelve fresh Emperor pairs and eight Deity pairs | Turn-75 production +7.6%/+10.1%, mixed later Emperor costs |
| Research detour alone | `f0c79ad09` | Twenty fresh pairs above | Retained; early gain with a smaller final change |

The first worked/unworked routing experiment was rejected separately in
[PR #3883](https://github.com/MartinHalvorson/CIVVIS/pull/3883) and
`2026-10-03-production-worked-tiles.md`. The idle reservations missed early
jobs because the relevant improvement technology was absent. Increasing
Builder bids could not improve tiles whose improvement was still illegal.

## Verification and limits

Four focused tests cover forecast versus a completed Mine, parent/memo
isolation, worked forests, actual science and prerequisite timing, threatened
cities, adaptive controllers, and the actual research selector taking a legal
Construction unlock across the era window. The complete locked CI-profile Rust
suite passes: 4,539 tests, with 49 library tests and four documentation examples
ignored. Changed-line formatting and Clippy checks pass.

Four archived state prefixes were replayed with `civvis_orders --serve
--fresh-board --victory domination --explain`. At turn 50 the candidate chooses
Horseback Riding toward Construction; the other three order lists (75, 100,
125) are identical. Replay files are recorded beside this note. They are
proposed orders and do not establish a live production outcome.

Further work should measure the live production ramp after this research
change reaches a new game, then address remaining worker service and industrial
capacity against public rival production. The native production report and raw
paired rows allow those comparisons without treating a late rate spike as an
early economic lead.
