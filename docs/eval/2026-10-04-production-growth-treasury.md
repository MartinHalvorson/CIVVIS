# Local Builder purchase placement: exploratory production screen

The existing opt-in `treasury-at-work-2-2` did not show a reliable fast production gain over the deployed `treasury-at-work-2`. Keep it disabled. This report preserves eight native games and four explanatory control reruns. It does not change runtime policy or claim high-level AI production parity.

## Frozen comparison

Before play, freeze one probe binary and source, the linked engine library, seeds, and treatment in the smoke manifest. Both arms apply the same deployed ledger; only the focal candidate additionally enables `treasury-at-work-2-2`. That existing option places the zero-Builder purchase in the lowest-production eligible city with local work, rather than allowing empire-wide work to justify a purchase in any lowest-production city. Both versions count queued head Builders and preserve a working treasury reserve.

Use four majors, 60×38 Pangaea, six city-states, Online speed, 150-turn cap, default victories and barbarians. Focal Gran Colombia explicitly targets Domination and alone is exempt from Emperor/Deity handicaps. Rivals use the adaptive fleet. Seeds are Emperor 61006800–61006801 and Deity 61006900–61006901, paired by seed. These four seeds are now consumed. This is a two-pair-per-difficulty exploratory screen, without fresh confirmation or a preregistered promotion threshold.

The linked library was compiled from `afba14116415131f730a462dff4e1d7811aced93`. A scoped Git diff verifies identical `src/`, `data/`, Cargo manifests, and deployed ledger at `dd1f10d2b9c5594dbaf693ddf5926f60b39e9cb7` and this task's parent `d603617e2a07761351ab27bf14f52cdef7f925ef`. Intervening main changes concern reports and host tools. The manifest records library, source and binary SHA-256 hashes and compiler arguments.

## Results

Production and cumulative production are empire totals. Ratios are the mean within-seed focal production divided by the strongest rival's production, allowing the strongest rival to change between arms.

| Difficulty | Turn | Pairs | Production, control → candidate | Change | Cumulative change | Rival ratio, control → candidate |
|---|---:|---:|---:|---:|---:|---:|
| Emperor | 50 | 2 | 33.70 → 33.25 | −1.34% | −0.46% | 0.424 → 0.406 |
| Emperor | 75 | 2 | 56.70 → 51.20 | −9.70% | −2.56% | 0.468 → 0.393 |
| Emperor | 100 | 2 | 84.10 → 96.60 | +14.86% | −1.77% | 0.427 → 0.454 |
| Deity | 50 | 2 | 20.65 → 20.65 | 0% | 0% | 0.224 → 0.224 |
| Deity | 75 | 2 | 29.65 → 29.65 | 0% | 0% | 0.156 → 0.156 |
| Deity | 100 | 2 | 35.00 → 35.00 | 0% | 0% | 0.133 → 0.133 |
| Deity | 150 | 2 | 50.80 → 50.80 | 0% | 0% | 0.062 → 0.062 |

At Emperor turn 75 one pair is worse and one unchanged; at turn 100 one is better and one unchanged. Turn 75 science falls 6.15% and culture falls 2.55%; military power rises 72.53%. The later production rise accompanies a different trajectory and does not establish faster early production growth: cumulative production remains lower through turn 100. Deity checkpoint CSVs and final outcomes are byte-identical between arms.

There are no focal wins. Neither Emperor arm has an elimination. Each Deity arm has one elimination. Emperor seed 61006800 ends at turn 119 with winner 2 in both arms; 61006801 ends at turn 151 with winner 3 in control and turn 119 with winner 1 in candidate. Thus no paired Emperor turn 125/150 checkpoint exists. Preserve missing checkpoints and raw final outcomes; do not forward-fill finished worlds. Elimination zeros observed before the world's finish remain in the aggregates.

## Explanatory control diagnostics

Rerun only the same consumed control seeds with a read-only observer recording empire gold/income, field and queued head Builders, city current production, queue and completion estimate at its current rate, actual Builder gold quote, and worked bare tiles legally permitting a mine, quarry or lumber mill. These four reruns are not additional independent strength samples. Both diagnostic CSVs exactly reproduce their original control CSVs.

- Deity 61006900: turns 50/75/100 have zero field and queued Builders, but only 34.0/6.1/18.9 gold against a 108-gold quote. Worked bare legal productive plots exist. A location change cannot overcome unaffordability even before adding the reserve. At turn 100 the surviving city's current production is zero; its extreme queue ETA is an instantaneous estimate, not a forecast.
- Deity 61006901: at turn 25 a queued Builder is roughly 1.27 turns from completion, gold 92 versus a 100-gold quote, and no worked bare legal productive plot is recorded. At turn 75 there is one field Builder, no queued Builder, 170.5 gold versus a 116-gold quote, and eight worked bare legal productive plots across the empire. Both treasury versions intentionally restrict this purchase to zero Builders, so placement alone cannot address possible insufficient coverage. At turn 100 there are four field Builders and six cities.
- Emperor turn 75 queued Builders are approximately 1.86 and 1.20 turns from completion, with three field Builders in each seed. These sampled queues do not support treating delayed completion as the demonstrated bottleneck.

The observer counts legal improvement types, not guaranteed production deltas, safety, route accessibility, Builder charges or future growth. A field count does not establish that a worker can reach the needy plots promptly. Sparse checkpoints cannot attribute every purchase or identify the sole cause of the production gap. Investigate effective worker coverage and spending opportunity costs before constructing the next treatment; validate actual productive work and survival on fresh paired seeds.

## Artifacts and validation

`*-smoke-probe.rs` and the frozen smoke manifest describe the treatment. `*-smoke-*-control/candidate.csv` and `.txt` retain checkpoint data and all final outcomes. The original smoke summary remains archived; `*-summarize.py` recomputes the stricter `*-smoke-paired-summary.json`, checks expected seeds and finite values, and retains missing checkpoints and all outcomes. `*-diagnostic-probe.rs` and the diagnostic CSV/text files preserve the explanatory observer. The archive manifest hashes every tracked experiment artifact. The `.rs.txt` files preserve the exact executed source bytes matching the frozen hashes; the adjacent `.rs` files differ only by rustfmt formatting for repository quality.

Run `python3 docs/eval/2026-10-04-production-growth-treasury-summarize.py` to recompute paired results. Native probe compilation and all twelve runs completed successfully; diagnostic checkpoint outputs reproduce control exactly. Full repository validation is recorded in the final PR description. No actual Firaxis game was controlled by this experiment, and no live production improvement is established.
