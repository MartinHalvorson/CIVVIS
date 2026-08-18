# The promotion matrix can see the lanes the front line loses to

_2026-08-18 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

#2038 made `--field` expressible and demonstrated it changes what the board can
produce. It deliberately left the next question open: *"whether the field should
become the screen for denial treatments, and which agents belong in it, is an
experimental-design question… it should be made on more than one arm."*

So: **on more than one arm, and with the composition itself measured.** Should
the promotion matrix carry a contested profile, seated with what?

## How it was measured

All runs at the deployment shape — 6 players, 74x46, 9 city-states, Online, 250
turns, all six victories, `pangaea`/`flat`/fixed civilizations. `--field` seats
the four chairs the two entrants do not take.

1. Each of the three unscreened congress arms against `advanced`, **60 pairs /
   120 games**, seed 33000000, field `live_target_diplomatic,live_target_culture`.
2. Field composition, `advanced` vs `advanced_v1`, **30 pairs / 60 games**, seed
   34000000, field `live_target_diplomatic × 3, live_target_culture` — three
   diplomacy seats instead of two, to test whether the diplomatic yield is a
   seat-count problem.

## What it measured

**The contested profile reliably produces culture. It does not produce
diplomatic.**

| field | games | religious | culture | diplomatic | score |
|---|---:|---:|---:|---:|---:|
| none (the shipped screen) | 40 | 24 | **0** | **0** | 16 |
| `diplomatic, culture` ×2 | 120 | 90 | **24** (20%) | **2** (1.7%) | 4 |
| `diplomatic` ×3 + `culture` | 60 | 52 | 5 (8%) | **3** (5%) | 0 |

⚠ **Adding diplomacy seats does not add diplomatic victories.** Three of them
produced 3 of 60; two produced 2 of 120. Both are rare and the difference is
inside what 60–120 games can resolve. The diplomatic yield is not a seat-count
problem, so the obvious lever does not exist: **CIVVIS's own diplomacy lane
rarely completes at all**, which is what `docs/AI_GAPS.md` and the negative
victory-lane gates already say. Culture is the half this profile fixes; the
diplomatic half of the hole — 41 of the 74 stolen live games — stays open, and
naming that is more useful than the profile pretending otherwise. Two
diplomacy seats also yielded **more** culture (20% against 8%), so the
even split is what shipped.

**★★★★★ AND THE DECOMPOSITION IS NOT A DECOMPOSITION.** All three arms, 60
pairs each, same seed stream, on the board built for them:

| arm | flags | paired-map score | Elo (CI) | wins broke | score broke |
|---|---|---:|---:|---:|---:|
| `advanced_congress_counter` | target | 50.0% (44.4..55.6) | +0 (−39..+39) | 0 | 3 |
| `advanced_congress_votes` | votes | 50.0% (44.4..55.6) | +0 (−39..+39) | **0** | **0** |
| `advanced_congress_counter_hard` | both | 49.6% (43.9..55.2) | −3 (−43..+36) | **1** | **7** |

`advanced_congress_votes` earned the harness's own non-measurement warning
(#2003) at 60 maps:

> ⚠ nothing differed: all 60 maps were neutral on wins AND on terminal score, so
> `advanced_congress_votes` and `advanced` played the same games. The verdict
> above is not evidence about the treatment — it did not fire on this profile.

Not "null on the contested board" — **it never once changed a decision in 120
games.** A tighter interval around exactly 50.0% is a better-measured
non-measurement, and reading it as evidence about vote-buying would be the
mistake this repository keeps paying for.

**Both flags together break 7 maps where the parts break 3 and 0.** That is
superadditive, and a treatment cannot be superadditive with a flag that does
nothing — so the second flag is not independent of the first, it is
*conditional* on it. The mechanism is in the code: `take_turn` buys the extra
votes only when the ballot's target equals the rival `victory_denial` names,
while with `congress_counter_leader` off the ballot is aimed at the *diplomatic
leader* instead. Those are different empires most of the time, because
`victory_denial` picks whoever is closest to any victory and this board is
decided by religion three quarters of the time.

The two flags were split so a combined arm could not hide which half did the
work — the right instinct, and this repository has retracted four mechanism
stories told off combined arms. **Splitting them made one half unable to fire at
all**, which is the same failure from the other side: `advanced_congress_votes`
has never measured anything, and neither has any conclusion drawn from it. That
includes the standing "there is no headroom there to take" on the `world_leader`
veto, which was taken on a fieldless board **and** through a flag that could not
fire.

⚠ Repairing that is a change to `src/ai/advanced.rs`, not to this binary, and it
belongs in its own PR with its own claim on the most contended file in the
repository. Recorded here as the next round.

## What was decided

**Shipped: `deployment-contested`, a third promotion profile.** The deployment
shape exactly — same players, map, city-states, turns, speed and victory set — so
the only difference from `deployment-online` is who else is in the game.

**`NoRegression`, deliberately, and that is the whole of the policy choice.** A
third `Strength` bar would make every future treatment clear a profile it was
never designed for, and there is no evidence that is a good trade. A tripwire
refuses a *measured regression* on the contested board and lets an inconclusive
reading pass, so it adds what was missing without adding a hurdle. Raising it is
a separate decision needing its own evidence.

⚠ Its numbers are **not** comparable to `deployment-online`'s. Two entrants hold
two chairs there instead of six, so a game is one contest rather than three and a
fixed pair count carries less information. The two profiles are different
questions, never a replication of each other.

Cost: a matrix run gains a third child of roughly the deployment profile's
weight. `matrix_job_budgets` is generalized from a hard-coded pair to a weighted
split over N profiles, and its test now states invariants — every worker used,
none starved, a heavier profile never given less than a lighter one — rather than
a table of exact splits. The table is what broke when the third profile arrived,
and a table is the part nobody reads before adding one.

**What this does not settle.** Whether vote-buying pays is still unmeasured,
because the arm that exists to measure it cannot fire alone. The next round is
that repair — one definition of who the counter points at, shared by the ballot
and the weight — and *then* the measurement. Not a longer run: the harness has
now said twice that a longer run of a treatment which never fires buys nothing,
and the 60-pair reading above is exactly what buying it looks like.
