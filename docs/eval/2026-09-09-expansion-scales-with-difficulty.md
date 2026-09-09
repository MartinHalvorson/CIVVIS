# `expansion-scales-with-difficulty` — the opening band is a King-level number

**Gene:** `expansion-scales-with-difficulty` · `Kind::OptIn`, off by default ·
`src/ai/advanced/expansion_scales_with_difficulty.rs`

## The claim

The measured opening band that the whole expansion stack aims at is five
cities by turn 60. That number was read off a King-level field. At Emperor,
Immortal and Deity the rival majors are handed a fixed percentage of every
yield *and* free Settlers, so a five-city empire is out-produced by
construction. The target, its deadline and the Settler cadence should scale
with the rung; with the gene off, nothing changes at all.

## Evidence

### The band, and the field it was measured on

`src/ai/advanced/expansion_schedule.rs` carries the corpus in its own module
doc — `tools/civ6_run_report.py --aggregate` over `~/civvis-civ6-runs/control`,
218 completed live runs, 2026-08-25:

| cities at turn 60 | runs | wins | rate |
|---|---:|---:|---:|
| 1–3 | 127 | 0 | 0% |
| 4 | 56 | 1 | 2% |
| 5 | 23 | 4 | 17% |
| 6 | 10 | 4 | 40% |
| **outside 4–6** | **128** | **0** | **0%** |

Nine wins, all nine inside the band, one-sided Fisher *p* = 2.6 × 10⁻⁴. That
is the evidence `EXPANSION_BAND_FLOOR = 5` and
`rapid_city_expansion::city_target` both rest on, and this gene does not
dispute a line of it. What it disputes is the *transfer*: those runs are the
ladder's King-level control corpus, and the first live science win the ladder
recorded (`docs/civ6_ladder.json`, t234, 2026-09-01) was a King seat.

### What a rung actually is

`data/difficulties.json`, rows above Prince (`order` 3, the unhandicapped
reference rung):

| rung | order | AI science/culture | AI production/gold | free units per AI |
|---|---:|---:|---:|---|
| King | 4 | +8% | +20% | warrior, builder |
| Emperor | 5 | +16% | +40% | 2 warriors, builder, **settler** |
| Immortal | 6 | +24% | +60% | 3 warriors, 2 builders, **settler** |
| Deity | 7 | +32% | +80% | 4 warriors, 2 builders, **2 settlers** |

Both scaling terms are city count. The yield bonus is *per rival city*, so a
rival's advantage is its percentage multiplied by its own width; and from
Emperor upward the free Settlers are that same bonus paid forward as extra
cities. `Game::difficulty_spec().order` is the field, and
`Game::handicap_exempt` is why reading the rung is right even for a seat that
takes no handicap itself: `--handicap rivals` exempts the *measured* seats
precisely so the rivals keep the rung's bonuses.

### What the ladder says about the rung

The Emperor rung stands at **0 wins in 111 games** (2026-09-08 census,
`docs/EVAL_STATUS.md` and the opportunity ranking of that date), with nineteen
games lost outright to a rival science or culture finish — a rival economy
arriving first, not a battlefield loss. The 2026-08-31 finding
"THE EMPIRE BUILDS FORTS, NOT SCIENCE" measures the same gap from the other
side: 12–33 techs behind in every deep game. Human play at these rungs answers
it with eight to twelve cities by turn 100, funded by the expansion policy
cards and by Builder chops.

## Design

Five legs; four of them shipped.

### 1. Targets and deadlines that scale with the rung

Pure functions, all in the gene's own module:

- `level_above_prince(g) = difficulty_spec().order − PRINCE_ORDER(3)`, zero at
  Prince and below.
- `city_target(level) = OPENING_BAND_CITIES(5) + level`, capped at
  `WIDE_CAP(10)` — King 6, Emperor 7, Immortal 8, Deity 9.
- `deadline_turn = band_turn + standard_duration(WIDE_DEADLINE_TURNS_PER_LEVEL
  (10) × level)` — speed-scaled, so an Online ladder game and a Standard-speed
  game get the same content rather than the same integer. Deliberately tighter
  than the live founding cadence (cities 2–6 at t37/71/89.5/118.7/150.2, about
  twenty live turns a seat): the cadence has to compress, not merely extend.
- `cities_by_hundred = city_target + WIDE_SECOND_TARGET_BONUS(2)`, also capped
  at `WIDE_CAP`, asked for by `WIDE_SECOND_SHARE(0.4)` of the clock — turn 100
  of the ladder's 250-turn game, the horizon the human answer is quoted at.
- `pace(g)` is two straight ramps: 1 → `city_target` by the deadline, then
  `city_target` → `cities_by_hundred` by the second horizon, flat after it.

