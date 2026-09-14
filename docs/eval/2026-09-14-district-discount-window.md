# Lock an expiring district discount

## Strategy and engine audit

Victoria's [district-discount analysis](https://forums.civfanatics.com/resources/civ-vi-district-discounts.27783/)
(accessed 2026-09-14) explains why district unlock order matters: completing
research can increase the number of available district families and remove
an underbuilt family's discount. Placement locks the cost. This is expert
strategy advice, not a controlled estimate of the resulting win-rate gain.

CIVVIS already models those mechanisms in `district_underbuilt_discount`,
`district_cost_for_placement` and `item_cost_for_city`. Its ordinary production
scorer uses today's cost but does not value the possibility that a discount
will expire. Existing district planning coordinates sites and construction
order. The new rule handles the price deadline without suspending research
or placing speculative foundations by switching queues twice.

## Independently screenable hypothesis

`lock-expiring-district-discount` is a default-off opt-in. The ordinary
production shortlist calls it after scoring items and applying policy-card
preferences. It operates only in an idle, loyal, peaceful city without a
recent attack or a threatened-city, Recovery or Conquest posture.

A currently selected technology or civic must be at least 90% complete and
unlock a specialty district. This is a progress window, not a claim that
research must complete on the next turn. Only legal, unplaced districts
scoring at least 85% of the ordinary best item are considered. Existing
foundations have already locked their costs and receive no premium.

One forecast per qualifying menu grants the pending unlocks on a private
copy. Both current and future discounts come from the engine's existing
function; its visibility expands to the crate, with no game-rule change.
The forecast must show an actual discount reduction and a higher placement
cost. Growth in the ordinary progress-based price alone does not qualify.
The district score increases by the future/current price ratio, capped at
18%. This can decide a close build choice but cannot rescue low-value filler.
The thresholds and cap are hypotheses, not fitted optimum values.

The forecast removes observed host menu prices because those quote today's
trees. Before trusting it, the native model must reproduce the current
actual quote within one production point. A discrepancy leaves the scores
unchanged. Agreement today cannot prove every future host modifier is modeled;
the rule remains an opt-in for that reason as well as unmeasured strength.

Cost and discount queries retain unique-district pricing, the Government
Plaza's smaller discount, completed districts and existing foundations. A
normal `Produce` action locks the chosen district's price. Research and
existing emergency/expansion reservations retain their own authority.

## Verification plan

Tests cover a close decision, rejected filler, the real production-shortlist
hook, technology and civic unlocks, stable discounts, active queues,
defensive priorities, Government Plaza pricing, and matching/mismatched host
quotes. A controlled normal-turn scenario places the district before research
completes and compares its locked price with a clone that waits to place it.

A preregistered six-game smoke probe uses seeds 914358000–914358005 and
`--genes lock-expiring-district-discount --difficulty emperor`: six majors,
74x46 Continents, nine city-states, Online speed, all victory conditions,
250-turn clock. Majors use Emperor; barbarians retain the harness's independent
Deity setting. The probe is an execution check and is excluded from the
promotion ledger. It is not sized to establish a win-rate improvement.

The clean source revision is `b6b63343e6023764969439432245da38cfb60c40`.
The binary's SHA-256 is
`eb1c2a34cc73472285b61c0d9a55ac47232f475caa67a80f77b9c819d7a19d5b`;
both are recorded in the raw header. The binary was built using
`cargo build --profile ci --locked --features developer-tools --bin gene_screen`.

```sh
CIVVIS_COMMIT=b6b63343e6023764969439432245da38cfb60c40 \
  target/ci/gene_screen --genes lock-expiring-district-discount \
  --games 6 --start-seed 914358000 --jobs 3 --difficulty emperor \
  --out /Users/martbot/civvis-runs/2026-09-14-district-discount-window/rows.jsonl
target/ci/gene_screen \
  --analyze /Users/martbot/civvis-runs/2026-09-14-district-discount-window/rows.jsonl \
  --json docs/gene_screens/fires/2026-09-14-district-discount-window.json
```

## Completed result

All six preregistered games and 36 seats completed without a simulation error.
Nine seats had the gene on and 27 had it off; there were four Science endings,
one Culture ending and one Score ending. The analyzer artifact is
[`2026-09-14-district-discount-window.json`](../gene_screens/fires/2026-09-14-district-discount-window.json).

| Outcome | On minus off | Approximate 95% interval |
| --- | ---: | ---: |
| Win rate | -7.41 percentage points | -34.56 to +19.75 |
| Score share | -1.09 percentage points | -4.66 to +2.48 |

These intervals use the analyzer's standard errors and span zero. The sample
provides no basis to claim stronger play, promote the gene, or tune its
thresholds. No compute-cost estimate is available from this small run.

The firing-evidence gate accepts the committed artifact. Its nonzero independent
seat contrast is not direct instrumentation of the heuristic firing. The
controlled production-menu and normal-turn price-lock tests establish the
specific causal behavior; the six games establish execution compatibility.

Final local validation: `cargo test --profile ci --locked` passed 3,774 tests
(with 53 existing ignored tests), including all seven new tests. The Python
registry, append-point, manifest and firing suites ran 237 tests with one existing
skip. Registry generation, evaluation manifest, deployment-cost, firing-evidence,
Rust formatting and diff-whitespace checks passed. No game rules or deployment
defaults changed.

After the probe, later registrations on main were integrated, preserving their
existing gene positions and placing this new gene last. The discount-scoring
behavior, tests and exposed engine helper are unchanged from the probed revision.
The only subsequent edit to the scoring module removes two redundant explicit
dereferences requested by Clippy. The 3,774-test local run preceded the final
registry refresh. Each refresh is validated with the Rust quality gate (including
compilation), append/metadata/firing checks, and the full PR CI gate.
