# The ladder proxy, round 2: the Domination lane's war, a Settler the board kept refusing, and the live seat's thin economy

_2026-09-24 · `claude-opus55-0923` on `martins-m4-air` · simulator rounds, no native games_

Follow-up to `docs/eval/2026-09-24-ladder-proxy.md` (round 1, #3751). Same
instrument (`examples/ladder_proxy.rs`, private, not committed), same shape
(4 majors, Tiny 60×38 Pangaea, Online, 6 city-states; the focal seat is the
live controller, `handicap_exempt`; three rival seats are `AdvancedAi::new()` +
`enable_live_bridge()` at the rung). Every batch below is 32 games at **King**,
Gran Colombia, seeds 37140000+, full 250-turn clock, paired by seed. New
instruments this round: an army audit (share of the seat's land strength
within 3 / 6 tiles of an enemy it is at war with, Reserve and Siege force
sizes, each war turn), a district and building census at turns 50/100/150,
refused-`FoundCity` counters, and a per-turn trace of the denial target and
each rival's victory pressure as the seat's own decision view reads them.

Arm names below in `code` are prototypes in the private bench, not registry
genes — except `lane-delegates-production`, which #3753 adds to the registry.

## Correction to round 1: seat 0's edge is not start placement

Round 1 suspected start placement for seat 0's 38.6% in symmetric games.
Measured over 300 seeds each, start quality (food and production within the
start's workable ring, and which seat holds the best start) is balanced across
seats both on the ladder's 4-player Pangaea and on a 6-player Continents map
(on Continents, seat 0 holds the best start in 54 of 300 maps against 47–62
for the other five seats). The edge is turn order, not the map. Paired
comparisons are unaffected.

## A Settler the board kept refusing (fixed in #3752)

The King Domination profile's seed 37140001 held **2 cities from turn 30 to
turn 65** while every rival held 3–5. One of its Settlers stood on (40, 16)
for 39 turns. A per-turn probe that applies `FoundCity` to a clone of
the authoritative board and of the seat's decision view read, every turn:
`board: "cannot found city here" · view: ok` — Geneva, a city-state the seat had
never seen, stood three tiles away. The fog-honest executor plans on the view
and replays on the board; nothing carried the refusal back, so every frame of
every turn chose the same site again. It also capped expansion: with two
Settlers alive (one frozen, one waiting) the empire counted its Settler
pipeline full, and at turn 43 the capital, at 15.3 Production, began a Campus
where Babylon's capital was producing a Settler every seven turns.

The native bridge had already met this against Civilization VI and built the
channel for it (`Game::blocked_city_sites`, filled from host refusals). #3752
fills the same channel in-engine: a tile the board refuses a `FoundCity` on —
only when the board's own `can_found_city` says no — is kept on the controller
for the rest of the game and blocked in every later view.

| | refused `FoundCity` orders per seat-game (King, 32 games) |
|---|---:|
| live focal, no memory | 27.3 (max 295) |
| rival seats, no memory | 22.9 (max 438; 17% of seat-games ≥ 30, 8% ≥ 100) |
| live focal with the memory | 0.84 (max 5, each a distinct site) |

With the memory the seed above holds 5 cities at turn 60 and 7 at turn 80
(against 2 and 4). Over 32 paired games the focal's cities at turn 60 rise
4.22 → 4.50 and at turn 100 7.19 → 7.59, population at turn 100 31.1 → 32.6;
score share +0.09 pp (z +0.16). The 18 games without a focal refusal are
byte-identical to the baseline. civvis.ai (omniscient since #3749) and the
native seat (the bridge's channel) do not change; every `gene_screen` seat does.

## The Domination lane: what the war does

Baseline (live controller, Domination lane, King, 32 games): 0 wins, score
share 13.8%, alive at the end 29. Per game the seat declares 0.94 wars on
majors and 0.59 on city-states, attacks cities 13.4 times, holds 0.41 captured
cities at the end and no captured capital. In each war turn 29.7% of its land
strength stands within three tiles of an enemy and 61.4% within six.

### Three defects the traces showed

1. **A peace offer at the moment of capture.** #3671 lets a Domination seat
   offer its current front peace "to counter a rival victory threat" when the
   denial layer names another rival as the Culture or Religion leader. In
   seed 37140001 that leader was Mali, whose Culture pressure read 48–53
   around the 50% preparation bar, flickering on and off turn to turn; each
   "on" offered peace to the current front, and the AI accepted. At turn 182
   the seat made peace with Babylon while Lagash stood at walls 0/400, city
   54/200; at turn 193 with Gaul during the siege of Lutetia. Three turns
   later each time the threat read off again (Mali "Diplomacy 52", "Religion
   50"), and the army never moved on Mali. The clause has no hysteresis and,
   unlike the tide clause beside it, never asks whether something is about to
   fall (`one_war_prizes_in_reach`). PR #3671 validated it on recorded-state
   replays and said so: they "cannot establish ... a win-rate improvement".
2. **The objective board's Reserve flickers.** `assess_board` pre-maps last
   turn's forces — the Reserve included — into `assignment`, computes the
   leftovers as units in no force, and then *replaces* the Reserve's units
   with those leftovers. Last turn's Reserve units are in no leftover list and
   are dropped: on alternate turns about half the leftover army is in no force
   at all. A four-round unit probe reads Reserve `[11]`, none, `[11]`, none.
   Units in no force take the per-unit path toward the plan's target; units
   in the Reserve hold on its tile. In the traced game the Reserve held 9–16
   land units through a two-front war while the siege had 4–5.
3. **A siege left in Invest.** `siege-train` moves Invest → Reduce only on a
   sealed ring or after three turns with a shooter in range; Lutetia's walls
   fell to 0/400 and its garrison to 111/200 at turn 190, and over the next
   five turns the city healed back to 200/200 while the train read "invest".

### Arms (King, 32 paired games each against the baseline)

| arm | Δ score share (z) | city attacks / game | captured at end | note |
|---|---:|---:|---:|---|
| `domination-majors-only` (never a city-state campaign) | −0.05 pp (−0.13) | 13.4 → 3.2 | 0.41 → 0.12 | the lane's fighting was mostly city-states |
| `victory-threat-peace-guard` (the clause needs an urgent threat and nothing falling) | +0.25 pp (+0.97) | 13.4 → 13.5 | 0.41 → 0.53 | cities held 0.12 → 0.22 |
| `reserve-keeps-its-units` (the flicker fixed alone) | −0.36 pp (−0.50) | 13.4 → 7.4 | 0.41 → 0.28 | the flicker's ungrouped half had been marching at the target |
| `siege-surge` (leftovers join the top Siege force) | −0.64 pp (−0.91) | 13.4 → 7.6 | 0.41 → 0.38 | Siege force 4.8 → 8.2 units, fewer attacks |
| `siege-train` off (forced on the live seat) | +0.09 pp (+0.19) | 13.4 → 7.1 | 0.41 → 0.31 | the doctrine is not the passive part: without it the seat attacks even less |
| `reserve-marches-at-war` (a steady Reserve at peace; at war every leftover marches ungrouped) | −0.48 pp (−0.71) | 13.4 → 8.2 | 0.41 → 0.22 | alive 29 → 27 |

The surge is the informative null: more units under the siege doctrine made
*fewer* attacks than the same units on the per-unit path — yet removing the
doctrine altogether cut attacks just as much. Nothing tried at the level of
the war moved the lane's score; its deficit is set before the war starts
(next section).

## The live seat's economy: wider, thinner, and behind its own genome

The census compares, at turn 100, what the focal seat has built against the
average rival seat, on the same 32 King seeds. "Deploy" is the focal seat
playing exactly the rivals' agent (`AdvancedAi::new()` +
`enable_live_bridge()`: no lane target, no forced-live list) without the
handicap; "live" is the live controller as round 1 defined it.

| turn 100 | live Domination | live Science | deploy | rivals (avg) |
|---|---:|---:|---:|---:|
| cities | 7.2 | 7.2 | 6.2 | 6.9 |
| population | 31.1 | 33.2 | 34.3 | 47.0 |
| trade route capacity | 1.0 | 1.8 | 3.0 | 5.3 |
| Granary | 2.7 | 3.4 | 4.2 | 5.9 |
| Market / Lighthouse | 0.0 / 0.0 | 0.6 / 0.1 | 1.4 / 0.4 | 2.4 / 1.0 |
| Commercial Hub / Harbor | 0.2 / 0.2 | 1.3 / 0.2 | 2.3 / 1.3 | 3.0 / 1.5 |
| Holy Site / Theater Square | 0 / 0 | 0 / 0 | 1.1 / 0.8 | 1.9 / 0.8 |
| Campus / Library | 4.3 / 3.1 | 5.6 / 3.6 | 3.2 / 2.9 | 3.9 / 3.7 |
| Industrial Zone | 1.2 | 1.5 | 0.0 | 0.1 |
| production share on Settlers / military / buildings (to t100) | 20 / 18 / 23% | 18 / 14 / 28% | 11 / 10 / 35% | 7 / 10 / 35% |
| **score share at the end** | **13.8%** | **14.9%** | **17.7%** | — |

Paired over the 32 seeds, **deploy beats live Science by +2.85 pp (z +3.25) and
live Domination by +3.92 pp (z +3.89)**; live Science beats live Domination by
+1.07 pp (z +1.70). The live controller expands one city wider by turn 100 on
the same total population, spends almost twice the deployment genome's share
on Settlers and half again on military, and skips the cheap growth and trade
buildings (Granary, Market, Lighthouse) and every faith and culture district.
King carries no Food handicap (`data/difficulties.json`: +8% science, culture,
faith; +20% production, gold; a free Warrior and Builder), yet the deploy seat
— the rivals' own agent — still trails them by 11 citizens at turn 100: the
Production bonus compounds into Settlers, Granaries and districts. The live
configuration costs 1–3 citizens more than that, and its score gap is larger
than its population gap.

### The tax is the lane target, not the forced list

Three more arms on the same seeds complete a 2×2: lane target (none, i.e.
adaptive `civvis`, or Science) × the forced-live list (off or on). "Deploy" is
the adaptive cell without the list.

| score share (King, 32 paired) | forced list off | forced list on |
|---|---:|---:|
| adaptive (no lane target) | **17.7%** | **17.8%** |
| Science target | 15.5% | 14.9% |

| paired difference | Δ share | z |
|---|---:|---:|
| forced list, adaptive seat | +0.14 pp | +0.12 |
| forced list, Science seat | −0.58 pp | −0.80 |
| Science target, list off | −2.26 pp | −2.78 |
| Science target, list on | −2.98 pp | −4.14 |

Settlers and military take 11% / 10–11% of an adaptive seat's production to
turn 100 and 18% / 14% of the Science seat's. The forced list is neutral; the
lane target is where the live seat's economy goes: it spends the difference on
Settlers and soldiers and arrives at the second half one city wider and several
buildings poorer.

**Where the tax comes from.** A per-turn tally of the focal plan's grand
strategy (8 games each) finds the two seats in the same posture most of the
first half — Expansion 91% of turns 1–125 for the Science seat, 86% for the
adaptive seat, which spends the other 14% in the Prophet race — both aiming at
10 cities. What differs is the production manager under that posture. In the
turn driver an assigned lane sends every city through the strategic scorer
(`advanced_production`); an unassigned seat runs the scorer only in Recovery,
under `expansion_dispatch`, or for an appointed war, and then hands every
remaining queue to `advanced_support_production` and the baseline city
governor (`delegated_cities`). The strategic scorer prices a Settler at
920 + 4 × site value and carries lane-only terms — `INDUSTRIAL_BUILDING_DEBT`
for every assigned lane, a great-work veto outside Culture, a Holy Site veto
on Science — while the baseline governor builds the Granaries, Monuments,
Markets and shrines the census shows missing.

Two narrower hypotheses were tried first and found wanting:

| arm (Science target, King, 32 paired) | Δ share vs the Science seat (z) | Δ vs deploy (z) |
|---|---:|---:|
| `science-keeps-holy-sites` (the Holy Site veto lifted) | ±0 — byte-identical | — |
| `lane-waits-for-specialization` (the adaptive strategy chain until halfway) | +0.46 pp (+1.08) | −2.39 pp (−2.50) |
| both together | +0.48 pp (+1.17) | — |
| both, with `lane-delegates-production` | +1.45 pp (+3.26) | +0.06 pp over delegation alone (+0.25) |

The veto never binds: no Holy Site ever reaches the top of the scorer's
ranking under the Science and Expansion weights (Faith 0.4–0.5); the adaptive
seat builds its Holy Sites during its Prophet-race turns, when the strategic
scorer is not running its cities at all.

### The repair: `lane-delegates-production` (#3753)

The gene gives an assigned lane the unassigned seat's dispatch until the
specialization clock (halfway): the strategic scorer only in Recovery, under
`expansion_dispatch` or for an appointed war, then `advanced_support_production`
and the baseline governor, which also spends the Gold. The lane's plan, its
reservations and victory purchases are untouched, and from halfway its cities
return to the scorer that prices the victory's own districts and projects.

| arm (King, 32 paired) | Δ share vs the same lane (z) | Δ vs deploy (z) | captured at end | cities held |
|---|---:|---:|---:|---:|
| Science + `lane-delegates-production` | **+1.39 pp (+2.86)** | −1.45 pp (−1.44) | — | — |
| Domination + `lane-delegates-production` | +0.97 pp (+1.20) | −2.95 pp (−3.11) | 0.41 → 0.53 | 0.12 → 0.28 |

| turn 100, Science seat | without | with the gene |
|---|---:|---:|
| Settlers / military / buildings, share of Production | 18 / 14 / 28% | 10 / 9 / 39% |
| Granary | 3.4 | 5.1 |
| Market / Lighthouse | 0.6 / 0.1 | 2.5 / 0.8 |
| Commercial Hub / Harbor | 1.3 / 0.2 | 3.2 / 1.3 |
| trade route capacity | 1.8 | 4.4 |
| Industrial Zone | 1.5 | 0.0 |
| cities / population | 7.2 / 33.2 | 6.1 / 34.7 |

It recovers about half of the Science lane's tax and a quarter of the
Domination lane's, and the Domination seat takes and keeps more cities, not
fewer, with its first-half queues out of the scorer's hands.

### At Emperor the lane pays for itself

Round 1's Emperor seeds (51000000+, 32 games, random civilizations), current
build:

| Emperor, 32 paired | score share | alive at the end | cities at turn 100 |
|---|---:|---:|---:|
| Science lane (live) | 11.3% | 29 | 6.6 |
| deploy (unassigned) | 11.2% | 26 | 5.6 |
| Science + `lane-delegates-production` | 11.7% | 31 | 5.7 |

Deploy against the Science lane: −0.15 pp (z −0.19). Against rivals that open
with a free Settler and take +40% Production, the assigned lane's wider,
better-armed first half is what keeps the seat alive (29 against 26) and it
costs nothing in score; at King the same investment is over-insurance and
costs 3 points. With `lane-delegates-production` the Science seat
moves +0.33 pp (z +0.66) and survives more often (31 against 29): the gene is
not a King-only trade. Its turn-100 economy at Emperor — Granary 4.5, Market
2.4, trade capacity 4.0 — is the unassigned seat's or better, on a city fewer
than the lane without it.

### Version two: the whole game (#3754)

Version one hands the cities back to the strategic scorer at halfway. A
second version keeps the unassigned dispatch for the whole game; the lane's
reservations and victory purchases still run ahead of the routine governors.

| 32 paired | v2 against the lane | v2 against v1 | v2 against unassigned |
|---|---:|---:|---:|
| Science, King | **+2.35 pp (z +4.78)** | +0.96 pp (z +2.78) | −0.49 pp (z −0.46) |
| Science, Emperor | +0.30 pp (z +0.66) | −0.03 pp (z −0.10) | +0.45 pp (z +0.50) |
| Domination, King | +1.38 pp (z +1.67) | +0.41 pp (z +0.95) | −2.53 pp (z −2.50) |
| Domination, Emperor | ±0.00 pp (z 0.00; alive 23 → 25) | — | — |

On the Science lane at King version two closes the lane's whole tax: 17.2%
against the unassigned seat's 17.7%. Technologies at turn 150 hold (38.6
against 39.0 without delegation; Campuses 5.8 against 7.2). #3754 ships it as
`lane-delegates-production-2`, in one mutually exclusive family with version
one, and forces it on the live seat in version one's place.

## What was decided

- **Shipped**: #3752 (a refused city site is blocked in every later view of
  the fog-honest seat), #3753 (`lane-delegates-production`: +1.39 pp at King,
  +0.33 pp at Emperor on the Science lane, +0.97 pp at King on the Domination
  lane) and #3754 (`lane-delegates-production-2`, the whole game: +2.35 pp and
  +1.38 pp at King). Both are opt-in and screened like any gene; the live seat
  forces version two through `deploy/live-force-on.txt`.
