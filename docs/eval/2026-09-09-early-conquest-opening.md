# The opening that takes a neighbour's city

_2026-09-09 · `early-conquest-opening` · opt-in gene, drafted for review_

## What was asked

**Can the seat open a war it actually wins, in the window where an Emperor
rival's handicap is still small?**

The live seat has never won at Emperor. The rung is not a slider on the same
game: Emperor rivals take **+16%** on every yield, Immortal **+24%**, Deity
**+32%**, and each of them is handed a free Settler every era. The ladder's
record is **0 wins in 111 Emperor games**, 19 of them lost to a rival science
or culture finish. Parity play compounds the rival's bonus for a hundred and
fifty turns and then loses to it. A human who beats those rungs does not
out-develop the handicap; they take a neighbour's cities in the first sixty
turns, when the handicap is a few dozen yields and a city is two Archers and
a Warrior away.

Two live measurements say the shipped controller cannot improvise that war,
and the design answers both rather than assuming them away:

| evidence | number | where it bites |
|---|---|---|
| combat trade | **0.45 kills per loss** | a war opened on an empire-wide power ratio is fought at that rate |
| city ledger | **65 cities lost, 2 taken** | the shipped elective war has never converted |
| unit deaths | **234 of 304 carry `no_visible_threat`** | the blow came from a tile the unit could not see when it chose to stand there |

The last row is the important one for a *march*: it is not a tactics problem
inside the fight, it is a tile-choice problem on the way to it.

## How it was measured

Nothing here is a measurement of the gene's value. This round records a
**design, a test suite, and a fires probe** — the CI ratchet
(`tools/gene_fires.py --max 0`) that proves a registered gene can change a
game at all. `docs/GENE_SCREEN.md` §"A probe's win Δ is not a measurement of
the gene" is the standing rule: a 12-game probe has an interval far wider
than any effect worth shipping, and pricing belongs to the continuous screen.

Pre-registered before the probe ran:

| arm | seed window | games | seats | measured seats |
|---|---|---|---|---|
| `early-conquest-opening` at `--p-on 0.5` | 26090900..26090911 | 12 | 72 | 3 of 6 chairs per game |

Shape (the rung the live seat actually meets, not the stock screen):

```
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --games 12 --jobs 4 --genes early-conquest-opening \
  --p-on 0.5 --difficulty emperor --rivals firaxis-mix --handicap rivals \
  --rival-chairs 3 --start-seed 26090900 --out target/early-conquest-opening.jsonl
target/ci/gene_screen --analyze target/early-conquest-opening.jsonl \
  --json docs/gene_screens/fires/2026-09-09-early-conquest-opening.json
```

`--handicap rivals` exempts the measured seats from the rung's bonuses so
only the three rival chairs play with them — the asymmetry the live seat
meets. That, and `--rivals`/`--rival-chairs`, make this a probe and **not** a
ledger source; no ledger row is added and `python3 tools/genes.py write` was
run without `--source`.

The sample will not be grown to get a favourable sign.

### The design under test

Five steps, each inert while the flag is off (every entry point returns
before reading the board), so the controller is byte-identical off.

1. **Target** (`conquest_target`). Among rivals we have **met**, a city whose
   centre we have **explored**, within `CONQUEST_REACH_TILES` (12, which is
   `CAMPAIGN_V2_REACH`) of our capital, whose owner has at most
   `CONQUEST_MAX_RIVAL_CITIES` (3) **known** cities, and which the shipped
   `campaign_target_legal` mask permits. Ranked: the rival's capital first,
   then the lightest **visible** garrison, then the nearest, then the lowest
   id. Re-evaluated every turn until the force commits.

   Fog honesty is by construction and is the part `city_campaign.rs` does not
   have: existence is read from `players[pid].explored` — the idiom
   `opening_archery_goal` uses for a barbarian camp — and the garrison from
   the turn-start visibility frame plus `unit_visible_to`. The scan never
   reads a city or a defender the seat has not seen. **Reach is the wrapped
   world distance** every campaign reach in this controller is measured in
   (`CAMPAIGN_REACH`, `CAMPAIGN_V2_REACH`, `rival_is_in_campaign_reach`), not
   a route search: a per-turn A* from the capital to every known rival city
   would be a full-map search on every turn of the opening, and it would
   disagree with the reach the rest of the campaign code already enforces.

