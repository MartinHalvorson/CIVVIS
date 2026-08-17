# Production as value over cost

The production decision's stated goal: estimate the reward of every item a
city can produce, divide by what it costs, and build the highest ratio. This
document records how much of that system already existed, what was missing,
what #<PR> adds behind flags, and how each addition gets its number.

## What already existed (map of `AdvancedAi::production_value`)

Every candidate — units, buildings, districts, wonders, repairs, projects,
formations — is ranked by one scoring function whose tail is

```text
raw * production_category_gene(item)        # the seven #1520 genes
    / (7.0 + turns)                         # turns = remaining cost / city production
```

so the system is **value per turn, heavily damped**, not value per cost: the
`+7` compresses a 20× cost difference into ~3.4× of score. That damping is
deliberate and measured territory — the district *order* inside the economy
block is settled (`docs/GENOME.md`, all 24 orders within 2 SE), and re-pricing
existing arms is the repository's most reliable null (`docs/GENOME.md`,
`docs/AI_GAPS.md` §"where strength is not"). This change therefore does not
touch the normalizer or re-weight any existing arm.

Per the operator's checklist, what the arms already covered:

- **Buildings** — yields × per-strategy weights + housing/amenity need +
  named debts. Reward estimation exists.
- **Military objectives** — the chooser reads `threatened`,
  `offensive_conquest`, a per-strategy `desired_military`, and a standing
  `WarPlan` short-circuit. "Always defend, sometimes attack" exists.
- **Projects** — a full arm: repairs by missing defense, space race by lane,
  district repeatables by yield-over-horizon plus the Great Person race.
  "Useful when working toward something" exists.

## What was missing, and what this change adds

Three gaps were *absences*, not mis-weightings — the class of change that has
historically paid (`docs/GENOME.md`: actuation vs. valuation).

### 1. The Builder was priced by headcount, never by its work

The arm was `ceil(cities/2)` and a flat 260. It never looked at a tile, so a
fully-improved empire and a virgin one bid identically — and the floor
withhold measured the overbuild costing terminal score
(`docs/EVAL.md` §production Builder floor, 225/175, p=0.0142).

`builder_reward_survey` (arm **`advanced_builder_survey`**) prices the
Builder as the engine prices it: `Game::builder_charges` says how many jobs
the unit will carry (base 3 + buildings/wonders/Serfdom/Dynastic Cycle), a
survey of every owned tile says what the best open jobs are worth, and the
next Builder is bid at the sum of the best `charges` jobs *after* the
Builders already alive claim the head of the list. The same survey sizes the
`delegated_cities` quota, so the repricing reaches the lanes that build
through `BasicAi::cities` — the production default under most plans.

The survey values a job the way the user-facing spec asks:

- **Connecting a luxury** — worth `10 + 9 × needy_cities` (cap 4, the
  allocation reach) for the empire's *first* copy; a duplicate is a 2-point
  trade chip. The movement scorer's flat 14 cannot tell Silk #1 from Wine #3.
- **Connecting a strategic** — 45 for the first source (it unlocks unit
  lines), 18 while the stockpile has headroom, 6 near the cap. The flat 30
  could not tell Horses at turn 20 from a fourth Iron mine at turn 200.
- **Tiles likely to be worked** — yield value counts in full on tiles the
  city's own citizen plan works and at 0.4 otherwise; housing, tourism and
  resource connection are not worked-gated by the engine and are never
  discounted. Housing within the 3-tile ring scales with
  `city_housing_headroom` — the measured population ceiling.

### 2. No unit had a cost term, and uniques were invisible

Unit choice was max-power within a role band; Sumeria's War Cart (30
strength / 55 production, the best strength-per-cost of its era) won only
what its raw strength bought, and a unique with base-identical stats (Tagma,
Nau) was invisible. `unit_cost_efficiency` (arm **`advanced_unit_efficiency`**)
adds up to +45 for leading the role's strength-per-production and +85 while
the unit is the civilization's own unique — a window the tech tree closes.
Respects `civ_blind`.

### 3. What was deliberately NOT built

- **No per-city production rollout.** M4 is retired on measurement
  (`docs/SUPERHUMAN.md`: evaluator not blind, horizon-stable to 200, and the
  objective is not win probability). Nothing here consumes a learned value.
- **No global re-normalization.** Replacing `/(7+turns)` with `/cost` would
  re-rank every settled arm at once — a whole-surface valuation tune with a
  documented null prior and no bisectable parts.
