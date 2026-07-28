# Countering the victory leader

If somebody is about to win, everybody else should try to stop them. The AI
already believes this: `victory_denial` names the rival closest to a victory
and hands back a counter-strategy, and `urgent_victory_threat` lets a terminal
clock waive the ordinary war-readiness checks so a response does not have to
wait for a comfortable force ratio.

Nothing had ever measured whether any of that works. `leader_census` does, and
the answer is that the layer detects the winner almost perfectly, stops them
never, and at deployment scale is paid for in development.

## The instrument

`leader_census` runs full games and reads, every turn and for every living
major:

- **the honest meter** — `Game::victory_threat`, the arithmetic the victory
  screen shows;
- **the meter the AI gates on** — `AdvancedAi::rival_pressure`, a second
  implementation of the same question;
- **who names whom** — `denial_target` across every living major, so a firing
  is counted where the decision is made;
- **who moves** — how many majors are at war with each empire at once.

Three diagnostic seams on `AdvancedAi` (`rival_pressure`, `denial_target`,
`denial_is_urgent`) expose the decision rather than re-deriving its formula. A
second implementation of "who is about to win" is how a HUD and an AI end up
disagreeing, which is the thing #291 landed `victory_races` to prevent.

```
cargo run --profile ci --bin leader_census -- \
  --players 6 --maps 16 --width 74 --height 46 --city-states 9 \
  --turns 400 --seed 940000 --jobs 6
```

Diagnostic only: nothing reads it, and no agent can name it.

## Measure where it runs

**The first four readings of this investigation were taken off the deployment
path and several of them reversed.** They ran at 24×16 with no city-states.
The exhibition seats 6 players on 74×46 with 9 (`server.rs` size profile:
2→44×26/3, 4→60×38/6, 6→74×46/9, 8→84×54/12, 10→96×60/15, 12→106×66/18) —
**567 tiles per empire against 64**.

On the cramped map `desired_cities` asks for 4–6 and empires hold 1.69, so land
binds and 88–97% of games end in a religious checkmate. At deployment scale
most games run to the turn limit and are decided on score.

| reading | 24×16 | 74×46 |
|---|---|---|
| victory mix | religious 88–97% | score 11, religious 4, culture 1 (of 16) |
| first denial → win | median 16 turns | median 101 turns |
| `victory_threat` ≥78 fires | every winner | 9 of 16 winners |
| meter lag (AI − honest) | 0 | −17 turns (the AI is *earlier*) |
| winner never faced >1 belligerent | 69% | 31% |
| follow-through | 43% | 54% |

Anything below that is quoted at the deployment profile.

## The alarm is a winner detector

It never misses and it never helps.

| | 4p 24×16 | 6p 74×46 |
|---|---|---|
| base rate of winning | 25.0% | 16.7% |
| **won given the layer ever named them** | **85.6%** | **66.7%** |
| won given named *and* then fought | 78.7% | 66.7% |
| wins nobody ever named | 0% | 0% |

That conditional is confounded, and the confound is the finding: reaching high
victory pressure is *caused* by being about to win, so a meter thresholded on
it cannot separate "about to win" from "won". At deployment scale the 101-turn
warning is not evidence either — with wins decided on score at the turn limit,
the expansion term fires at `turn*4 >= max_turns*3`, which is turn 300 of 400
exactly. The alarm is early because of a clock, not because it saw anything.

## Score is the instrument that predicts

At `end − K`, does each candidate signal already put the eventual winner on
top? 16 maps, 6p 74×46, base rate 16.7%. "Settles" is the median turns before
the end from which the winner holds that lead and never gives it back.

| signal | K=25 | K=50 | K=100 | K=150 | K=200 | settles |
|---|---|---|---|---|---|---|
| **score** | **94%** | **75%** | **75%** | **69%** | **62%** | **135** |
| AI meter | 94% | 81% | 62% | 12% | 31% | 70 |
| `victory_threat` | 38% | 31% | 31% | 12% | 44% | 42 |
| cities | 50% | 50% | 50% | 50% | 38% | 181 |
| techs | 62% | 56% | 50% | 50% | 38% | 164 |
| military | 69% | 56% | 56% | 44% | 19% | 51 |
| tourists | 50% | 44% | 38% | 25% | 19% | 112 |
| religion race | 31% | 25% | 19% | 19% | 6% | 141 |

Score names the winner 62% of the time **200 turns out** and settles on them a
median 135 turns before the end. `victory_threat` is at or below the base rate
at four of five leads. The religion race is *anti*-predictive at every lead.

