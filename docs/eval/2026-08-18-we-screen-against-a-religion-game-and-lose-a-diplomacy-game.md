# We screen against a religion game and lose a diplomacy game

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

#2006 made the live ladder report what actually ends its games, and the answer
was not the clock: **74 of 310 configured attempts are lost to a rival
completing a victory condition before turn 250 — diplomatic 41, culture 24**,
religious 5, technology 3, conquest 1.

Every treatment this project screens is measured on the deployment evaluator
profile. So: **does that profile ever produce the victories that are killing
us?**

## How it was measured

`audit`, 12 games at the deployment shape — 6 players, 74x46, 9 city-states,
Online, 250 turns, seeds 31000000–11 — reading the victory type each game
ended on. Then the same question through `ai_eval` on a second profile variant
(pangaea, fixed civilizations, 12 games, seed 31500000), to check the answer is
not one map script's habit.

## What it measured

**The two distributions are very nearly disjoint.**

| victory | native evaluator (12 games) | live ladder losses (310 attempts) |
|---|---:|---:|
| religious | **9** | 5 |
| science / technology | **3** | 3 |
| culture | 0 | **24** |
| diplomatic | **0** | **41** |
| domination / conquest | 0 | 1 |

The second profile variant agrees where it matters: religious 6, score 5,
culture 1, and **diplomatic 0, domination 0** again.

So the profile every arm is screened on is decided by religion three quarters
of the time, and the front line loses to diplomacy and culture more than half
the time. **A treatment aimed at denying a diplomatic victory cannot be
measured here, because no diplomatic victory happens.** It will play out as an
inert arm, be filed as a null, and nothing in the report will say the question
was the wrong one to ask of this profile.

That is not hypothetical: `advanced_congress_counter`,
`advanced_congress_votes` and `advanced_congress_counter_hard` are all sitting
unscreened in the registry, and all three are aimed at exactly the condition
this profile never produces.

The reason is not mysterious. The evaluator seats `AdvancedAi` in every chair,
and `AdvancedAi` routes to religion. The live bridge sits one seat against
Firaxis' AI, which routes to diplomacy and culture. **We screen against
ourselves and deploy against somebody who plays differently.**

## What was decided

**Shipped as reporting, and only reporting.** Every run now says which victory
conditions its games actually ended on, and names the enabled ones that never
occurred:

```
victory conditions exercised, over all 12 games: religious 6, score 5, culture 1
⚠ enabled but never produced here: science, diplomatic, domination. A treatment
  aimed at one of those cannot be measured on this profile — an inert reading
  would say nothing about it
```

The fix this points at is a profile whose rivals pursue the lanes Firaxis'
pursues — the `live_target_diplomatic` and `live_target_culture` arms already
exist and could be seated as opponents. That is an experimental-design change
with real consequences for every recorded number, and it is not made here on
one sitting's evidence.

⚠ Twelve games is a small sample for a claim about a distribution. Zero of
twelve is strong for "rare", not proof of "never", and the second variant is
twelve more. What is not in doubt is the direction: the profile is dominated by
a condition the front line barely loses to, and barely produces the two it
loses to most.
