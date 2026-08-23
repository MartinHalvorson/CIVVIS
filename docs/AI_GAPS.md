# AI status: what works, what fails, and what is still missing

This is the current assessment of the game-playing AI. `docs/EVAL.md` is the
chronological experiment log; older entries there preserve what was believed at
the time and are not automatically the current conclusion.

The same rule now applies inside this file. **The list immediately below is the
only live ranking here.** Three sections used to compete for that role; all
three are still present, under `Superseded ranking` headings, because what was
believed and why is evidence — but none of them is an instruction any more.

## Current ranked next work (2026-08-17)

Ordered by the largest remaining decision risk rather than by how local the next
code edit is. This is the 2026-07-31 turn-frame ordering further down, minus the
entries the dated sections after it record as answered, plus the one entry the
later rankings dropped without ever resolving. Nothing here is re-ranked on
judgement: every surviving entry keeps the relative position its own list gave
it, and the note on each says what has since moved under it.

1. **Test one rational composite, pre-registered.** Evaluate the live policy
   deck plus direct envoy production on new stable deployment and compact
   prefixes, preserving the matrix gate and recording build opportunity costs.
   Mark it as a deployment comparison; do not attribute its outcome to either
   component or sweep parameters on the confirmation seeds. *(Turn-frame #2.
   The 2026-07-31 full-prefix resolution below closed the single-component
   question in both directions — neither the 120-map direct envoy-production
   treatment nor the 300-map live-policy control cleared the unchanged matrix
   rule, though both pointed +30 Elo-equivalent at deployment — which is what
   leaves one bounded composite worth running. No dated section since records
   the composite as run.)*
2. **Validate a fresh action-conditioned candidate.** Use the trainer and
   fixed-threshold evaluator on disjoint action corpora; do not tune coverage on
   the deployment profile, and do not integrate a candidate before the mirrored
   gameplay gate. A state-value argmax is not an action-value policy.
   *(Turn-frame #4. The infrastructure half landed 2026-08-17: the corpus →
   trainer → schema-validated artifact → abstaining loader path is reproducible
   and refuses to emit on held-out BCE, coverage, or gated-lift failure. What
   remains is exactly the evidence — a fresh external screen and the mirrored
   A/B. No policy file is embedded and no default AI loads one.)*
3. **Search the full expansion investment, then price strategic search.** Model
   production, population, travel, settlement, and payback together; retain
   expensive macro search only if a genome-matched deployment comparison pays
   for its measured turn cost. *(Turn-frame #5. `AdvancedAi::coupled_expansion`
   now prices one legal Settler as a bounded investment against a 90-turn payoff
   horizon and is registered as a typed evaluator arm, off in production
   (2026-08-17). The modelling is done; the disjoint gameplay screen that would
   promote or reject it has not run. The strategic-search half is untouched.)*
4. **External calibration.** Complete retained games against Firaxis' AI and
   humans with named settings. Internal Elo remains an internal ruler.
   *(Carried from the 2026-07-30 ranking, the last list to contain it: the two
   rankings after it dropped this entry without recording an answer, and
   “Internal ratings are not external strength” is still a live failure below.
   The two Settler-rung claims out of 119 attempts are the first external
   datapoints and the file already says what they establish — that the
   integration path can finish a game in front, not that the controller is
   strong.)*

Answered, and therefore **not** carried forward:

- **Separate discovery from confirmation effect sizes** (turn-frame #1) —
  landed. `ai_eval` now compares full inclusive `[seed, seed + pairs - 1]`
  intervals, rejects any overlap, fails closed on `u64` overflow, and applies
  the check in matrix mode before the fixed compact/deployment stride. The word
  *disjoint* is now mechanical rather than a convention (2026-08-17, below).
- **Screen the fog-honest major controller** (turn-frame #3) — screened and
  decided in #1940. The opt-in `AdvancedAi::fog_honest()` arm consumes
  observation, memory and belief end-to-end and is exposed as `fog_honest`; its
  first deployment-shaped screen scored 15.0% (95% Wilson CI 5.2%..36.0%, 20
  paired maps, seed prefix `920000..920019`), the matched compact screen was
  exactly neutral, and stock `advanced` retained the strength gate. The arm
  shipped; the incumbent did not change. The successor the screen itself names
  is to improve fair-play economic planning *before* re-running the gate —
  which is not yet a ranked item because no ranking in this file has priced it
  (2026-08-17, below).
- **Policy-deck transfer confirmation** (2026-07-30 #4) — resolved against the
  treatment by the full-prefix follow-through below; it survives only as the
  first component of item 1.

Nothing on this list is licensed to skip the dated sections below. Several of
them record ★★★★★ defects with named, unrun remedies (the great-work veto's
district-vs-slot key; the five bridge flags the `live` arm is missing). Those
are findings with a route, not ranked work, and they are deliberately left where
their evidence is.

For implementation details see `docs/AI_GUIDE.md`; for the run-by-run evidence
and its corrections see `docs/EVAL.md`; for the rating/seating contract see
`docs/closed/LEAGUE.md`. Closed lanes keep their writeups in `docs/closed/`, including
the two that answer the recurring “spend the terminal Faith” question with a
measured null: `docs/closed/FAITH_CONVERSION.md` (the conversion-policy
development screen, stopped with no gameplay integration) and
`docs/closed/TERMINAL_FAITH_OPPORTUNITIES.md` (the frozen descriptive contract
over 376 terminal major seats holding a mean 4,486.5 Faith). A closed writeup
records what was measured, never what the code does now.

## First: what “AI” means in CIVVIS

CIVVIS has no runtime LLM, prompt pipeline, or hosted-model dependency. The
agents that play the game are local Rust controllers. The browser's AI plan and
reasoning views display deterministic `PlanReport` and `Journal` records emitted
by those controllers; they are not generated explanations.

There are seven concrete `Ai` implementations, with different purposes:

| implementation | purpose | live by default? |
|---|---|---|
| `RandomAi` | legal-action baseline | no |
| `BasicAi` | cheap deterministic heuristic | city-states and barbarians |
| `AdvancedAi` | stateful hierarchical scripted play | yes, directly or through league variants |
| `NeuralAi` | value-net-guided war-rollout experiment | no; resolves to `BasicAi` without a net |
| `StrategicAi` | victory-lane rollout search over `AdvancedAi` | offline league anchor only |
| `PolicyAi` | one-ply value-net tactical experiment | no; resolves to `AdvancedAi` without a net |
| `ProductionSearchAi` | per-city build rollout experiment | evaluator-only negative result |

The generic `Oracle<A>` wrapper is not an entrant. It deliberately cheats by
granting a subsystem for free so an ablation can measure the maximum headroom in
that subsystem.

The supervised exhibition uses `--league auto`: it copies the committed roster
to a mutable runtime directory and makes a rank-weighted choice from the exact
table size's top three conservative outright winners. The lower 95% Wilson win
bound orders that pool; the exact leader/civilization placement rating breaks a
tie. Seating avoids repeats, exhausts profiled winners before an unprofiled
fallback, and retains the old placement-only policy until at least three exact
profiles exist. Those entries are scripted `AdvancedAi` variants and baselines.
The only searching roster entry, `strategic`, is marked `league_only` and is
excluded from exhibition and auto-play choices. Without a seated league, every
non-human major uses stock `AdvancedAi`; minors and barbarians use `BasicAi`.

The repository ships and embeds the champion in `data/evolved/best.json`. It
stores the 40 genes that existed when it was bred; the seven added by #1520
fill from `#[serde(default)]` at their identity, so the artifact keeps loading
unchanged. It ships no `valuenet.json`. That distinction matters:
the genome changes a scripted policy, while a value net would be a learned
model. `ai_eval` reports the artifact provenance and effective fallback for
every named entrant.

Two tools reach the real Civilization VI executable, but neither runs the Rust
agent unchanged. The grounding mod imports only the economic subset of a league
genome and leaves tactics to Firaxis' AI. The computer-control mod is a separate
Lua heuristic controller that issues its own orders. The first has two
explicitly anecdotal 60-turn strategy-transfer datapoints; the second has
**claimed the Settler rung twice on 2026-08-16** out of 119 attempts since
2026-08-10 (`docs/CIV6_LADDER.md`). That is the project's first external
result and the smallest one available — Settler is the rung where Firaxis'
AI takes no bonuses. It establishes that the integration path can finish a
game in front, not that the controller is strong.

## Where the AI is used well

### 1. Scripted hierarchy coordinates a very large rules surface

`AdvancedAi` is where most effective play lives. It keeps persistent grand
strategy, victory route, campaign, force-group, settlement, builder, city-role,
and threat state. Research, civics, governments, policies, diplomacy,
production, purchases, religion, trade, envoys, Congress, espionage, and unit
orders consume that shared state instead of behaving as unrelated greedy
systems.

Its tactical layer also performs real bounded search. Candidate attacks are
applied to cloned games and checked against the opponent's best forcing reply.
`StrategicAi` is therefore not “the only agent that searches”; it is the agent
that adds periodic macro rollouts.

The scripted agent has a strong recorded regression result: on the recorded
six-player 74×46, six-city-state Online benchmark, `advanced` measured +207
Elo-equivalent against the frozen `advanced_v1` control and passed the promotion
gate. Separate exact full-game tests complete all six victory conditions without
injected progress. The first result is evidence of relative strength on that
profile; the second is evidence of coverage, not human-level skill.

### 2. Cheap controllers are used where their cost is appropriate

`BasicAi` is deterministic, small, and fast. Using it for city-states and
barbarians avoids paying the persistent-planning cost of a major-civilization
agent for actors with a narrower job. `RandomAi` remains a clean zero-point for
legality, determinism, and tournament sanity checks.

### 3. The evaluation system finds failures before deployment

The engine is deterministic and cheaply cloneable. `ai_eval` uses paired maps
and swapped seats, reports win and terminal-state diagnostics separately, names
the exact player/map/speed profile, exposes macro-search review counts, and
prints artifact provenance. Promotion gates are based on wins rather than an
internal Elo label.

That instrumentation has caught several failures that an aggregate win rate
would hide:

- missing model files silently turning named learned agents into scripted ones;
- duel priors preventing `StrategicAi` from reaching its rollouts;
- a value function that is predictive but never changes the lane argmax;
- action features that are visible but reward the wrong causal direction;
- small-map gains that disappear or reverse on a larger Online game;
- score-share improvements that do not produce more victories.

The deterministic `Journal` is useful here too: it reports the actual rule,
candidate, and reason used by the controller, without pretending to be a
post-hoc natural-language explanation.

### 4. Search, evolution, and learned representations are useful laboratories

On the four-player 24×16 Standard benchmark, deeper/frequent macro search has
won controlled comparisons: `strategic_deep` beat `strategic` by about +45
Elo-equivalent in its pre-registered promotion run. The embedded evolved genome
also beat stock weights by +58 on that source profile. Those are real,
profile-scoped results.

The learning data surfaces are substantial even though they are not deployed:

- `obs_tensor` supplies 25 fog-honest spatial planes plus public/global values;
- `selfplay` exports tensors and terminal labels;
- `decision_features` makes many more action consequences visible;
- `action_space` stably encodes all 77 action variants and destination context;
- counterfactual and Q-data tools measure sibling actions without changing the
  source trajectory.

These tools have answered research questions. They have not yet produced a
learned controller that wins.

## Where it is failing

### 1. Production agents cheat on fog

`BasicAi` and stock production `AdvancedAi` still read the full `Game`, including
information a seated player cannot observe. The HTTP observation and spatial
tensor honor fog, but the incumbent policy does not consume either. The new
opt-in `AdvancedAi::fog_honest()` closes that architecture gap for a complete
major turn: it plans production, diplomacy, campaign, and tactics on one
turn-start fog-redacted world, carries controller-owned belief memory, and
replays only the resulting actions on the authoritative game. It is a
correctness implementation, not yet a strength-promoted replacement; the
incumbent remains unchanged until a paired deployment screen resolves the
cost. `advanced_belief_pressure` remains the narrower evaluator arm for
isolating the memory term inside stock `AdvancedAi`.

### 2. No learned policy ships

`tools/train_spatial.py` writes a PyTorch checkpoint that no Rust agent loads.
The scalar value-net trainer can write `evolved/valuenet.json`, but that file is
generated, untracked, and absent from the shipped tree. In a normal checkout:

| requested name | effective behavior |
|---|---|
| `neural` | champion-weight `BasicAi` |
| `policy` | champion-weight `AdvancedAi` |
| `strategic` | macro search evaluated by score share (`strategic_score`) |

Calling these names “learned agents” without stating the artifact is therefore
incorrect.

The next-step boundary is now implemented without changing that conclusion.
`tools/action_policy_train.py` trains a low-capacity, replica-aware pairwise
ranker from fresh `q_counterfactual` games, splits by independent game, and
refuses to write a candidate unless held-out BCE, fixed 0.70 abstention
coverage, and gated return lift all clear their declared bars. The reusable
`tools/action_conditioned_eval.py` consumes that artifact and refuses to open
an untouched external profile after a failed selection gate. Rust's optional
`valuenet::ActionPolicy` loader validates the same schema and returns to the
scripted expert for missing, malformed, tied, or low-confidence candidates.
This closes the reproducible action-conditioned training and safety boundary;
it does **not** promote a model or change the live controller. A fresh external
profile and a mirrored gameplay A/B remain required evidence for that separate
decision.

### 3. Predicting outcomes has not produced a safe action objective

The 25-feature value net uses full-state empire aggregates. Historical models
beat a constant predictor on grouped held-out games, but that establishes
correlation with eventual wins, not the consequence of a candidate action.

The tactical policy made the distinction measurable:

- With 25 features, 96% of evaluated tactical candidates left the value
  unchanged, so the learned layer mostly declined to act.
- A 34-feature vector raised action visibility from 44.5% to 86.1%.
- The resulting `policy_wide` agent then won only 14.2% against `advanced`,
  about −313 Elo-equivalent.
- It maximized enemy contact because contact correlates with strong empires
  pressing an attack in the training games. Chosen moves increased contact far
  more than the average legal move while losing material.
- Freezing exactly the two contact terms restored 50.0%, causally confirming
  the mechanism. It did not create an improvement.

The first learned advantage/ranking heads also failed to transfer across
profiles or confidence gates. Destination context can imitate which move the
scripted expert chose, but imitation and value prediction are not evidence that
the move wins more games.

### 4. Production rollout search optimizes the wrong endpoint

`ProductionSearchAi` won 45.0% against the scripted governor (9 paired-map
directions for, 21 against, sign p=0.0428). Replacing score share with the scalar
value net changed one game. Raising the horizon ceiling from 40 to 200 left 54
of 56 choices unchanged.

The evaluator can distinguish the builds and sees them after their payoff has
landed; it still ranks them worse than the hand-written sequencing. The failure
is the unresolved-game proxy, not simply a short horizon.

### 5. Search and evolution overfit their measurement profile

Results that passed at four players on 24×16 Standard did not transfer to the
recorded six-player 74×46, six-city-state Online benchmark:

| treatment | small Standard profile | 6p 74×46, 6 city-states, Online |
|---|---:|---:|
| evolved genome vs stock | +58, PASS | −9, inconclusive |
| `strategic` vs `advanced` | +117, PASS | −47, wins inconclusive; terminal score worse |
| cheap search vs `advanced` | +16 | −63, significant against |
| `advanced` vs `advanced_v1` | +114 | +207, PASS |

The final row shows that the larger profile can resolve a real difference; it
is not merely compressing every comparison. The completed 300-map confirmation
for `strategic` at the larger profile does not exist, so neither “search wins in
deployment” nor “search is a proven regression” is supported.

The live exhibition now rotates through 4–10 seats, matching stock map sizes,
map scripts, and flat/planet shapes. That makes any single-profile “strongest
agent” label even less defensible.

### 6. The search surface is narrower than the policy and often bypassed

The active gene vector is 47 entries, 40 of them until #1520. The policy
appetites and the policy-deck and dedication selectors stored beside it in
`Weights` are not genes. Research order, city roles, strategic gates, and many
tactical decisions are also outside the vector.

**Production valuation was the largest thing outside it, and is now partly
inside.** `AdvancedAi::production_value` is 1,014 lines that rank every
candidate build, every city, every turn. Measured on the body of that function:
**291 numeric literals, 93 of them distinct, and zero reads of `Weights`.** Not
one of the 40 genes reached a build priority, so no run of `evolve` had ever
tuned one, the macro search had never perturbed one, and every change to that
ranking had to be a hand-picked constant defended in prose. The tree already
said so about a single one of the 93 — `settler_price` exists because *"920 is
a hardcoded literal: not a gene, so no run of `evolve` has tuned it"* — without
noticing the other ninety-two.

#1520 adds seven category multipliers (`p_military`, `p_builder`, `p_trader`,
`p_building`, `p_district`, `p_wonder`, `p_project`) applied to the matching
arm's score, after the refusal sentinel so a gene tilts what a city wants and
never argues with what it may not build. Each defaults to 1.0 and the multiply
lands only on a positive score; the same `ai_eval advanced advanced_v1` run
built before and after is byte-identical across 24 games, so the default
genome — which `BasicAi::new()` and `AdvancedAi::legacy()` both carry — ranks
builds exactly as it did.

⚠ **This widens what can be searched. It is not a strength result and must not
be cited as one.** Whether the surface contains anything worth finding is an
open question, and answering it needs an `evolve` run against the win-based
gate, with a control that separates the wider surface from simply more
breeding.

### ★★★ The settler's price is not a live control surface

An eighth gene was implemented and then cut on its own evidence, which is worth
recording because it independently confirms a conclusion this document reached
from the other direction:

```
gene_census --games 96 --start-seed 93500000
  p_settler   stock 1.000  probe 4.000   97% identical   nearly inert
  p_building  stock 1.000  probe 4.000   30% identical   live
```

**Scoring the settler at four times its shipped value leaves 97% of games
outcome-identical.** `docs/EVAL.md` 2026-07-28 found production preemption a
null and diagnosed why: the settler is not blocked by losing a ranking, it is
blocked *before* the ranking is consulted — "no city at pop 2" on 23.8% of
seat-turns, a growth constraint no price can buy past. The census says the same
thing from the valuation side, on a different mechanism and different seeds.

It follows that `settler_price` and the 920 literal it scales are an argument
about a knob that barely connects, which is the more useful form of the
complaint that started this: the problem with the 93 literals was never only
that they are untuned. Some of them cannot matter.

The other seven probe live at 17–67% identical under the same treatment, so
none of them ships silent — which is the standing complaint two paragraphs
below.

An older report said “11 of 48 genes” were silent, but the implementation's
vector had 40 entries when that was written — 47 since #1520 — and that
report's table names only ten. Treat the ratio as
a historical bookkeeping error, not a current genome fact. The supported
finding is narrower: causal probes have found multiple parameters that do not
change the sampled `AdvancedAi` games, often because the live hierarchical path
bypasses the `BasicAi` consumer. Silence on a finite sample is not proof of
global inertness.

Standalone evolution still has an objective problem. Score share is cheap and
correlated with winning, but controlled changes have improved score
significantly without changing wins. Direct win-rate selection is too noisy for
its cheap per-genome fitness estimate, so its separate win-based promotion gate
remains essential.

The continuous league no longer shares that mistake: parent choice, niche
elites, and retirement now use conservative Wilson bounds on outright wins at
the current round's exact table size, with placement Glicko only breaking a
tie. Retained raw matches backfill old rosters without assigning an invented
size to older aggregate history. On the production log, mixed-history evidence
would breed `advanced_evolved` then `advanced`; exact six-player evidence ranks
`g28-28` 12/33 (22.2% lower bound), `g676-58` 5/12 (19.3%), then `advanced`
7/27 (13.2%). That aligns the objective; it does not yet establish that the
next offspring generation improves.

### 7. Search is not live-validated and is materially more expensive

One `StrategicAi` among five scripted seats measured about 6.4× the all-scripted
game-turn cost on three early-game seeds; an all-searching fleet measured about
29×. That makes a single offline entrant feasible, not free. The shipped roster
keeps it as a `league_only` anchor so the league can compare against an axis
breeding cannot create without putting that unresolved cost/strength trade into
the exhibition.

The original structural probe overstated joint-search headroom because it
reconstructed the lane pass under the doctrine in force; the live lane pass
uses the unchanged base genome. Against that corrected comparator, a
deployment-profile run on 20 mirrored 6p 74×46 Online maps split **every map**:
20/40 wins per arm, +0 Elo-equivalent (95% −148 to +148). Joint search changed
only 10/268 eligible rollout reviews (3.7%) while evaluating 28 branches rather
than the sequential policy's 11. That interval does not prove parity, but it
does not justify buying the extra rollout compute. The treatment remains an
evaluator-only control and production stays sequential.

### 8. Internal ratings are not external strength

League Glicko and tournament Elo compare agents inside CIVVIS. Difficulty
handicaps test how the same internal controller responds to bonuses; they do not
calibrate it against a human or Firaxis' AI.

The first external rung now exists, and it is small. The separate Civilization
VI Lua controller **beat Settler on 2026-08-16** — `CIV6_LADDER.md` rung 1,
run `civvis-20260816T054344Z`, a victory event naming our own team at turn 251
of a configured 250-turn game — and won again the same day. That is a real
external datapoint and it is the weakest rung of eight, on the difficulty where
Firaxis' AI receives no bonuses at all. Two wins in the 119 attempts since
2026-08-10 is not yet a capability, and Settler says nothing about Prince. The
external ruler now reads one notch above zero; it is still not calibrated.

The fixed-profile, fixed-anchor tournament ledger now gives internal versions a
replayable longitudinal baseline: compare the order-independent direct Elo and
its pair-score interval against `advanced_v1` at the same game count and under
the profile-bound `advanced / advanced_v1 / basic / random` controller roles.
The canonical 40-game ledger records `advanced-20260730` at direct 1708.2
against the 1500 anchor from a 31/40 pair score (95% Elo 1588.7–1841.0); its
order-sensitive online path is separately labelled 1588.5. The rules
fingerprint, four-player 60×38 Standard profile, source-pinned anchor, raw
games, and immutable controller identities make that number replayable rather
than portable to a different experiment. This fixes an internal measurement
problem; it does not supply the missing external rung.

Claims such as “world-class,” “superhuman,” or “three times stronger” are not
supported. The defensible form is always: agent A beat agent B, on a named
profile, under a named decision rule.

## 2026-07-31 ranked intervention audit

The first implementation set ranked envoy acquisition first, strategy
commitment second, and a cross-profile promotion gate third. The ordering came
from replicated headroom rather than code aesthetics: suzerainty has the
largest oracle win-rate ceiling, and the deployment evaluator records 2–3
unanchored midgame plan changes per seat-game.

Both direct hypotheses were implemented behind independent evaluator arms and
then rejected as defaults. Influence infrastructure and influence-aware policy
selection increased the named resource in some compact samples, but component
controls could not attribute the promising deployment direction to either
mechanism. Corrected trigger-scoped commitment reliably reduced churn and
pointed +26 on compact and +35 at deployment, but both 20-map results were
inconclusive; deployment therefore failed the strength gate. These are useful
bounded results: more envoys are not automatically worth their opportunity
cost, and less plan motion is not yet established as stronger adaptation.

The systematic item did survive. `ai_eval --matrix` now requires a strength
PASS on the six-player Online deployment and no statistically established
regression on the compact Standard safety profile. Insufficient evidence fails
closed, both profiles run concurrently under one job budget, and matrix mode
owns all outcome-affecting profile flags. This closes the specific process hole
that allowed compact-only gains to acquire unqualified “stronger” labels.

A fresh 40-map comparison of retained `advanced` with `advanced_v1` on that
nine-city-state, randomized-civilization deployment profile was itself
inconclusive: 47.5% (−17 Elo-equivalent, 95% CI −124..+89). That does not erase
the recorded +207/PASS result on the older six-city-state fixed-roster profile,
but it does prevent a profile-independent “top-tier” claim. The strongest
supported conclusion is that `advanced` remains the production incumbent and
no tested replacement cleared the deployment gate.

## Superseded ranking (2026-07-30): next priorities

*Superseded — kept as the record of what was believed and why. The live
ranking is [Current ranked next work](#current-ranked-next-work-2026-08-17)
at the top of this file.*

The first of the three rankings this file accumulated. Items 1–4 were
re-expressed by the two rankings after it; item 5, external calibration, was
not, and is item 4 of the current list.

1. **Fog-honest major controller.** Extend the bounded belief-pressure use into
   a major civilization that consumes the existing observation, memory, and
   belief surfaces end-to-end. This is the largest remaining rules-integrity
   gap and creates honest uncertainty for every later policy improvement.
2. **Action-conditioned return with external-profile calibration.** Expand the
   counterfactual Q/advantage corpus well beyond the current 52-game sample,
   reserve the deployment profile as an untouched calibration set, and require
   selective abstention when the model is out of distribution. Do not greedily
   maximize another state-outcome correlate.
3. **Cost-aware expansion search.** Expansion is the second replicated oracle
   ceiling, but seven decision treatments failed because the oracle removed
   settler production and population costs. The evaluator-ready
   `advanced_coupled_expansion` arm now values the full paid
   build-settle-payback sequence; its disjoint outcome screen is still pending.
4. **Policy-deck transfer confirmation.** The existing live deck, not the new
   influence terms, produced the clearest direction in the envoy decomposition.
   Its first 20-map matrix scored 53.8% (+26) at deployment with 52.1% terminal
   score, while compact safety was inconclusive rather than harmful. Extend the
   same pre-declared seed prefix until the anytime gate resolves; promote only
   if deployment passes and compact continues not to retain the incumbent.
5. **External calibration.** Complete retained games against Firaxis' AI and
   humans with named settings. Internal Elo remains an internal ruler.

For implementation details see `docs/AI_GUIDE.md`; for the run-by-run evidence
and its corrections see `docs/EVAL.md`; for the rating/seating contract see
`docs/closed/LEAGUE.md`.

## 2026-07-31 full-prefix resolution

The fixed-prefix follow-through strengthens the audit's negative conclusion.
Neither a 120-map direct envoy-production treatment nor a 300-map live-policy
control cleared the unchanged matrix rule. Both pointed +30 Elo-equivalent on
deployment, and the envoy treatment raised deployment envoys from 14.3 to 19.3
and suzerainty share from 0.41 to 0.70, but both Wilson lower bounds still
included 50%. Strategy commitment regressed over 120 maps, and champion-weight
commitment regressed sharply. Stock `advanced` therefore remains the production
incumbent by failed-replacement logic, not by a claim of universal superiority.

The work also found a measurement defect with direct implications for future
research: extending `--pairs` used to move the deployment seed prefix because
the profile offset depended on sample size. Matrix profile seeds now have a
constant stride. Effective controller aliases and champion consumption are also
canonicalized through the same table used by construction; strict evaluator
construction now fails closed on degraded artifacts, and effective self-play
also fails closed in promotion mode.

## Superseded ranking (2026-07-31): next ranked work after the identity implementation

*Superseded — kept as the record of what was believed and why. The live
ranking is [Current ranked next work](#current-ranked-next-work-2026-08-17)
at the top of this file.*

Superseded by the turn-frame ranking immediately below, which says so in its
own closing line.

The former first item is now landed: every selectable arm has a typed
`AgentSpec`, evaluator preflight prints its actual comparison axes, and an
unlabelled multi-axis result cannot start. The remaining work should build on
that boundary rather than re-running the old stringly comparisons.

1. **Separate discovery from confirmation effect sizes.** Treat a promotion run
   as a decision, not an unbiased estimate: record its result as discovery and
   use disjoint, pre-registered confirmation maps for the reported effect size.
   This is R3 in `EVAL_INTEGRITY.md`.
2. **Test one rational composite, pre-registered.** Combine the live policy deck
   with direct envoy production, because the two controls independently moved
   deployment outcomes and the latter demonstrably moved the resource. Use new
   stable, disjoint 300+ map prefixes, preserve compact safety, and record build
   opportunity costs. Mark it as a deployment comparison; do not attribute its
   outcome to either component or run a parameter sweep on the confirmation
   seeds.
3. **Build a fog-honest major controller.** Extend the now-tested bounded
   belief-pressure surface into a major civilization that consumes observation,
   memory, and belief end-to-end. This remains the largest rules-integrity gap
   and is prerequisite to honest learned policies.
4. **Validate a fresh action-conditioned candidate.** The trainer, artifact
   loader, and selection gate now exist; the remaining work is data collection
   on disjoint profiles and the mirrored gameplay A/B. Keep deployment
   untouched until selection passes, and fall back to scripted play outside the
   supported distribution. A state-value argmax is not an action-value policy.
5. **Screen the full expansion investment.** The bounded
   `advanced_coupled_expansion` treatment now models settler production,
   population, escort, travel, settlement, and payback together. The oracle
   ceiling is large, but seven city-target treatments failed because the oracle
   removed those costs; only a disjoint gameplay screen can decide whether the
   paid policy earns promotion.
6. **Price strategic search on deployment.** Compare the searching controller
   with a genome-matched sequential control and keep it out of the live league
   unless its measured gain justifies its roughly 6.4× turn cost.

## Superseded ranking (2026-07-31): ranked next work after the turn-frame repair

*Superseded — kept as the record of what was believed and why. The live
ranking is [Current ranked next work](#current-ranked-next-work-2026-08-17)
at the top of this file.*

The direct ancestor of the current list: items 2, 4 and 5 survive there, items
1 and 3 have since been answered, and the resolution of each is recorded at the
top rather than here.

The current-main battlefront follow-up froze the already-promoted observation
boundary for one major turn. The opt-in `AdvancedAi::fog_honest()` arm now
extends that boundary across the complete major turn; it does not yet prove a
strength gain or change the incumbent/champion distinction. The next work
remains ordered by the largest remaining decision risk rather than by how local
the next code edit is:

1. **Separate discovery from confirmation effect sizes.** Keep promotion as a
   decision and quote effect sizes only from a disjoint, pre-registered
   confirmation prefix.
2. **Test one rational composite.** Evaluate the live policy deck plus direct
   envoy production on new stable deployment and compact prefixes, preserving
   the matrix gate and recording build opportunity costs.
3. **Screen the fog-honest major controller.** Run the opt-in arm on disjoint
   compact and deployment prefixes, measure replay refusals and throughput,
   and promote it only if the matrix gate pays for the information boundary.
4. **Validate a fresh action-conditioned candidate.** Use the new trainer and
   fixed-threshold evaluator on disjoint action corpora; do not tune coverage
   on the deployment profile, and do not integrate a candidate before the
   mirrored gameplay gate.
5. **Search full expansion investment, then price strategic search.** Model
   production, population, travel, settlement, and payback together; retain
   expensive macro search unless a genome-matched deployment comparison pays
   for its measured turn cost.

This supersedes the ordering immediately above: evaluator trust comes first,
and the now-resolved policy/envoy prefixes justify one bounded composite before
broader search.

## 2026-08-01 settlement-site intelligence

The shared production `AdvancedAi` path now prices the city a settler is trying
to reach rather than treating every legal plot as an interchangeable founding
opportunity. The default scorer gives early ring-one food extra weight,
explicitly rewards freshwater, keeps production and resource value, and looks
ahead to the strongest Campus, Industrial Zone, Commercial Hub, Harbor, Holy
Site, and Theater Square adjacency opportunities around the prospective center.

It also subtracts known operational risk: visible military attack envelopes,
nearby visible enemy cities and barbarian camps, isolation from the existing
empire, and the movement cost and visible threat exposure of the actual route.
Settlers refuse a visible direct attack tile, try a lower-risk progress step,
and re-plan rather than founding on a newly exposed tile. The behavior is on
for `advanced`, weighted/targeted Advanced variants, and the StrategicAi
controllers through their shared constructor; the historical `legacy` control
keeps the old score.

This closes the local site-selection and transit-safety gap. The remaining
economic question is now isolated behind the evaluator-only
`advanced_coupled_expansion` arm: it charges production, population cost,
escort availability, travel, founding lag, visible safety, and city payback
before a Settler can win a queue. The arm is deliberately not in production and
has no outcome claim until a disjoint screen resolves it.

## 2026-08-17 coupled expansion is a paid evaluator treatment

The previous expansion score treated a legal Settler as a large fixed prize and
left the real trade implicit. `AdvancedAi::coupled_expansion` now routes the
adaptive Expansion lane through strategic production and prices one legal
Settler as a bounded investment. Its score uses the existing settlement
forecast for the first four jobs, then subtracts the exact remaining Settler
production, the population point and recovery interval, estimated travel time,
escort availability, visible settlement safety, and the founding lag. A
90-standard-turn payoff horizon closes late candidates that cannot earn back
the investment.

The implementation does not clone terminal games or grant a free city; the
closed `expansion_investment` experiment remains the higher-cost terminal
counterfactual validation harness. `advanced_coupled_expansion` is registered
as a typed evaluator arm and is **off in production**. The focused tests pin the
dispatcher seam, a legal early paid candidate, and rejection when the remaining
turns cannot cover founding and payback. A replicated gameplay result is still
required before promotion.

## 2026-08-10 the live-bridge repair bundle does not transfer to native play

`AdvancedAi::enable_live_bridge` carries 41 measured repairs, and the doc
comment on each one is unusually convincing: an army admitted as a clique at
`command_radius` and then judged at half of it, clearing its own muster gate on
5 of 85 turns; a siege that walks away from a city at 25 hp and hands the
defender 200 hp of healing back; a relief column that marches at the besieger
nearest *itself* rather than the one killing the city; an army target already
reading satisfied on 94 of 188 war turns.

Every one of them is gated "live-bridge only", and the stated reason is always
versioning: the frozen `advanced_v1` anchor and the recorded ladders have to
keep running the controller they were rated with. Nothing in the tree ever
claimed the repairs were *wrong* natively — only that turning them on would
move a rating anchor.

**So the bundle had never been priced against `advanced`.** `live` has only ever
been compared with its own `live_without_*` ablations, and each of those holds
one flag off inside a bundle that still contains the other forty. That measures
a link. It cannot measure the chain against no chain at all, and the repairs are
serially coupled — readiness gates the march, the march gates the siege, the
siege gates the capture, and the army target decides whether there is anything
to march with.

`advanced_synergy` is the 37 of those repairs that fix a CIVVIS defect rather
than encode a rule of Firaxis' game, applied to the stock production
controller. `live_trader_route_adapter`, `live_religious_purchase_guard` and
`solvent_faith_army` are excluded as Firaxis semantics. `joint_tactics` remains
excluded from the native live bundle on evidence (§7 above), but the existing
bounded search is now the promoted controller's automatic route on the separate
Battlefield arena. That route is measured on the skirmish benchmark rather than
claimed as a whole-game win improvement; `advanced_v1` and explicit withholds
remain greedy. The war (23) and economy (14) halves are separate arms so the
composite's interaction is measured rather than assumed.

| arm | compact-standard | deployment-online |
|---|---:|---:|
| `advanced_synergy_war` (23 repairs) | −47, sign p = 0.0007 | −85, sign p = 0.0000, RETAIN |
| `advanced_synergy_war`, replication | −44, sign p = 0.0005 | −72, sign p = 0.0004, RETAIN |
| `advanced_synergy_economy` (14 repairs) | −31, sign p = 0.0488 | −65, sign p = 0.0005 |
| `advanced_synergy` (all 37), discovery | −60, sign p = 0.0000 | −129, RETAIN |
| **`advanced_synergy`, CONFIRMED** | **−76** (95% CI −140..−13) | **−108** (95% CI −172..−43) |

⚠ **Quote the confirmed row.** A promotion run is a decision, not an unbiased
estimate: the discovery effect size is conditioned on having fired the gate and
is biased away from parity. The confirmed row is `--confirm` on seed prefixes
disjoint from discovery, and it is the one that is quotable — this is R3 in
`EVAL_INTEGRITY.md` doing its job, and note that it moved the compact estimate
*further* from parity (−60 to −76) and the deployment estimate *toward* it
(−129 to −108). The direction of the bias is not knowable in advance, which is
why the disjoint prefix is run rather than reasoned about.

On the confirmation prefixes the multi-profile gate reads **0/2 profiles
cleared**: both profiles now retain `advanced` outright rather than one
retaining and one landing inconclusive.

The war replication is a disjoint seed prefix (86000000 against 83000000) run
after #1512 landed, which changed `remembered_objective_strength` — a repair
this half carries. Giving a never-seen city the conservative floor instead of
the `hostile <= 0` sentinel is worth something (−85 to −72) and is nowhere near
parity. Two independent prefixes, same direction, both significant.

⚠ Seed bookkeeping, recorded because this document has been wrong about a seed
stream before: the war replication was launched at base 86000000 while the
composite confirmation's *deployment* profile also derives 86000000 (base
85000000 plus one `MATRIX_PROFILE_SEED_STRIDE`). The numeric ranges overlap.
No games are shared — the war replication's 86000000 prefix is the four-player
24×16 compact profile and the confirmation's is the six-player 74×46 online
one, so the same integer seeds generate different maps under different rules —
but the two runs should not be described as drawing from disjoint *streams*,
only as measuring different arms on different profiles.

```
ai_eval advanced_synergy advanced --matrix --pairs 120 --seed 81000000
  compact-standard    41.5% (95% Wilson 33.0..50.4)   direction 15 / 53 / 52
  deployment-online   32.3% (95% Wilson 24.6..41.1)   direction 11 / 42 / 67
  multi-profile promotion gate: RETAIN advanced — cleared 1/2 profiles

ai_eval advanced_synergy advanced --matrix --pairs 120 --seed 85000000 --confirm 81000000
  compact-standard    39.2% (95% Wilson 30.9..48.1)   direction  8 / 61 / 51
  deployment-online   35.0% (95% Wilson 27.1..43.9)   direction 15 / 44 / 61
  multi-profile promotion gate: RETAIN advanced — cleared 0/2 profiles
```

**Both halves lose independently and the damage is close to additive**
(−85 and −65 against a composite −129 at deployment; −47 and −31 against −60 on
compact — halves and composite compared at discovery, the only prefixes on
which all three were run). There is no single villain to bisect out and no destructive
interaction between them: the bundle is not one bad repair poisoning thirty-six
good ones.

### Where the loss actually shows up

At deployment the arm is not out-expanded — cities land at 6.25 against 6.31,
population 73.6 against 74.7. It is out-*developed*:

| | `advanced_synergy` | `advanced` |
|---|---:|---:|
| districts | 27.0 | 24.1 |
| buildings | 75.9 | 94.1 |
| builders | 2.39 | 4.08 |
| gold | 216.0 | 727.6 |
| military | 749.7 | 1030.0 |
| science victories | 47 | 127 |

More districts, fewer buildings, 3.4x less gold, and a smaller army. The
economy is being spent on district and housing infrastructure it cannot fund,
and the military that infrastructure was supposed to pay for is smaller, not
larger.

### What this is evidence of

This is §5 above — results overfitting their measurement profile — with an axis
that matters more than map size: **regime**. Nearly every measurement quoted in
those doc comments comes from a live Civilization VI bridge run
(`civvis-2026…Z`), where CIVVIS issues orders into Firaxis' engine and reads
Firaxis' economy. The native engine is a different decision problem, and a
repair validated in one does not transfer to the other.

`muster_at_command_radius` states the mechanism against itself: it trades an
army that never advances for one that advances spread over six hexes and "can
be defeated in detail". Natively that trade loses.

⚠ This does **not** say the deployed Civilization VI agent is carrying harmful
changes — it is playing a different engine, which is exactly the point, and no
measurement here reaches it. What it does say is that the "live-bridge only"
gating, adopted as versioning conservatism, turns out to be load-bearing on
strength grounds as well, and that promoting any of these repairs into the
native controller on the strength of its doc comment would have cost real Elo.

The arms are eval-only and nothing in the shipped tree changes. Two guards keep
the bundles from drifting apart:
`engine_repairs_are_the_live_bridge_minus_the_firaxis_semantics` asserts the
flag-level partition and `engine_repair_tags_partition_the_bridge` asserts it
for the tag lists `differing_axes` reports.


## 2026-08-11 what the production controller is now known to be worth, flag by flag

`AdvancedAi::new()` routes through `promoted_policy_envoy`. Historically that
constructor turned on thirteen behaviours, while `configured` turned on ten
more for every non-`legacy` agent. Before this audit, **all twenty-three shared
one composite number** between them — the 2026-08-01 promotion. That is the
condition that let a component costing forty-one Elo ship and sit unnoticed
for ten days. The 2026-08-17 cleanup now leaves the two confirmed null arms at
their configured defaults; the evaluator and live bridge still expose them
explicitly so their negative evidence remains reproducible.

Each was priced by **withholding** it and running the paired evaluator at the
deployment shape. The table is the current state; `docs/EVAL.md` has the runs.

| flag | measured | status |
|---|---|---|
| `city_target_floor = 6` | **−41 Elo** (matrix PASS, 400 pairs, seed 8600000) | **REMOVED** #1504 |
| `settlement_safety` | **+31 Elo** (65/101, p=0.0064) | keep, now measured |
| `settler_commit` | **+30 Elo** (60/95, p=0.0061) | keep, now measured |
| the four war flags together | +32 / +34 at all-six victories; **+13, p=0.47 on the gate** | gate REJECT, off |
| ├ `siege_muster` + `home_defense` | +21 selected → **+15, p=0.18 fresh** | gate REJECT, off |
| └ `tactical_strategy` + `unit_objective_memory` | +11, p=0.32 | null |
| `plan_city_target` | null (64/64, p=1.0000) | keep |
| `bounded_recovery` | null over 600 maps, two seeds | **removed from production 2026-08-17**; live bridge/evaluator arm retained |
| `envoy_infrastructure` | null at 800 games | **removed from production 2026-08-17**; evaluator arm retained |
| `deny_leaders` | near-inert — **370 of 400 maps unchanged** | keep |
| `battlefront_observation` | null (49/56, p=0.5584) | keep |
| the four economy flags together | null (79/87, p=0.5871) | keep |

**Two of twenty-three were worth measuring and one was actively harmful.** The
rest are nulls or near-inert. That is the honest shape of this controller: it is
not a stack of small wins, it is a stack of small nothings with one liability in
it, and the liability was found only because every part was priced separately.

### 2026-08-17 measured-null cleanup

The production constructor now keeps the confirmed nulls out of the stock
controller. `envoy_priority` remains on because it is the actuation mechanism
that can reserve a legal Diplomatic Quarter → Consulate → Chancery stage;
`envoy_infrastructure` was only the netless valuation term around that path.
`bounded_recovery` remains available to `enable_live_bridge` and explicit
evaluator bundles, but `advanced_without_bounded_recovery` resolves to
`advanced` and fails closed as a historical alias. This removes measured-null
work from every ordinary production turn without erasing the controls or the
600/800-game negative records that justify the decision.

### The four traps this audit walked into, so the next reader does not

1. **`AdvancedAi::new()` plus a flag is usually a no-op.** The constructor
   already sets most of them, so an "add" arm is byte-identical to the control
   and reports a clean, meaningless null. **Withhold, never add.** Three arms in
   `elo.rs` had this defect and now fail closed as self-play.
2. **`ai_eval --city-states` defaults to zero.** Any arm acting through minors —
   envoys, influence, suzerainty, the Diplomacy lane — measures nothing on the
   stock profile. `ai_eval` now warns; the promotion matrix does seat them.
3. **`ai_eval`'s defaults are not the deployment, and can flip an effect's
   sign.** `d_holy` measured +20 Elo at 4p 24x16 (52.9%, 95% CI 50.1%..55.7%,
   1200 maps, seed 4400000, PR #1469) and parity at the shape the exhibition
   runs (+2, CI −46..+50, 400 maps, seed 5900000); it shipped and was reverted
   the same day, PR #1491. Gate on the deployment shape from the first run, or
   name the profile in the claim.
4. **A composite gate licenses the composite, never its parts.** The eight-flag
   remainder read +9 net (CI −25..+43, 400 maps, seed 9300000) and contained a
   war half at +32 (97/60, p=0.0039, seed 10800000) offset by an economy half at
   −7 (79/87, p=0.5871, seed 10700000). Bisect before concluding a group is inert.

### What the audit says about where strength is not

Every motion symptom `audit` reports was chased and none converted: a settler
idling two hundred turns on foundable ground (~5 Elo, unresolvable), ninety-four
circling warriors (1.21% of unit-turns), and an army standing unfortified on
7.73% of unit-turns declining a free +6 defensive strength (**−2 Elo, resolved
null over 72 discordant maps**). **`audit` is an excellent defect detector and a
poor value estimator** — a large symptom is a reason to look, never a reason to
expect Elo.

The same holds for the genome. Eight of its forty genes cannot change a game at
all (`src/bin/gene_census.rs`), the roster's winners are worse than the
incumbent when copied (46.9%, 95% CI 42.6%..51.3%, Elo −22, 500 maps, seed
5000000, PR #1486), and `docs/GENOME.md`'s conclusion stands: no
parameter tune has ever promoted. **The only gain in this audit came from
deleting work, not adding or re-weighting it.**

### The victory-set question the audit raised is settled (2026-08-14)

The promotion matrix ran **three of six victory conditions**
(`science,culture,domination`, hard-coded since #658) on both children, so
religious, diplomatic and score victories were invisible to it, and it produced
two recorded divergences from the deployment configuration: `d_holy`'s
three-victory read said −44 Elo where the deployment set says parity (#1491),
and it retained a war-flag withhold that replicated +32/+34 where the
exhibition plays (seeds 10800000/11000000). The question was settled on its own
evidence, not for a treatment that wanted it: the **Strength** child
(`deployment-online`) now plays all six victories — the estimand it certifies
is deployment, so its games must end every way a deployment game can — while
`compact-standard` keeps the three-victory set for its measured ~23% higher
decisive-map rate, which is exactly what a NoRegression tripwire should buy.
The full decision, its cost, and the single pre-registered corrected-gate
re-run (the war half, seed 17000000, terminal decision rule) are in
`docs/EVAL.md` (2026-08-14, "the Strength profile now plays the deployment's
victory set"). Prior verdicts stand as decisions of the instrument that made
them.

That pre-registered re-run has since answered: **PASS** — deployment-online
55.5% (Wilson CI 51.5%..59.4%), Elo +38 (CI +10..+66), sign p=0.0000,
e-process crossed at map 57 (600 pairs, seed stream 18000000), compact-standard
no-regression ACCEPT with direction *for* the withhold (167/123, p=0.0114).
The four war flags (`siege_muster`, `home_defense`, `tactical_strategy`,
`unit_objective_memory`) left `promoted_policy_envoy` on 2026-08-14; their
withhold arms are aliases of `advanced`, the re-addition treatment is
`advanced_war_half`, and the anchor is untouched (compatibility re-pin). The
count of production behaviours priced by removal rises to two shipped removals
(`city_target_floor` −41, the war half ~+32..+38 across 1,400 pairs on three
disjoint seed streams; see `docs/EVAL.md` 2026-08-14, "the war half leaves the
shipped controller").

## 2026-08-17 end-to-end fog-honest major

The first complete fair-play controller now exists as the opt-in
`AdvancedAi::fog_honest()` arm. At the beginning of a major turn it refreshes
the controller-owned `BeliefState`, opts into persistent player map memory,
and creates one disposable planning world from current visibility plus those
last-known tiles. Hidden foreign units are absent; unseen terrain is an
explicit unknown prior; and a foreign City Center is represented only by its
last-seen owner, health, walls, and displayed combat strength. Production,
diplomacy, campaign selection, and tactics all consume that same world. The
action tape is then replayed against the authoritative game, where hidden
blockers and combat remain the legality authority.

This closes the architecture gap without silently changing the incumbent.
The strict evaluator now exposes the arm as `fog_honest`, and its first
deployment-shaped screen (20 paired maps, seed prefix `920000..920019`) made
the decision explicit: stock `advanced` retained the strength gate. The
fog-honest arm scored 15.0% (95% Wilson CI 5.2%..36.0%), with the incumbent
favoured on 15 of 20 map directions. The matched compact-standard screen
(20 pairs, `910000..910019`) was exactly neutral at 50.0% (95% Wilson CI
29.9%..70.1%) and therefore inconclusive; deployment is the required strength
profile, so the arm is not promoted into the incumbent. Focused tests prove
redaction, invariance to unseen enemy
movement/health changes, stale-city reference cleanup, and a short two-major
replay. The next useful work is to improve fair-play economic planning before
re-running the gate, not to relabel this negative screen as a promotion.

## 2026-08-17 the great-work veto outranks the treatment that pays for great-work buildings

★★★★★ **Unpriced, and it makes an existing measured null unattributable.**

`production_value`'s `Item::Building` arm opens with a hard veto
(`src/ai/advanced.rs:18482-18489`): any building carrying a great-work slot
returns `-10_000.0` when a `victory_target` is set to anything other than
Culture. Roughly 115 lines later the same arm pays `CULTURE_BUILDING_DEBT` to
Theater Square buildings whose district is already standing (`:18597`).

Every Theater Square building except `marae` has great-work slots
(`data/buildings.json`: amphitheater `writing:2`, art_museum `art:3`,
archaeological_museum `artifact:3`, broadcast_center `music:1`, film_studio
`music:1`). So on any seat with a non-Culture target the veto returns first and
`culture_building_debt` cannot fire for the buildings it exists to buy. The
treatment is live only on an untargeted seat (`AdvancedAi::new()`), which is
what `advanced` is in the evaluator — so the arm measures the mechanism, while a
targeted deployment gets none of it. **Any reading taken from a targeted seat is
a reading of the veto, not of the treatment**, and the two are not currently
distinguished anywhere.

Two further consequences, both unmeasured:

1. The veto is keyed on the great-work slot rather than on the district, so it
   also refuses `national_history_museum` — a **Government Plaza** building
   (`data/buildings.json`, `great_work_slots: {any: 4}`). A science-targeted
   seat declines a Government Plaza building because a culture lane it is not
   playing would have wanted it. Whether that is intended is not recorded.
2. The veto is total rather than a discount, so a targeted seat builds no
   Amphitheater at any price, and the Amphitheater is also the civic-yield
   building on that district.

This bears directly on the objective list, because #1871 made Culture
selectable: a Culture-targeted seat is the *only* configuration in which either
mechanism has ever been able to act, and none of the 307 recorded ladder
attempts ran one.

**Do not tune this.** It needs an arm and a pre-registered run, not a judgement:
price the veto's district-vs-slot key (`advanced_great_work_veto_by_district`)
against stock on the deployment profile, and re-read `culture_building_debt` on
a Culture-targeted seat where it can actually fire. The measurement route and
its integrity rules are in `docs/EVAL.md`; the standing prior on this ledger is
that most such repairs measure null.

## 2026-08-17 the `live` evaluator arm is five flags short of the agent that plays

★★★★★ **The one comparison that is supposed to be exact, is not.**

`AdvancedAi::enable_live_bridge` carries an explicit instruction
(`src/ai/advanced.rs:4349`):

> ⚠ ADD NEW BRIDGE FLAGS HERE, not in the binary, or the arm silently stops
> matching the deployment

Five flags are set in the binary. `src/bin/civvis_orders.rs` calls
`enable_live_bridge()` and then turns on five more:

| flag | in `LIVE_TREATMENTS` | has a `disable_*` twin |
|---|---|---|
| `parallel_settlers` | no | **no** |
| `host_settler_pop` | no | **no** |
| `explore_dead_targets` | no | **no** |
| `explore_commit` | no | yes |
| `bank_envoys` | no | yes |

`enable_live_bridge` turns on 68 flags and `LIVE_TREATMENTS` has 68 rows, so the
registry test `live_bundle_and_registry_agree` (`advanced.rs:27912`) passes — it
walks the *bundle*, and a flag set outside the bundle is invisible to it. The
test enforces the rule only against people who already followed it.

Three consequences, all unmeasured:

1. **`elo.rs`'s `live` controller is a different agent from the one that plays
   Civilization VI.** It is sold as the deployment's twin and is five flags
   short of it. Every `live` vs `live_without_*` reading is therefore taken
   against a base that is not the deployment.
2. **None of the five can be withheld.** `--without` resolves against
   `LIVE_TREATMENTS` (#1884), so these five ship unpriceable — the same defect
   the eleven missing control arms had, one level up.
3. **The per-run provenance line under-reports the deployed configuration by
   five**, so a ladder row's own record of what it ran is incomplete.

**The remedy, and why it is not a one-line move.** Registering them means
writing three new `disable_*` methods, adding five `LIVE_TREATMENTS` rows,
bumping the array to 73, and moving the five `enable_*` calls out of the binary
into the bundle. The deployed agent does not change — the same flags end up on —
but the **`live` arm does**, by gaining the five it should always have had.
That makes prior `live`-arm readings non-comparable with later ones, which is a
decision to register rather than a change to slip in: rows either side of it are
not comparable, and the entry that makes the move should say so in `docs/EVAL.md`
the way every other identity change here does.

Recorded now because #1884 has just made the eleven *registered* treatments
withholdable, and these five are what is left.

## 2026-08-17 discovery and confirmation prefixes are now mechanically disjoint

The evaluator already labels a gate-selected point estimate as a `DISCOVERY
ESTIMATE` and a later `--confirm` run as `CONFIRMED`. The remaining integrity
hole was the word *disjoint*: before this repair the guard rejected only an
identical base seed. A discovery run on `1000..=1049` could therefore be
"confirmed" on `1025..=1074`, reusing half the selected maps while claiming an
independent estimate.

`ai_eval` now checks the full inclusive `[seed, seed + pairs - 1]` intervals,
rejects any overlap, and fails closed if either endpoint would overflow `u64`.
Matrix mode applies the same check before adding its fixed compact/deployment
stride, so both profile streams inherit the separation. The focused evaluator
tests cover adjacent prefixes, partial overlap, same-base confirmation, and
overflow; a CLI smoke run exits before starting a game on an overlapping pair.

The confirmation estimate remains the quotable number. A pooled point estimate
would still contain the discovery prefix selected on the gate, so it is not
printed as a headline unless per-map results are retained for a separate,
explicit diagnostic.

## 2026-08-17 the action-conditioned policy boundary is now reproducible

The repository now has one path from a fresh causal corpus to a safely
reviewable candidate. `tools/action_policy_train.py` retains all four matched
doctrine returns as pairwise posterior targets, keeps train/selection games
disjoint, and refuses to emit an artifact when held-out BCE, fixed-threshold
coverage, or gated lift fails. `tools/action_conditioned_eval.py` enforces the
same 34-state + 133-action schema and will not inspect an untouched external
profile after selection fails. `valuenet::ActionPolicy` validates the emitted
schema and abstains on malformed, tied, or low-confidence candidate sets.

This is an infrastructure completion, not a strength claim: no policy file is
embedded, no default AI loads it, and the incumbent remains scripted. A future
fresh external screen and mirrored gameplay A/B must earn promotion; the
rejected historical Q corpora remain closed.