2. **Reservation** (`conquest_reservation`, `conquest_defers_the_settler`).
   While a target stands and the standard turn is before
   `CONQUEST_COMMIT_DEADLINE` (60), the **capital's** production is reserved
   for `CONQUEST_RANGED` (3) shooters and `CONQUEST_MELEE` (2) melee bodies,
   priced into `production_value`'s military arm the way
   `early_archers_value` is, and waived past the army ceiling the same way.
   When the best shooter the empire can train is a range-one Slinger, the
   node that upgrades it earns `CONQUEST_RESEARCH` in `tech_value` — the
   `early-archers` beeline, reusing `early_archers_node`.

   It is a **reservation, not a bid**: while bodies are missing the capital's
   Settler arm returns `-10_000`, exactly as
   `threatened_recovery_holds_settlers` does. It never defers the **first**
   Settler (`CONQUEST_FIRST_SETTLER_CITIES` = 2 — an empire of one city has
   nothing to conquer with, and every recorded win came from the 4–6 cities
   @ t60 band), never fires in any city but the capital, and never fires
   while the capital is `threatened` — the barbarian alarm, the plan's
   threatened city, or a city hit in the last four turns. The defence
   sentinel outranks the whole opening.

3. **Assembly and declaration** (`conquest_declaration`). The force gathers
   at a rally tile `CONQUEST_RALLY_MIN`–`CONQUEST_RALLY_MAX` (2–3) tiles from
   the target, chosen as the dry passable tile on that ring nearest our
   capital — "on our side" is exactly that and needs no separate geometry.
   When ≥ `CONQUEST_ASSEMBLY_SHARE` (0.8) of the force stands within
   `CONQUEST_ASSEMBLY_RADIUS` (2) of the rally **and** the preview covers the
   city's bill, the cheapest legal war opens through `preferred_war_opening`
   — a casus belli when one is free, otherwise the surprise war. That last
   filter is `raid_opening`'s rule and it matters:
   `preferred_war_opening` can answer with a `Denounce`, which is not a
   declaration and must not be mistaken for one by a force already at the
   ring.

   The **preview** is the shipped `campaign_city_requirement` — defenders
   within `CAMPAIGN_DEFENDER_RADIUS`, the city's own strength, its walls at
   `WALL_STRENGTH_PER_100_HP`, all moved by the tech edge, times
   `CAMPAIGN_SUPERIORITY` — plus a melee capturer and `CAMPAIGN_MIN_BODIES`.
   It is deliberately used **in place of** the host's `SimulateAttackInto`
   reading: `Game::host_preview` is a live-mirror field that does not exist
   in a simulated game, its only reader is `battle_planner_3`, and it was
   measured to over-predict our damage by ~9 HP over 700 strikes.

   The declaration then **pins the campaign**: the plan is written into
   `self.campaign`, `city_campaign_active()` accepts it, and
   `campaign_target`/`campaign_objective_city` are what `assess` reads — so
   from that turn the shipped force groups, staging ring, siege train and
   pillage step are aimed at this city and nothing re-aims them.
   `maintain_city_campaign` returns early while the opening owns the plan, so
   the two never fight over it; with the gene off that guard is exactly
   `false`.

4. **Vision guard** (`conquest_blind_tile_penalty`). A strike-force body is
   charged `CONQUEST_BLIND_TILE_PENALTY` (40) for ending its move on a tile
   whose 1-ring holds a tile it cannot see, unless a friendly unit stands
   beside it. This is the `no_visible_threat` answer. It is a term in the
   deployed mover's one-ply tile score — the same seam `close-as-a-body` and
   `screen-the-shooters` use — and it is **scoped to the force**: a scout's
   job is to stand where it cannot see. 40 outranks any single tile of
   objective progress (`objective_progress × progress` ≈ 3) and the cohesion
   term together, and stays finite so a force with no seen tile still moves.

5. **Continuation** (`conquest_after_a_capture`). While the war's own kills
   per loss is ≥ `CONQUEST_KILLS_PER_LOSS_FLOOR` (1.0 — break-even is already
   twice the live 0.45), the campaign extends to the rival's next **known**
   city; below it, the shipped peace desk is asked for terms. The rate is
   counted by this module from the engine's own `kills` counter and the
   force's roster, because `kills_per_loss` is a Python KPI computed after
   the fact by `tools/live_ledger.py` and **does not exist inside the
   simulator**. A war that has cost nothing is not evidence against itself,
   so a zero-loss war reads at the floor.

Abandonment: if the bill is still uncovered `CONQUEST_ABANDON_TURNS` (20,
`CAMPAIGN_PATIENCE`) standard turns after the force first assembled, the
reservation is released and the reason is journalled; an opening that never
assembles expires at the commit deadline. Every decision — opening, holding,
declaring, continuing, closing, releasing — writes a `think!` line.

### Tests

`src/ai/advanced/early_conquest/tests.rs`, 17 focused tests, each asserting
the OFF behaviour on the same board first:

- registry row is opt-in and off in both controllers; the toggles are twins;
  the named constants are the ones this document states
- target: an unmet rival is no target; a met rival's explored capital in
  reach is; a rival beyond the reach is not; **a city on a tile we have never
  seen is not a known city**; three known cities is an opening and four is
  not; the capital outranks a lighter second city, and a garrison we can
  actually see breaks the tie
- reservation: melee and Slinger bodies priced, civilians not; only the
  capital; zero while threatened; zero once the force is complete; two Scouts
  filed under the census's `melee` are not two strike bodies
