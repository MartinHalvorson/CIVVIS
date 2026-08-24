# The gap to Civilization VI

Three questions get asked repeatedly, and answering any one of them today means
reading four documents and a few hundred memory notes:

1. What are the biggest gaps between CIVVIS and Firaxis' Civilization VI?
2. Why does a CIVVIS simulation not *look* like Civ 6?
3. Why does CIVVIS' measured strength not translate to the real game?

This page is the synthesis. It owns no findings of its own: every number below
is carried from the document that measured it, and that document stays the
source of truth. Read this to orient, then follow the link.

- rules **data** against the shipped game — [FIDELITY.md](FIDELITY.md)
- rules **behaviour** against a running game — [GROUNDING.md](GROUNDING.md)
- the agent's own strengths and failures — [AI_GAPS.md](AI_GAPS.md)
- what the live bridge can and cannot actuate — [CIV6_COMPUTER_CONTROL.md](CIV6_COMPUTER_CONTROL.md)
- the difficulty ladder's record — [CIV6_LADDER.md](CIV6_LADDER.md)

---

## 1. What the gap actually is

### The numbers are right. The content is not.

`tools/civ6_fidelity.py` against a real Gathering Storm install reports **0
divergent fields across 27 tables**, 1,367 fields compared. That axis is
finished, and it is worth saying plainly because it is the axis people assume
is the problem.

What the same report shows instead is its *only in Civ VI* column — content the
game ships and CIVVIS does not model at all.

> ⚠ **Do not read a coverage number out of this page, and do not paste one in.**
> Run the tool:
>
> ```sh
> python3 tools/civ6_fidelity.py     # the only-in-Civ-VI column, per table
> python3 tools/civ6_modifiers.py    # the modifier census
> ```
>
> This is the `ROADMAP.md` convention — *"generated in `docs/EVAL_STATUS.md`; do
> not duplicate those numbers in prose"* — applied here because **this page has
> already been wrong about them once.** On 2026-08-23 its frozen table said
> GreatPeople 184 / Beliefs 22 / Projects 17 while the tool said 148 / 17 / 11,
> and `docs/MODIFIERS.md` said 825 rows implemented while its own tool said
> 1,552. A census moves every time content lands; prose does not.

For orientation only, as of 2026-08-24: the largest remaining rows are **Units**
and **Promotions** (16 of the latter are spy promotions), with GreatPeople and
Beliefs both closing fast — the Great Person roster reached 147 of 213 in #2377
and **all 23 pantheons are modelled** as of #2381, taking Beliefs to 5.

Add ~30 civilizations that have not shipped, and the corporation-made luxuries
(`cosmetics`, `jeans`, `perfume`, `toys`) that `civvis validate` reports never
spawn.

### The largest single gap is the modifier interpreter

Nearly all Civ 6 *content* is not code. A leader ability, a wonder, a belief, a
policy card and a governor promotion are the same construction: a row in the
shipped `Modifiers` table naming an `EffectType`, a `CollectionType`, arguments
and a `RequirementSet`. CIVVIS hardcodes each one in Rust, which is why every
new civilization is an engineering task.

`tools/civ6_modifiers.py` prices it — **3,405 rows across 698 distinct
effects**:

| status | effects | rows |
|---|---:|---:|
| implemented | 25 | 825 |
| partial | 3 | 340 |
| unmodelled | **669** | **2,085** |
| out-of-scope | 1 | 155 |

The tail is long: 32 effects reach half the rows and the other half needs 666
more. See [MODIFIERS.md](MODIFIERS.md) for the ranked backlog.

### The difficulty ladder is transcribed, and "it's in the DLL" was never a reason not to look

⚠ **This section first said the larger half of the ladder "does not exist in
shipped data" and was "designed here, not transcribed". That was wrong, and two
PRs on 2026-08-23 disproved it independently.** It is kept as written-down
evidence of how easily an unchecked "it lives in the executable" propagates.

