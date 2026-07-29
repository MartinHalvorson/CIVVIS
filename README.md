# CIVVIS

### Notes from Martin:

A good bit of vibecoding. Still a bit buggy, apologies for any slop. 

### The rest is AI generated and maintained:

A whole
AI-vs-AI game below, first settler to space victory (click for the full video):

[![Spectate mode: a whole AI-vs-AI game on a Planet world of canal-ringed islands — six civilizations settle a globe of hexagons, the camera turns the planet and drops onto the Grand Canals II shelves and channels, Babylon puts the first satellite up on turn 189 and lands on the Moon and Mars, and three expeditions race for another star until Babylon's arrives on turn 282 for the science victory](docs/exhibition.gif)](docs/exhibition.mp4)

*Six AI civilizations, Grand Canals II on a globe at Small size, Online speed
with the turn cap lifted so the space race could finish. Recorded live off the
spectator's own view — standings, victory tracker and each AI's live plan — with
the camera flown out to the Moon, Mars and the voyage as each launch happened.
The waiting is compressed; the camera work is close to real time.*

## How the AI actually plays

Every civilization in that clip is an agent. There are six of them in the
codebase, arranged as a ladder, and each rung was built to fix a specific
weakness in the one below it. What follows is what each one is, where it runs,
and — the part that matters — how well it actually works, because most of these
have been measured against each other and two of the fancier ones lose.

### The ladder

**`RandomAi`** — picks uniformly from `legal_actions`. It exists as the zero
point of the scale and nothing else. It wins about **5%** of games in a
four-player tournament where parity is 25%.

**`BasicAi`** — a deterministic lightweight heuristic agent: expand, build,
defend, take the obviously good action now. It runs every **city-state and
barbarian** in a normal game, so it is on screen constantly even when no major
civilization is using it. Around **19%** in the same tournament. It reads full
state and does not honour fog, which makes it a sparring partner rather than a
fair-play opponent.

**`AdvancedAi`** — the workhorse, and the agent playing every major civilization
in the video. This is where nearly all of the actual Civ-playing intelligence
lives. It is *stateful and hierarchical*: it carries persistent grand strategy, a
victory plan, campaigns, force groups, settlement plans, builder queues and a
threat model between turns, and everything else is subordinated to that plan.
Research, civics, policy cards, government, Secret Societies, diplomacy,
production, spending, religion, trade routes, envoys, Congress ballots and unit
orders are all steered by the same medium-term goal rather than decided
independently each turn.

It also reads the public victory race for every rival and reacts to it: an
imminent science or score win becomes a military-denial target, a culture lead
triggers defensive Culture and Tourism investment, a religious lead is met with
theological pressure or military denial, a diplomatic lead redirects Favor and
envoys. Economic plans persist for five turns to stop strategic thrashing,
interrupted early by a surprise war, a newly threatened city, or a rival about to
win.

**`AdvancedAi` is roughly three times as strong as `BasicAi`** and is the
benchmark everything else is measured against. `advanced_v1` is the pre-upgrade
version, frozen on purpose as a regression control.

**`NeuralAi`** — `BasicAi` play with one decision upgraded. War declarations are
decided AlphaZero-style-in-miniature: clone the game, roll each branch forward
with fast scripted agents, and let a trained value net judge the resulting
positions. It is the proof that lookahead works in this engine, and its scope is
deliberately one decision type.

**`StrategicAi`** — the one agent that genuinely searches. Every `review_every`
turns it simulates staying adaptive *and* committing to each enabled victory lane
for `horizon` rounds, with rivals played by `AdvancedAi`, and commits to a lane
only when the targeted policy beats its adaptive parent by a real margin.
Positions are judged by the trained value net when one exists on disk, and by
score share otherwise. Three priors can answer before the rollouts ever run — a
public victory threat, irreversible Prophet investment, and duel geometry —
because a short economic rollout cannot discover those in time.

**`PolicyAi`** — the learned net used as the policy rather than as an advisor.
Each turn it scores the legal action set by applying every candidate to a clone
and evaluating the resulting position, then commits the best improvement. That is
one-ply net-guided search over the real action space. Without a trained net on
disk it falls back to `AdvancedAi` rather than playing badly.

### How they are trained and rated

Three separate loops, which are easy to confuse:

- **`civvis evolve`** — a genetic algorithm over the **48-gene `Weights` vector**
  that parameterises `AdvancedAi`'s strategy and combat doctrine. Each genome
  plays the reigning champion on shared maps; a challenger is promoted only on a
  sequential probability ratio test *plus* no regression on fixed holdout maps.
  Note the split, which is deliberate and load-bearing: **breeding consumes score
  share and combat-achievement share, while champion promotion depends only on
  wins.**
- **`civvis league`** — a persistent Glicko-2 league over named strategies,
  including parameterised `AdvancedAi` variants. Each round is one rating period;
  every finished game decomposes into pairwise results by placement. Strong
  strategies breed, confidently weak ones retire. Glicko-2 rather than Elo
  because the roster churns, and a newborn strategy needs to converge quickly
  without being culled on a small sample.
- **`civvis tournament` / `ai_eval`** — fixed head-to-head evaluation. Elo per
  `(player, leader, civilization)` combination, with multiplayer games scored as
  pairwise results at `K/(n-1)` scaling.

### How good is it, really

Tournament, 30 games × 4 players × 250 turns, parity = 25%:

| agent | Elo | win rate |
|---|---|---|
| `advanced` | 1154.5 | **56%** |
| `basic` | 1022.4 | 19% |
| `random` | 823.1 | 5% |

Head-to-head against `advanced`, which is the honest test:

| matchup | result | verdict |
|---|---|---|
| `advanced` vs `basic` | 66% | a real, large gap |
| `strategic` vs `advanced` | 54–56% | above parity, inside the noise band at n=50 |
| `policy` vs `advanced` | 40–42% | **the learned policy is worse than the scripted one** |

That last row is the important one and it is not a typo. Using the value net as
the policy loses to hand-written heuristics, even after the net was retrained on
GPU and verified genuinely predictive — validation BCE **0.3685** against
**0.5623** for a constant predictor. The net knows things; the agent built on it
does not yet convert that into wins.

Two caveats that should travel with every number above:

- **Elo between our own bots is a closed system.** It says one bot is 130 points
  better than another bot. It says nothing about how either would do against a
  human or against Firaxis' AI.
- **Against a real handicap the gap shows.** A challenger taking 58.3% of seats
  at Prince takes **16.7% at Deity**, where the opposition gets +80% Production
  and Gold and +32% Science. Beating our own baseline is not the same as playing
  well.

### What is known to be broken

Documented rather than hidden, because it is the most useful part of the record:

- **A quarter of the genome is inert.** Driving each gene to both ends of its own
  bounds, **11 of 48 genes produce zero divergence** over 12 trials — the playing
  agent never consults them.
- **The learned components are inert where it counts.** The value net is
  well-calibrated and yet never changes a lane choice; `PolicyAi` scores unit
  actions using 25 *empire-level* features that an individual move cannot change,
  so its computed gain is exactly zero on 96% of the candidates it clones.
- **Search is under-provisioned, and it is the one thing measured to pay.**
  `StrategicAi` reaches its rollouts about 1.5 times per seat per game — half its
  reviews are answered by the cheap priors. Doubling its compute, either as twice
  the reviews or twice the horizon, wins significantly more maps at 120-map power
  (p=0.0023 and p=0.0025). Quadrupling does not help further.
- **Evaluation config is not deployment config.** Results measured at 4 players on
  a small map do not transfer to the 6-player exhibition on a large one: one
  change measured at +16 in the eval harness came back at −63 in the deployment
  configuration.

The short version: the agent that plays well is the scripted one, the only
intervention with a reproducible win is *more search*, and every learned
component so far is either wired to a decision it cannot change or evaluated in a
configuration that does not predict the real one. `docs/AI_GAPS.md` keeps the
ranked list of what is missing; `docs/EVAL.md` keeps the runs behind every number
on this page.
