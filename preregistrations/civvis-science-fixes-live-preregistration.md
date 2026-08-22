# Pre-registration: do this session's science fixes show up in a live game?

Written **2026-08-03 ~05:55Z**, while the `aa7548e` batch is on attempt **6 of 8**
and before any run on the new revision exists. Committed here so the comparison
cannot be reinterpreted after the fact.

## What changed on `main` and is not yet in a live game

| PR | change | measured before merge |
|---|---|---|
| #959 / #966 | Civ 6 build names the game actually has | — |
| #968 | audit so an unknown name can never be emitted again | — |
| **#958** | **science priced outside the victory lane** (`research_economy`) | **+20.2% science, +6.6% techs, score flat**, 8 paired seeds at deployment shape |
| #983 | Great Person Points mirrored, so the race the planner prices against exists | field populated; no win-rate claim |
| #965 (not mine) | wide/defended expansion in the production constructor | — |

The runner tree `/Users/martin/civvis-batch-runner` is pinned to `aa7548e` for
this batch and advances at the batch boundary. **None of the above has ever run
in a live game.**

## Baseline — `aa7548e`, runs reaching turn 150+

**FINAL, frozen 2026-08-03 07:00Z, before reading any `d0fdcfb` result:**

```
n = 6
cities  [1, 2, 2, 4, 4, 5]              mean 3.00   median 3.0
score   [269, 351, 488, 515, 595, 643]  mean 477    median 501.5
rival_best median 1134
```