`data/difficulties.json` audits against the shipped database as **exact**, on
both halves. `MajorStartingUnits`, `BonusMinorStartingUnits` and the
`BarbarianAttackForces` bands match cell for cell at every rung — and so do the
handicaps that were supposed to be unverifiable, because they are ordinary
shipped `Modifiers` rows (#2375):

| shipped modifier | CIVVIS |
|---|---|
| `HIGH_DIFFICULTY_{SCIENCE,CULTURE,FAITH}_SCALING`, +8/rung off Prince | `ai_yield_pct` 8/16/24/32 ✅ |
| `HIGH_DIFFICULTY_{PRODUCTION,GOLD}_SCALING`, +20/rung | `ai_yield_pct` 20/40/60/80 ✅ |
| `HIGH_DIFFICULTY_COMBAT_SCALING`, base −1 then +1/rung | `ai_combat_strength` 0/1/2/3 ✅ |
| `HIGH_DIFFICULTY_UNIT_XP_SCALING`, +10/rung | `ai_xp_pct` 10/20/30/40 ✅ |
| `HIGH_DIFFICULTY_FREE_{TECH,CIVIC}_BOOSTS`, +1/rung, **two** modifiers | `ai_era_boosts` 1/2/3/4, as both a Eureka and an Inspiration set ✅ |

And where a number genuinely is not in the database, the binary can still be
**read**: #2372 disassembled `Rules::Units::Instance::GetUpgradeCost` in
`GameCore_XP2_FinalRelease.dll` and named every `GlobalParameters` field by
offset, settling a formula that had stood on community documentation for the
project's whole life — and finding two real divergences in the process
(a missing `PURCHASE_DIVISOR` round-down, and a minimum applied before the
formation multiplier instead of after).

> ⭐ **The rule this replaces the old caveat with:** "it lives in the compiled
> DLL" is a statement about *effort*, not about *knowability*. Before writing a
> number down as designed-here, check the shipped modifier rows, then the
> binary. Both were cheaper than the caveat.

What is genuinely DLL-side and not transcribed: the shipped AI's own
decision-making, and the `AiLists`/`TreeData` plumbing that gates it by
difficulty. Five shipped Rise & Fall Loyalty difficulty rows are known and
simply not carried yet (`FREE_CITY_INFLUENCE`, the two `DARK_AGE_CITIES_LOST_*`
rows, and the two `*_DIFFICULTY_HUMAN_MARTIAL_LAW` rows) — wiring, not
mechanism.

> ⚠ A Deity result here is still a statement about **CIVVIS' Deity** until the
> real game is played at that rung — but now because CIVVIS' *opponent* is not
> Firaxis' AI, not because the handicap numbers are invented. Only
> [CIV6_LADDER.md](CIV6_LADDER.md) settles the other question.

### Dynamics are barely measured

[GROUNDING.md](GROUNDING.md) is the half that measures behaviour rather than
constants, and it has run once. The combat damage roll was fit over 194
combats: the form is right and the driver is right, the mean sits ~2% low, and
3.6% of rolls fall below the modelled band. The escalating differential stack
in [FIDELITY.md](FIDELITY.md) phase 4 — derived-value differ, action-replay
differ, distribution tests, fuzzing, CI freeze — is mostly unbuilt;
`tools/civ6_differential.py` is the first reusable piece of stage 2.

The one indicative pacing comparison at turn 31 puts expansion, tech pace and
army size in the same place, and **score** low: CIVVIS' leader scores 27 where
the real game's leads 42.

### The victory meta is a different game

This is the gap that corrupts measurements rather than merely being one.

On the left, how a native 250-turn Online game ends. On the right, the rival
victories that ended a *live* game **before** the turn cap, counted from the
host's own Hall of Fame database rather than from our ledger:

| ending | native 250-turn screen | live: rival wins before the cap |
|---|---:|---:|
| religious | 28–48% | 8 (earliest t74, mean t143) |
| culture | 11–18% | **27** (earliest t145) |
| **diplomatic** | **0–1%** | **32** (earliest t201) |
| technology / science | 1–2% | 4 |
| score (at the clock) | 38–52% | *not in this column — the cap is where our own wins live* |
| domination | **0%** | 0 |

Diplomatic plus culture are **83% of every early loss on the live seat**.
Natively they barely happen, and for diplomacy [FIDELITY.md](FIDELITY.md)
explains why structurally: Gathering Storm pays Diplomatic Victory Points to
the first-place finisher of scored competitions, recurring through the whole
second half of a game, and **a native CIVVIS game has none of them**. 41 of 209
terminal live games end in a rival's diplomatic victory (19.6%); the contested
native screen produces 2 in 120 (1.7%). *"That twelvefold gap is structural,
not tactical."*

Domination is 0% at every map and every clock — **one** game out of 1,513
headless ones, on a board that did not exist when the first zero was measured,
and none live at all. That reproduces in the pure Rust engine with no host
involved, so whatever stops the army winning is in CIVVIS' own decision-making,
not in the live bridge, the actuation layer, or the host's refusals.

---

## 2. Why a simulation does not look like Civ 6

**Presentation.** The viewer is a 2D `<canvas>`; Civ 6 is a 3D terrain renderer
with leader scenes. Where art has been *measured* off the install rather than
imitated, it now matches — the tile-yield cluster plates and the yield icons
are cut from `TEXTURE_YieldOverlayAtlas` itself, and PR #2275 gave a *played*
seat Civ 6's own HUD arrangement. Where it has not, it does not: the unit flag
textures are packed inside `InWorld.blp`, whose table of contents was never
cracked, and a *spectated* game still wears the laboratory — a rankings table
across the map, arena stats down the side.

**The worlds are not Civ 6's worlds.** The exhibition deals `land_only`,
`lakes`, `inland_sea`, `pangaea`, `continents`, `small_continents`, `islands`,
`water_world`, `true_start_earth` and `tenins_ball`, on `flat` **or `planet`**
shapes, at 4–10 players (`tools/spectator_supervisor.py`). A good share of what
a watcher sees is a globe or a tennis ball, which Civ 6 has never drawn.

**And the behaviour reads wrong even before the art does.** Apostle swarms and
religious wins around turn 174–181; no domination ever; world eras that jump
because `era_from_progress` takes the `max` over majors of a player's best
single tech or civic (that one is documented intent, see `docs/MECHANICS.md` —
not a bug).

⚠ **This section used to name a fourth item — "0–3 wonders a game, four of six
stock civilizations structurally never build one" — and it was false in both
regimes.** #2376 ran the census nobody had run:

| | seats/runs | finish ≥1 wonder | wonders each |
|---|---:|---:|---:|
| native, deployment genome | 237 seats | **91.6%** | **6.54** |
| live Civ 6 ladder | 64 runs ≥ t200 | **91%** | **7.05** |

The `-10_000` sentinel is real, but the gate beside the Egypt/China clause is
`plan.strategy == Culture`, which `assess` awards on progress — so **any** empire
enters the lane. The identity clause is doing less than nothing: Egypt and China
average **4.88** wonders a seat against **6.60** for the other forty-eight. And
the live figure is the exact complement of what was previously written down: 91%
finish *at least one*, not 91% finishing none. **There is no wonder gap in either
regime, and no headless-versus-live gap either.**

---

## 3. Why strength does not translate

Ranked by weight. This is the part that matters most and is the least obvious.

### 3.1 Every improvement this repo ever promoted was profile-specific

| comparison | 4p 24×16 Standard (the old gate) | 6p 74×46 Online (deployment) |
|---|---|---|
| gen-14 champion genome | **+58, gate PASS** | **−9**, CI excludes +58 |
| `strategic` | **+117, gate PASS** | **−47** |
| `strategic_cheap` | +16 | **−63**, sign p=0.0018 against |
| `advanced` v `advanced_v1` — **never promoted** | +114 | **+207, gate PASS** |

That last row is what makes the rest interpretable: a profile that resolves a
large effect at gate strength is not an insensitive instrument. It is
specifically the promoted edges that vanish there. The champion turned out to be
a **small-map domination genome** — at 96 tiles per player its combat genes cash
out; at 567 tiles per player nobody is reachable, the game is a science/culture
race, and its tall faith-and-wonder genes are a liability.

⚠ **Read every figure in that table as an upper bound.**
[EVAL_INTEGRITY.md](EVAL_INTEGRITY.md) §4 shows why: each is the point estimate
of the run that promoted it, and conditioning on "passed the gate" conditions on
the estimate being large, so `E[observed | PASS] > true effect`. Re-measured on
disjoint seeds at the same profile, the **direction and the significance
replicate and the size does not** — `+207` came back `+86`, `+114` came back
`+98`, `+58` came back `+61`, and `strategic_deep`'s promoted `+45` came back
**−8** (CI −27..+12, 220 maps, PR #482), which *excludes* the promoted effect
rather than merely failing to reproduce it.

There are at least three regimes, and the eval config picks one:

```
                                        cities   score
ai_eval --players 6 --turns 250 24x16    1.74     ~114
LIVE ladder (n=91 runs >=200 turns)      3.0       330
headless soak (4 majors)                 5-7 per major
```

Neither default matched the deployment. **Check the `cities` column against the
live median before trusting any screen result.**

### 3.2 The instrument cannot see what actually kills the live seat

The gene screen draws every seat's genome from the same controller, so the
field never pursues a diplomatic or culture victory. Those are 83% of live
losses. This has already produced a wrong conclusion: `congress_counter_leader`
was left off because a census found *"no diplomatic victory in 40 games —
there is no headroom there to take"*, and that census ran headless, **in a
regime where diplomatic victories do not happen at all.**

### 3.3 Every live gap found so far was ACTUATION, not decision

The AI decides correctly and the bridge drops it. The pattern is remarkably
consistent:

| what was decided | what happened |
|---|---|
| a Settler, 83 consecutive turns | `PARAM_INSERT_MODE` missing — the engine built **nothing**, `applied: true` throughout |
| peace, 93 turns of one run | no `peace` kind existed in the translator — a losing war could never be exited |
| policies, ~6 orders a turn | the engine discarded every add because `clearList` was empty (#805) |
| a spy, 550 of 5,618 production orders | 84% refused as unplayable; `Game::spies` empty all game |
| an upgrade | 83% refused |
| theological combat | still has no live verb at all |
| a World Congress vote of more than 1 | **139 multi-vote asks, 0 ever registered** (#2334) |

`applied: true` counts the request, not the effect. `ended: true` counts a
`pcall` that did not throw. A null field is the signature of the bug, not
evidence of absence. And for most of the record the mod's own Lua ladder — not
CIVVIS — built 61–86% of production.

### 3.4 The tuned agent is omniscient; the live seat is not

`AdvancedAi::fog_honest()` consumes observation, memory and belief end to end.
The strong decision-maker being tuned is partly one that knows things a real
player cannot — and the cost of taking that away is now measured rather than
guessed at. The first screen read **15.0%** on 20 paired maps with a 95% CI of
5.2–36.0, which could never have settled anything. Re-screened in #2386:

> **`fog-honest` wins 6.6% against stock's 30.5% — −23.9 pp, z −7.83.** The
> number did not move; it stopped being compatible with parity.

**But the mechanism is not what everyone assumed, and that is the useful part.**
A matched-state decision diff (24 games, 334 decision points, world *and*
controller cloned so both arms start byte-identical) shows the fog-honest
planner's economic *judgement* is nearly intact: production within 5% of the
omniscient controller **at the same state**, improvement and combat within 1%.
164 of 280 first divergences are `move`.

What it loses is *execution*, and nothing had ever counted it — the replay
boundary discarded every `apply` result. Over 2,371 fog-honest turns, 97.0% of
planned actions land, but a refusal hits **37.8% of turns**, concentrated where
fog has no business being:

| refused | share |
|---|---:|
| `trade` | **67.3%** |
| `produce` | **24.9%** |
| `move` | 2.8% |
| `attack` | 0.4% |

Production and trade legality is a fact about the seat's **own empire**, fully
visible to it under fog. That concentration is a defect, not a consequence of
limited vision, and it is the named next job.

⚠ The obvious repair was tried and is worse: `fog-honest-2` (drop the stale
remainder of a refused tape and re-plan) loses to version 1 by 5.7 pp on wins.
Dropping the tape at the first refusal treats independent per-actor plans as one
dependent chain.

### 3.5 Tuning has never worked here

[GENOME.md](GENOME.md) closes with the scoreboard: *every* measured attempt to
make this agent stronger by **tuning parameters** has returned null — the
policy appetites three ways, the opening book two ways, the war-declaration
threshold, and about a thousand rounds of whole-genome evolution. Its scoreboard
closes every optimization game — policy cards, build order, tech order, civic
order, city expansion, war timing.

A working prior follows, and it is worth stating in the weaker form the evidence
actually supports:

> **Ask whether the agent does the thing badly, or does not do it at all.**
> Re-pricing the first has been the perpetual null. Repairs to *reachability* —
> an intent the agent already forms that never arrives at the queue, the unit or
> the host — are where the exceptions have come from.

⚠ **That is a post-hoc partition and a prior for choosing the next experiment,
not a law, and the obvious citation for it does not survive contact with the
record.** `d_holy` 2.0→5.6 is widely quoted here as the actuation repair that
paid. It measured +20 Elo at 4p 24×16 (52.9%, 95% CI 50.1%–55.7%, 1,200 maps,
seed 4400000, PR #1469) — and **parity at the shape the exhibition runs** (+2,
CI −46..+50, 400 maps, seed 5900000). It shipped and was reverted the same day
in **PR #1491**. So it is not a counterexample to §3.1; it is another instance
of it. Anyone citing an actuation win should name the run and the profile, or
say it is a discovery estimate.

### 3.6 The live instrument itself was starved

A live game takes ~44 minutes and `main` moved every ~35 minutes, so for most
of the record **no revision ever accumulated more than one game** — at a
per-game score stdev of 178, that distinguishes nothing. Fixed by #1853's
pinned batches; then two live seats forked the ladder ledger (#2332).

---

## 4. Where the ladder actually stands

Every attempt is self-recorded in `~/civvis-civ6-runs/civvis_ladder.jsonl`.
Completed games (turn ≥ 200), all at **Settler**, the easiest difficulty in the
game:

| week of | completed | led at the end | wins | median lead | median cities | **our score ÷ best rival's** |
|---|---:|---:|---:|---:|---:|---:|
| 2026-07-27 | 42 | 0 | 0 | −930 | 2 | **0.19** |
| 2026-08-03 | 64 | 0 | 0 | −711 | 5 | 0.39 |
| 2026-08-10 | 70 | 3 | 2 | −717 | 5 | 0.38 |
| 2026-08-17 | 59 | **18** | **7** | **−218** | **8** | **0.81** |

Rung 1 (Settler) was claimed 2026-08-16, rung 2 (Chieftain) 2026-08-18.

### ⚠⚠ The "we score a quarter of the leader" figure is four weeks out of date

That last column is the number this project has quoted about itself more than
any other, and **it has moved further than anything else on this page.** The
figures in wide circulation — *0.26 median over 99 runs*, *"we score a quarter"*,
*"we finish last in 49 of 55"* — were measured on the **2–5 August** corpus. On
the most recent full week the median is **0.81** (mean 0.80, n=59), independently
reproduced from `public_stats.wonder_count`-era run records at **0.79 mean /
0.81 median** over 64 runs (#2376).

The decompositions built on the old ratio are stale in the same way and should
not be steered from: *"we hold 75–80% of the leader's empire size while scoring
26%"* described a seat that no longer exists, and the components it blamed have
since been measured as fine (wonders, above) or closed. `docs/EVAL.md`'s
pre-Aug-16 science decomposition carries the same warning already.

**What survives:** the median completed game still finishes *behind* (lead −218),
so a ~20% deficit at the cap is the current shape of the problem — not a 74% one.
Nothing above Chieftain has been measured, and every figure here is at **Settler**,
the easiest difficulty in the game.

Recompute rather than quoting, the way that column was:

```sh
python3 - <<'PY'
import json, statistics, collections, datetime
rows = [json.loads(l) for l in open('/Users/martin/civvis-civ6-runs/civvis_ladder.jsonl') if l.strip()]
by = collections.defaultdict(list)
for r in rows:
    if (r.get('last_turn') or 0) < 200: continue
    if not (isinstance(r.get('rival_best'), (int, float)) and r['rival_best'] > 0): continue
    d = datetime.date.fromisoformat((r.get('finished_utc') or r['utc'])[:10])
    by[str(d - datetime.timedelta(days=d.weekday()))].append(r['last_score'] / r['rival_best'])
for w in sorted(by):
    print(w, len(by[w]), round(statistics.median(by[w]), 2))
PY
```

---

## 5. What follows from all of this

Four rules this project keeps re-learning, kept here so they are re-read rather
than rediscovered:

1. **Measure where it ships.** A result at a profile the deployment does not
   use predicts nothing about the deployment — not the magnitude, and not the
   sign. *(Now in force for the gene ledger without a switch being thrown: 99 of
   101 priced genes are decided by a `standard`-shape screen — #2385.)*
2. **A headless null is not a live null.** Use the headless screen to reject
   what is harmful or inert in *both* regimes. It cannot price a fix aimed at a
   threat that regime does not contain.
3. **Fix what never happens before re-pricing what happens badly.** Every
   measured exception has gone the same way — most recently the fog-honest
   planner, which chooses well and cannot land its orders.
4. ⚠ **Re-measure before you repeat a number from this page.** On 2026-08-23,
   **six** premises taken from documents and memory turned out to describe a
   build that no longer existed — the wonder lane, the score ratio, the
   difficulty handicaps, the upgrade formula, the religious verbs, the globe
   determinism fix. Every premise checked against *code* held; every one taken
   from *prose* was stale. This document is prose.

> The failure mode is not carelessness, it is velocity: `main` takes hundreds of
> merges a day, and a census moves every time content lands. Prefer the command
> to the table, cite the run and the profile beside any effect size, and treat a
> figure with no date as a hypothesis.