- ordering: the first Settler is never deferred, the second is, a threatened
  capital is not, a filled reservation releases it, and the window closes at
  the deadline
- research: the shooter's node is chased, an unrelated node is not, and the
  credit stops once the node is held
- declaration: nothing declared without a force; assembled + covered bill
  declares and pins the campaign; a garrison the force cannot pay for is not
  declared on; the patience window releases; an opening that never assembles
  expires
- vision guard: a seen ring costs nothing, a friendly beside waives the
  charge, an unseen ring alone is charged, a body outside the force keeps the
  shipped score, and off the term is zero
- continuation: a paying war moves to the next known city, a war under the
  floor closes and offers peace, and the rate reads the engine's `kills`
  counter
- switching the gene off drops the opening and the campaign ownership

## What it measured

Provenance: source commit `7f5222f0` (the branch head at probe time, working
tree clean of anything but this round's own files), binary
`target/ci/gene_screen` built at that commit with `--features
developer-tools`, batch **complete — 36 of 36 intended measured seats
(100%)**, seeds 26090900..26090911 reserved and played. Raw rows
`target/early-conquest-opening.jsonl`; analysis
`docs/gene_screens/fires/2026-09-09-early-conquest-opening.json`.

18 seats on, 18 seats off, across 12 games at Emperor with three
firaxis-mix rival chairs carrying the rung's bonuses and the measured seats
exempt.

| column | Δ (on − off) | ± 1 SE | z |
|---|---|---|---|
| **win** | **+16.7 pp** | ± 9.8 | **+1.70** |
| **score share** | **+0.90 pp** | ± 2.59 | +0.35 |
| adjusted win (OLS over every screened gene) | +16.7 pp | ± 9.8 | +1.70 |
| games completed | −1.62 pp | ± 1.95 | −0.83 |
| forgotten | −0.64 pp | ± 0.95 | −0.67 |
| techs @ standard t150 | −0.17 | ± 1.71 | −0.10 |
| techs @ end | −4.28 | ± 4.94 | −0.87 |
| science / turn | −56.97 | ± 62.32 | −0.91 |

Raw win rate: **22.2% on (4 of 18 seats) against 5.6% off (1 of 18)**.

The screen's own read is **`~` — unresolved at this size, which is not the
same as no effect**. The gene does not clear the family-wise bar (|z| ≥ 1.96)
and neither does any other gene in this batch, so there is no sign agreement
to lean on. What the probe establishes is only what it is for: every paired
statistic is non-zero, so the gene **fires** — the tag reaches a decision on
a real board — and `python3 tools/gene_fires.py --max 0` is green with it
committed.

Two things are worth naming honestly rather than reading as results. The win
column is the largest in the batch and points the way the design predicts,
but at 18-vs-18 seats an interval of ±9.8 pp is consistent with anything from
−2.5 to +35.9, and the resolvable delta at this size is **27.4 pp** on wins
and **7.2 pp** on share — both larger than the point estimates. And the
science columns all lean negative (techs @ end −4.28, science/turn −57), which
is exactly the cost a conquest opening should be expected to charge; every one
of them is well inside its own interval, so none of it is measured either.
Nothing here is a price.

## What was decided

**Nothing is priced and nothing is deployed.** `early-conquest-opening` ships
as `Kind::OptIn`, off in `AdvancedAi::new()` and `AdvancedAi::legacy()`, with
no ledger row and no entry in the deployment genome. The probe exists to
satisfy the fires ratchet, not to price the gene; a real verdict needs the
continuous screen at the Emperor shape, and the interval below is far too
wide to read a sign off.

### Limitations

- **A 12-game probe is not a measurement.** Its win-Δ interval is tens of
  percentage points wide. Reading a sign off it is exactly the error
  `docs/EVAL.md` records from 2026-08-17.
- **Reach is world distance, not path length.** The design brief asked for
  path distance; the repo's campaign reach is `wdist` everywhere, and a
  per-turn route search over every known rival city is a full-map search. A
  city across a strait can therefore be named a target and then fail to
  assemble, which the abandon rule catches after 20 standard turns rather
  than before the reservation opens. Priced against a cheaper reach test,
  this is the honest cost.
- **The vision guard is a score term, not a veto.** A force with no seen tile
  to stand on still moves; the term is 40, not infinite. Whether 40 is the
  right number is unmeasured, and it is the first thing a screen should vary.
- **The kills-per-loss rate is the force's, not the war's.** It counts the
  strike force's own bodies, so a loss elsewhere in the empire during the
  same war does not close the campaign. That is deliberate — the rate is
  meant to price *this* campaign — but it is not the same number the operator
  reads out of `tools/live_ledger.py`.
- **The handoff makes `city-campaign-2` a no-op while the opening's war
  runs.** The two are compatible (the guard is one early return), but a
  screen that turns both on measures the opening, not the pair.
