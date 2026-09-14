# `boost-planner`, 2026-09-09

An opt-in gene that plans the Eurekas and Inspirations the beeline is about to
walk past — a six-technology, four-civic horizon, a trigger cost table, and at
most three committed side objectives carrying the turn they expire, each a
share-of-value premium on the one production or Builder choice that fires it.
Off by default; off, every path is byte-identical.

Reviewed 2026-09-09 (senior review, same PR): the research deferral and the
`coastal_city` settle-site seam were cut, a commitment-expiry defect was fixed,
the `chase-every-boost` stacking bound was stated and tested, and the screen
gained the `boosted_share` read this gene is measured by. See *Review* below.

Code: `src/ai/advanced/boost_planner.rs`, tests in
`src/ai/advanced/boost_planner/tests.rs`.

## Why

At Emperor and above the handicap hands every rival a flat science and culture
multiplier (+16 % to +32 %) **and** free technology and civic boosts. A boost
is the one research multiplier the handicap does not scale for us: 40 % of a
node's cost (`data/techs.json`, `data/civics.json`; Near Future Governance pays
90), earned by doing something the empire was often going to do anyway.

Measured on the live King/Emperor ladder (ledger runs of 2026-08-30 to
2026-09-01, 32 runs past turn 100, recorded in the module header of
`src/ai/advanced/chase_every_boost.rs`): of the technologies the seat
researched, **13–40 % had been boosted**; of the civics it adopted, **0–26 %,
typically 5–12 %**. The one near-win on Emperor (run `20260901T132005Z`) lost
the space race by a single turn with 29 of 72 techs boosted and 11 of 43 civics
inspired. A strong human boosts most of both trees. That gap is a research-pace
lever the size of the whole late-game deficit.

## Why this is not another chase

The coverage half of the problem is already closed. `chase-every-boost`
(version one) expands every trigger the engine can judge into the chase table
and ships **on** — rank 31 of `GENE_HEURISTIC_RANKING.md`, +0.77 % on/off.

The *disciplined-looking* half is where the family has failed:

| Gene | Rank | Default | Diff | Last three batches (per 10k seats) |
|---|---:|---|---:|---|
| `chase-every-boost-2` | 250 | off | **−0.57 %** | **−31 / −11 / −3** |
| `eureka-chasing-builder-2` | 243 | off | −0.49 % | −23 / −6 / −13 |
| `boost-first-research` | 205 | off | −0.12 % | +39 / −8 / −7 |
| `boost-first-research-2` | 204 | off | −0.11 % | −11 / +0 / – |

`chase-every-boost-2`'s reading is the one this gene is written against: a
premium that prices *every* reachable trigger — even narrowed to the node
currently under study — makes stale and irrelevant boosts compete with the
current plan, and a city builds the trigger instead of the Settler. The two
older negatives share one mechanism: both paid an **absolute** premium.
`eureka-chasing-production`'s 4.0 points per beaker added +234 to a Trebuchet
worth 95 and +400 to an Entertainment Complex; `boost-first-research` v1 added
+37 above a `sqrt(cost)` divisor that reads 7 for an ancient node, against
unlocks worth single digits, and it "stopped preferring the boosted node among
comparable ones and started taking whatever was boosted".

Breadth is not the missing thing. **Commitment with a deadline** is.

## Design

1. **Horizon** (`boost_horizon`, `BOOST_HORIZON` = 6, `BOOST_CIVIC_HORIZON`
   = 4). The beeline picker's own comparator — the
   `min_by(beeline_step_cost)` that `advanced_research`'s `goal_pick` uses —
   run forward over the legal frontier: at each step the unheld nodes whose
   prerequisites are all held or already picked, cheapest effective step first,
   ties by name exactly as the picker breaks them. The node under study leads
   the list. Each entry carries its projected research start turn and a
   deadline: the start turn for a node not yet begun, and the projected
   **completion** turn for the node already under study, because the engine
   credits a boost that lands mid-research.