- **No change to shipped behavior.** Both flags are off in
  `promoted_policy_envoy`; `advanced` ranks builds bit-identically to before
  this change. The composite lesson (`docs/EVAL.md` #1516) is one flag, one
  arm, one number.

## How each addition gets its number

Fires-checks (done, in `cargo test --lib`):

- `a_spawned_builder_carries_the_charges_production_priced` — the priced
  charge count is the spawned charge count.
- `the_builder_survey_prices_work_where_the_quota_priced_headcount` — the
  flag reaches the arm, jobs come back best-first, live Builders monotonically
  depress the next one's price.
- `a_first_luxury_outbids_a_duplicate_and_a_worked_tile_outbids_an_idle_one`
- `the_civs_own_unique_unit_earns_its_window` — and a rival civ gets nothing.

Pre-registered pricing, one run each, not swept, decided by the matrix gate
and nothing else:

```sh
ai_eval advanced_builder_survey advanced --matrix --pairs 400 --seed 25000000
ai_eval advanced_unit_efficiency advanced --matrix --pairs 400 --seed 26000000
```

Whatever these return is the result. A PASS earns a `--confirm` on a disjoint
seed before any effect size is quoted (`docs/EVAL_INTEGRITY.md` R3); only
then does a flag move into `promoted_policy_envoy`, each with its own
`disable_*` withhold so it stays individually priced. A null or a RETAIN
closes the arm with its number, exactly as `advanced_build_first` — the
still-unrun pre-registered point on the #1520 genes — is waiting on seed
95000000.

## Results — both runs made, both RETAIN (2026-08-17)

Both pre-registered runs were made the day the arms merged, on the reserved
seeds, at 400 pairs, and neither promotes. The fires-checks above had already
confirmed both treatments change games, so these are real nulls rather than
inert treatments.

| arm | compact-standard (NoRegression) | deployment-online (Strength) | verdict |
|---|---|---|---|
| `advanced_builder_survey` (seed 25000000) | ACCEPT | INCONCLUSIVE → REJECT; seat wins 385 vs 415 of 2400 | **RETAIN `advanced`** |
| `advanced_unit_efficiency` (seed 26000000) | ACCEPT — 50.2%, +1 Elo (CI −33..+35), 63/56 directions, p=0.58 | INCONCLUSIVE → REJECT — 47.5%, −17 Elo (CI −51..+17) | **RETAIN `advanced`** |

(The builder-survey run's per-profile effect lines were lost to a truncated
pipe; its verdict lines and seat-outcome census survived. Effect sizes from a
RETAIN are non-quotable regardless — `docs/EVAL_INTEGRITY.md` R3.)

**Reading.** The flags stay off in `promoted_policy_envoy`; the arms stay
registered as the re-opening question. This is the valuation-null prior
paying out one more time (`docs/GENOME.md`): both changes price an existing
decision *better* rather than making the agent do something it never did,
and better pricing of a decision that already actuates has never converted
to wins on this engine. Do not re-run these arms at more pairs — the
deployment directions are flat, not under-resolved. Re-open only with a new
mechanism (e.g. the survey driving builder *movement* differently, not just
production counts), and gate it the same way.

## The axis closes: the other two open ends, priced the same day

The two follow-ups the survey left open were both run on 2026-08-17,
pre-registered, 400 pairs each. Neither is inconclusive.

**`advanced_build_first`** (seed 95000000, reserved by #1524 and unrun for a
week) — the building-over-district tilt on the #1520 genes, motivated by
`advanced_synergy` losing 108 Elo while ending on more districts and fewer
buildings:

| profile | score | verdict |
|---|---|---|
| compact-standard | 51.3% (+9 Elo, CI −25..+43), 66/52, p=0.23 | ACCEPT (inconclusive) |
| deployment-online | 48.8% (−9 Elo, CI −43..+25), 40/50, p=0.34 | REJECT (inconclusive) |

**RETAIN.** The tilt hypothesis does not convert; the last pre-registered
point on the production category genes is now a recorded null.

**`advanced_every_lane`** (seed 27000000, arm registered by the follow-up to
#1861) — the *routing* question: run the strategic governor's valuations
natively under Science/Culture/Religion/Diplomacy, where production
otherwise goes through `BasicAi::pick_item`. The fires-check smoke diverged,
so the arm bites. The gate answer is decisive, and it is the day's most
informative number:

| profile | score | direction | verdict |
|---|---|---|---|
| compact-standard | 41.1% (**−62 Elo**, CI −97..−28) | 49/166, p=0.0000 | RETAIN — e crossed at map 104 |
| deployment-online | 36.6% (**−95 Elo**, CI −131..−60) | 46/153, p=0.0000 | RETAIN — e crossed at map 78 |

**RETAIN, 0/2, significant against.** The valuation governor, handed every
lane's cities natively, loses heavily to the scripted cascade it replaces.
So the "structural hole" reading — that the missing piece was reach — is
refuted on the native profile: the cascade is not an inferior stand-in for
the valuations, it is the stronger controller for those lanes. (The live
bridge keeps `governor_every_lane` on its own evidence in its own regime —
`docs/FIDELITY.md`'s headless≠live applies; this number says nothing about
the bridge bundle.)

**Where this leaves the axis.** Four pre-registered gates, four RETAINs, two
of them significant against extending the valuation system's reach or
re-tilting its genes. The value/cost estimation layer is built, priced, and
bounded: it earns its keep where it already runs, and neither sharper
pricing nor wider routing beats the incumbent. The axis is closed pending a
genuinely new mechanism.