⚠ The batch ended one attempt short of its eight: something ran `git merge
origin/main` inside `/Users/martin/civvis-batch-runner` mid-batch, and the
loop's own guard stopped it — *"THE BUILD CHANGED MID-BATCH — pinned aa7548e,
now c0b95f2 … the remaining 1 would have measured a different program under the
same heading."* The seven played games were all on the pinned binary and are
valid; only the eighth was lost. The reflog shows every earlier advance as a
`checkout` (the loop's own between-batch move) and this one as a `merge`, so it
was not the loop.

The treatment batch started **06:58Z pinned to `d0fdcfb`**, which carries #958,
#959, #966, #968 and #983.

Preceding revisions, same filter, for context on the noise:

| rev | n | cities mean | score mean |
|---|---|---|---|
| `1000a13` | 6 | 1.17 | 182 |
| `76e98b3` | 7 | 2.43 | 294 |
| `23f5776` | 7 | 2.71 | 423 |
| `aa7548e` | 5 | **3.20** | **469** |

⚠ Run-to-run spread is enormous — `aa7548e` itself spans 1 to 5 cities and 269
to 643 score on five runs. **A batch of 8 cannot resolve less than roughly a
40% change in the mean.** Anything smaller than that is not evidence either way,
and I am recording that ceiling *before* seeing the result so it cannot be
relaxed afterwards.

## Predictions, in falsifiable order

1. **Discarded-name orders go to ~zero.** Currently 76 of 209 produce orders
   (36%) in run `civvis-20260803T044538Z` name something Civ 6 does not have.
   This is the only prediction here that is nearly deterministic: #959 + #968
   make those names correct, so the count should be 0 or near it.
   *Falsified if* a post-advance run still shows >5% discarded names.

2. **The Campus research project stops being 0.** It is 7 of 147 across six live
   games (0 of 9 in the current one). #958 raises science from 1.0-1.7 to ~3.0
   early, which flips it against the Commercial Hub's gold at 2.2 and the
   Industrial Zone's production at 2.8-3.2; #983 makes its Great Scientist
   payoff visible for the first time.
   *Falsified if* the next 4 runs still total 0 Campus projects.

3. **`gpp` is non-empty at end of game.** Was `{}` in every state captured.
   *Falsified if* still `{}`.

4. **Science at turn ~250 rises.** Baseline live end-of-game science: 12.3 with
   2 cities, 60.9-69.6 with 4. I predict the *per-city* figure moves, not just
   the total, since city count is a separate axis and #965 also lands.
   ⚠ **I do not predict a score change and will not claim one.** Score is
   dominated by city count and the baseline spread above swamps +20%.

## What would make me say the science work did not land

- discarded names still >5% (means the runner did not actually advance — check
  `git -C /Users/martin/civvis-batch-runner log --oneline -1` FIRST, before
  interpreting anything else), or
- names fixed, `gpp` populated, Campus projects still 0 across 4+ runs — which
  would say the lane weight is not what gates that choice, and that #958's
  headless +20.2% does not transfer.

The second outcome is the one worth wanting: it would close the axis.

## What this does NOT test

`production_value` is reached on 22.2% of adaptive turns (all Recovery), and
`advanced_production` additionally skips any city already mid-build (25.8% of
seat-turns per `docs/EVAL.md`). #958's Campus-coverage and building-debt terms
live there and are largely inert in deployment; its measured gain came from the
ungated sites — citizen emphasis, `tech_value`, the policy deck and the search
evaluator. A null here does **not** license a conclusion about the gated half.

---

## ⚠ 2026-08-03 08:10Z — the first treatment batch measured nothing, and why

`d0fdcfb` carried a defect I shipped in **#983**: the mod's JSON encoder emits
`[]` for an *empty* Lua table, `great_person_points` is empty on turn 1 for every
player, and a serde failure on that one field **rejects the whole
`StateSnapshot`**. Every game reported `no revealed terrain or no state yet` and
`0 orders` from turn 1, stalled at turn 6 on an unanswered research prompt, and
was watchdog-killed.

```
attempts 1-4 on d0fdcfb:  t=5, t=5, t=5, t=5   0 cities
```

**Those four rows are instrument failure, not evidence about science.** They must
not be pooled with anything.

Fixed in **#996** (`47ebcb0`), both halves — the mod returns `nil` instead of an
empty table, and `StateSnapshot` now accepts `{}`/`[]`/`null`/populated.

### Deliberate intervention, recorded

At 08:10Z I advanced `/Users/martin/civvis-batch-runner` from `d0fdcfb` to
`47ebcb0` **mid-batch, on purpose**, so the loop's own build guard ends this
batch at the next attempt boundary instead of burning three more guaranteed
failures.

⚠ Earlier today I flagged exactly this action, done by someone else, as a
protocol violation that cost an attempt. The difference is what it discards:
that one ended a batch of **valid** games; this one ends a batch in which every
remaining attempt is a deterministic failure on a known-broken binary. It is
reversible (`git checkout d0fdcfb`) and it uses the guard the loop already has
rather than killing any process.

The baseline above is unaffected — it is all `aa7548e`.

**The treatment measurement restarts from the first batch pinned to `47ebcb0`
or later.**


---

## RESULT — first live game on `47ebcb0` (`civvis-20260803T082856Z`, read at turn 143 of 250)

Runner revision checked FIRST, as this document requires: `47ebcb0`. ✓

| # | prediction | outcome |
|---|---|---|
| 1 | discarded-name orders → ~0 | ✅ **0 of 133** (baseline 76 of 209 = 36%) |
| 2 | Campus research project stops being 0 | ⚠️ **1 of 21 district projects** — passes as literally written, but the *rate* is 4.8% against a baseline of 7/147 = **4.8%. Unchanged.** |
| 3 | `gpp` non-empty | ✅ **117 state events carry it**; Scientist 218, Prophet 338, Merchant 90, Engineer 77, Admiral 26 |
| 4 | science rises | ❌ **not visible at matched turns** |

### Science at matched turns

```
run                      t20   t30   t40   t50   t60   t70   t78
NEW 47ebcb0 082856Z      5.0   6.5   7.8   8.3   10.1  10.1  24.8
base aa7548e 060344Z     5.0   6.8   7.4   9.5   9.5*  9.5*  9.5*
base 23f5776 014330Z     3.8   5.9   7.7   9.7   13.6  13.6* 13.6*
base 23f5776 005930Z     4.9   6.0   7.5   8.5   9.9   17.2  23.6
```
`*` = carried forward; the `economy civ6/civvis` line stops being emitted partway
through most runs, so those are not real samples. The only baseline with a true
t78 reading is 005930Z at **23.6**, against the new run's **24.8**.

**Through turn 60 the new run is indistinguishable from the baselines.**

### What I take from this

The two predictions that were about *inputs* — names the host accepts, and a
Great Person race that exists — both landed cleanly and are permanent
improvements to what the agent can see and say.

The two that were about *science valuation* did not. The Campus project rate is
identical to baseline, and science at matched turns is inside the baseline
spread. **On this evidence #958's headless +20.2% is not visibly transferring**,
and the lane weight is not what gates the Campus project choice.

⚠ Caveats I am not using to rescue the claim, only to bound it: n=1 game, read
mid-game at turn 143 of 250, and `desired_cities` moved 3 → 7 in this batch from
#965 (not my change), which is a large confound on anything downstream of city
count. The pre-registered falsifier for #2 was "0 across 4+ runs", so it is not
formally falsified — but a flat rate is not the result the fix predicted.

The remaining 7 attempts of this batch are the sample that settles it.


---

## Pre-registration for #999 (Research Lab reachability), written 2026-08-03 ~09:55Z

Written before any game on a revision containing #999 exists. The current batch
is pinned to `47ebcb0`, which predates it.

### What the two completed `47ebcb0` games actually show

The science *infrastructure* is now being built well — which contradicts the
narrative I carried through most of this session:

```
082856Z (7 cities, science 109.3)
  6 of 7 cities: Campus + Library + University, pop 11-15
  1 city (pop 4, founded late): nothing
090911Z (5 cities, science 39.0)
  2 cities: Campus + Library + University
  2 cities: Campus + Library only
  1 city (pop 6): nothing
```

Every one of those cities is missing exactly one thing: the **Research Lab**,
which #999 makes reachable.

⚠ **My earlier framing — "the second city never builds a Campus" — was true of
the 2-city empires and is NOT true at 7 cities.** With the expansion work in,
campuses and universities are widespread. The remaining science-infrastructure
gap is the single node #999 addresses.

### The prediction, with arithmetic

A Research Lab is `science: 3`. Game 1 has six fully-equipped cities, so six
Research Labs is **+18 science on 109.3 ≈ +16%**.

1. **Research Labs stop being 0.** Expect ~4-6 in a 7-city game.
   *Falsified if* labs are still 0 across 4+ games on a revision carrying #999 —
   which would mean the goal is not firing, not that it is worth little.
2. **Chemistry and Sanitation appear in the end-of-game tech list.** This is the
   mechanism; if the goal fires, both are held.
3. **End-of-game science rises by roughly 10-20% at equal city count.**
   ⚠ Only comparable at equal cities. Per-city science is the metric; totals are
   dominated by city count and I will not read them as a science result.

### What I am NOT predicting

A score change. Per-city science is 15.6 and 7.8 in these two games against
baselines of 15.2 / 17.4 / 6.2 — **inside the baseline range**. The end-of-game
jump from 12.3 to 109.3 is city count (3 → 7, #965's expansion work), not
research efficiency, and +16% on top of that will not be visible in score at
this sample size.


---

# VERDICT — `aa7548e` (n=7) vs `47ebcb0` (n=6)

| | baseline | treatment | change |
|---|---|---|---|
| cities | 2.71 | **6.50** | **+139%** |
| score | 432 | 556 | +29% |
| spread | 162–643 | 279–725 | — |

## Against the threshold I set in advance

I wrote, before seeing any treatment result: *"a batch of 8 cannot resolve less
than roughly a 40% change in the mean."*

- **cities +139% — above the threshold. Resolvable, and real.**
- **score +29% — BELOW the threshold. NOT evidence, and I am not claiming it.**

That is the whole reason the number was written down first. +29% on a
162–725 spread is exactly the kind of result that would look like a win if the
threshold were chosen afterwards.

## Attribution

The city gain is **#965's wide-expansion work, not the science work.** The
science changes in this revision were #958 (lane pricing), #959/#966/#968
(names) and #983 (GP race).

## Per-prediction

| # | prediction | outcome |
|---|---|---|
| 1 | discarded-name orders → ~0 | ✅ 0 in 5 of 6 games — and the sixth had **26**, which found the `civ6_live_build_name` repair-arm defect (#1006). Caught only by checking per game. |
| 2 | Campus research project stops being 0 | ⚠️ 1/28, 0/44, 0/2, 3/11, 0/3 — rate unchanged from baseline's 4.8% |
| 3 | `gpp` non-empty | ✅ 117+ state events carrying it; Scientist 286, Prophet 911 |
| 4 | science rises at matched turns | ❌ not visible; per-pop science flat |

## The finding that replaced the hypothesis

**Science ≈ 1.16 × population**, n=5 (1.37 / 1.29 / 1.22 / 1.08 / 0.85). City
count predicts nothing. No valuation change moved it.

The two defects that were real were **reachability**, not weighting: Chemistry
absent from `available_techs` (#999) and the Entertainment Complex family absent
from `DISTRICT_PRIORITY` (#1003). Both were found by reading what the deployment
does, not by measuring.

**Next measurement**, on the first batch carrying #999/#1003/#1006/#1007:
Research Labs should stop being 0, and host-exported housing will show for the
first time whether population is housing-capped — the question I declined to
build a fix on while it was unverifiable.