2. **Trigger cost table** (`boost_trigger_class`). Each horizon node's
   `BoostSpec` is read through the engine's own `Game::boost_progress` — the
   same arm `boost_met` compares — and classified:
   - `Satisfied`: `have >= need`.
   - `Cheap(action)`: **work the empire already does**, and only three forms:
     an improvement type it has already put on the ground, a unit kind it
     already fields and needs exactly one more of, or a district family it
     already builds. Each guard is the "already" half; a trigger the empire
     would have to *start* doing is not cheap.
   - `Expensive`: wonders, war acts, kills, religion, pantheons, great people,
     national parks, themed museums, a city site (`coastal_city`) — and
     **buildings**, deliberately:
     `chase-every-boost` and `eureka-chasing-production` already price those
     and both ship on, so this gene adds nothing on top of a premium that
     already exists.
   - `ImpossibleNow`: the trigger's own technology or civic is unresearched, or
     only time, growth or contact advances it.

   Only `Cheap` becomes a side objective.

3. **Side objectives with deadlines** (`boost_planner_refresh`,
   `BOOST_MAX_ACTIVE` = 3, `BOOST_PREMIUM_PCT` = 15). A side objective is a
   **commitment**: once taken up it stands, with the deadline it was given,
   until the boost is collected, the node is researched, the trigger stops
   being cheap, or the deadline passes. Only the free slots under the cap are
   filled from the horizon, richest boost first. That carrying-forward is what
   gives the deadline force, and it is the structural difference from a chase,
   which re-prices every reachable trigger every turn and therefore follows the
   beeline's every wobble.

   The premium is 15 % of the candidate's **own positive value**, at two
   seams: production (`production_value`) and Builder work
   (`improvement_value_with_appeal`). Being a share rather than a sum, it can
   re-order two choices the planner already rated within 15 % of each other
   and can do nothing else — in particular it can never lift a choice priced
   at or below zero. That is the direct answer to the two absolute-premium
   negatives above.

   **Stacking with `chase-every-boost`**, which ships on and prices the same
   two seams: by design (v1 is the shallow coverage of every trigger, this is
   the deep commitment to at most three), and bounded. v1's premium is held
   under `CHASE_PRODUCTION_RAW_FRACTION` / `CHASE_BUILDER_VALUE_FRACTION`
   (one half) of the same positive base, so the stack is at most 65 % of the
   choice's own value, is zero wherever the choice is worth nothing, and
   cannot compound — each reads the raw value, never the other's premium.
   Tested (`stacked_on_chase_every_boost_the_premium_stays_a_bounded_share`).

   A commitment whose node comes under study keeps its window to the
   projected completion turn (`boost_commitment_deadline`), because the
   engine credits a boost mid-research; the window is read each turn and
   never written back, so the turn research moves off the node the given
   deadline is the window again.

4. **The three things a boost never outranks** (`boost_planner_stands_down`).
   - **Defence.** While any owned city is under real pressure
     (`threatened_city`, the empire-level test the recovery plan itself uses),
     or the plan names this city as threatened, every premium stands down.
   - **Settlers in the opening band.** Inside the expansion band and behind its
     winning pace (`expansion_band_turn` / `expansion_pace`; every recorded win
     came from four to six cities by turn 60), the *production* premium stands
     down, so a trigger can never take a Settler's slot. Builder charges are
     untouched — a charge does not compete with a Settler, and the opening is
     where a Eureka is worth most.
   - **Victory-project reservations.** In the science drive's launch city
     (`science_drive_launch_city`) the queue belongs to the space projects and
     no premium is paid there at all.

5. **Journal.** Creation, collection and expiry are written at `Research` /
   `Decision` with the boost's own trigger and deadline, so a run's `why.log`
   says what the planner committed to and whether it collected.

## Review (2026-09-09)

What the senior review changed, and why, so it is not rebuilt by accident:

- **The research deferral was cut.** As written it could only fire when the
  picked node itself cost at most the three-turn window
  (`finish_turns <= fire_turns <= 3`), so it defended at most 40 % of a
  three-turn node — about 1.2 turns of research — and its Builder half took
  any charged Builder anywhere in the empire as "fires next turn". Two picker
  hunks in `advanced.rs`, two `pub(super)` widenings
  (`boost_trigger_is_queued`, `item_trigger_key`), three methods and two
  constants went with it. The engine's own mid-research credit already covers
  the case the deferral was for.
- **The `coastal_city` settle-site seam was cut.** It was the only placement
  objective, it paid on `settle_value_visible` — the empire's least
  reversible decision — and its whole payout is Sailing's 20-beaker Eureka.
  The class now reads `Expensive` (a site wins on its own merits), and the
  test asserts it stays so with a Settler walking.
- **A commitment no longer expires the turn its node starts.** A not-started
  node's deadline is its projected *start* turn; the refresh dropped the
  commitment the turn after research began — the turn the trigger is worth
  most — and re-took it from the horizon a turn later under a fresh deadline.
  `boost_commitment_deadline` now reads the window to projected completion
  while the node is under study, without writing it back.
- **`Item::Formation` no longer satisfies a `Unit` objective**: a Corps from
  the queue is a later-era build the ancient `units_of:` triggers never name;
  the premium attaches to the exact satisfying choice only.
- **The stacking bound with `chase-every-boost` is stated and tested** (see
  *Design* 3): at most 65 % of the choice's own value, zero at or below zero.
- **The screen now exports the gene's own metric.** The junior's note that
  "the screen does not export a `boost_totals` column" was half right: every
  row has carried `techs_researched` / `techs_boosted` / `civics_adopted` /
  `civics_inspired` since `chase-every-boost`'s probe of 2026-09-01, but
  `--analyze` had no contrast on them. `gene_screen --analyze` now reports
  `techs_boosted_share_pp` and `civics_inspired_share_pp` per gene — the
  on − off Δ, in points, of the share of researched techs (adopted civics)
  that arrived boosted, from the same clustered contrast as the win column —
  and prints a `boosted share` block. Tested
  (`boosted_share_is_the_on_minus_off_share_of_nodes_that_arrived_boosted`).

## Tests

`cargo test --profile ci --locked --lib boost_planner` — 19 tests:

- the gene is a native opt-in, off in both controllers;
- the horizon respects prerequisites, never repeats, never moves its start
  turns backwards, begins at the beeline's own cheapest legal step, and does
  not itself depend on the flag; the node under study leads it and keeps its
  window to completion while a future node's deadline is its start turn; the
  civic horizon is four and the technology horizon six;
- classification per class: satisfied; improvement cheap only once that
  improvement is already on the ground, with all three tile requirements
  (`improvement:`, `improvement_on_resource:`, `improve_resource:`) and
  `ImpossibleNow` without the gating technology; unit cheap only at one owned
  and exactly one more (zero owned → expensive, two more → expensive, third →
  satisfied); district cheap only for a family already built; a city site
  never cheap, Settler walking or not; fifteen strategic-spending triggers
  expensive and twelve unreachable ones impossible; **and a sweep over every
  shipped boost row in both trees asserting none classifies cheap on an empty
  opening board**;
- the cap keeps at most three and keeps the richest; a commitment stands on its
  deadline turn and is dropped the turn after; a collected boost ends its own
  objective; a commitment whose node comes under study is kept to the
  projected completion and falls back to its given window when research moves
  off the node;
- the premium is 15 % of the choice's own value, zero for an item no objective
  names, zero on a non-positive value; stacked on `chase-every-boost` the two
  premiums stay under 65 % of the value and pay nothing at or below zero; the
  Builder premium respects the tile requirement (resource, bare tile, wrong
  improvement);
- the three stand-downs: the opening band silences production but not Builder
  work, a threatened city silences production, the empire-wide defence
  stand-down silences both;
