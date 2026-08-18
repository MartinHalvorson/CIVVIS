# The screen can seat the rivals Firaxis actually plays

_2026-08-18 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

The round three files up — *we screen against a religion game and lose a
diplomacy game* — measured that the deployment evaluator profile produces
**zero diplomatic and zero culture victories**, while the live Civilization VI
ladder loses **41 games to a rival's diplomatic victory and 24 to culture**. It
named the consequence: `advanced_congress_counter`, `advanced_congress_votes`
and `advanced_congress_counter_hard` sit unscreened in the registry because the
board can never produce the condition they answer, so each would read as an
inert null that says nothing. And it named the fix it would not make on one
sitting's evidence: *"a profile whose rivals pursue the lanes Firaxis' pursues —
the `live_target_diplomatic` and `live_target_culture` arms already exist and
could be seated as opponents."*

Two questions, then. **Does seating them actually produce those victories?**
And is the resulting arm reading different from the inert one?

## How it was measured

`ai_eval` seats the two entrants and nobody else: every one of `--players`
chairs is `a` or `b`. `--field` names the agents that fill the chairs the
entrants do not take. Fieldless is the default and byte-identical to before.

Three runs, all at the deployment shape — 6 players, 74x46, 9 city-states,
Online, 250 turns, all six victories enabled, `pangaea`/`flat`/fixed
civilizations:

1. `advanced` vs `live_target_diplomatic`, 6 pairs, seed 31000000 — the cheapest
   possible test of whether a diplomacy-seeking agent on this board wins that
   way at all. (Half the chairs, so this is not a field; it is the existence
   check the field idea rests on.)
2. `advanced_congress_votes` vs `advanced`, **20 pairs / 40 games**, seed
   32000000, **fieldless** — the screen as it ships.
3. The same arm, the same 20 pairs, the same seed 32000000, with
   `--field live_target_diplomatic,live_target_culture`.

Runs 2 and 3 share their seed stream, so they are the same forty maps.

## What it measured

**A diplomacy-seeking agent does win that way here.** Run 1, 12 games:
`religious 5, diplomatic 3, score 2, culture 1, science 1`. Against a profile
that had produced diplomatic 0 of 12 and culture 0 of 12, seating one lane agent
produces four of the five enabled conditions.

**And the field turns an unmeasurable arm into a measured one.** Same arm, same
forty maps:

| | victory conditions over 40 games | `advanced_congress_votes` | maps that broke |
|---|---|---|---:|
| fieldless (as shipped) | religious 24, score 16, **diplomatic 0, culture 0** | **50.0%** (CI 26.3%..73.7%), Elo **+0** (CI −179..+179) | **0** |
| `--field live_target_diplomatic,live_target_culture` | religious 23, **culture 11, diplomatic 6** | 48.8% (CI 24.3%..72.0%), Elo −9 (CI −198..+164) | 1 on wins, 1 on terminal score |

The fieldless run's own report is the finding, and #2003's guard wrote it
without being asked:

> ⚠ nothing differed: all 20 maps were neutral on wins AND on terminal score, so
> `advanced_congress_votes` and `advanced` played the same games. The verdict
> above is not evidence about the treatment — it did not fire on this profile.

Exactly 50.0% and exactly +0 Elo, from forty games in which the treatment never
once changed a decision, on a board where not one diplomatic or culture victory
occurred. That is the inert reading the previous round predicted, produced on
demand. With the field the same arm fires and two maps break.

⚠ **Nothing is promoted here.** The field reading is `INCONCLUSIVE` at 20 maps
and its interval spans −198..+164; a treatment whose direction rests on one
broken map out of twenty is not a result. What is established is narrower and
more useful: the arm is now *measurable*, and it was not before.

⚠ Twelve and forty games are small samples for claims about a distribution.
Zero-of-forty is strong for "this profile does not produce these lanes" and
6-and-11-of-40 is strong for "this one does". Neither is a claim about how large
the treatment effect is.

## What was decided

**Shipped as an opt-in profile axis, and nothing else changed.**

- `--field` is empty by default, and the fieldless path was checked
  byte-for-byte against `origin/main` on a 4-pair run: identical output apart
  from one added line naming the field as `none`. Every number in `docs/EVAL.md`
  remains reproducible.
- `--matrix` refuses `--field` alongside the other eleven profile flags. Who
  else is on the board decides which victories are reachable at all, which makes
  it the strongest profile axis there is, and the promotion matrix's two
  recorded profiles are fieldless.
- `game_score` and `terminal_score_share` are now indexed by seat rather than by
  agent name. Naming the winner was correct only while the two entrants
  partitioned the board; with a field, the name test scored **a field seat's
  victory as a win for the incumbent** — so a denial treatment that failed to
  stop a diplomatic victory would be penalised and one that stopped it would
  gain nothing, which is the single arrangement that makes the arm look worse
  exactly when it works. A test pins all three outcomes, including that the
  fieldless board cannot produce the third.
- A field run prints its seating, and states that its numbers are not comparable
  to fieldless ones.

**What this does not settle.** Whether the field *should* become the screen for
denial treatments, and which agents belong in it, is an experimental-design
question with consequences for every future recorded number. This round makes
the axis expressible and demonstrates that it changes what the board can
produce. Choosing the standard field is the next round's decision, and it should
be made on more than one arm.
