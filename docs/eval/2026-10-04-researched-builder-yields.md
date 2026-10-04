# Rejected researched-production correction for Builder jobs

The candidate changes real Builder job rankings in focused fixtures but does
not improve the early production ramp in the paired pilot. Restore the scorer;
retain the earlier production-research unlock policy from #3886.

## Hypothesis

The named-Domination Builder scorer uses printed improvement yields and
subtracts the standing improvement's printed value. That production component
does not account for a researched Mine bonus or the same bonus given up by a
replacement. The candidate at `91e9f1cf5` reconciles only that component with
`Game::improvement_yield_change`: modeled local production after the candidate
improvement minus production before it, including researched bonuses and any
removed feature. The comparison is read-only and host corrections cancel.

Resource, boost and tourism premiums retain their existing values. The separate
weak-city foundation premium remains conservative and based on printed gains.
No Builder production reservation, routing, capture-safety, queue, engine,
rule or ledger change is added. Adaptive and other named victory lanes retain
their scores. A more accurate local score does not guarantee that work is
completed, worked by a citizen, or valuable enough to change an empire's ramp.

Eleven focused tests pass, including two new cases. The real Builder controller
chooses and completes a researched two-production Mine when its printed value
would favor a Lumber Mill. Replacing a researched Mine with a legal Farm gives
up two production; a pillaged Mine does not supply that standing yield.
The tests also retain the existing resource, reservation, route and queue
checks. Fixtures required `Arc::make_mut` for the observed city adjustment and
a fresh movement allowance after entering a Hill. Neither correction changes
the candidate runtime, and both precede candidate pilot games.

## Frozen paired pilot

The fresh control is compiled at initial checkpoint `37a27d5f0`, whose runtime
matches task base `153850667` (merged #3890). The candidate is frozen at
`91e9f1cf5`. Full hashes are in the adjacent manifest. The ledger is the same
in both arms. No tuning occurs inside the block.

Run all four Emperor seeds `61005400..61005403` and all four Deity seeds
`61005500..61005503` in both arms. The probe uses four majors on 60×38 Pangaea,
six city states, Online speed, a 150-turn cap and randomized rivals. Gran
Colombia is the focal civ, with `AdvancedAi::targeting(Domination)` and the
gene ledger. Only the focal player is exempt from difficulty bonuses; rivals
use `AdvancedAi::fleet` with Emperor/Deity bonuses. All victory conditions
remain enabled. These are native simulations, not Firaxis games or proof of
competitive live-game strength.

| Difficulty | Turn | Matched games | Production control → candidate | Change | Cumulative production | Science | Culture |
|---|---:|---:|---:|---:|---:|---:|---:|
| Emperor | 50 | 4 | 36.225 → 36.225 | 0.00% | 0.00% | 0.00% | 0.00% |
| Emperor | 75 | 4 | 69.000 → 69.000 | 0.00% | 0.00% | 0.00% | 0.00% |
| Emperor | 100 | 4 | 97.400 → 99.900 | +2.57% | +0.70% | −1.32% | +2.34% |
| Emperor | 125 | 3 | 130.500 → 133.833 | +2.55% | +0.42% | −0.05% | −0.70% |
| Emperor | 150 | 3 | 173.600 → 173.200 | −0.23% | +1.13% | −0.31% | −2.19% |
| Deity | 50 | 4 | 26.475 → 26.475 | 0.00% | 0.00% | 0.00% | 0.00% |
| Deity | 75 | 4 | 35.950 → 35.950 | 0.00% | 0.00% | 0.00% | 0.00% |
| Deity | 100 | 3 | 42.833 → 43.133 | +0.70% | +0.41% | −1.93% | +1.56% |
| Deity | 125 | 3 | 53.100 → 56.067 | +5.59% | +1.94% | +0.60% | −6.79% |
| Deity | 150 | 3 | 46.500 → 44.600 | −4.09% | +2.63% | −27.71% | −11.10% |

Every paired checkpoint through turn 75 is unchanged. At turn 100, only one
pair per difficulty raises production. Later checkpoint means are conditional
on both games reaching that turn; eliminated focal players remain included
while their worlds continue. The summary retains all missing checkpoint seeds,
pair directions, cities, production ratios and final outcomes.

There are no focal wins in either arm. All Emperor focal players survive;
one Deity focal player is eliminated in each arm. Native turn-cap adjudications
are reported as engine outcomes, without interpreting them as Firaxis wins.
At turn 75 the mean focal/strongest-rival production ratio is unchanged:
0.609 on Emperor and 0.173 on Deity. The four-seed samples do not support a
production-parity claim or comparison with other seed blocks.

Reject on the pilot: no early rate or cumulative gain, small later production
changes, and a late Deity science loss. Reserved confirmation seeds
`61005600..61005611` and `61005700..61005707` were not run. Restore both changed
source files to the task base. The prototype remains recoverable from the PR
checkpoint, while the final diff preserves only evidence.

## Validation and reproduction

The prototype passes all eleven focused tests and the full locked CI-profile
suite: 4,544 tests, 53 ignored. Experimental changed-line Rust quality passes.
The restored tree passes 4,542 tests (53 ignored). It matches main
`153850667` in source, rules and gene ledger. Final changed-line quality also passes; the final diff contains only evidence.

Compile the library on each frozen checkpoint with
`cargo build --profile ci --locked --lib`, then use the archived probe:

```sh
rustc --edition=2021 -C opt-level=3 \
  --extern civvis=target/ci/libcivvis.rlib -L dependency=target/ci/deps \
  docs/eval/2026-10-04-researched-builder-yields-probe.rs -o /tmp/production-probe
/tmp/production-probe 61005400 4 emperor
/tmp/production-probe 61005500 4 deity
python3 docs/eval/2026-10-04-researched-builder-yields-summarize.py pilot
```

Keep the same ledger, settings and seeds in both arms. The manifest records
the full source commits and source, fixture, library, binary, ledger and
artifact hashes. The summarizer checks unique checkpoints and every expected
final outcome, then regenerates the archived summary.

This isolates another insufficient lever: local job-price corrections alone
do not move early production on these seeds. Subsequent work should measure
the formation of productive cities and completed investments before changing
their priorities, and retain survival and research alongside production.