- **Recommended to the operator, not changed**: the native lane. At King the
  unassigned lane (`civvis`) beat the assigned Science lane by 2.3–3.0 pp and
  the Domination lane by 3.9 pp; at Emperor the assigned lane cost nothing.
  With #3754 on the live seat, 0.49 pp of the Science lane's King gap remains
  and 2.53 pp of the Domination lane's.
  The Domination lane stays the weakest at both rungs (round 1: −2.7 pp
  against Science at Emperor; here −1.07 pp at King).
- **Not shipped**: `domination-majors-only` (null), `reserve-keeps-its-units`
  (negative), `siege-surge` (negative), `reserve-marches-at-war` (negative),
  `siege-train` off (null), the Holy Site veto lift (inert),
  `lane-waits-for-specialization` (+0.46 pp, z +1.08; nothing on top of
  delegation).
  `victory-threat-peace-guard` read +0.25 pp (z +0.97, captures up) on the
  first 32 seeds and +0.03 pp on a second 32 (37140032+): pooled over 64 games
  +0.14 pp (z +1.06) — too small to carry a gene.
- **The Reserve flicker is a real defect whose fix alone measured negative**:
  the flicker's ungrouped half was doing the marching. A repair has to give the
  Reserve work at war, not only keep it steady — and marching all of it at
  the front (`reserve-marches-at-war`) measured worse still.

⚠ The proxy's rivals are our own deployment genome with the rung's bonuses,
not Firaxis' AI: its King is harder than the native King, and a gene's native
effect is read on the ladder, not here.
