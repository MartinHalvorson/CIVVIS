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

## Changing what the alarm asks for is also null

Four of the seven races already answer themselves — culture with culture,
religion with religion, diplomacy with diplomacy. The two that answer with an
army are Science and Expansion, exactly the two the evidence above argues
against. `advanced_counter_in_lane` answers Science with Science and Expansion
with Expansion, leaving the alarm, its timing and its target untouched.

It fires as designed. With the whole table on it (`--arm in_lane`) the alarm
still rings on 13.4% of player-turns against the shipped 14.0%, while
follow-through drops 48% → 40% and the leader's post-alarm war exposure drops
43% → 32% of turns. Same detection, different answer, fewer wars.

**And it is null.**

| reading | seed 960000, 120 pairs | seed 970000, 360 pairs |
|---|---|---|
| paired score for `advanced` | 46.2% | **49.9%** (Wilson 44.7–55.0) |
| Elo-equivalent | −26 | **−1** |
| win direction | 2 / 107 / 11, p=0.0225 | 15 / 329 / 16, **p=1.0000** |
| terminal score | 23 / 58 / 39, p=0.0559 | 106 / 151 / 103, **p=0.8900** |
| resolution | wins on 13 maps | wins on 31, score on 209 |

The first run was pre-registered to be refuted by "regression to 49–51%" and
landed on 49.9%. Pooled the direction is 27 to 17, sign p=0.1742 — and pooling
a discovery seed with its own confirmation is not a legitimate test. **PR #411
also read 46.2% first and also evaporated**; wins resting on 13 of 120 maps
cannot carry a sign test whatever its p-value says.

## The gold difference is the reserve schedule, not an unspent surplus

**This section first claimed that an empire which declines a war banks the
savings and never spends them. That was wrong, and the correction is kept here
next to it.** The claim was read off aggregate columns in two eval summaries
without checking what moves gold in this engine, which is the exact mistake
that produced four retracted attributions in `docs/SUPERHUMAN.md`.

`advanced_gold_spending` keys its treasury reserve off the plan:

| plan | reserve | at 5 cities |
|---|---|---|
| Conquest, Recovery | 75 + 25/city | 200 |
| Religion | 150 + 50/city | 400 |
| Science | 250 + 50/city | 500 |
| Expansion | 250 + 75/city | 625 |
| Culture, Diplomacy | 300 + 75/city | 675 |

A war empire spends down to 200; a builder empire holds 625. So **any**
treatment that moves plan mix away from Conquest raises the gold held, with
nothing saved and nothing wasted. Measured directly, 16 maps at the 4-player
deployment profile:

| arm | conquest | expansion | mean gold per major-turn |
|---|---|---|---|
| ship | 25% | 31% | 322 |
| in_lane | **19%** | **37%** | **361** |

The treatment moves 6 points of player-turns from Conquest to Expansion —
exactly what answering a Science or Expansion threat in-lane instead of with an
army should do — and 6% of the 425-gold reserve gap is ~26 gold against the 39
observed. Same mechanism, same order of magnitude.

**So there is no unspent-savings finding, and gold utilisation is not the lead
this page previously said it was.** What is left is the reserve schedule
itself, which nothing here has calibrated.

The raw columns that prompted the wrong reading, kept because they are real:

| run | arm | gold | cities | pop | tech | districts | builds |
|---|---|---|---|---|---|---|---|
| ablation | `advanced` | 623.9 | 5.09 | 59.4 | 41.6 | 18.3 | 57.1 |
| ablation | blind | **734.2** | 5.16 | 60.1 | 41.7 | 18.5 | 58.0 |
| confirm | `advanced` | 633.9 | 5.22 | 60.9 | 41.9 | 18.8 | 59.3 |
| confirm | in-lane | **750.6** | 5.20 | 60.5 | 42.1 | 18.6 | 59.3 |

Every arm that declines a war ends ~18% richer and identical on every other
column — same cities, population, techs, districts and completed builds. Both
halves of that are explained above: the gold by the reserve schedule, and the
flat development by the treatment simply not being worth anything.

`advanced_counter_stand_down` exists and is tested — it would decompose "stop
declaring" from "race them" — but it decomposes an effect that did not survive
its confirmation, so it has not been run.

## The instrument change is null too

