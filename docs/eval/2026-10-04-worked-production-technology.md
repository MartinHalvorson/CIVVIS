# Worked-production technology extension: withheld

Keep the production-unlock research rule merged in #3886. This extension does
not demonstrate a faster early production ramp. The final runtime source in
this PR is identical to its task base; only evidence and reproduction files
remain. The production-growth goal remains open.

## Hypothesis and frozen candidate

Checkpoint `6042efd2b` extends the named victory research forecast to installed
worked improvements and technology effects ending in `_production`. It compares
the engine's modeled tile production before and after a speculative technology.
An installed Mine can pay Apprenticeship's extra production without another
Builder charge. Bare tiles retain their best currently legal Builder opportunity;
already improved tiles receive no imagined replacement operation. Pillaged Mines
receive no extra credit. Existing early-clock, prerequisite-cost, threatened-city
and Recovery guards remain.

This distinguishes immediate installed yield from potential yield that still
requires a Builder. It does not forecast all citizen reassignment, neighboring
effects, or city modifiers. Technologies without a matching effect or improvement
unlock are outside this candidate search. Correct local accounting is insufficient
evidence for promotion.

Six focused tests pass, including the real research selector, actual Mine yield
change, memo/parent isolation, legal bare-tile upgrades, missing time, and pillage.
The complete experimental suite passes 4,541 tests, with 49 library tests and
four documentation examples ignored. Changed-line formatting and quality pass.
The initial scaffolding compile omitted a qualified `Pos` type; that was corrected
before the frozen source and any candidate games.

## Fresh paired games

The [probe](2026-10-04-worked-production-technology-probe.rs) fixes four majors,
60×38 Pangaea, six city-states, Online speed and a 150-turn cap. Gran Colombia
in seat zero uses `AdvancedAi::targeting(Domination)` and the compiled gene ledger
without difficulty bonuses. Randomized rivals use `AdvancedAi::fleet` with
Emperor or Deity player and barbarian difficulty. All victory conditions remain
enabled. These are native simulations, not Firaxis or live strength trials.

Twelve Emperor seeds `61004800`–`61004811` and eight Deity seeds
`61004900`–`61004907` were fixed before candidate outcomes, with no replacement
or within-block tuning. Control is the research-only binary measured for #3886
at `f0c79ad09`. Its runtime matches task base `99ec5708a` apart from formatting
and tools/web/docs changes. The ledger hash is unchanged. The
[manifest](2026-10-04-worked-production-technology-manifest.json) records source,
library, binary, replay and artifact hashes.

The observer reads once at turn start. Cumulative production sums those observed
rates; it is not production spent. Later checkpoints include only pairs where
both games reach that turn. Early endings differ, so later results describe a
conditional subset rather than every registered game.

| Difficulty | Turn | Matched pairs | Production control | Candidate | Change | Cumulative change | Science change | Culture change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Emperor | 50 | 12 | 27.60 | 27.60 | 0.0% | 0.0% | 0.0% | 0.0% |
| Emperor | 75 | 12 | 47.11 | 47.29 | +0.4% | +0.2% | −1.3% | −2.2% |
| Emperor | 100 | 12 | 68.62 | 68.39 | −0.3% | +1.6% | +1.0% | −4.4% |
| Emperor | 125 | 7 | 103.02 | 119.80 | +16.3% | +5.7% | −3.7% | +2.9% |
| Emperor | 150 | 4 | 100.30 | 113.26 | +12.9% | +5.8% | +3.1% | +4.0% |
| Deity | 50 | 8 | 27.73 | 27.73 | 0.0% | 0.0% | 0.0% | 0.0% |
| Deity | 75 | 7 | 33.37 | 33.37 | 0.0% | 0.0% | 0.0% | 0.0% |
| Deity | 100 | 7 | 48.96 | 48.96 | 0.0% | +0.02% | −1.5% | 0.0% |
| Deity | 125 | 6 | 65.28 | 67.94 | +4.1% | +2.0% | −2.5% | −0.6% |
| Deity | 150 | 5 | 67.90 | 94.90 | +39.8% | +7.3% | +11.9% | +26.5% |

At turn 75, ten Emperor pairs have identical production, one is higher and one
lower. All seven matched Deity pairs have identical production. Later gains are
concentrated: two of seven Emperor pairs improve at 125, and two of five Deity
pairs improve at 150; most others are unchanged. Every outcome remains in the raw
CSV and logs. Both arms have zero focal wins. Neither Emperor arm loses the
focal empire; each Deity arm has one focal elimination.

The mean per-game production ratio to the strongest rival at turn 75 moves from
0.403 to 0.408 on Emperor and stays 0.182 on Deity. At Deity turn 150 it moves
from 0.116 to 0.144 in the five matched games. The late percentage increase does
not establish competitiveness with the AI. **Withhold the extension and restore
the runtime.** This decision is about the requested early ramp, not a claim that
every later effect is negative.

## Reproduction and live replay

Build each source independently, then compile the probe:

```sh
cargo build --profile ci --locked --lib
rustc --edition=2021 -C opt-level=3 --extern civvis=target/ci/libcivvis.rlib \
  -L dependency=target/ci/deps \
  docs/eval/2026-10-04-worked-production-technology-probe.rs -o /tmp/production-probe
/tmp/production-probe 61004800 12 emperor
/tmp/production-probe 61004900 8 deity
python3 docs/eval/2026-10-04-worked-production-technology-summarize.py
```

The [summary](2026-10-04-worked-production-technology-summary.json) retains all
checkpoint means, matched counts, pair directions, production ratios and final
outcomes. Four previously archived live prefixes were replayed using the candidate
orders binary with `--serve --fresh-board --victory domination --explain`. Orders
at turns 50, 75, 100 and 125 are identical to the #3886 research-only control.
These are proposed orders, with no actuation and no live production claim.

The [read-only installed-improvement probe](2026-10-04-worked-production-technology-live-installed-probe.rs)
suggests these upgrades are a smaller early lever in the sampled live game: at
turn 50, Apprenticeship increases only one currently worked installed Mine.
The observed gains are retained in `2026-10-04-worked-production-technology-live-installed.txt`.
This observation does not establish the best general policy. Next measure Builder completion time, travel, useful
charge coverage and productive jobs that wait behind ongoing city construction.
