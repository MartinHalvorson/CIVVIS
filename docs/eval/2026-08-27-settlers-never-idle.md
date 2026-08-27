# Settlers that sit in the city: how it was allowed, what it costs, and the gene that ends it

_2026-08-27 · PR #2655 · `settler-never-idles`, pinned on_

## What was asked

Operator: *"our settlers often just sit around in the capital or other
cities. this is a huge mistake. investigate how this was allowed to happen
and fix the behavior so this can't happen. default genes to on right away."*

## How it was allowed to happen

`AdvancedAi::advanced_settler_step` (`src/ai/advanced.rs`, ~1,000 lines) is
the Settler's whole turn. Read end to end it has more than a dozen branches
that return `false` — the unit stands still — and every one of them was added
against one live run's anecdote:

| hold | gate | added for |
|---|---|---|
| every preferred site refused → `return false` | `loyalty_rate_alarm \|\| frontier_loyalty` | fogged colonies that revolted, 2026-08-16 |
| `beyond_loyalty_reach`: no own city within 7 tiles *and any unexplored plot within 9* refuses a site | `frontier-loyalty` (host-only) | Arpinum/Lugdunum revolts |
| every refused site retired for **30 standard turns** (`settler_dead_sites`) | always | a settler marching back to a doomed plot |
| `settlement_tile_risk > 30` on the target or the next step | `settlement_safety` (always on) | three settlers walked beside strength-20 warriors |
| "waits for its guard" | `live-formationless-settler-shadow` (host-only) | two doorstep captures, 2026-08-15 |
| "waits outside a barbarian's reach" | `civilian-out-of-reach` (on, +29/+20/+31) | eight settlers taken in 104 turns |
| hysteresis: a dropped target set aside 6 turns for every own settler | `settler-target-hysteresis-2` (on) | four settlers orbiting one site |

None was ever charged for the turns it costs. The holds that matter are gated
on host-only genes or on `loyalty-rate-alarm`, the screen cannot price
host-only genes, and `docs/AI_GAPS.md` had already recorded the consequence
under *"the settler-idling lane cannot be screened at all, and that is why
it has not moved"*. The compounding is the mechanism: a refusal retires the
site for thirty turns, the next pick is refused and retired, and within a
few turns every site the ranking offers is "dead" — so the search returns
nothing and the code holds, silently, for the rest of the game. #2648 (this
morning) added one local probe after three refusals; it uses the same
filters and holds when that fails too.

`docs/EVAL.md` (2026-08-10) had measured the older form of the defect —
"two hundred turns of a unit standing on ground it could settle" — and left
its repair (`settler_founds_when_stalled`) off because it could not be
*promoted on wins* at any affordable sample. The operator's directive today
is the opposite policy for this lane, and this file records it.

## The instrument