The shipped score term is a clock, not an observation: it fires only once
`turn*4 >= max_turns*3`, so every leader trips it at turn 300 of 400 alike,
however far ahead they are. `early_score_alarm` reads the margin instead — 78
at 20% ahead of the next empire, 100 at 50% ahead — from
`standard_duration(60)`.

It works as an instrument, and on its own it is a war engine:

| arm | denial fires | first denial → win | follow-through | conquest | expansion | gold |
|---|---|---|---|---|---|---|
| ship | 13.9% | median 78 turns | 43% | 25% | 31% | 322 |
| early | 37.5% | median **284** | 63% | **35%** | 19% | 313 |
| early_build | 35.8% | median 143 | 33% | **14%** | **52%** | 470 |

Warning time nearly quadruples. Feeding the shipped Conquest counter, the
earlier alarm takes plan mix from 25% to 35% conquest and follow-through from
43% to 63% — the direction that costs seats. Paired with `counter_in_lane` it
reverses to 14% conquest and 52% expansion: **21 points of plan mix**, the
largest behavioural change of any arm here.

**And the outcome does not move.** 120 pairs at the deployment profile, seed
990000: `advanced` 51.7% (Wilson 42.8–60.4), win direction 21/82/17 sign
p=0.6271, terminal score 49.4% with 54/0/66 and p=0.3153 on all 120 maps
resolving. The registered prediction was 48–54%, most likely null; it landed on
48.3%.

## Where this leaves the question

Every measured way of stopping a leader in this engine is null or negative:

- deleting the whole response is a dead heat on wins at both map scales;
- a coalition does not help — one or two belligerents wins 4.4% and 10.7% of
  its seats against a 16.7% base;
- changing what the alarm asks for is null across 480 maps and two seeds;
- and the alarm cannot be made earlier from `victory_threat`, which is at or
  below the base rate at four of five leads.

| treatment | what it changed | result |
|---|---|---|
| `advanced_blind_to_leaders` | deletes the response entirely | dead heat, two map scales |
| a coalition (not built) | — | 1–2 belligerents win 4.4% / 10.7% of seats vs a 16.7% base |
| `advanced_counter_in_lane` | what the alarm asks for | **49.9%** over 360 confirming maps |
| `advanced_early_score_build` | when it fires *and* what it asks for | **48.3%**, sign p=0.6271 |
| `advanced_early_score_alarm` | when it fires | 54.6% at 120 maps → **51.4% at 360**, p=0.3821 |
| `advanced_evolved_blind` | the ablation on shipped weights | running |

**Nothing available at this layer counters a leader in this engine.** The layer
detects the winner almost perfectly, it changes behaviour substantially in
every direction it has been pushed, and it changes no outcome in any of them.

### The one arm that looked different, and did not survive

`advanced_early_score_alarm` — the earlier alarm feeding the *shipped* Conquest
counter, registered as "below 50% because it adds war" — scored **54.6%** at
120 pairs (Elo-equivalent +32, win direction 13/83/24, sign p=0.0989). The
registered prediction was refuted and the reading was the opposite of what the
dogpile table implied.

It did not survive. 360 pairs at disjoint seed 992000: **51.4%** for the
treatment (`advanced` 48.6%, Wilson 43.5–53.8, Elo −10), win direction 48/254/58
sign **p=0.3821**, terminal score 50.6% *favouring* `advanced` at 191 to 166 on
357 resolving maps. The registered call for the confirmation was "regression to
49–52%, sign p > 0.05".

**That is the second 120-pair discovery run in this investigation to read ~54%
and regress to null on a disjoint 360.** The other was `advanced_counter_in_lane`
at 53.8%, p=0.0225 — a *stronger* discovery p-value than this one — which came
back 49.9%. Pooled across both its seeds this arm is 82 directions to 61,
p=0.0941, and pooling a discovery seed with its own confirmation is not a test.

The lesson is cheap to state and was expensive to learn twice: **at 120 pairs on
this eval, wins rest on ~30 informative maps, and a sign test on 30 maps
produces a p < 0.10 often enough that it cannot be treated as evidence.** Every
positive reading in this branch has come from a discovery run of that size, and
neither survived.

## The finding that is not about AI strength

Everything above measures whether the *AI* can act on a leader, and the answer
is no. But one reading here is not about the AI at all, and CIVVIS is a
spectator product, so it is worth separating out.

`obs.rs::victory_progress_json` publishes `Game::victory_races` — the five race
meters — and that is what the viewer's victory screen shows. This investigation
measured those meters as predictors of the eventual winner at the deployment
profile:

