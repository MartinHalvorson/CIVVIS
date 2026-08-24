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
game ships and CIVVIS does not model at all:

| table | only in Civ VI |
|---|---:|
| GreatPeople | 184 |
| Units | 58 |
| Promotions | 26 (16 of them spy promotions) |
| Beliefs | 22 |
| Projects | 17 |
| Policies | 11 |

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

### Half the difficulty ladder cannot be transcribed at all

[FIDELITY.md](FIDELITY.md) audits `data/difficulties.json` against the shipped
database. The transcribable half is **exact** — `MajorStartingUnits` bonus
units match cell for cell at every rung, and the barbarian raid switch matches
Civ 6's own threshold and directions.

The other half does not exist in shipped data. `ai_yield_pct`,
`ai_combat_strength`, `ai_xp_pct` and `ai_era_boosts` — the larger effect by
far — live in the compiled GameCore DLL. So do city-state quest selection and
the shipped AI itself. Those numbers are **designed here, not transcribed**.

> ⚠ Therefore "the agent wins 0/200 at Deity" is a statement about **CIVVIS'
> Deity**, not about Civ 6's. Only [CIV6_LADDER.md](CIV6_LADDER.md), which
> plays the actual game, can settle the other question.

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
religious wins around turn 174–181; no domination ever; 0–3 wonders in a whole
game because `AdvancedAi` scores `Item::Wonder` at the `-10_000` refusal
sentinel unless the civ runs a Culture strategy or a Score target, so four of
six stock civs structurally never build one out of a 53-wonder roster; world
eras that jump because `era_from_progress` takes the `max` over majors of a
player's best single tech or civic (that one is documented intent, see
`docs/MECHANICS.md` — not a bug).

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
Its first deployment-shaped screen scored **15.0%** (95% CI 5.2–36.0, 20 paired
maps). The strong decision-maker being tuned is partly one that knows things a
real player cannot. [AI_GAPS.md](AI_GAPS.md) names the successor itself:
improve fair-play economic planning *before* re-running that gate.

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

| week of | completed | led at the end | wins | median lead | median cities |
|---|---:|---:|---:|---:|---:|
| 2026-07-27 | 42 | 0 | 0 | −930 | 2 |
| 2026-08-03 | 64 | 0 | 0 | −711 | 5 |
| 2026-08-10 | 70 | 3 | 2 | −717 | 5 |
| 2026-08-17 | 59 | **18** | **7** | **−218** | **8** |

Rung 1 (Settler) was claimed 2026-08-16, rung 2 (Chieftain) 2026-08-18.

So it *is* beginning to translate, and the trajectory is the most encouraging
number on the project. It is also not yet a capability: the median completed
game still finishes behind, and nothing above Chieftain has been measured.

---

## 5. What follows from all of this

Three rules this project keeps re-learning, kept here so they are re-read
rather than rediscovered:

1. **Measure where it ships.** A result at a profile the deployment does not
   use predicts nothing about the deployment — not the magnitude, and not the
   sign.
2. **A headless null is not a live null.** Use the headless screen to reject
   what is harmful or inert in *both* regimes. It cannot price a fix aimed at a
   threat that regime does not contain.
3. **Fix what never happens before re-pricing what happens badly.** Every
   measured exception has gone the same way.
