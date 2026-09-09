# `boost-planner`, 2026-09-09

An opt-in gene that plans the Eurekas and Inspirations the beeline is about to
walk past — a six-technology, four-civic horizon, a trigger cost table, at most
three committed side objectives carrying the turn they expire, and a
three-turn research deferral. Off by default; off, every path is
byte-identical.

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
   - `Cheap(action)`: **work the empire already does**, and only four forms:
     an improvement type it has already put on the ground, a unit kind it
     already fields and needs exactly one more of, a district family it already
     builds, or a coastal city while a Settler is already walking. Each guard
     is the "already" half; a trigger the empire would have to *start* doing is
     not cheap.
   - `Expensive`: wonders, war acts, kills, religion, pantheons, great people,
     national parks, themed museums — and **buildings**, deliberately:
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

   The premium is 15 % of the candidate's **own positive value**, at three
   seams: production (`production_value`), Builder work
   (`improvement_value_with_appeal`) and placement (`settle_value_visible`).
   Being a share rather than a sum, it can re-order two choices the planner
   already rated within 15 % of each other and can do nothing else — in
   particular it can never lift a choice priced at or below zero. That is the
   direct answer to the two absolute-premium negatives above.

4. **The three things a boost never outranks** (`boost_planner_stands_down`).
   - **Defence.** While any owned city is under real pressure
     (`threatened_city`, the empire-level test the recovery plan itself uses),
     or the plan names this city as threatened, every premium stands down.
   - **Settlers in the opening band.** Inside the expansion band and behind its
     winning pace (`expansion_band_turn` / `expansion_pace`; every recorded win
     came from four to six cities by turn 60), the *production* premium stands
     down, so a trigger can never take a Settler's slot. Builder charges and
     settle sites are untouched — they do not compete with a Settler, and the
     opening is where a Eureka is worth most.
   - **Victory-project reservations.** In the science drive's launch city
     (`science_drive_launch_city`) the queue belongs to the space projects and
     no premium is paid there at all.

5. **Research deferral** (`boost_planner_defer_pick`, `BOOST_DEFER_TURNS` = 3).
   The node the picker chose yields its slot when, and only when:
   - it is itself a live side objective's node;
   - that trigger is **in progress**, not merely possible — a unit, building or
     district at the front of an owned city queue (`boost_trigger_is_queued`,
     the strict test `boost-wait-research-2` uses), or an improvement with a
     Builder standing that has a charge to spend;
   - the boost lands within three turns; and
   - the empire would otherwise finish the node **first** — if the node
     outlives its own trigger the engine's mid-research credit reaches it and
     nothing needs deferring.

   The replacement must be on the same beeline (when the lane forced a goal, it
   must lead to that goal) and must cost no more than three turns of research.
   Those two bounds together are the design's requirement that the lane's next
   unlock is never pushed back further than the boost window itself: the lane
   loses order, never progress.

6. **Journal.** Creation, collection, expiry and each deferral are written at
   `Research` / `Decision` with the boost's own trigger and deadline, so a
   run's `why.log` says what the planner committed to and whether it collected.

## Tests

`cargo test --profile ci --locked --lib boost_planner` — 20 tests:

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
  satisfied); district cheap only for a family already built; coastal city
  cheap only while a Settler walks; fifteen strategic-spending triggers
  expensive and twelve unreachable ones impossible; **and a sweep over every
  shipped boost row in both trees asserting none classifies cheap on an empty
  opening board**;
- the cap keeps at most three and keeps the richest; a commitment stands on its
  deadline turn and is dropped the turn after; a collected boost ends its own
  objective;
- the premium is 15 % of the choice's own value, zero for an item no objective
  names, zero on a non-positive value; the Builder premium respects the tile
  requirement (resource, bare tile, wrong improvement);
- the three stand-downs: the opening band silences production but not Builder
  work, a threatened city silences production, the empire-wide defence
  stand-down silences both;
- the deferral: a merely possible trigger is not a commitment; a node the
  trigger beats home is not deferred; a queued trigger landing after the node
  and inside the window defers, and the replacement costs ≤ 3 turns; the same
  trigger pushed past the window does not; with a forced goal the replacement
  always leads to that goal, and a goal nothing leads to leaves the pick alone;
- off, every entry point returns zero/`None` and nothing is memoised.

## Fires probe (not a ledger source)

```sh
target/ci/gene_screen --games 12 --jobs 4 --genes boost-planner --p-on 0.5 \
  --difficulty emperor --rivals firaxis-mix --handicap rivals --rival-chairs 3 \
  --out docs/gene_screens/fires/2026-09-09-boost-planner.json
```

<!-- PROBE RESULT -->

A twelve-game probe is not a measurement (`docs/GENE_SCREEN.md`, *"A probe's
win Δ is not a measurement of the gene"*); it exists to show the gene fires at
all, which is the precondition `tools/gene_fires.py --max 0` enforces. Pricing
belongs to the continuous screen.

## Gaps and open questions

- **The deferral is narrow by construction.** It requires the node to be
  buyable inside the same three turns the trigger needs, which on an opening
  board is rare — the tests have to build a rich capital to reach the regime at
  all. Most of the gene's effect will come from the premium, not the deferral.
  Whether the deferral earns its complexity is a question for the screen, and
  it can be cut without touching anything else.
- **`chase-every-boost` ships on**, so on the deployment genome this gene's
  premiums stack on top of that gene's. They are capped at different fractions
  of different bases and cannot both be paid on the same *decision* more than
  once each, but the interaction is unmeasured; an interaction reading against
  `chase-every-boost` off would be worth having.
- **The Builder half of "in progress"** is the loose one: a Builder standing
  with a charge is a weaker commitment than an item at the front of a city
  queue. It is bounded by the same three-turn window and by the objective being
  cheap and live, but a tighter test — the Builder actually assigned to that
  tile — would be better if the objective board exposes one.
- **`coastal_city` is the only placement objective**, and it pays on the settle
  site rather than on the Settler. If the screen says the site seam is noise,
  that class can be dropped to `Expensive` and the seam removed.
- **The horizon is a projection of the picker, not the picker itself.** A
  forced lane goal can overturn any step, which is why objectives expire; but
  it does mean the horizon can commit to a boost on a node the lane never
  takes. The deadline bounds the cost of that to at most one commitment slot
  for its window.