| what the screen shows | 100 turns out | 150 | 200 |
|---|---|---|---|
| `victory_threat` (max of the five races) | 31% | 12% | 44% |
| religion race | 19% | 19% | 6% |
| **score margin (not shown as a race)** | **75%** | **69%** | **62%** |

Base rate 16.7%. **A viewer watching the victory screen 150 turns from the end
is looking at meters that are at or below chance, while the thing that names
the winner two-thirds of the time is not presented as a race at all.** Score
appears in the standings; the *margin over the field*, which is what carries
the signal, does not.

This is an observation about presentation, and it is deliberately not backed by
a strength claim — no eval can tell you what a spectator should be shown. It
does have one piece of evidence behind it that the AI-side conclusions do not:
it does not depend on any response working, so none of the six nulls above bear
on it.

`leader_census --arm ship` reproduces the table.

### What this does not license

It does not license deleting the layer. CIVVIS is a spectator product, and this
machinery is what makes an AI *visibly* move against a runaway leader — it
reaches the reasoning log, the war log, and the plan the HUD shows. Measured as
play it is worth zero; measured as behaviour a viewer can read it is doing
something none of the numbers here price, and this investigation never tried
to.

And it does not yet license the opposite either. An earlier commit of this page
said "do not spend more effort improving it — five treatments, two map scales,
four seeds, every one null", written before the sixth treatment came back at
54.6%. That sentence is suspended, not withdrawn: five of six still say the
layer cannot be improved, and the sixth is one 120-pair discovery run of
exactly the size that has already produced a false positive here.

**The recommendation waits on seed 992000.** If it regresses, the sentence
stands as written. If it holds, then the thing that counters a leader in this
engine is an *earlier* alarm answered by a *war* — the combination this
investigation spent most of its length arguing against — and the page will need
rewriting from the dogpile table down.

## The counter that is not paid for in development

Everything above is a *war-shaped* counter, and the mechanism the nulls settled
on is a cost: "nothing is recovered by not fighting because the alternative use
of the resources is not being made either", with the bill showing up as
terminal score (44 map-directions to 65, sign p=0.055) and 18% more gold held.

The World Congress is the one counter in this engine that has no such bill.
`Game::resolve_congress` refunds a vote on a losing outcome **in full**, and a
vote on the winning outcome but the wrong target at **half**. Diplomatic Favor
has no sink but votes and deals. An empire can therefore oppose a leader, be
wrong, and pay nothing — which is not true of a single arm on this page.

`congress_census` measures it. Diagnostic only: it never changes a decision,
and no agent can name it.

### Where the twenty points come from

A diplomatic victory needs 20 Diplomatic Victory Points, and three things award
them: the stock **+1 for an exact prediction** (any voter backing the winning
outcome *and* target, on any resolution), **`world_leader`** (+2 to its target
on outcome A, −2 on B), and wonders/techs carrying
`diplomatic_victory_points`.

| source | 4p 60×38, 6 CS, 24 maps | 6p 74×46, 9 CS, 16 maps |
|---|---|---|
| **exact prediction** | **1088 — 99.5%** | **1261 — 99.9%** |
| `world_leader` A | 4 — 0.4% | 0 — 0.0% |
| wonders/techs | 1 — 0.1% | 1 — 0.1% |
| `world_leader` B | −160 | −136 |

The lane is not won by the diplomacy resolution; it is won by voting with the
majority, over and over. `congress_choice` ends `base + observed(choice) *
35.0` — a bandwagon term worth 35 per vote already cast — so ballots converge,
the whole table predicts exactly, and the whole table collects +1. Mean final
DVP per major is 10.4 and 12.4; median *peak* is 14 and 16 of the 20 needed.

⚠ **Special Sessions award nothing.** They resolve down
`resolve_emergency_session`, which convenes a coalition and refunds the losing
side but never pays a Diplomatic Victory Point. Crediting their voters the
stock +1 is how this census first read a residual of −124 in 1262; excluding
them took it to **1 point in 1262**. 47 of 178 sessions at 6p are Emergencies.

### The diplomatic veto is already total — do not build there

| reading | 4p | 6p |
|---|---|---|
| `world_leader` resolutions | 82 | 68 |
| leader denied −2 | **95.1%** | **98.5%** |
| leader gained +2 | 1.2% | 0.0% |
| votes cast on the leader | 3 A / 310 B | **0 A / 393 B** |
| rival ballots opposing / abstaining | 231 / 0 | 326 / 0 |
| **diplomatic victories** | **0 of 24** | **0 of 16** |

