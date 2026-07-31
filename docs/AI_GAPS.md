# AI status: what works, what fails, and what is still missing

This is the current assessment of the game-playing AI. `docs/EVAL.md` is the
chronological experiment log; older entries there preserve what was believed at
the time and are not automatically the current conclusion.

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

The repository ships and embeds the 40-gene champion in
`data/evolved/best.json`. It ships no `valuenet.json`. That distinction matters:
the genome changes a scripted policy, while a value net would be a learned
model. `ai_eval` reports the artifact provenance and effective fallback for
every named entrant.

Two tools reach the real Civilization VI executable, but neither runs the Rust
agent unchanged. The grounding mod imports only the economic subset of a league
genome and leaves tactics to Firaxis' AI. The computer-control mod is a separate
Lua heuristic controller that issues its own orders. The first has two
explicitly anecdotal 60-turn strategy-transfer datapoints; the second has no
completed ladder attempt. They establish integration paths, not external
strength.

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

`BasicAi` and production `AdvancedAi` read the full `Game`, including
information a seated player cannot observe. The HTTP observation and spatial
tensor honor fog, but the production policy does not consume either. This
prevents a fair-play claim and removes scouting, memory, and uncertainty from
the decision problem. `advanced_belief_pressure` now proves one bounded
evaluator-only use of player-visible memory in the repaired city-pressure path,
but it neither replaces the controller's other full-state reads nor cleared its
whole-game promotion gate.

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

The active gene vector is 40 entries. The policy appetites and the policy-deck
and dedication selectors stored beside it in `Weights` are not genes. Research
order, city roles, strategic gates, and many tactical decisions are also outside
the vector.

An older report said “11 of 48 genes” were silent, but the implementation's
vector has 40 entries and that report's table names only ten. Treat the ratio as
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
calibrate it against a human or Firaxis' AI. The separate Civilization VI Lua
controller has no recorded completed ladder attempt in `CIV6_LADDER.md`.

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

## Next priorities

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
   settler production and population costs. Search or value the full
   build-settle-payback sequence instead of raising a city target again.
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
`docs/LEAGUE.md`.

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
canonicalized through the same table used by construction; degraded artifacts
and effective self-play fail closed in promotion mode.

## Next ranked work after the full-prefix audit

1. **Finish the structural evaluator specification.** Replace stringly factory
   selection with a fallible `AgentSpec`/`builtin_arm` boundary that compares
   every behavior-defining axis before a run. The shared alias resolver closes
   the observed champion and historical-alias failures, but it is not yet the
   exhaustive typed specification described in `EVAL_INTEGRITY.md`.
2. **Test one rational composite, pre-registered.** Combine the live policy deck
   with direct envoy production, because the two controls independently moved
   deployment outcomes and the latter demonstrably moved the resource. Use new
   stable, disjoint 300+ map prefixes, preserve compact safety, and record build
   opportunity costs. Do not run a parameter sweep on the confirmation seeds.
3. **Build a fog-honest major controller.** Extend the now-tested bounded
   belief-pressure surface into a major civilization that consumes observation,
   memory, and belief end-to-end. This remains the largest rules-integrity gap
   and is prerequisite to honest learned policies.
4. **Learn action-conditioned advantage with abstention.** Expand the
   counterfactual action corpus, reserve deployment as an untouched calibration
   set, and fall back to scripted play out of distribution. A state-value argmax
   is not an action-value policy.
5. **Search the full expansion investment.** Model settler production,
   population, escort, travel, settlement, and payback together. The oracle
   ceiling is large, but seven city-target treatments failed because the oracle
   removed those costs.
6. **Price strategic search on deployment.** Compare the searching controller
   with a genome-matched sequential control and keep it out of the live league
   unless its measured gain justifies its roughly 6.4× turn cost.

This supersedes the ordering immediately above: evaluator trust comes first,
and the now-resolved policy/envoy prefixes justify one bounded composite before
broader search.
