# Production ramp audit and rejected worked-tile candidate

The production-growth goal remains open. The final controller is unchanged by
this experiment. Twelve fresh Emperor pairs do not justify promoting the
worked-tile scoring candidate, despite a favorable three-pair turn-100 pilot.

## Live evidence

`tools/civ6_production_report.py` reads the first state of each turn and the
plot records available by that state. Rivals' total production and city counts
come from their public statistics; a partial visible city roster cannot supply
those totals. Missing plot coverage, turns, and checkpoints stay explicit.
Its rate sum is observed start-of-turn production, not production spent.

Three recent King/Online/Simón/Tiny Pangaea Domination traces are recorded in
[the fixed live report](2026-10-03-production-worked-tiles-live-baseline.json).
They use the separate ongoing live branch, not this experiment's baseline.
They are diagnostic evidence, not matched treatment games.

In `civvis-20261004T033533Z`, turn 100 has nine cities and 79.01 production
per turn, against Germany's nine cities and 173.14. Our 8.78 production per
city is less than half Germany's 19.24. Thirty-one of our 52 worked noncenter
tiles have production and no improvement; all 52 have plot records. The seat
has one Builder carrying three charges. At turn 150, our production is 90.91
against the best observed rival's 366.69. Bare productive tiles are an audit
lead: these counts do not establish that every tile is legal or safe to improve.

## Candidate and decision

The candidate is preserved in checkpoint `5cb344deb` in this PR's history.
Named victory Builders replace printed improvement yields with a read-only
local forecast from the engine's tile-yield function, including researched
bonuses, standing improvements, pillage, and feature removal. Worked land gets
full yield credit; unworked land gets one quarter. Resource access, tourism,
boosts, quests, travel costs, and existing safety checks retain their separate
valuation. Adaptive controllers keep their previous policy.

The one-quarter coefficient is a hypothesis. Native citizens can change jobs
after an improvement, so a fixed current assignment can undervalue land they
would work next. The local forecast also does not simulate all neighboring or
empire-wide consequences. Correct local accounting alone does not prove a
better production ramp.

Four development seeds, 61003000–003, initially show approximately +13.6%
turn-100 production across the three matched games that reach that checkpoint.
Across all four turn-75 pairs the increase is only +0.6%. One treatment empire
is eliminated. These mixed results motivated fresh confirmation rather than
promotion.

Twelve fresh seeds, 61003100–111, use the identical profile and coefficient:

| Checkpoint | Matched games | Production/turn control | Candidate | Change | Rate-sum change | Science change | Culture change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 25 | 12 | 16.14 | 16.14 | 0.0% | 0.0% | 0.0% | 0.0% |
| 50 | 12 | 30.30 | 30.72 | +1.4% | −0.1% | +1.0% | +1.9% |
| 75 | 12 | 52.66 | 53.80 | +2.2% | +0.2% | −1.4% | +0.9% |
| 100 | 11 | 75.68 | 74.57 | −1.5% | −0.2% | +2.1% | −2.8% |
| 125 | 8 | 125.58 | 113.44 | −9.7% | −5.4% | −16.8% | −27.7% |
| 150 | 5 | 155.04 | 161.29 | +4.0% | −4.2% | −5.8% | −1.3% |

Later rows include only seed pairs where both games reach the checkpoint.
Game-ending censoring differs between arms; those rows are descriptive,
conditional comparisons, not estimates for every game. No focal seat wins
any of the twelve confirmation games in either arm. The sample is small and
does not establish strength or equivalence to Firaxis. There is no persuasive
early cumulative gain, and accepting the later science/culture tradeoff would
not serve the production goal. **Reject the candidate; restore the controller.**

## Reproduction and validation

The [probe source](2026-10-03-production-worked-tiles-probe.rs) fixes four majors,
60×38 Pangaea, six city-states, Online speed, a 150-turn cap, Emperor majors
and barbarians, randomized rivals, and Gran Colombia in seat zero. Only the
focal seat is handicap-exempt. It uses `AdvancedAi::targeting(Domination)`
with the gene ledger; rivals use `AdvancedAi::fleet`. This is a native
development probe, not the current forced live genome or a Firaxis trial.

Baseline is clean claim revision `45137644f`. The candidate was built from the
working tree whose two implementation-file hashes match checkpoint `5cb344deb`;
its binary stamp still names the earlier claim. The
[manifest](2026-10-03-production-worked-tiles-manifest.json) records that
distinction, source and binary hashes, seeds, setup, replay hashes, and artifact
hashes. The tracked probe is the formatted version of the compiled source.

Build each revision with `cargo build --profile ci --locked --lib`, then:

```sh
rustc --edition=2021 -C opt-level=3 --extern civvis=target/ci/libcivvis.rlib \
  -L dependency=target/ci/deps docs/eval/2026-10-03-production-worked-tiles-probe.rs \
  -o /tmp/production-probe
/tmp/production-probe 61003100 12 emperor
```

Both arms' development and confirmation CSVs and outcome logs are preserved
beside this note. Raw rows retain every checkpoint; losses and early endings
are not removed. Games, not interacting seats, are the experimental units.

Four recorded-state prefixes were replayed with `civvis_orders --serve
--fresh-board --victory domination --explain`. At turn 100, the candidate
replaces Builder 3342338's local Quarry order with movement toward a worked
production job. The turn-50, 75, and 125 order lists are unchanged. These are
proposed orders, not actuated outcomes or proof of a production gain.

The experimental controller passes thirteen focused tests and the complete
locked CI-profile suite: 4,539 passed, 49 library tests and four documentation
examples ignored. Tests compare forecast and actual improvement yields,
researched Mine bonuses, worked-tile routing through both routing branches,
resource access, host residuals, memo isolation, and existing safety fallbacks.
That green suite did not override the negative confirmation result.

The final report has three tests covering duplicate frames, turn gaps, public
rival totals, builder charges, plot chunks, unknown coverage, and incomplete
live lines. The final controller diff against the task base is empty.

Next investigation: Builder supply and arrival time, local productive work,
housing and useful trade capacity. Pricing a better job cannot accelerate a
city while it has no Builder available to perform it.