Hooks: `AdvancedAi::expansion_deadline_turn`, `::expansion_cadence_horizon`,
`::expansion_pace_now` and `::expansion_wide_city_target`. The first three are
*exactly* `expansion_band_turn` and `expansion_pace` with the gene off; the
fourth is `None`. `assess` raises `desired_cities` to the rung's horizon with a
`max`; `city_target_meets_the_map`'s practical-site room still cuts the number
back down afterwards. The Science contract does **not**: `assess` applies
`SCIENCE_CITY_TARGET_CAP` (6) as a `min` after every expansion arm, and a
`min(6)` after a `max(9)` is 6 — the ordering that once swallowed a bare
widening of `land_grab`. The ladder's default lane is Science, so the cap is
raised to the rung's horizon (`cap.max(cities_by_hundred)`) while the gene is
on and reads its shipped constant with it off. (Review fix; the draft applied
the cap over the target and was inert on the ladder's own lane.)

### 2. A Settler cadence that reaches past the capital

Two mechanisms had to move, because the reservation is not the veto:

- **The reservation.** `higher_level_strategy`'s `Debt::Expansion` arm — the
  `expansion-best-idle-city` family — asks for a Settler in the best idle city
  while `counts.settlers == 0` *and* while no city in the empire has one
  queued. This gene supplies the horizon, the pace and the walker rule to that
  same arm (`expansion_wide_cadence_admits`, at most
  `WIDE_PARALLEL_SETTLERS(2)` in flight, and only behind a capital busy with a
  Settler or a district), and exempts the *capital's* queued Settler from the
  "already serviced" filter. Selection, ranking, idle-queue rule,
  production-value and site-gate admission are all the shipped ones.
- **The pipeline.** `settler_in_flight_allowed` answers **1** for the ordinary
  empire and `production_value` returns −10 000 for a Settler past that width,
  which is what actually blocked the second factory in testing.
  `expansion_wide_pipeline` is the second slot. ⚠ It counts **founded cities**,
  not walkers: `expansion_pace_shortfall` counts a walker as a city, and
  `expansion_schedule` documents what that cost live — a walker that took 28
  turns to seat credited the empire with a city it did not have for every one
  of them. Both pipelines are bounded by the same `desired_cities` hard cap,
  and when both speak the wider wins.

⚠ This gene is **not** a member of the `expansion-best-idle-city` version
family. It has its own tag and field and clears nothing; v1 and v2 remain
independently screenable and either may be on beside it. What it shares with
them is the code path.

### 3. Chops — **unavailable in this simulator, leg not implemented**

The design called for one reserved Builder charge per Settler in production on
the best harvestable tile in the producing city's radius. This engine has no
harvest. `grep -n 'harvest\|chop' src/game.rs` returns three lines and all
three are the reasoning-log sense of the word ("the moment its plan is
harvested", `src/game.rs:6064` and `:31827`); `data/improvements.json` has no
harvest action and `Action` has no variant for one. Adding a Feature-removal
yield rule to the engine to serve one opt-in gene would be a rules change
wearing a gene's clothes, so the leg is recorded here and left unimplemented.
The nearest thing the engine does model is the Ancestral Hall's free Builder
in every new city, which leg 4 prices.

### 4. Cards and the Hall

- `strategic_policies`' timed-economy block already front-loads Colonization
  and Expropriation while expansion is active, but only for
  `wide-map-capacity` and `rapid-city-expansion-2`. This gene joins that
  condition, so the +50% Settler card reaches the slot the turn Early Empire
  lands rather than after the late portfolio has taken it.
- `production_value`'s `expansion_hall` term prices the Ancestral Hall's
  `settler_production_pct: 50` and `free_builder_new_city: 1`
  (`data/buildings.json`) by how many seats the empire is short. It was gated
  on `expansion_hall && land_grab`; this gene is a second gate on it. The
  term's own `scale` already fades it to zero once the empire is not short, so
  neither gate needs its own shortfall test.

### 5. Safety — unchanged, on purpose

Nothing is relaxed. The walker-aware site gate (`settler_site_gate`) is what
pauses the cadence when no acceptable unclaimed seat lies inside the safe
radius; the settler escort and shelter genes, the housing and amenity floors,
the threatened-city and barbarian-alarm skips and the plan's own
`desired_cities` ceiling all still run. This gene raises a target and widens a
condition; it never sends a Settler where the shipped code would refuse to.

## Tests

`src/ai/advanced/expansion_scales_with_difficulty/tests.rs` (10):

- registry row is opt-in and ships off in both controllers; twin toggles
- level and target at all eight rungs, and both caps
- deadline scaling per rung, speed-scaled, and the unscaled Standard-speed
  reading (`band + 20` at Emperor)
- the two-ramp pace: 1 at t0, 7 at Emperor's deadline, 9 at the horizon, flat
  after, and monotone at every turn in between
- **off is byte-identical**: at five rungs × seven turns, deadline, horizon,
  pace, level, target, shortfall and both pipelines are the shipped values
- **the Science contract is raised, not applied over the rung**: at Emperor,
  half-clock, a Science seat plans 6 cities off and 9 on
- `expansion-schedule` alone keeps its exact shipped schedule at Deity
- the wide pipeline counts founded cities, stops at two slots, respects the
  city target as a hard cap and closes past the horizon

`src/ai/advanced/higher_level_strategy/tests.rs` (5 new, filed beside the code
path they exercise):

- a capital busy with a Settler hands the reservation to the best *other* idle
  city; `expansion-best-idle-city-2` asks for nothing in the same position
- a capital busy with a district is busy; one busy with a Monument is not; an
  idle capital is still chosen by both arms
- the cadence admits at most two walkers, and only behind a busy capital
- **pause on no safe site**: with every tile outside the cities' rings drowned
  and no Shipbuilding, the site gate holds and the cadence asks for nothing

## Validation

- `cargo test --profile ci --locked` — exit code recorded in the PR
- `tools/genes.py check`, `tools/eval_manifest.py --check`,
  `tools/genome_cost.py check`, `tools/test_genes.py`,
  `tools/test_eval_manifest.py`, `tools/test_treatment_append_points.py`,
  `tools/gene_fires.py --max 0` — all green
- Generated tables rewritten with `tools/genes.py write` and
  `tools/eval_manifest.py --write`. ⚠ `docs/gene_ledger.json` was reverted: a
  clean `origin/main` worktree produces the *same* 41-line rewrite of the
  deployed-set list from `genes.py write`, so that drift is pre-existing
  staleness on `main` and not this gene's, and `genes.py check` is green
  either way.

## Fires probe

`target/ci/gene_screen --games 12 --jobs 4 --genes
expansion-scales-with-difficulty --p-on 0.5 --difficulty emperor --rivals
firaxis-mix --handicap rivals --rival-chairs 3`, written to
`docs/gene_screens/fires/2026-09-09-expansion-scales-with-difficulty.json`.

36 measured seats of 72 chairs, **14 on / 22 off**, seeds 26081900..26081911,
all 12 games complete. Rows analysed with `gene_screen --analyze --json` into
the summary artifact.

| axis | on | off | Δ | z |
|---|---:|---:|---:|---:|
| win | 14.3% | 13.6% | **+0.6 pp ± 12.0** | +0.05 |
| score share | — | — | **−1.47 pp ± 1.79** | −0.82 |
| done (game finished) | — | — | −3.02 pp ± 4.66 | −0.65 |
| techs @ standard t150 | — | — | −0.86 ± 1.15 | −0.75 |
| techs at end | — | — | −1.81 ± 4.19 | −0.43 |
| science / turn | — | — | +3.83 ± 46.48 | +0.08 |

Read honestly: **nothing here is a result.** Every interval covers zero by a
wide margin, the win interval is ±12 pp on 36 seats, and the two axes that
would carry the design's claim if it worked — score share and techs@150 — both
lean *negative*. The probe establishes only what it is for: the gene fires
(`win_se_pp` 11.99, `share_se_pp` 1.79, both non-zero, which is the documented
signature of arms that played different games), and `tools/gene_fires.py
--max 0` is green with 279 of 279 tabled genes shown to fire.

⚠ `cities_at_60` is **not** a column this screen emits, so the design's own
primary metric was not measured here. The nearest recorded quantities are the
science-pace columns above. Measuring cities at the band turn needs either the
live run report (`tools/civ6_run_report.py --aggregate`) or a new screen
column; that gap is listed below.

⚠ A 12-game probe is not a measurement
(`docs/GENE_SCREEN.md` §"A probe's win Δ is not a measurement of the gene"):
it establishes only that the gene reaches the game and changes it. Pricing
belongs to the continuous screen.

## Known gaps

1. **Chops are absent** (leg 3), so the production that funds eight to ten
   cities has to come from the cards, the Hall and the cities themselves. This
   is the largest single difference from the human answer the design cites.
2. **The probe ran with the Science cap applied over the gene.** The 12-game
   fires probe below was played by the draft, in which `SCIENCE_CITY_TARGET_CAP`
   clamped the rung's horizon back on any specialised Science seat; review
   moved the cap to `cap.max(rung horizon)`. The probe still shows the gene
   fires (its opening and cadence legs are cap-independent), but its numbers
   are not the shipped code's, and it was never a measurement in any case.
   `science-expansion-phase` remains a separate, still-unscreened
   counter-hypothesis; composing the two is out of scope.
3. **`WIDE_DEADLINE_TURNS_PER_LEVEL` is a judgement, not a measurement.** Ten
   standard turns per rung is argued from the live founding cadence, not fitted
   to it. It is the obvious first thing a screen should vary.
4. **Prince and below still gain the second cadence.** At level 0 the target
   and deadline are exactly the shipped ones, but the post-band ramp to seven
   cities still runs. That is intentional — the corpus says nothing about
   turns after 60 — but it means the gene is not a strict no-op at Prince.
5. **No live-seat evidence.** Everything here is the simulator plus the
   recorded corpus; the live Emperor ladder has not run this gene.
6. **`cities_at_60` is unmeasured.** The screen has no such column, so the one
   number the whole design is argued from was not observed on the probe. Until
   it is — a screen column, or a live run — the causal chain "rung → wider
   target → more cities at the band turn → more score" is asserted at its
   middle link and measured at neither end.
7. **The probe's score-share and techs@150 both lean negative.** Both are
   statistically silent at this size, but they lean the wrong way, and the
   honest reading is that a wider empire may be paying for its extra Settlers
   out of exactly the Campus production the Emperor rung is already short of.
   A continuous screen should settle this before the gene is defaulted on.