- off, every entry point returns zero and nothing is memoised.

## Fires probe (not a ledger source)

Re-run after review on the reviewed code, from a `--features developer-tools`
`ci` build:

```sh
target/ci/gene_screen --games 12 --jobs 4 --genes boost-planner --p-on 0.5 \
  --difficulty emperor --rivals firaxis-mix --handicap rivals --rival-chairs 3 \
  --out docs/gene_screens/fires/2026-09-09-boost-planner.jsonl
target/ci/gene_screen --analyze docs/gene_screens/fires/2026-09-09-boost-planner.jsonl \
  --json docs/gene_screens/fires/2026-09-09-boost-planner.json
```

The rows are `docs/gene_screens/fires/2026-09-09-boost-planner.jsonl`; the
analysis is `docs/gene_screens/fires/2026-09-09-boost-planner.json`. Twelve
games, 36 measured seats of 72 chairs (14 on, 22 off), seeds
26081900..26081911, majors at Emperor with the rung's bonuses given only to
the three `firaxis-mix` rival chairs.

| Column | Δ (seats on − seats off) | z |
|---|---:|---:|
| **techs boosted share** (`techs_boosted_share_pp`) | **−1.84 ± 2.19 pp** (38.0 % on, 39.8 % off) | −0.84 |
| **civics inspired share** (`civics_inspired_share_pp`) | **+0.46 ± 2.76 pp** (38.7 % on, 38.2 % off) | +0.17 |
| win | −3.9 ± 13.4 pp (14.3 % on, 18.2 % off) | −0.29 |
| score share | −3.14 ± 1.31 pp | −2.39 `*` |
| techs @ standard t150 | −0.46 ± 1.04 | −0.44 |
| techs @ end | −2.45 ± 3.65 | −0.67 |
| science / turn | −34.1 ± 35.7 | −0.95 |

Read honestly. The first two rows are the gene's own instrument, and they say
the plan **did not move the share of the tree that arrived boosted** at this
size: both arms sit at ~38–40 % of techs and ~38 % of civics, and the Δ is
inside two points either way. The score-share row carries a screen flag
(`read: "share hurts * (thin)"`), which on 36 seats is one flag in twenty-two
by chance and is not a measurement; but nothing here points the mechanism's
way. The junior's first probe (before review, with the deferral and the site
seam in) read win +14.9 ± 12.9, share −2.26 ± 1.78, techs @150 −0.97 ± 1.21 —
the same picture, a different draw.

The probe's only job was met: every statistic is non-zero, so the gene fires
(`tools/gene_fires.py --max 0` exits 0). A twelve-game probe is not a
measurement (`docs/GENE_SCREEN.md`, *"A probe's win Δ is not a measurement of
the gene"*); pricing belongs to the continuous screen, and the
`techs_boosted_share_pp` column is now the row to read there. A boosted share
that does not move on the screen either is the signal to remove the gene: a
15 % premium on three committed objectives may simply be too small to change
what a city builds when `chase-every-boost` already prices every trigger.

## Gaps and open questions

- **The probe's boosted share did not move.** Both arms boost ~38–40 % of
  their techs. Either the committed objectives are rarely cheap on a
  competitive board, or a 15 % share-of-value premium does not re-order what a
  city builds once `chase-every-boost`'s capped half-share is already on the
  same items. The screen's `techs_boosted_share_pp` row decides; a flat share
  there is the removal signal.
- **`chase-every-boost` ships on**, so on the deployment genome this gene's
  premiums stack on top of that gene's. The stack is bounded (at most 65 % of
  the choice's own value, zero at or below zero; tested), but the interaction
  is unmeasured; an interaction reading against `chase-every-boost` off would
  be worth having.
- **The horizon is a projection of the picker, not the picker itself.** A
  forced lane goal can overturn any step, which is why objectives expire; but
  it does mean the horizon can commit to a boost on a node the lane never
  takes. The deadline bounds the cost of that to at most one commitment slot
  for its window.
