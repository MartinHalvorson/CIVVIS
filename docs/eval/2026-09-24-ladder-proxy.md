# The ladder proxy: where the seat loses to a handicapped field, and the defects it found

_2026-09-24 · `claude-opus55-0923` on `martins-m4-air` · simulator rounds, no native games_

## What was asked

**Where does the live seat's controller lose to a handicapped field, and
what in the controller can close that gap?** The live Civilization VI record
is Emperor 2 wins in 291 attempts, King 1 in 391, Immortal 0 in 35; the
median live seat holds 4 cities at t60 and trails the best rival by 10 techs
at t100 (28 vs 38 at Emperor) and 18 at t150.

## How it was measured

An in-engine proxy of the live shape, run from a private detached worktree
(`examples/ladder_proxy.rs`, not committed): 4 majors, Tiny 60×38 Pangaea,
Online, 6 city-states, all victories, barbarians at their Emperor default.
The focal seat is `handicap_exempt` and plays the live seat's controller —
`targeting(lane)` + `enable_live_bridge_universe` +
`apply_gene_ledger_with_forced_live(deploy/live-force-on.txt)`; the three
rivals are `AdvancedAi::new()` + `enable_live_bridge()` at the rung, so the
rivals are our own deployment genome with the rung's bonuses. Seeds
51000000+ unless noted; batches are paired by seed, so an arm comparison is
the same maps and civilizations. A neutral seat would win 1 in 4 — but seat 0
is not neutral here; see *Seat 0 is not a neutral chair* below. Every
absolute win rate in this round is seat 0's.

## What it measured

### Lanes at Emperor (32 games each, full 250-turn clock, the same 32 seeds)

| lane (focal target) | wins | focal alive at end | score share | Δ share vs Science (paired) | cities t60 / t100 | techs t100 (focal / best rival) |
|---|---:|---:|---:|---:|---:|---:|
| **Science** (the tree default) | 0 | **31** | **11.6%** | — | 4.2 / 6.6 | 23.1 / 38.4 |
| Culture | 0 | 28 | 11.5% | −0.1 pp (z −0.2) | 4.1 / 6.4 | 22.4 / 39.1 |
| adaptive (`civvis`) | 0 | 26 | 10.6% | −1.0 pp (z −1.5) | 3.6 / 5.9 | 23.3 / 38.7 |
| Religion | 0 | 26 | 9.5% | −2.1 pp (z −2.8) | 4.0 / 6.5 | 21.0 / 39.1 |
| Domination (the current native lane) | 0 | 22 | 9.0% | **−2.7 pp (z −3.8)** | 4.4 / 6.6 | 22.9 / 38.7 |

Deity with the Domination lane: 0/32, eliminated in 13, 7.1% share, 22.9
techs at t100 against the best rival's 47.3.

⚠ No lane wins at Emperor in this proxy, because the rivals are our own
genome with +40% Production/Gold and a free Settler — a harder field than
Firaxis' AI. The ordering is still the evidence: the Domination lane is the
weakest and the least survivable of the five.

### Calibration: at Prince the live controller is at parity

16 games, 120 turns, Science lane: 3 wins, **22.0% share**, 26.4 techs at
t100 against 27.1. Without the forced live list: 20.7% (Δ −1.3 pp, z −1.2).
At Emperor without the forced list: −1.1 pp (z −2.6). So the Emperor gap is
the handicap, not a broken policy — and closing it means the seat has to
out-play its own genome by roughly the handicap's margin.

### Nulls (paired, Emperor Science lane, 16 games × 120 turns unless noted)

| arm | Δ share | note |
|---|---:|---|
| barbarians off | −0.1 pp (z −0.1) | absolute cities and techs rise, the rivals' rise equally |
| a Settler "pump" reservation (live seat, Emperor) | −0.2 pp (z −1.4) | cities t60 unchanged: raiders at the gates and slow Production, not the ranking, bind it |
| the same pump on the deployment genome (Prince, 32 × 150 turns) | −0.7 pp (z −1.3) | never fires before t60 (cities t30/t60 identical); t100 cities fall 7.2 → 6.6 |
| `camp-party` on | **−1.4 pp (z −2.4)** | pulling the whole army to a camp costs more than the camp does |
| camps within 4 (not 7) raise the barbarian alarm | +0.45 pp (z +0.8; 32 games × 150 turns, Prince, deployment genome) | wins 6 → 8 |

### The waste audit — and the defects it found