`settler_idle_census` (`src/ai/advanced/settler_idle_census.rs`, `#[ignore]`)
plays whole games — 6 players, 60×38, Online, 250 turns, seeds 98,000,000+ —
with a recording `Journal` on every major, and for every Settler a major
owns records each turn as MOVE, FOUNDING or IDLE; for the idle turns it
records whether the unit stood on an own city tile and the reason the seat's
own journal gave ("Settler HELD short …", "waits for its guard", "waits
outside a barbarian's reach", "sets … aside", "refuses a site before
walking"), and when the journal said nothing, what the board offered
(`best_settler_target`, `any_settle_site`, `has_practical_settle_site`).

Two arms. `deployment` is `enable_engine_repairs()`, the genome a native
game ships. `live` is `enable_live_bridge()`, the Civilization VI seat's
genome — host-only genes included — given a native board. The host-only
genes were written for the live seat and are never screened natively, so the
`live` arm says what the seat's settler logic does when it is handed a
board, not what the native game ships; the operator watches the live seat.

```bash
CIVVIS_CENSUS_MAPS=8 CIVVIS_CENSUS_ARMS=deployment,live \
  cargo test --profile ci --lib settler_idle_census -- --ignored --nocapture
CIVVIS_CENSUS_OPT_INS=settler-never-idles …   # the gene on, in a build where it is not yet pinned
```

## What it read, before the gene

Eight maps each.

| genome on a native board | settlers | settler-turns | idle | idle on an own city tile | settlers idle ≥10 turns in a city | longest in-city streak p90 / max | alive at the end, never founded |
|---|---|---|---|---|---|---|---|
| deployment | 285 | 3,149 | 19.5% | 4.2% of all turns | 2.1% | 1 / 19 | 13 |
| **live seat** | 772 | 51,215 | **85.6%** | **33.7% of all turns** | **28.1%** | **93 / 185** | **398 of 772** |

On the live seat's genome 45.5% of Settlers were idle on the tile they were
built on, the turn they were built; the median build-to-first-move was one
turn but the p90 was 53; the five worst stood in the city that built them for
149–185 turns and never moved. The reasons, live arm:

| idle turns | of which in a city | why |
|---:|---:|---|
| 26,247 (59.9%) | 13,173 | **NO TARGET**: a legal site exists near a city but none is reachable/ranked for this settler — the search returned nothing and the code held |
| 5,360 (12.2%) | 627 | holds a target and never stepped (the sea-escort link, the opening settler, a linked formation that does not move) |
| 4,280 (9.8%) | 1,259 | waiting for a guard (`guard_wait` set, no line this turn) |
| 4,168 (9.5%) | 829 | sets its target aside (hysteresis): a city within three tiles — and finds nothing else |
| 1,102 (2.5%) | 404 | sets its target aside: marked dead for this settler |
| 846 (1.9%) | 406 | NO TARGET although the picker offers a site: the loyalty verdict refused it |
| 300 + 286 + 116 + 40 | — | waits for its guard · safe-step guard rejected every neighbour · loyalty forecast refusal · waits outside a barbarian's reach |

The deployment genome's own idle turns split differently — 27.9% "no target
although a legal site exists", 24.6% "the safe-step guard rejected every
neighbour", 18.2% loyalty-forecast refusals (`loyalty-rate-alarm` ships on),
5.5% out-of-reach waits — and concentrate late (t150+: 301 of 614), when the
map is nearly full and a Settler is built for a site that is gone by the
time it walks out.

## The gene

`settler-never-idles` (`src/ai/advanced/settler_never_idles.rs`,
`Kind::OptIn`, pinned on in `OPERATOR_DEFAULT_ON`; byte-identical off):

1. **Exhaustion never holds.** When the preferred search returns nothing,
   the Settler asks two wider questions: the advanced ranking over a
   fourteen-tile radius (the whole map after Shipbuilding) with this
   Settler's retirements, the empire's threat deferrals and the fog guesses
   set aside, refusing only a site the engine's own Loyalty calculation says
   revolts inside twenty turns (`settler_exhaustion_target`, tier 2); then
   any legal reachable site, nearest first (tier 3). Failing both it founds
   where it stands if the engine allows and the city would hold, and
   otherwise writes "Settler is stranded" to the journal — a hold is never
   silent.
2. **A watchdog bounds every other hold.** A Settler that has stood on one
   tile for `SETTLER_IDLE_PATIENCE` (2) turns marches on one rule only: never
   end the turn on a tile a visible hostile can reach next turn — the
   barbarian reach flood `civilian-out-of-reach` already plays, plus the
   movement allowance of every visible at-war major unit. A softer risk
   score, a guard that has not come, a forecast about fogged ground: none
   may hold a Settler past its patience. A linked Settler is unlinked first.
3. **The guard wait returns the turn** so the watchdog can see it.

## What it read, with the gene

| genome | settlers | settler-turns | idle | idle on an own city tile | settlers idle ≥10 turns in a city | longest in-city streak p90 / max | alive at the end |
|---|---|---|---|---|---|---|---|
| deployment | 285 → 276 | 3,149 → 2,685 | 19.5% → **9.2%** | 4.2% → **1.0%** | 2.1% → **0.0%** | 1 / 19 → 0 / 6 | 13 → **3** |
| live seat | 772 → 631 | 51,215 → 23,896 | 85.6% → **47.2%** | 33.7% → **7.1%** | 28.1% → **5.5%** | 93 / 185 → **3 / 63** | 398 → **191** (founded 373 → **440**) |

On the live seat's genome the median build-to-first-move fell from one turn
to zero and the p90 from 53 turns to 2; the share of Settlers idle on their
birth tile the turn they were built fell from 45.5% to 28.5%, and the ones
that were are now the ones whose every exit a raider can reach. What remains
of the live arm's idle turns is a different defect: 53.8% are "no legal site
anywhere in reach" — the host-only production genes (`land-grab`,
`parallel-settlers`, `host-settler-pop`) keep building Settlers after the
map is full for that seat, which is a production gate's business, not a
Settler's — and 24.5% carry a stale `guard_wait` marker on a Settler the
livelock detector has stood down (see below).