**#291 excluded score from `victory_threat` on purpose**, because folding it in
"would turn this back into 'who has the biggest empire'". That is right for
*detecting* a win — score is a standing measured when the clock runs out, not a
race to a threshold, so its meter fills for everybody as a game ages. It is
exactly backwards for *warning* about one: at deployment scale who has the
biggest empire is the only thing that predicts the winner at a lead long enough
to act on, and every race meter is noise until the last 50 turns.

The same reversal shows up in the two meters. `rival_victory_pressure` has a
score term and `victory_threat` does not, so at deployment scale the AI's
duplicate runs **17 turns ahead** of the victory screen. The duplication is a
real hazard; on this map the AI's copy is the better instrument.

## Naming is not declaring, and nobody piles on

- **54% follow-through**: 61 of 112 distinct (observer, target) pairs the layer
  named ever went to war with the empire they named. The declaration path gates
  on `plan.strategy == Conquest`, `major_wars == 0`, a target city within 18
  tiles, and `campaign_staged_for_war`.
- **The table agrees and acts alone**: 4 or 5 distinct rivals name the eventual
  winner in every one of 16 games, and 31% of winners never faced more than one
  belligerent at a time.

The Emergency machinery that could organise a coalition already exists
(`Emergency`, `emergency_objective`, and the declaration path already lets an
emergency target bypass `campaign_target_legal`) — but only a city capture
convenes one, and that trigger already gates on being the **score leader**,
the instrument this census found is the right one. Wiring a victory emergency
would be a small change.

**It is not justified.** Win rate by the most rivals ever at war with an empire
at once, at deployment scale:

| peak belligerents | seats | wins | win rate |
|---|---|---|---|
| 0 | 4 | 1 | 25.0% |
| 1 | 45 | 2 | **4.4%** |
| 2 | 28 | 3 | **10.7%** |
| 3+ | 19 | 10 | 52.6% |

One or two belligerents is far *worse* than the 16.7% base rate. War is costly
to the empire fighting it. The 3+ bucket is confounded by leading, as
everywhere else here, but a coalition that is going to work has to knock the
leader off sometimes, and 10 of 19 of them won.

## What the response is worth

Two pre-registered ablations, both against `advanced_blind_to_leaders` —
identical to `advanced` except `victory_denial` is silent and
`urgent_victory_threat` always returns false. It still reads the same pressure,
so it ablates the *response*, not the perception.

| | 6p 24×16, seed 930000 | 4p 60×38 / 6 CS, seed 950000 |
|---|---|---|
| paired score for `advanced` | 50.8% (Wilson 42.0–59.6) | 51.2% (Wilson 42.4–60.0) |
| Elo-equivalent | +6 | +9 |
| win direction | 3/116/1, p=0.6250 | 7/109/4, p=0.5488 |
| **terminal score** | 50.1%, 33–32, p=1.0000 | **49.8%, 44–65, p=0.0549** |
| resolution | wins on 4 maps, score on 65 | wins on 11 maps, score on 109 |

**Wins are a dead heat both times.** At deployment scale terminal score is not:
the arm that never reacts to a leader develops better on 65 map-directions to
44, and holds more gold (734 vs 623, +18%), cities, population, science and
production.

So the counter-leader response is not free. It is paid for in development and
buys nothing measurable in wins — an empire abandoning its own economy to chase
a leader it cannot catch. p=0.0549 is borderline and it is one seed at one seat
count, but the census points the same way from the other side.

## What is closed

- **"Respond harder to the leader."** Deleting the entire response is a dead
  heat on wins at both scales, and costs the responder economy at deployment
  scale.
- **"Organise the dogpile."** The machinery exists and the table already agrees
  on the target; concentrating force is not what is missing.
- **`victory_threat` as a warning instrument.** At or below base rate at four
  of five leads. Correct for the victory screen, useless for an alarm.

## What is open

Keep the alarm and change what it asks for. Four of the seven races already
answer themselves — culture with culture, religion with religion, diplomacy
with diplomacy. The two that answer with an army are Science and Expansion, and
those are exactly the two the evidence above argues against.
`advanced_counter_in_lane` answers Science with Science and Expansion with
Expansion, leaving the alarm, its timing and its target untouched, so it
isolates the response's shape where `advanced_blind_to_leaders` bounds its
existence.

Pre-registered in `civvis-counter-in-lane-preregistration.md`: wins null,
terminal score ahead by roughly the margin the blind arm showed. A terminal
score that comes back flat or against refutes the mechanism — it would mean the
cost is in the alarm's replanning rather than in the wars it starts, and no
rewrite of the counter can recover it.
