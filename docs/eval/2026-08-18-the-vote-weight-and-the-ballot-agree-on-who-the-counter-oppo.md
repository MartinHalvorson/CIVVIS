# The vote weight and the ballot agree on who the counter opposes

_2026-08-18 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

The round before this one screened all three unscreened congress arms on the new
contested profile and found something it had predicted in writing beforehand:

| arm | flags | wins broke | terminal score broke |
|---|---|---:|---:|
| `advanced_congress_counter` | target | 0 | 3 |
| `advanced_congress_votes` | votes | **0** | **0** |
| `advanced_congress_counter_hard` | both | 1 | **7** |

Both flags together break 7 maps where the parts break 3 and 0. **A treatment
cannot be superadditive with a flag that does nothing**, so the vote-weight flag
was not independent of the target flag — it was conditional on it. The predicted
mechanism: `congress_choice` aims the penalty at `denied.or(diplomatic_leader)`
while `take_turn` asked `victory_denial` directly, and those are the same empire
only when `congress_counter_leader` is on.

So: **is that the mechanism, and does the arm fire once it is repaired?**

## How it was measured

One change: three concepts get one definition each —
`congress_diplomatic_leader`, `congress_denial_target`,
`congress_counter_target` — and both the ballot and its weight ask the last one.
The weight predicate is extracted from the middle of `take_turn` into
`congress_ballot_opposes_the_counter_target`, because being inline is why nothing
could test it.

Then the identical run: `advanced_congress_votes` against `advanced`, **60 pairs
/ 120 games**, seed 33000000, `--field live_target_diplomatic,live_target_culture`
at the deployment shape. Same arm, same maps, same field as the reading above —
only the repair differs.

## What it measured

**The arm fires.**

| | wins broke | terminal score broke |
|---|---:|---:|
| before (`victory_denial` asked directly) | 0 of 60 | **0 of 60** |
| after (one shared counter target) | 0 of 60 | **3 of 60** |

Before, the harness printed its own non-measurement warning: *"all 60 maps were
neutral on wins AND on terminal score… it did not fire on this profile."* After,
it does not, because the treatment changed the game on three maps. The predicted
mechanism was the mechanism.

**And on this board it does not pay.** `advanced_congress_votes`: **50.0%**
(95% betting CI 44.4%..55.6%), Elo-equivalent **+0** (CI −39..+39),
`INCONCLUSIVE` after 60 maps. Victory conditions over the 120 games: religious
90, culture 25, score 5.

⚠ **That is now a null and not a nothing, and the difference is the whole
round.** The same number, from the same maps, meant nothing a day ago. It is
still weak evidence: a treatment that fires on 3 of 60 maps cannot move a paired
score much whatever it does on those three, so the interval above is close to
what 60 maps of *no* treatment would give. What it rules out is a large effect,
not a small one.

⚠ The diplomatic yield was 0 of 120 on this run against 2 of 120 on the same
seeds before the repair. Both are consistent with the ~2% rate the contested
profile was measured at; neither is evidence about the treatment, and reading a
two-game difference either way would be exactly the mistake this file exists to
prevent.

## What was decided

**Shipped as a defect repair, and it changes no shipped behaviour.** Both flags
are off in `AdvancedAi::new()`, so production `advanced` and the live seat are
untouched — checked rather than asserted: six paired 6p 74x46 150-turn games
against a binary built from `origin/main` produced byte-identical game reports.

The repair also carries the `target != pid` guard every counter branch of the
scoring table already had. Without it, an empire holding the most Diplomatic
Victory Points is its own counter target, and on a non-diplomatic lane
`world_leader` outcome B on itself outscores outcome A on itself — so the
treatment would have bought votes to strip its own points.

**What this retracts.** `congress_counter_votes`'s doc said it backs "a ballot
cast against the empire closest to a victory". It backed a ballot aimed at an
empire the ballot was never aimed at, which is to say it backed nothing. Every
reading of `advanced_congress_votes` before today measured a treatment that could
not fire, and that includes the standing conclusion that the `world_leader` veto
has "no headroom there to take" — taken on a fieldless board *and* through a flag
that could not fire.

**What this does not settle.** Whether backing the veto pays. The reading above
says only that it is not a large win on a board that produces a diplomatic
victory about twice in a hundred games. The live ladder loses 41 games to that
condition, and closing the gap between those two facts — a screen that produces
diplomatic victories at a rate resembling Firaxis' AI — is still the open
problem, and #2042 recorded that adding diplomacy seats does not solve it.