With the gene the deployment genome's remaining idle turns are the bounded
safety holds — the safe-step guard (115 of 248), the out-of-reach wait (48),
the opening settler's own wait (42) — and the longest any Settler stood in a
city was six turns, with a barbarian in reach of every exit.

## The fires probe and the screen

`gene_screen --games 6 --genes settler-never-idles --start-seed 99400000`
(`docs/gene_screens/fires/settler-never-idles.json`): 36 seats, 22 on /
14 off, win 22.7% v 7.1% → **+15.6 pp ± 6.9**, share +1.9 pp ± 2.3 — the
gene fires, on a probe that resolves nothing finer.

A first standard screen (`--games 300 --p-default-on 0.5`, seeds
99,500,000+) was read at 77 games / 462 seats: win −3.8 pp, z −1.10, 95% CI
[−10.4, +2.9]; share −0.02 pp — unresolved — **and compute +4.9% ± 2.3 per
treated seat**. That cost was the first version's exhaustion search: a
stranded Settler paid the whole search every turn, and the tier-3 scan
priced every legal plot in a fourteen-tile radius with a growth forecast
for a tie-break. Both were cut (tier 3 sorts by distance alone; a Settler
found stranded on a tile is not asked again from it for five turns) and the
screen was restarted on the shipped code as `--games 200`; its reading is
below.

**The 200-game screen** (`gene_screen --games 200 --jobs 10 --genes
settler-never-idles --p-default-on 0.5 --start-seed 99500000`, the standard
shape, binary `7eeb4c1a` — the cost fix in, the arrival-verdict fix below it
not yet): 1,200 seats, 601 on / 599 off, win 16.1% on v 17.2% off →
**−1.1 pp ± 2.2 (z −0.48)**; score share −0.04 pp ± 0.36 (z −0.12);
compute cost **−0.5% ± 1.0** per treated seat, whole-game time −1.2% ± 1.8.
Unresolved (`~`): the design resolves ±6.1 pp on wins at 80% power, and the
gene sits well inside that. Read with `docs/EVAL.md`'s 2026-08-10 entry on
the same lane — "a defect's drama and its Elo are unrelated quantities" —
this is the expected shape: a Settler that stops standing in the capital is
worth cities and score in the census and nothing a 200-game screen can see
on wins. The cost is the finding that matters for the fleet: the first
version's +4.9% is gone. The gene ships on by the operator's directive; the
reporting batches will price it beside every other gene from here.

## What is deliberately not done

- No production change: a Settler is still built on the same gates
  (`has_practical_settle_site`, the serialization, pop ≥ 2). The census's
  "no target although a legal site exists" is the *Settler's* search
  disagreeing with the *city's* site check; the gene makes the Settler agree.
- The loyalty forecast is not removed: it still ranks and still vetoes a
  site that revolts inside twenty turns. What it may no longer do is hold a
  Settler for the rest of the game on a guess about fogged ground.
- `civilian-out-of-reach`'s waits and the safe-step guard are untouched
  inside the patience window; the watchdog's own rule is the same exact-reach
  rule, so it cannot walk a Settler into a capture those genes would have
  refused.
