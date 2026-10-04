# Rejected fast-city Builder service

The fast-city Builder reservation did not improve the early production ramp.
Do not retain its controller changes. The measured control already includes
#3886's earlier research toward productive worked-tile improvements.

## Hypothesis and implementation

Experimental runtime checkpoint `9b03cbed7531cc20a43c6cdd6ac6d326aeb47ae5`
adds a named-Domination reservation. An idle city, or one running a repeatable
non-Spaceport project, can supply nearby worked production jobs if it finishes
a Builder within 16 Standard production turns (8 Online). It uses actual legal
improvement yield differences, counts only owned worked unimproved tiles,
subtracts nearby field Builder charges and prices at most three new charges.
It requires at least four additional production and enough remaining time to
repay the Builder cost after a six-turn travel/operation allowance. That
allowance is a forecast, not a measured route or guarantee of completed work.

The rule skips threatened empires, Recovery, negative income, insufficient
standing land defenders, an already queued Builder, and the existing empire
Builder ceiling. It protects ordinary construction, first-trade capacity and
emergency amenity reservations. Existing Builder movement and capture safety
choose the actual jobs. Adaptive and other victory lanes retain their policy.

Checkpoint `9831ca2631ae0c2b5da18d9401e7f2bbdba7477f` changes only fixtures:
the slow city's production is recalibrated after its worked forests are placed,
and the late-clock case uses the duration clock. Online production costs scale
by 50%, while the 160-Standard-turn eligibility duration ends at Online turn
106. The runtime threshold was unchanged throughout the experiment.

## Paired pilot

Run four Emperor seeds `61005000..61005003` and four Deity seeds
`61005100..61005103`, with both arms on every seed. No tuning occurred within
the block. The candidate is the frozen reservation above; the control binary
is the retained research-only controller at `f0c79ad09`. Its difference from
this task's base `6b6832f06` is formatting only in `advanced.rs`; engine, rules,
ledger and controller behavior match.

The standalone probe uses four majors on 60×38 Pangaea, six city states,
Online speed, a 150-turn cap, randomized rivals, and Gran Colombia as the
focal civ. Only the focal player is exempt from difficulty bonuses. The focal
controller is `AdvancedAi::targeting(Domination)` with the gene ledger; rivals
use `AdvancedAi::fleet` with Emperor or Deity bonuses. All victory conditions
remain enabled. These are native simulation games, not Firaxis games or
evidence of competitive live-game strength.

| Difficulty | Turn | Matched games | Production control → candidate | Change | Cumulative production | Science | Culture |
|---|---:|---:|---:|---:|---:|---:|---:|
| Emperor | 50 | 4 | 30.150 → 30.150 | 0.00% | 0.00% | 0.00% | 0.00% |
| Emperor | 75 | 4 | 51.388 → 50.562 | −1.61% | +0.17% | +0.88% | +0.08% |
| Emperor | 100 | 3 | 79.400 → 78.833 | −0.71% | +0.83% | −7.85% | −6.30% |
| Emperor | 125 | 3 | 108.683 → 109.283 | +0.55% | −0.13% | −6.54% | −0.22% |
| Emperor | 150 | 2 | 70.975 → 113.725 | +60.23% | +7.83% | +2.08% | +2.09% |
| Deity | 50 | 4 | 19.175 → 19.175 | 0.00% | 0.00% | 0.00% | 0.00% |
| Deity | 75 | 4 | 23.763 → 23.763 | 0.00% | 0.00% | 0.00% | 0.00% |
| Deity | 100 | 4 | 37.125 → 29.000 | −21.89% | −6.49% | −15.67% | −11.75% |
| Deity | 125 | 4 | 42.025 → 18.812 | −55.23% | −15.51% | −55.23% | −55.74% |
| Deity | 150 | 2 | 10.800 → 15.125 | +40.05% | −8.54% | +78.79% | +32.44% |

At turn 75, three Emperor pairs are identical and one loses production;
all four Deity pairs are identical. At turn 100, each difficulty has one
production gain and one loss, with the remaining matched pairs unchanged.
The large turn-150 percentages come from only two matched games per
difficulty, after other games have finished. They do not establish an early
ramp or cancel the earlier losses. Checkpoint means use only pairs reaching
that turn, including eliminated focal players when the world is still running.
The summary lists all missing checkpoint seeds and all final outcomes.

There are no focal wins in either arm. All four Emperor focal players survive;
one Deity focal player is eliminated in each arm. A turn-limit adjudication is
reported as the native engine reports it, without treating it as a Firaxis win.

Reject on the pilot. Reserved confirmation seeds `61005200..61005211` and
`61005300..61005307` were not run. Restore the runtime files to the task base
and remove the experimental module/tests from the final tree. The prototype
remains recoverable from the PR checkpoints. No retained Builder, routing,
engine, rule, ledger or real-game setting change is proposed by this report.

## Validation and reproduction

The prototype's three focused tests exercise the actual governor, cross-city
worked jobs, charge coverage, slow and committed queues, and safety/clock
boundaries. They pass after the two fixture corrections above. The full locked
CI-profile suite passes 4,542 tests, with 53 ignored, on the prototype.
Both experimental and final changed-line quality pass. After merging main
`77a8970911f310f831e7524c7cf92410aa128270`, the restored tree also passes
4,542 tests (53 ignored); main added three unrelated plot-refusal tests while
the three prototype tests were removed. The final source, rules and gene
ledger match that main revision exactly.

Build the library on the frozen experimental checkpoint with
`cargo build --profile ci --locked --lib`, then compile the tracked probe:

```sh
rustc --edition=2021 -C opt-level=3 \
  --extern civvis=target/ci/libcivvis.rlib -L dependency=target/ci/deps \
  docs/eval/2026-10-04-fast-builder-service-probe.rs -o /tmp/production-probe
/tmp/production-probe 61005000 4 emperor
/tmp/production-probe 61005100 4 deity
```

The probe source is archived in the final report commit. Use it alongside the
runtime checkpoint above in an isolated checkout. Compile the control against
`f0c79ad09` and use the same seeds, settings and ledger. The adjacent manifest
records full commits and binary, library, ledger, source and artifact hashes.
Recompute all archived paired metrics and outcome accounting with:

```sh
python3 docs/eval/2026-10-04-fast-builder-service-summarize.py pilot
```

The next question is what completed production is displacing productive
investment: this rule can change later outcomes without increasing the early
rate. Actual completed jobs and construction opportunity costs need to explain
the next experiment, rather than treating forecasted Builder work as output.