The productive instrument was not an arm but an audit: what is idle **at the
end of a seat's own turn** (the engine processes a seat's cities in its own
`begin_turn`, so a turn-start reading counts every just-completed queue as
idle and is an artifact).

1. **Fog-honest seats lost 8.7% of their Production to refused wonder
   orders** (27 rival seats, Emperor, 100 turns; worst seat 17.9%). The
   decision view redacted unseen cities and the wonders standing in them, so
   the planner kept ordering a wonder already built elsewhere; the
   authoritative board refused it through every replanned frame and the city
   ended its turn empty. Fixed in #3748 (the view now carries the built set,
   never the builder, on `host_unavailable_wonders`): **8.71% → 0.17%**.
2. **civvis.ai seated the stock agent, not the deployment genome.**
   `Session::ai_fleet` said "the deployment genome — AdvancedAi with the
   gene ledger applied" and seated `AdvancedAi::new()`. At the site's own
   shape (Prince, 4 majors), the ledger genome planning on the authoritative
   board won **24/48** as one seat against three stock agents (95% CI
   36–64%), and a stock agent won **4/48** against three of it (8.3%) — both
   in seat 0, whose symmetric rate is 38.6% with the deployment genome and
   16.7% (3/18) with the stock agent. Seated in #3749, at about 4× the AI
   compute per turn in native release builds.

3. **A wonder site on a strategic resource the seat has not revealed** was
   the next leak the audit's guard found (0.74% of Production after #3748):
   the view hides the resource, the board's placement rule refuses the tile,
   and every replanned frame chose the refused order again. Fixed in #3750
   in the executor — a refused `Produce` becomes a per-city
   `blocked_production` key in the next frame's view for the rest of the
   turn — with a guard that fails above 0.5% (it read 1.95% before #3748 and
   0.74% after; 0.00% now). Refused orders on the guard game fell 133 → 72.

Also measured, not acted on: at Prince the deployment genome uses 12% of
its trade capacity at t50 and 51% at t100. A native-only Trader gate that
refuses an origin only for a raider within three tiles lifts that to 67% and
93%, and on 96 paired full games moved score share **+1.4 pp (z +2.1)** and
survival 90 → 95 — but wins **38 → 35** (discordant 7 up, 10 down, sign test
p 0.63). Mixed; not shipped.

### ⚠ Seat 0 is not a neutral chair

The harness seats the focal agent in seat 0 every game. In **symmetric**
games — four identical deployment genomes, 114 full games — the seats won
**44 / 20 / 29 / 21**: seat 0 takes 38.6%, not 25%, and it already leads at
t30 (2.62 cities against 2.37–2.49) and t100 (26.8 techs against 24.8–25.9).
The pattern is not monotone in turn order, which points at start placement
(`assign_starts_by_bias` falls back to the spawn list in seat order) as much
as at moving first. Paired comparisons in this round are unaffected — both
arms share the seat — but **every absolute win rate above is seat 0's**, and
#3749's description compared against "1 in 4"; the correction is on #3749.
On civvis.ai the person plays seat 0, so if this is placement it is also a
fairness question worth its own round.

### The Domination lane in its own profile (King, Gran Colombia, seed 37140005)

The seat out-expanded the field — 10 cities at t106, the most on the board —
and then did not fight: the campaign read "aimed at Khmer — not yet at war"
for 50 turns (t71–t121) with no Siege row on the Objective Board (Siege rows
exist only once at war) while the grand strategy stayed Expansion until 10
cities of 10 wanted; at t121 it retargeted a city-state and declared on it,
and lost to Egypt's culture victory at t181. The same shape as
`docs/eval/2026-09-22-domination-progress-probe.md`: the lane's bottleneck
is the war it never starts, not the siege it fights.

## What was decided

- Shipped: #3748 (wonder view), #3749 (civvis.ai seats the deployment
  genome), #3750 (refused orders blocked for the rest of the turn, with a
  Production-loss guard). civvis.ai/test served #3749 from 06:48 UTC; a
  headless-Chrome check there played 109 game turns in 115 s with no console
  errors.
- **Recommended to the operator, not changed:** the native lane is a host
  setting (`~/.civvis-victory-lane` / verification policy on the Civ VI
  seat); the tree default is already `science`. On this evidence the
  Domination lane is the weakest and least survivable choice at Emperor.
- Not shipped: the trade gate (mixed), the Settler pump (null), the narrower
  camp alarm (null), `camp-party` (negative).