Opposition is unanimous, free, and never loses. **"Counter the diplomatic
victory harder" has no headroom available to take.**

Note the leader supplies some of those B votes itself — 393 against 326 rival
ballots over 67 resolutions is the leader opposing itself every time. That is
**not a bug**. B carries the vote regardless, so B:self collects the +1
prediction and nets −1, where A:self predicts wrongly and nets −2.

### The congress is aimed at an empire that cannot win

Every leader-targeting term in `congress_choice` — `world_leader` B,
`trade_policy` B, `public_relations` A — resolves its target as
`diplomatic_leader`, the empire holding the most Diplomatic Victory Points.
Read over congress sessions of decided games:

| instrument | 4p (214 sessions, base 25.0%) | 6p (181 sessions, base 16.7%) |
|---|---|---|
| **dvp leader is the eventual winner** | **24.8%** | **14.4%** |
| score leader is the eventual winner | 61.2% | 60.8% |
| pressure leader is the eventual winner | 54.2% | 43.6% |
| dvp and score leader agree | 28.0% | 19.9% |

**At the base rate on one profile and below chance on the other.** This is the
same shape as the instrument table further up this page — score predicts, the
victory meters do not — but it costs more here, because this is the one lever
that is free to pull.

Three resolutions carry a *targeted* penalty, and all three are mis-aimed:

- **`trade_policy` B** — a total trade embargo on its target. Aimed at the DVP
  leader, and usually beaten by A-on-ourselves (260) anyway.
- **`migration_treaty` B** — −20% growth and loyalty pressure. Scores **0.0
  against every rival**, so the penalty can never be aimed at anybody at all.
- **`border_control_treaty` B** — no tile annexation from border growth. Aimed
  by raw territory: the only one of the three already using something other
  than Diplomatic Victory Points, and still not the empire about to win.

And nobody pays for a vote: rivals held enough Favor for a *third* vote on 289
of 326 ballots at the exhibition profile and bought one **zero** times, because
`take_turn` weights a ballot by the voter's own plan and never by the stakes.

### What was built

`advanced_congress_counter` (`congress_counter_leader`, default off) points
those three resolutions at the empire `victory_denial` already names.
`world_leader` is deliberately left alone — its ±2 moves Diplomatic Victory
Points and nothing else, and the veto above is already at 98.5%.

`advanced_congress_votes` (`congress_counter_votes`) is the decomposition arm:
shipped aim, but a ballot opposing the named empire is backed with the second
and third vote. `advanced_congress_counter_hard` sets both.

Fires-check first — 12 maps, 4p 60×38, 6 city-states, seed 983000,
`congress_census --arm`:

| arm | ballots naming own denial target | ballots with a bought vote | targeted penalties passed | landed on the eventual winner |
|---|---|---|---|---|
| `ship` | 2.5% (19/773) | 0.6% (5) | 7 — 0.58/game | 4/7 — 57.1% |
| **`counter`** | **7.3%** (58/794) | 0.8% (6) | **17 — 1.42/game** | **12/17 — 70.6%** |
| `votes` | 2.5% (19/773) | **1.9%** (15) | 7 — 0.58/game | 4/7 — 57.1% |
| `hard` | 7.7% (60/776) | **7.2%** (56) | 17 — 1.42/game | 12/17 — 70.6% |

Base rate 25.0%.

**`counter` fires**: it nearly triples both the aimed ballots and the penalties
that actually pass, and lands them on the eventual winner 70.6% of the time.

**★ The vote-weight lever is inert, and this retired it without an eval.**
`votes` triples the bought votes against `ship` and then reproduces the aimed
ballots, the penalty count and the landing rate *exactly* — 19/773, 7, 4/7.
`hard` against `counter` is the same: nine times the bought votes, an identical
17 and 12/17. **Extra votes flip no resolution in either aim**, because the
bandwagon term makes ballots converge and winning margins are wide, so one
voter's extra two votes cannot carry an outcome. `advanced_congress_votes` and
`advanced_congress_counter_hard` are recorded here rather than evaluated.

Pre-registration for the surviving arm:
`/Users/martin/civvis-congress-counter-preregistration.md`, written before the
run, predicting **wins null at 48–52%**. Both of its fires-check predictions
held, the second more strongly than it was written.
