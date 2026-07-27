# Evaluation baselines

Recorded reference numbers so strength and health regressions are visible.
Re-run the battery after any AI or rules batch and compare against the most
recent entry; update this file (append, don't overwrite) when numbers move
for an understood reason. All commands are deterministic for a given build
and seed set.

```bash
civvis soak --games 12 --players 4 --turns 350 --start-seed 100
civvis tournament --ais advanced,basic,random --games 30 --players 4 --turns 250 --quiet
victory_eval --games 2 --players 2       # all six targets, stock turn limits
ai_eval advanced basic --pairs 100 --seed 4000   # paired, low-variance
ai_eval advanced basic --pairs 100 --difficulty emperor   # against the ladder
```

## 2026-07-22 — commit fba4785 (session F baseline)

**Test suite:** 421/421 lib tests pass.

**Soak** (12 × 4-player, 350 turns, seeds 100-111): 12/12 completed, no
panics, no anomaly flags.

| Victory | Games | Notes |
|---|---|---|
| religious | 8 | t170–t291 |
| score (turn cap) | 4 | no other victory landed by t350 |

**victory_eval** (2 seeds × 6 targets): 12/12 PASS — every victory type is
reachable end-to-end by the real game loop (science t689/t957, culture
t432/t458, religious t86/t159, diplomatic t425/t395, domination t583/t132,
score t301).

**Findings (exploit-hunt / balance):**

1. **Religious victory dominates advanced self-play** — 8/12 at four
   players when no AI is told to pursue a specific victory. Either faith
   output is over-tuned relative to Civ 6 pacing, or the AIs under-invest
   in religious defense (inquisitors/theological combat) relative to how
   hard they push their own religion. A strong optimizer will farm this
   lane; worth a targeted balance/defense pass.
2. **Nobody ever dies** — majors_alive was 4/4 in all twelve games and all
   six city-states survived every game. Wars happen but never conclude in
   elimination at 4p/350t. Real Civ 6 games kill civilizations; the
   military AI is likely too conservative about finishing sieges or picks
   unwinnable-but-safe postures. (`victory_eval --target domination`
   passes, so conquest works when explicitly pursued.)
3. Score-cap games (4/12) suggest the turn-350 horizon is short for
   science/culture lanes at 4p — consistent with victory_eval reaching
   science at ~t700-950 and culture at ~t430-460 on small 2p maps.

**Tournament** (30 × 4p × 250t, seed 0, K=24; 25% win rate = parity):

| AI | Elo | Games | Win rate |
|---|---|---|---|
| advanced | 1154.5 | 36 | 56% |
| basic | 1022.4 | 43 | 19% |
| random | 823.1 | 41 | 5% |

`random` winning at all (2 games) is worth an eye: at the 250-turn cap the
score ranking can crown a passive seat in a table with no advanced player.
The sanity floor holds, but sub-350-turn tables measure score racing as
much as victory play.

**ai_eval** (`advanced basic --pairs 25 --seed 4000`, mirrored 2p, avg
159t): advanced wins 39/50 (78%), ahead on every economic diagnostic
(score 194 vs 139, tech 15.3 vs 11.4, production 37 vs 25). Victory mix:
religious 27/50 across both seats — the same religious dominance the soak
shows at 4 players, now confirmed head-to-head (basic banks 452 faith it
never converts, advanced converts at less than a third of that).

## 2026-07-22 — religious balance batch (session F, after 311119a)

Rules fix (stock Civilopedia rule): faith-purchased religious units now
adopt their own city's majority religion, so non-founders can field
adopted-faith Missionaries; Missionaries spread the unit's faith and
reconvert home first. AdvancedAi: every strategy now runs a home religious
defense, triggered while conversion is in progress (any rival faith at 60%
of the city's strongest pressure), not after the majority already flipped.

**Soak, same seeds 100-111** (12 × 4p × 350t):

| Variant | religious | score-cap | other |
|---|---|---|---|
| before batch | 8 | 4 | 0 |
| majority-flip trigger only | 11 | 1 | 0 |
| 60%-pressure trigger (shipped) | **3** | 9 | 0 |

The majority-flip variant proved the timing thesis: by the time a rival
faith holds a city, the pressure race is lost and defense spending is
wasted. Triggering at 60% pressure turns religion from a near-free lane
(11/12) into a contested one (3/12); games now run long and 9/12 hit the
turn cap on score, consistent with the earlier finding that 350 turns is
short for the science/culture lanes at four players. 427/427 tests.

**Mirrored 2p is intentionally less affected** (advanced beat basic 39/50
with religious 27/50 both before and after): with only two faiths on the
map, a converted non-founder rarely has a third adopted faith to buy —
matching real Civ 6, where a duel against a committed religious player is
genuinely hard to defend without your own religion.

**StrategicAi first probe** (`ai_eval strategic advanced --pairs 8 --seed
5000`, mirrored 2p, avg 177t): strategic wins 9/16 (56%), all nine on
score, with markedly stronger empires (score 230 vs advanced's usual ~195,
military 257 vs ~103, tech 18.8 vs 15.3). Small sample — treat as "at
least parity"; a 25-pair run should decide promotion to exhibition seats.

## 2026-07-22 — score formula + game length (session F, after 0bd6734)

Two rules fixes, both verified against the Civilopedia:

1. **Score formula was not Civ 6's.** The engine scored 10/city, 3/Citizen,
   3/district, 2/civic and **1 point per unit**. Gathering Storm scores
   3/civic, 5/city, 2/district (4 unique), 1/building, 1/Citizen, 5/Great
   Person, 10 for founding a religion + 2 per foreign follower city,
   2/technology, 15/wonder, plus Era Score — and nothing for units. Ties now
   resolve through the shipped tiebreaker chain. This is not cosmetic: score
   decides every capped game and feeds `evolve` fitness, Elo placement, and
   `StrategicAi::position_value`, so the AI was being paid to hoard units
   and population rather than build wonders and Great People.
2. **Standard speed is 500 turns**, and the engine already models
   Standard-speed costs everywhere — but the default-speed CLI path kept
   each command's historical budget (simulate 250, soak 120), so a
   "Standard" game played half a game and ended on an arbitrary cutoff.

**Soak, same seeds 100-111, now at the stock 500 turns:**

| Outcome | 350t, old score | 500t, GS score |
|---|---|---|
| religious | 3 | 8 |
| score (turn cap) | 9 | **1** |
| diplomatic | 0 | 2 |
| culture | 0 | 1 |

Games are now decided by real victories instead of an arbitrary cutoff —
only one of twelve reaches the turn limit, and four different victory
types appear. 436/436 tests.

**Top open balance lead: religion still wins 8/12 at full length.** The
60%-pressure home defense fixed the *early* runaway (it was 11/12 before),
but over a full 500 turns religion still converts the world more often
than any other lane completes. Next probe: whether Missionary/Apostle
spread pressure and the passive ±9-tile pressure match the stock numbers,
and whether two defensive Missionaries is simply too small a budget.

**ai_eval advanced vs basic under the corrected score** (25 pairs, seed
4000): advanced 33/50 (66%), down from 78% under the old formula — the
old scoring was inflating advanced's edge by paying for its larger unit
count and population. 66% is the honest number.

**StrategicAi promotion gate** (25 pairs, seed 6000): strategic 27/50
(54%) over advanced. Above parity but inside the noise band at n=50, and
each decision costs six full rollouts. Verdict: keep as the builtin
`strategic` for further work; **not** promoted to the exhibition default.

## 2026-07-22 — the difficulty ladder as an external yardstick (session U)

Elo between our own bots is a closed system: it says one bot is 130 points
better than another, and nothing about whether either is any good. Now that
difficulty is a real setting (see [UNCIV_LESSONS.md](UNCIV_LESSONS.md)),
`ai_eval --difficulty <level>` gives an outside reference — the challenger
plays the *human* side of the handicap and its opponents play the AI side, so
"beats Emperor" means what a Civ player expects it to mean. Seats still swap,
which moves the challenger around the map rather than moving the handicap.

Reference run, `ai_eval advanced basic --pairs 6 --turns 90`, challenger
`advanced` against handicapped `basic`:

| Level | Challenger seat-win% | Challenger score | Opponent score |
|---|---|---|---|
| prince (no handicap) | 58.3% | 90.8 | 75.2 |
| deity | 16.7% | 80.9 | 154.7 |

Read that as calibration, not as a result: six pairs at 90 turns is a smoke
test, and Deity hands the opposition +80% Production and Gold, +32% Science,
Culture and Faith, +3 Combat Strength, four free boosts per era and seven
extra opening units. The point is that the axis exists and moves the right
way; the number worth tracking over time is the highest level the current
agent still beats at `--pairs 100`.
## 2026-07-22 — learned-policy rung and the real training loop (session F)

PyTorch installed (CPU build), so the loop below was **run**, not just
written. `civvis selfplay` now also writes `dataset.csv` (the 25 scalar
`evolve::features` + win label), closing the chain:
`selfplay -> dataset -> train_valuenet.py -> valuenet.json -> agents`.

**Value net, 60 self-play games / 5,998 samples:** val BCE **0.336** against
a constant-predictor baseline of 0.562 — the scalar net genuinely learns.
(Caveat: `train_valuenet.py` still splits by sample. `train_spatial.py`
splits by game, which is correct; the scalar trainer should follow.)

**Spatial net, same pipeline:** the by-game split is the honest one and it
changed the story completely. A per-sample split reported **98.8%**
accuracy; splitting by game gives **75.0%**, which is exactly the
majority-class baseline on a 4-player export (one seat per game wins), with
a *worse* BCE than the constant predictor. The trainer now always prints
the baseline and a `beats_baseline` verdict so this cannot be misread
again. Conclusion: the pipeline is correct, 24-60 games is nowhere near
enough data. This is the concrete argument for AI_GAPS item 8's scale.

**PolicyAi (`policy`) vs AdvancedAi** — 6 pairs, seed 7000, mirrored 2p:

| AI | wins | score | gold | military |
|---|---|---|---|---|
| advanced | 7/12 (58.3%) | — | — | — |
| policy | **5/12 (41.7%)** | 169.0 | 137.8 | 158.6 |

The learned policy **does not beat the scripted agent**. It plays full
legal games with the net choosing actions (one-ply value search over the
real action space), but it is weaker — and its Gold (137.8 vs advanced's
typical ~325) shows why: a one-ply evaluator happily spends the treasury on
whatever looks locally good. This is the expected first rung, and it maps
the remaining work precisely: a policy head trained on far more self-play,
multi-ply search, and credit assignment past one action.

## 2026-07-23 — the GPU loop actually runs (session F)

**The Blackwell card needs `cu128`.** `pip install torch` (default) gives a
CPU build; the `cu126` wheel reports `cuda True` but dies at the first
kernel with `no kernel image is available for execution on the device`,
because its arch list stops at `sm_90` and an RTX PRO 6000 Blackwell is
`sm_120`. The working recipe is:

```bash
pip install --force-reinstall --no-deps torch     --index-url https://download.pytorch.org/whl/cu128
python -c "import torch; print(torch.cuda.get_arch_list())"  # must list sm_120
```

`--force-reinstall` matters: pip sees the same version number across index
URLs and otherwise does nothing.

**First real GPU training run** — 144 self-play games, 18,405 samples, 28
games held out *by game*:

| | val BCE | verdict |
|---|---|---|
| constant-predictor baseline | 0.5623 | — |
| trained net (`torch/cuda`) | **0.3685** | **BEATS baseline** |

Both trainers now hold out whole games and print this baseline comparison,
so a leaked or useless model cannot be mistaken for a good one.

## 2026-07-23 — condemnation: correct rule, no balance change (session F)

Civ 6's standing military answer to a religious offensive is condemning
foreign Missionaries with military units (only while at war, or when the
World Congress allows it). The engine had the rule implemented correctly,
but `AdvancedAi` only invoked it when an enemy religious unit *already
shared its tile* — which essentially never happens — so the counter was
dead code. Military units near home now step onto an adjacent intruder and
condemn it.

**It did not change the balance.** On the same seeds (100-107) the victory
mix is identical before and after: 5 religious, 2 diplomatic, 1 culture.
That is worth stating plainly rather than claiming a win — and it points at
the actual blocker: condemnation requires *being at war* with the religious
leader, and these AIs largely are not. The remaining lever for the religion
runaway is therefore in `victory_denial`'s willingness to open a war (or
push the Congress vote) against a runaway faith, not in the condemn
mechanic itself.

## 2026-07-23 — the learned policy overtakes the scripted agent (session F)

The first `PolicyAi` rung lost 5/12, and a better net did not fix it: with
the leak-free GPU-trained value net it still scored **8/20 (40%)** against
`advanced`. So the bottleneck was never the net's calibration — it was the
one-ply architecture. A one-ply evaluator cannot see the cost of a
multi-turn commitment, so production, research and purchases all look
nearly free, which is exactly why the agent's treasury kept collapsing
(Gold 138 against advanced's ~325).

Restricting the net to action kinds whose whole effect lands **this turn**
(`TACTICAL_KINDS` — moves, attacks, strikes, fortify, pillage, condemn) and
leaving multi-turn economy to the scripted layer flips the result:

| Run | policy | advanced | policy Gold |
|---|---|---|---|
| unrestricted, 10 pairs | 8/20 (40%) | 12/20 (60%) | 138 |
| tactical-only, same 10 pairs | 11/20 (55%) | 9/20 (45%) | 685 |
| tactical-only, 25 fresh pairs (seed 8200) | **28/50 (56%)** | 22/50 (44%) | — |

Two independent seed sets agree, and the Gold column confirms the
diagnosis rather than just the outcome. This is the first configuration in
which a learned component beats the scripted agent head-to-head, and it
states the design rule plainly: give the net the decisions whose
consequences it can actually observe, and let search or scripting own the
ones it cannot.

## 2026-07-24 — threat-aware macro routing

`StrategicAi` had silently regressed behind its scripted parent. On 25 mirrored
duel maps (`ai_eval strategic advanced --pairs 25 --seed 10000 --turns 180`),
the original search layer won only **14/50 (28%)**. It still produced more
Science and Production, but `advanced` won 32 religious games while the search
agent finished 22 seats committed to Science. The cause was structural:
30-turn rollouts modeled every rival as `BasicAi`, fallback evaluation used
score share, and the next macro review could be 40 turns away from a sudden
victory threat.

The corrected router now:

- rolls candidate lanes against `AdvancedAi`, so counterfactual opponents
  exert the same victory pressure as the real benchmark;
- interrupts periodic search on public 0–100 victory-race progress, with the
  adaptive planner's 78% / 15-point margin and earlier whole-civilization
  religious warning;
- preserves invested Astrology/Holy Site/Prophet paths while a slot remains;
- treats an enabled duel religious race as mandatory victory geometry: only
  one foreign conversion is needed, so it commits while a Prophet is available
  and stays committed after founding;
- reports final explicit-target counts in `ai_eval`, making routing failures
  visible beside wins and economic diagnostics.

Same-seed result: **32/50 (64%)**, up 36 percentage points, with 30 religious
wins. A disjoint 25-map holdout (`--seed 12000`) reproduced it at **31/50
(62%)**. The duel prior is disabled in multiplayer: on 12 mirrored four-player
maps (`--seed 11000`), the generalized changes raised game wins from **8/24
(33.3%)** on unchanged mainline to **10/24 (41.7%)**, while StrategicAi kept
its Production and Science advantages. These are promotion signals, not a
claim of universal strength; the four-player sample should grow with the
league archive.

## 2026-07-24 — paired confidence and Elo-equivalent promotion gates

Raw wins from the two seat-swapped games on one generated map are correlated:
they share terrain, resources, civilizations, and much of the resulting game
geometry. `ai_eval` therefore treats each mirrored map as one independent
cluster. The challenger receives 1 for a sweep, 0.5 for a split, 0 for a
reverse sweep, and half credit for an individual game that ends without a
winner. It reports a conservative 95% Wilson interval using the number of maps,
not the larger and misleading number of games.

The same score and interval are transformed through the standard logistic Elo
expectation curve, so every comparison now has an Elo-equivalent point estimate
and confidence range. Promotion requires at least 20 independent maps and a 95%
lower score bound above 50%; an upper bound below 50% retains the incumbent.
Everything else is explicitly `INSUFFICIENT` or `INCONCLUSIVE` rather than being
promoted from a noisy headline win rate.

Re-evaluating the threat-aware Strategic benchmark illustrates the difference.
Its **32/50 (64%)** result came from 8 Strategic sweeps, 16 split maps, and one
Advanced sweep. The paired estimate is **64%, 95% CI 44.5–79.8%**, equivalent
to **+100 Elo, CI -38 to +238**. That is strong directional evidence and a large
point improvement over the old router, but it correctly remains
`INCONCLUSIVE` at 25 maps because the confidence interval overlaps parity.

## 2026-07-24 — adaptive control inside Strategic rollouts

The multiplayer router compared six forced victory targets but omitted its own
`AdvancedAi` parent as a candidate. It therefore had to commit at every review,
even when all explicit lanes were worse than remaining adaptive. On 20 mirrored
four-player maps (`--seed 13000`), the old router finished 35 of 80 seats on
Domination but produced one domination win; it scored **18/40 (45%)** overall.

`StrategicAi` now rolls out the adaptive parent beside every enabled explicit
lane. A target must beat adaptive by more than one score-share point before it
can take control, and a later review can return to adaptive without discarding
campaign or unit-role memory. Prophet commitments, duel religious geometry, and
urgent counter-routing still override the economic comparison.

On the same 20 maps the new router scored **21/40 (52.5%)**, converting three of
four Advanced sweeps into splits. On a disjoint holdout (`--seed 15000`) it
scored **17/40 (42.5%)** versus the old router's **16/40 (40%)**. Combined, the
change raises multiplayer results from **34/80 (42.5%, -53 Elo)** to **38/80
(47.5%, -17 Elo; paired-map 95% CI 32.9–62.5%)** and reduces forced targets from
75/80 final seats to 53/80. This is a replicated five-point improvement, not a
promotion over Advanced: the combined interval still overlaps parity.

The duel specialization is unchanged on its exact 25-map regression set:
**32/50 (64%, +100 Elo)** with 30 religious wins. Two broader value-shaping
experiments were rejected before this design: a victory-progress blend fell to
11/40, and generic commitment hysteresis fell to 15/40 on the first seed block.

## 2026-07-24 — full-game plan tracing

`ai_eval` now observes the reported victory target after every major-player AI
turn. It reports target exposure, switches per seat-game, the dominant target
over the whole game (final target breaks ties), and seat outcomes conditioned
on that dominant target. Bots without a `PlanReport` are explicitly
`unreported`; an Advanced/Strategic agent with no explicit target is
`adaptive`. This fixes a measurement problem in the earlier experiments: a
final target says nothing about how most of the game was played.

Fresh four-player holdout (`strategic advanced --pairs 20 --players 4
--turns 180 --width 24 --height 16 --seed 17000`):

| Result | Strategic | Advanced |
|---|---:|---:|
| Game wins | 19/40 (47.5%) | 21/40 (52.5%) |
| Paired-map Elo | -17 (95% CI -165..+131) | reference |
| Seat win rate | 23.8% | 26.2% |
| Target switches / seat-game | 2.27 | 0.00 |
| Adaptive exposure | 43.9% | 100.0% |

Strategic's final labels counted 31 domination seats, but only 10 seats were
domination-dominant over the full game. Those seats won 1/10; the 18
religion-dominant seats won 9/18. This is diagnostic association, not a causal
estimate—the router selects targets from the position, so hard positions can
select a particular lane. It is nevertheless a concrete ablation lead: test a
stricter proactive domination commitment while retaining urgent victory
denial, then accept it only on paired holdout maps.

## 2026-07-24 — deeper Strategic counterfactuals

The plan trace made poor domination-conditioned outcomes visible, but paired
ablation showed that the label was not the cause. On the first 10-map block
(`--seed 17000`), raising only domination's commitment margin changed no
outcomes (9/20 wins), and removing proactive multiplayer domination made the
result worse (8/20). Filtering an unwinnable non-founder religious lane and a
founded-religion near-tie prior were also outcome-neutral at 9/20. These
variants were rejected rather than accumulated.

The useful intervention was to give every counterfactual enough time to reveal
more of its real consequences. The default rollout horizon was tested at 30,
40, and 45 rounds on three disjoint 10-map blocks, always with the same maps
across arms:

| Seed block | 30 rounds | 40 rounds | 45 rounds |
|---|---:|---:|---:|
| 17000 | 9/20 | **11/20** | 10/20 |
| 18000 | 8/20 | **9/20** | 12/20 |
| 19000 | 7/20 | **11/20** | 10/20 |
| Combined | 24/60 (40.0%) | **31/60 (51.7%)** | 32/60 (53.3%) |

Forty rounds improves every independent block, moving the paired-map estimate
from 40.0% (-70 Elo, 95% CI 24.6%..57.7%) to 51.7% (+12 Elo, 95% CI
34.6%..68.3%). Forty-five rounds gains only one additional game across all 60
while requiring more search and finishing later in the simultaneous third
block, so 40 is the efficient frontier. This is a replicated strength gain,
not promotion over Advanced: the 30-map confidence interval still overlaps
parity. The exact 25-map duel regression remains **32/50 (64%, +100 Elo)**
with 30 religious wins, as expected because its mandatory religious route
bypasses the economic rollout horizon.

## 2026-07-24 — exact paired-map direction test

The Wilson interval answers the practical effect-size question conservatively,
but it deliberately assumes maximum variance and therefore cannot use the fact
that many mirrored maps split. `ai_eval` now also reports an exact two-sided
sign test over independent map directions: challenger-favored, neutral, and
incumbent-favored. Neutral maps remain in the paired score and Elo interval,
but contribute no direction evidence. The binomial tail is evaluated in log
space, so large evaluation runs remain numerically stable.

The two statistics answer different questions and neither replaces the other.
On the established 25-map duel set, Strategic has 8 favored maps, 16 neutral
maps, and 1 Advanced-favored map: exact **p=0.0391**, a significant directional
edge, while the 64% effect estimate still has a 44.5%..79.8% Wilson interval.
Conversely, the selected 40-round multiplayer search aggregates to 4 favored,
23 neutral, and 3 Advanced-favored maps across its three blocks; its direction
test is correctly inconclusive even though its point estimate is 51.7%.

Promotion policy is unchanged: it still requires the minimum map count and an
effect-size lower bound above parity. The exact sign test is an orthogonal
diagnostic for distinguishing a repeated paired direction from a noisy point
estimate, not a license to promote on a small or practically tiny edge.

## 2026-07-24 — anytime-valid promotion evidence

The Wilson/Elo interval is a fixed-sample effect estimate. It remains useful,
but repeatedly increasing `--pairs`, inspecting the result, and stopping on a
favorable look invalidates a fixed-sample significance rule. That is exactly
how AI development works: promising candidates receive more maps, weak ones
stop early. The promotion gate now adds an anytime-valid betting e-process,
following the bounded-mean confidence-sequence construction of
[Waudby-Smith and Ramdas](https://arxiv.org/abs/2010.09686) and the
time-uniform framework of
[Howard et al.](https://arxiv.org/abs/1810.08240).

Each mirrored map remains one bounded observation in `[0, 1]`, including
quarter scores from win/draw combinations. A pre-declared finite mixture of
positive bets tests a challenger edge; the symmetric negative bets test an
incumbent edge. The evaluator reports peak e-values and Ville bounds
`p <= 1 / peak_e`. Monitoring starts only at the existing 20-map floor, and a
5% two-sided budget is split into 2.5% per direction. Promotion or retention
now requires both:

1. the current conservative Wilson effect interval clears 50%; and
2. the corresponding anytime p-bound is at most 0.025.

This protects arbitrary repeated looks at longer prefixes of the same seeded
candidate run. It does not excuse testing many candidate implementations and
publishing only the winner; candidate search still needs pre-declared
development/holdout seeds. If both directional e-processes cross, the run is
reported inconclusive because that pattern signals nonstationarity or a
pathological map order. Neutral maps multiply wealth by one, so they neither
manufacture nor erase directional evidence.

End-to-end replay (`advanced basic --pairs 20 --turns 90 --seed 24000`) gave
Advanced 25/40 game wins across 6 favored, 13 neutral, and 1 Basic-favored
map. The fixed-sample outputs were 62.5% (Wilson 40.9%..80.0%, +89 Elo) and
exact sign p=0.1250. The new process reported peak e=3.651, anytime p<=0.2739,
no crossing, and `INCONCLUSIVE`—all three views correctly reject promotion
from a suggestive but small 20-map result.

## 2026-07-24 — grouped scalar value training and controller rejection

The scalar value trainer now fails closed on the legacy 26-column dataset.
Every accepted row must contain 25 features, its outcome, and a nonnegative
integer source-game ID. Games—not correlated turn snapshots—are shuffled once
into frozen 60/20/20 train, early-stopping validation, and final-test splits.
The final split is evaluated only after selecting the epoch on validation.
The trainer records BCE, Brier score, accuracy, ten-bin ECE, and BCE by turn
quartile; it refuses to write a model unless final-test BCE beats a constant
predictor fitted only on the training rows. `--scalar-only` makes the matching
self-play export practical by omitting spatial tensors while retaining the
grouped scalar CSV and three-column labels.

Two real corpora passed that offline gate:

| Training policy | Games / rows | Untouched games | Model BCE | Constant BCE | Model / constant Brier |
|---|---:|---:|---:|---:|---:|
| Advanced | 300 / 14,070 | 60 | **0.3165** | 0.5964 | **0.1018 / 0.2032** |
| score-share Strategic | 120 / 5,535 | 24 | **0.3767** | 0.6097 | **0.1203 / 0.2094** |

Both models beat the constant in every opening-to-endgame turn quartile. That
is necessary predictive evidence, but it is not controller evidence. Model
activation therefore used separately predeclared map blocks against the exact
40-round score-share binary:

| Candidate evaluator | Development | Holdout | Decision |
|---|---:|---:|---|
| pure Advanced-trained net | 9/20 vs 8/20 baseline (seed 25000) | **7/20 vs 12/20** (seed 26000) | reject |
| 25% Advanced net + 75% score share | 10/20 vs 8/20 (seed 27000) | 10/20 vs 10/20 (seed 28000) | reject: no replication |
| 25% Strategic net + 75% score share | **9/20 vs 10/20** (seed 32000) | not run | reject before holdout |

No trained artifact is shipped or auto-loaded. Without an explicit
`evolved/valuenet.json`, Strategic remains bit-for-bit on the proven score-share
evaluator. When an experimenter supplies a structurally valid 25→64→32→1
model, Strategic now limits it to 25% of the terminal estimate and falls back
to score share for a non-finite prediction. Loading also rejects wrong tensor
shapes and non-finite parameters. This is a distribution-shift guard, not a
claim that the current learned evaluator is stronger: the gameplay evidence
plainly says it is not ready. The next learned-value experiment needs labels
from the actual counterfactual rollout endpoints (or an off-policy correction),
not merely more snapshots from on-policy trajectories.

## 2026-07-24 — direct Strategic evaluator A/B control

`ai_eval` now accepts the evaluator-only `strategic_score` control. It uses the
same evolved strategy weights, 40-round horizon, lane policies, map, and seat
schedule as `strategic`, but forcibly disables `evolved/valuenet.json`. The
control is deliberately absent from persistent tournament choices: those
ratings are keyed by civilization and dominant plan, so treatment and control
would otherwise collapse into the same row. Direct paired evaluation can now
isolate terminal-value changes without comparing two separate binaries or
routing both variants through an Advanced opponent.

The evaluator also reports a bounded terminal-score diagnostic. Within each
game it computes the challenger's share of Civilization score across all
evaluated seats, then averages the two seat-swapped games into one independent
map observation. It reports map direction, an exact sign test, and the same
anytime-valid betting evidence used for bounded win scores. This is explicitly
not a promotion input: terminal score supplies earlier development signal, but
only wins can promote a controller.

The no-model invariant (`strategic strategic_score --pairs 4 --players 4
--seed 37000`) produced four neutral maps, 4/8 wins each, and digit-for-digit
identical economy, military, victory, and plan diagnostics. With the rejected
Advanced-trained model supplied at the regularized 25% weight (seed 38000),
wins still split 4/8 each and all four win maps were neutral, while paired
terminal-score share moved to 50.3% across two model-favored and two neutral
maps (exact sign p=0.5). Aggregate model-seat score was 100.9 versus 92.5 and
the model seats also led in population, tech, culture, military, routes, and
religious units. That is useful sensitivity validation, not evidence to revive
the rejected model: four maps are far below the 20-map monitoring floor and
neither win nor score evidence crossed.

## 2026-07-24 — counterfactual value-label export

Ordinary self-play snapshots train a predictor for the states an agent visits,
but Strategic ranks a different distribution: the state *after* projecting
its adaptive parent and every enabled victory lane for 40 rounds. The offline
exporter can now label that exact distribution:

```bash
civvis selfplay --counterfactual --scalar-only --ai strategic_score \
  --counterfactual-roots 2 \
  --games 100 --players 4 --turns 500 --every 40 --seed 40000 \
  --out counterfactual-selfplay
python tools/train_valuenet.py --dir counterfactual-selfplay
```

Counterfactual mode begins at Strategic's first review turn (30), then samples
at the requested cadence. It selects one living major per checkpoint with a
deterministic rotation to bound the multiplicative cost, and exports the
adaptive plus each enabled lane endpoint. A branch that already has a winner,
or whose candidate is dead, is omitted because `position_value` does not ask a
net to evaluate it. Roots handled by the duel-religion, urgent-counter, or
irreversible-religion priors are omitted for the same reason: `review` returns
before running economic rollouts in those cases.

Each retained endpoint is frozen as 25 scalar features, then continued to the
winner with the *same stateful branch agents* that produced it. This prevents
both terminal-feature leakage and policy resets at the labeling boundary.
`meta.json` records root and per-lane counts. All sibling lanes keep the source
self-play game's ID in `dataset.csv` and `labels.f32`; the grouped trainer must
therefore place the entire source game—not individual branches—into exactly one
of train, validation, or final test.

The mode fails closed unless both `--scalar-only` and the explicit no-model
`strategic_score` control are selected. A learned net can neither influence its
own root trajectory nor its rollout labels. Passing the offline BCE/Brier/ECE
gate remains only a prerequisite: a new artifact still stays out of production
unless it also beats `strategic_score` on predeclared development and holdout
maps in the direct paired evaluator.

An operational two-game smoke (`--turns 90 --seed 39100`) scheduled four
roots, retained the two whose reviews actually reached the value evaluator,
and wrote 14 rows: two each for adaptive, Science, Culture, Religion,
Diplomacy, Domination, and Score. Labels included both classes (5 wins, 9
losses), every CSV row had the required 27 fields, and spatial payloads were
empty. Repeating the same seeds with `--jobs 1` and `--jobs 2` produced
byte-identical `dataset.csv`, `labels.f32`, and `meta.json`.

Factoring the live rollout through the reusable endpoint runner did not change
Strategic behavior. A fresh no-model invariant replay (`strategic
strategic_score --pairs 2 --players 4 --turns 90 --seed 39200`) split every
map, every win, and terminal score exactly 50/50. All aggregate economy,
military, victory, plan-exposure, switch-count, and final-target diagnostics
were digit-for-digit identical between treatment and control.

### Bounded source trajectories

A high-fidelity pilot exposed a second multiplicative cost: an unlimited
full-500-turn source game continues paying every Strategic review even after
enough endpoint branches have already been labeled. Twelve standard-map games
at `--jobs 12` were still using roughly eight worker-equivalents after 30
minutes and had not reached deterministic writeout, so that naive run was
stopped rather than scaled.

`--counterfactual-roots N` now stops each *source trajectory* immediately after
its Nth scheduled root. It never truncates a retained branch: each endpoint is
still continued independently to a real winner under the full turn limit. A
value of zero preserves unlimited sampling. Two roots capture the evaluator
states at approximately turns 70 and 110 while avoiding all later source-game
reviews; this is the recommended first-pass corpus design for maximizing
independent games per unit of compute. Later-state corpora should use a
separately predeclared larger root budget and remain a distinct calibration
stratum rather than silently changing the sampling distribution.

The bounded operational smoke (`--counterfactual-roots 1 --turns 90 --seed
39300`) stopped the source exactly at turn 30 with no source-game winner, while
still writing seven terminally labeled rows—adaptive plus all six lanes—and
advertising one root and a 7×3 label shape in metadata. This distinguishes the
intended early source stop from incomplete branch labels.

## 2026-07-26 — every learned entrant was silently scripted

`evolved/` is generated and git-ignored, so a fresh clone has neither
`best.json` nor `valuenet.json`. `builtin_ai` falls back rather than fail,
which is right for playability and wrong for evidence: on such a checkout —
which is every checkout in this repository, and was every worktree on this
machine when this was found — the learned names are not learned agents.

| Requested | Definitional artifact | What actually plays without it |
|---|---|---|
| `evolved`, `advanced_evolved` | `best.json` | `advanced` (stock weights) |
| `neural` | `valuenet.json` | `basic` |
| `policy` | `valuenet.json` | `advanced` |
| `strategic` | `valuenet.json` | `strategic_score` |

So `ai_eval policy advanced` was `advanced` against `advanced`, and its
50/50 split was filed as a fact about a learned policy. The published
learned-agent numbers above (2026-07-23's 28/50 for tactical `policy`, and
every `strategic` result that credits the net) were produced against
artifacts that no longer exist anywhere on this machine; treat them as
unreproduced until the corpus is rebuilt and they are re-run.

`elo::builtin_provenance(name, dir)` now resolves a name through the same
loaders the agents use — an existing but unloadable `valuenet.json` is
reported missing, because that is what the agent does with it — and
`ai_eval` prints one provenance line per entrant before playing anything:

```
$ ai_eval policy advanced --pairs 1 --turns 40
policy: plays as advanced (missing best.json, valuenet.json)
advanced: scripted, no artifacts required
warning: policy and advanced both play as advanced; this run measures
advanced against itself and says nothing about either name
```

`--require-artifacts` exits 3 instead of recording an untrained result, and
`--artifact-dir` points the check at a non-default champion directory. The
collapse warning is the load-bearing half: a missing artifact is only a
problem when it makes two entrants the same agent, and that is precisely
the case a win rate cannot show you.

## 2026-07-26 — a duel never reaches the macro search

`ai_eval strategic strategic_score --pairs 20 --seed 21000 --turns 180`
returned **20 neutral splits on 20 maps** — not parity but bit-identical
play, which is what a value net that never runs looks like. The cause is
structural, not statistical: `review` answers from three priors before the
economic rollouts, and in a two-player game `duel_religious_race` is true
for essentially the whole game. `StrategicAi` in a duel is `AdvancedAi`
hard-targeted at Religion.

`StrategicAi` now keeps a `ReviewCensus` — reviews resolved by each prior
versus reviews that reached the rollouts — exposed through
`Ai::review_census()` and reported by `ai_eval`:

```
Macro search exposure (reviews that reached the rollouts):
  strategic   0/20 (0%) reached the rollouts; priors: duel-religion 8,
              urgent-counter 12, irreversible-religion 0
  strategic_score 0/20 (0%) ... (identical)
  warning: neither entrant reached its macro search, so this run compares
  priors and the scripted parent, not search or evaluator
```

Two consequences for the recorded baselines above:

1. **Every two-player `strategic` number measures a forced religious lane,
   not macro search.** The 2026-07-24 duel results (32/50 and the 31/50
   holdout, both with ~30 religious wins) are real improvements to the
   *priors* — which is what changed in that batch — but they are not
   evidence about rollout routing or about the evaluator. The four-player
   rows in that same entry are the ones that measured search.
2. `urgent_counter` fires on 12 of 20 duel reviews on its own, so removing
   only the religious prior would still leave the search mostly bypassed.

At four players the search does run — 26/44 (59%) and 22/43 (51%) exposure
on `--pairs 3 --players 4 --seed 11000 --turns 200` — and the A/B is
*still* exactly neutral: `--pairs 10` at those settings gave 10 neutral
splits on 10 maps. So with the search running and a calibrated net loaded
(grouped-holdout test BCE 0.4058 vs 0.5636 constant), the net never changed
a lane choice. `VALUE_NET_WEIGHT = 0.25` regularizes the learned estimate
toward score share hard enough that the blended argmax appears to equal the
score-share argmax on every review observed so far. That is the next thing
to measure directly, and it is a separate question from calibration: the
evaluator is good and inert.

Recommended settings for anything that claims to measure this agent's
search or evaluator: `--players 4` at minimum, and read the exposure line
before the win rate.

## 2026-07-26 — the tactical policy cannot see 96% of what it evaluates

`PolicyAi` scores each legal tactical action by applying it to a clone and
asking the value net how the position changed. The net's input is
`evolve::features`: twenty-five *empire aggregates* — cities, population,
owned tiles, techs, civics, military power, unit count, three yields, Gold
and score, mirrored for the leading rival, plus turn fraction. Repositioning
a unit changes none of them, so the gain is not noisy. It is exactly zero.

Measured over four 120-turn four-player games, 11,347 candidate evaluations:

| tactical kind | evaluated | changed the value | share |
|---|---|---|---|
| `attack` | 225 | 225 | **100.0%** |
| `move` | 9,481 | 185 | 2.0% |
| `fortify` | 1,445 | 0 | 0.0% |
| `ranged` | 152 | 0 | 0.0% |
| `city_strike` | 44 | 0 | 0.0% |
| **total** | **11,347** | **410** | **3.6%** |

A separate probe of the agent's actual decision points (648 turn starts, 5
games, mean 32.6 candidates each) found the best available action's gain to
be **exactly 0.0 at the median and at the 75th percentile**, with 560/648
below 1e-3 and only 109 clearing the 1e-4 commitment margin. So the agent
declines to act at 83% of its decision points and falls through to the
scripted layer. That, and not the net's calibration, is why the learned
tactical policy measures at parity: it is an attack selector wearing the
name of a policy.

This also reframes 2026-07-23's design rule. "Give the net the decisions
whose consequences it can actually observe" was applied at the wrong
granularity: `TACTICAL_KINDS` selects actions whose effects *land this
turn*, which is not the same property as actions the *features* respond to.
Ten of its twelve kinds fail the second test.

`fortify`, `air_patrol` and `air_rebase` are now dropped before the
candidate cap. They cannot change unit count, ownership, yields, Gold,
research or score, so no game state exists in which they move a feature —
pinned by `excluded_kinds_cannot_move_a_single_feature`, which walks every
legal candidate of three real games and asserts bit-identical features. A
zero gain can never clear the margin, so play is unchanged; what changes is
that ~13% of candidates no longer consume a clone *and a slot in the
`width` budget*. On a busy turn the stride that enforces that cap is what
decides which candidates survive, so a blind candidate does not merely cost
time — it displaces an attack.

The remaining gap is not fixable by training a better net on these inputs.
An evaluator whose inputs do not respond to an action cannot rank that
action at any level of calibration, which is AI_GAPS item 3 stated as a
measurement rather than a plan.

## 2026-07-26 — a second search axis, and the ceiling it ran into

The rollout planner has always had exactly one free variable: the victory
lane. `Doctrine` adds a second — `incumbent`, `expand`, `consolidate`,
`militarize`, each a bounded perturbation of the evolved genome, clamped to
the per-gene bounds evolution respects. With the lane fixed, each is
projected the same way a lane is and the best is adopted; the incumbent wins
ties, so the tuned genome stands unless the rollout says otherwise. This is
coordinate descent, four extra rollouts per review rather than seven times
four, and it depends on no learned evaluator: a rollout is simulation.

`AdvancedAi::reweight` swaps a running agent's genome while preserving
campaign, settler, builder and unit-role memory — the same contract as
`retarget`, one level down. Doctrines are always derived from the evolved
genome, never compounded onto the one in play, so repeated reviews cannot
ratchet a gene to its bound.

**Result: no measured effect.** `ai_eval strategic_doctrine strategic
--pairs 14 --players 4 --seed 31000 --turns 200` gave 14 neutral splits on
14 maps, with near-identical search exposure (86/182 against 88/184). The
axis is wired, live and deterministic; it simply does not fire often enough
to matter, for a reason worth recording:

- The projected spread between the best and worst doctrine is **0.0044 at
  the median** (max 0.0376, min 0.0) over 11 sampled reviews. The lane
  margin is 0.01, so a second axis reusing it can *never* choose:
  `DOCTRINE_COMMITMENT_MARGIN` is 0.002, set from that measurement. At 0.01
  the axis switched 0 times in 16 rollout reviews; at 0.002, 3 times in 19.
- Doctrine values frequently tie in pairs, because at a given review only
  one axis is live — a 40-round projection in which no settling decision
  arises leaves `expand` and `consolidate` bit-identical, and one with no
  war decision leaves `incumbent` and `militarize` identical.
- When they do differ, `militarize` was the argmax on 7 of 11 samples,
  `consolidate` 3, `expand` 1.

**The ceiling this ran into is the real finding.** Across the 14-map A/B,
86 reviews reached the rollouts over 28 games × 2 seats — about **1.5
rollout reviews per seat per game**. Half of all reviews are consumed by
priors (`urgent_counter` alone took 72 of 182), and reviews only start at
turn 30 and recur every 40. A macro search that makes one or two decisions
a game bounds every improvement hung off it: a better evaluator, a better
lane policy and a second axis all divide into the same tiny number of
decisions. Raising the search's decision count — cadence, or reducing prior
dominance — is worth more than improving any single decision, and should be
measured before more axes are added.

`strategic_doctrine` is eval-only and `doctrine_search` is off by default,
so `strategic` is unchanged. `StrategicAi::doctrine_values` exposes the
per-doctrine projections, because the spread between them is what decides
whether the axis can act at all — which no win rate would reveal.

## 2026-07-26 — search cadence: more decisions beat better decisions

The previous entry ended on a ceiling: the macro search reached the rollouts
about 1.5 times per seat per game, which bounds every improvement hung off
it. Cadence is the cheapest way to raise that — `review_every` is already a
field — so it was measured as a dose, with everything else (weights,
horizon, lane policy, priors) held equal. All runs are four-player,
`--turns 200`.

**Dose (seed 41000, 12 maps each, against `strategic`):**

| entrant | period | rollout reviews | games | maps for | maps against |
|---|---|---|---|---|---|
| `strategic` | 40 | 105 | — | — | — |
| `strategic_r20` | 20 | 158 (1.5×) | 13/24 (54.2%) | 1 | 0 |
| `strategic_r10` | 10 | 316 (3.0×) | 14/24 (58.3%) | 2 | 0 |

Monotone in the dose, and across those 24 maps not one broke *against* the
denser search.

**Confirmation on a disjoint seed set (52000, 24 maps, `strategic_r20`):**
28/48 games (58.3%), paired score 58.3%, Elo-equivalent +58 (CI −79..+196),
**5 maps swept to 1**, exact sign p=0.2188, anytime-valid e=6.993
(p≤0.1430). Rollout reviews 251 against 160 (1.57×).

Pooled over the two disjoint `strategic_r20` seed sets: 36 maps, 41/72 games
(56.9%), direction **6 for, 1 against, 29 neutral**.

**This is not promoted.** The gate requires the anytime-valid evidence to
cross, and e=6.993 is well short of the 40 that 2.5% needs. `strategic` is
unchanged and the cadences ship as eval-only entrants. The e-process is
anytime-valid, so more maps on `strategic_r20 strategic --players 4` can be
added to this evidence without inflating the error budget — that is the
cheapest open path to a promotion in this codebase right now.

**A falsified hypothesis, recorded so it is not retried.** An interrupt sets
`next_review = turn + review_every`, so a cheap prior read off the victory
screen appeared to postpone the expensive periodic projection it was never
meant to replace. `strategic_nodefer` holds the periodic schedule instead.
It changed the decision count by **one review** (95 against 96) and lost a
map. Interrupts do not meaningfully displace periodic searches; whatever
suppresses the decision count, this is not it.

Note on reading these runs: a mirrored seat-swapped A/B between agents that
differ slightly is close to powerless, because identical agents split every
map by construction. The count of maps that break neutral — and their
direction — carries the signal long before the win rate does.

## 2026-07-26 — marginal search compute buys frequency, not depth

The cadence result raised an obvious objection: `strategic_r20` spends
**twice** the search compute of `strategic`, so the gain might be compute
rather than frequency. Three arms answer it, all run on the *same maps*
(`--pairs 20 --players 4 --seed 52000 --turns 200`) so they are directly
comparable:

| arm | search compute | decisions | depth | games | maps for | maps against | sign p | e |
|---|---|---|---|---|---|---|---|---|
| `strategic_r20` | 2× | **2×** | 1× | 25/40 (62.5%) | **5** | **0** | 0.0625 | **6.99** |
| `strategic_r20h20` | **1×** | 2× | 0.5× | 22/40 (55.0%) | 4 | 2 | 0.6875 | 1.07 |
| `strategic_h80` | 2× | 1× | **2×** | 21/40 (52.5%) | 4 | 3 | 1.0000 | 1.00 |

`strategic_r20` and `strategic_h80` spend the identical doubling. Frequency
takes 5 maps and loses none; depth takes 4 and loses 3, which is exactly
what noise looks like. `strategic_r20h20` holds total rounds per game equal
to the baseline by halving the horizon, and still leans positive at 4–2 —
so frequency is worth something even when depth pays for it, though weakly.

Ordering, from best to worst use of a marginal unit of search compute:
**more reviews at the same depth > more reviews at less depth > more
depth**. The mechanism is unsurprising once stated: a 40-round projection
already looks further ahead than the position stays valid, so the binding
error is a commitment left stale, not a horizon left short. Lengthening the
projection refines an answer to a question the agent asked too long ago.

Three cautions on reading the table:

1. Nothing here clears the promotion gate; `strategic` is unchanged and all
   three are eval-only entrants.
2. The `strategic_r20` row shares its seed set with the 24-map run in the
   previous entry (5 for, 1 against). They are the same experiment at two
   sample sizes, **not** independent confirmations, and must not be pooled.
   Independent `strategic_r20` evidence remains seed 41000 (12 maps, 1–0)
   and seed 52000 (24 maps, 5–1).
3. A doubling is one point on a curve. `strategic_r10` quadruples reviews
   for 3.0× the rollout count and did not beat `strategic_r20` by a margin
   this design could resolve, so the returns are visibly not linear.

The practical consequence for anything built on this search: spend on how
often it runs, and treat the horizon as already long enough. The open idea
that follows is rotation — always project the adaptive baseline and the
incumbent lane, rotate one challenger per review — which cuts a review from
seven branches to three and buys frequency at *negative* compute cost.
`victory_progress` cannot do that pruning, because it deliberately scores
only concrete endgame progress and is zero for every lane in the early game
where most reviews happen.

## 2026-07-26 — rotating the projected lanes: cheaper, not better

If frequency is what marginal search compute should buy, the obvious next
move is to buy it at negative cost. A review projects the adaptive baseline
plus every enabled lane — seven branches. Rotation projects the baseline,
the lane currently in force, and one challenger that advances each review,
so a review costs about three branches instead of seven.

Keeping the incumbent in the set is load-bearing: `choose_rollout_target`
only returns a lane it actually projected, so a subset that omitted the lane
in force would drop it on the first review that failed to re-nominate it,
and the agent would thrash between whichever two lanes the cursor exposed.
`rotation_reaches_every_enabled_lane_on_a_cycle` pins that every lane still
comes up, on a fixed cycle rather than a lucky one.

Same maps as the previous entry (`--pairs 20 --players 4 --seed 52000
--turns 200`), with the compute column derived from the measured review
counts times branches per review:

| arm | branches | reviews | compute | games | maps for | against | e |
|---|---|---|---|---|---|---|---|
| `strategic_r20` | 7 | 1.65× | 2.00× | 25/40 (62.5%) | **5** | **0** | **6.99** |
| `strategic_rot10` | ~3 | 3.05× | 1.31× | 23/40 (57.5%) | 4 | 1 | 1.79 |
| `strategic_rot20` | ~3 | 1.65× | **0.71×** | 21/40 (52.5%) | 3 | 2 | 1.00 |

Rotation does what it was built to do: `strategic_rot20` runs at about 70%
of the baseline's search cost with 1.65× the reviews, and holds parity
(3–2). What it does not do is beat full-width density. `strategic_r20`
remains the strongest arm measured, and `strategic_rot10` — more reviews
than `r20` at two thirds of its compute — lands behind it.

The refinement to the previous entry is worth stating precisely: the gain
is not from re-planning more often, it is from **re-evaluating the whole
lane set more often**. Narrowing the branch set trades away the thing that
made the extra reviews valuable. Cadence and width are not interchangeable
currencies.

None of this clears the gate, and every arm here is eval-only; `strategic`
is unchanged. But the pattern across nine arms now is that they are *all*
inconclusive, and that is a statement about the instrument rather than the
arms: at roughly twenty maps a run, this design cannot resolve the effect
sizes the search produces.
## 2026-07-26 — the promotion gate was the only batch runner on one core

Nine search arms were measured this session and every one returned
INCONCLUSIVE. At some point that stops being a statement about the arms.
`src/parallel.rs` exists because "benchmarks, soaks, tournaments, and
self-play all play a batch of games that share nothing but the ruleset,"
and `benchmark`, `soak`, `tournament` and `selfplay` all use it. `ai_eval`
— the paired evaluator that decides whether an AI change may ship — did
not. It played every game on one core.

That is why a run affords twenty maps. Twenty maps cannot resolve the
effect sizes this search produces, so the instrument, not the ideas, was
the limit.

`ai_eval` now takes `--jobs` (default: available parallelism) and plays its
batch through `parallel::map`, which hands results back in index order. The
fold that accumulates metrics, paired scores and censuses still runs
sequentially over that ordered list, so a parallel run reproduces a serial
one exactly:

```
$ ai_eval strategic_r20 strategic --pairs 6 --players 4 --seed 52000 \
    --turns 150 --jobs 1  > serial.txt
$ ai_eval ... --jobs 12 > parallel.txt
$ diff serial.txt parallel.txt          # no output
serial   real 142.80
parallel real  31.89
```

**4.5× on a machine already at load 17 of 18 cores**, byte-identical
output. Games are chunked `--jobs` pairs at a time so peak memory holds a
chunk of finished games rather than the whole run.

Determinism is not incidental here, it is the whole contract: every game is
fully determined by its seed and shares nothing mutable, and
`parallel_batches_match_a_serial_run` pins that the paired scores from a
threaded batch equal the serial ones.

The practical consequence: the twenty-map runs in the entries above were
never a considered choice of sample size. Re-run anything worth deciding at
a hundred maps or more.
||||||| b07a0ea

## 2026-07-26 — the cadence effect is real; two corrections to get there

With `ai_eval --jobs` making large runs affordable, `strategic_r20` was
re-measured at 120 maps on a fresh seed (70000, four players, 200 turns).
Every independent seed set run this session, pooled:

| seed | maps | maps for | maps against | sign p |
|---|---|---|---|---|
| 41000 | 12 | 1 | 0 | 1.0000 |
| 52000 | 24 | 5 | 1 | 0.2188 |
| 60000 | 60 | 6 | 4 | 0.7539 |
| 70000 | 120 | **15** | **2** | **0.0023** |
| **pooled** | **216** | **27** | **7** | **0.00082** |

At 120 maps the anytime-valid e-process reached **73.4 and crossed at map
108** (p≤0.0136). Pooled over 216 independent maps the sign test is
p=0.00082. Reviewing the victory lane every 20 turns instead of 40 is a
real improvement.

**Correction 1.** The previous entry called this a replication failure on
the strength of the 60-map run (6–4). That was an over-correction from a
single seed set, made while treating one run as decisive — the same error
in the opposite direction to the one it was correcting. Seed-set variance
here is large enough that nothing below ~100 maps should be called either
way.

**Correction 2, and the more useful one.** The terminal-score share is flat
for every arm measured this session, 48.6%–50.8%, including this one
(50.1%; direction 55 for, 59 against, p=0.7789). That looked like proof the
win margins were noise. It is not, because wins and terminal score measure
different things: **wins count victories, terminal score counts economy.**
Cadence changes how often the agent re-picks its victory lane. It does not
change the genome, the play style, or the economy — so flat score with
better wins is precisely the signature a routing improvement should leave,
not a contradiction. The right reading of a disagreement is that it
localizes the change to routing rather than development.

`ai_eval` now prints how many maps each direction rests on, and says so
when they disagree:

```
direction resolution: wins rest on 5 of 20 maps that broke, terminal score on 18
note: wins favour X and terminal score favours Y. Wins count victories and
score counts economy, so this separates victory routing from development
rather than contradicting itself
```

That line is what makes the two statistics comparable. On seed 52000 the
win direction was 5–0 and the score direction 10–8 on the *same* twenty
maps: five maps against eighteen, which is why the 5–0 looked stronger than
it was.

**On the gate.** At 120 maps the verdict is still INCONCLUSIVE, because
promotion needs the Wilson interval to clear parity as well as the
e-process, and the interval is 46.5%–64.0%. That interval treats every map
as a maximum-variance Bernoulli draw, but 103 of these 120 maps scored
exactly 0.5, so the realised per-map variance is far below the assumed
worst case and the interval is correspondingly too wide. The gate is
conservative by construction for this data shape. **That is written down
here rather than fixed**: loosening a promotion rule to admit one's own
change is the wrong way round, and the choice belongs to whoever owns the
gate.

## 2026-07-26 — the gate's conservatism is real, and has no drop-in fix

The previous entry noted that `strategic_r20` cleared the anytime-valid
e-process at 120 maps (e=73.4, crossed at map 108) while the promotion gate
still read INCONCLUSIVE, because promotion also requires the Wilson
interval to clear parity and it spanned 46.5%..64.0%. Wilson assumes every
map is a maximum-variance Bernoulli draw; 103 of those 120 maps scored
exactly 0.5, so the realised variance is about 0.05 against an assumed 0.25.

Three candidate replacements were measured by simulation against the map
shape these runs produce (400 replications, 120 maps each, null mean 0.5,
one map in five breaking evenly either way):

| interval | coverage | mean width |
|---|---|---|
| Wilson (current gate) | **400/400 (100%)** | 0.176 |
| normal, sample variance | 372/400 (93.0%) | 0.079 |
| bootstrap percentile, 2000 resamples | 374/400 (93.5%) | 0.079 |
| empirical Bernstein (Maurer–Pontil) | — | **0.321** |

Read that as three failures rather than a menu. Wilson is not 95%
conservative on this shape, it is total, and pays 2.2× the width for it.
Both variance-adaptive alternatives land slightly *under* nominal, and an
interval that undercovers a promotion gate is worse than one that
over-covers. Empirical Bernstein — the textbook non-asymptotic choice for
bounded variables — is *wider* than Wilson here, because its additive
`3 ln(3/δ)/n` term dominates until n is far larger than these runs.

So no narrower interval on offer is also calibrated, and nothing is
changed. `no_narrower_interval_here_is_also_calibrated` pins all three
measurements so the next attempt starts from them.

**What this leaves.** The e-process is already the correctly specified
instrument for this data: non-asymptotic, anytime-valid, and it does not
assume a variance it cannot see. Requiring an additional worst-case
interval alongside it is not a second layer of rigour — it is a second,
mis-specified test that the first one has to carry. Whether the gate should
turn on the e-process alone is a policy question for whoever owns the gate,
and deliberately not settled here: the agent proposing a change is the
wrong party to widen the door it wants to walk through. The evidence needed
to decide is now in one place.

## 2026-07-26 — retraction: depth works too

The entry "marginal search compute buys frequency, not depth" is **wrong**
and is retracted here. It was measured at twenty maps. Re-run at 120 on the
same seed set (70000, four players, 200 turns), now that `--jobs` makes
that affordable:

| arm | 2× compute spent on | maps for | against | decisive won | sign p | e |
|---|---|---|---|---|---|---|
| `strategic_r20` | frequency | 15 | 2 | 88% of 17 | 0.0023 | 73.4 (crossed at 108) |
| `strategic_h80` | **depth** | **21** | **5** | 81% of 26 | **0.0025** | 57.9 (crossed at 118) |
| `strategic_r10` | 4× frequency | 19 | 7 | 73% of 26 | 0.0290 | 7.1 (not crossed) |

Doubling the horizon is a real improvement of the same size as doubling the
review rate. Both cross the e-process; 88% of 17 decisive maps and 81% of
26 are not distinguishable at this power. The twenty-map table that showed
depth at 4–3 — "exactly what noise looks like" — was itself the noise.

The mechanism offered for the retracted claim ("a 40-round projection
already looks further ahead than the position stays valid, so the binding
error is a stale commitment, not a short horizon") was a plausible story
fitted to a null result. It should not have been written that confidently
from 20 maps, and it is withdrawn with the claim.

**What survives, and is now measured three times over:** the macro search
is under-provisioned. It reaches the rollouts about 1.5 times per seat per
game, and *any* doubling of its compute — more reviews, or longer
projections — wins significantly more maps. Where the doubling goes appears
not to matter much. Quadrupling does not help further: `strategic_r10` at
4× reviews is the weakest of the three, so the returns bend well before
that.

**The methodological point, now demonstrated twice.** Two conclusions
published this session from twenty-map runs have inverted at 120 maps:
`strategic_r20` was called a replication failure on 60 maps and is real at
216; `strategic_h80` was called noise on 20 maps and is significant at 120.
Twenty-map runs on this evaluator are not weak evidence, they are
anti-evidence — they produced two confident and opposite errors. The
minimum useful size here is ~100 maps, which is one `--jobs 12` run of
about half an hour. Nothing below that should be written down as a
conclusion.

## 2026-07-26 — the search-compute frontier, and a gate that declines 19,720:1

Consolidating every arm measured at adequate power. All runs are four
players, 200 turns, mirrored seat-swapped maps, against `strategic` unless
stated. "for/against" counts mirrored maps whose direction broke.

| arm | search compute | maps | for | against | sign p | e | gate |
|---|---|---|---|---|---|---|---|
| `strategic_r20` (seed 70000) | 2× reviews | 120 | 15 | 2 | 0.0023 | 73 | INCONCLUSIVE |
| `strategic_r20` (seed 80000, pre-registered) | 2× reviews | **400** | 46 | 12 | 0.0000 | **19,720** | INCONCLUSIVE |
| `strategic_h80` (seed 70000) | 2× horizon | 120 | 21 | 5 | 0.0025 | 58 | INCONCLUSIVE |
| `strategic_r10` (seed 70000) | 4× reviews | 120 | 19 | 7 | 0.0290 | 7 | INCONCLUSIVE |
| `strategic_r20h80` (seed 70000) | 2× both | 120 | 29 | 7 | 0.0003 | 335 | **PASS** |
| `strategic_r20h80` (seed 90000) | 2× both | 120 | 24 | 8 | 0.0070 | 42 | INCONCLUSIVE |

Three things follow.

**The two doublings stack.** Reviews alone and horizon alone each land near
55–57%; together they reach 57–59%. `strategic_r10`, which spends the same
4× on frequency alone, is the weakest of the lot — so it is the *product*
of the axes that pays, not the total.

**The PASS did not replicate.** It cleared at seed 70000 because that run
measured 59.2% and Wilson needs about 58.9% at n=120; a 0.2-point margin.
The second seed set reproduced the *effect* (24–8, p=0.0070, e-process
crossed) and not the verdict. Pooled over both disjoint sets the effect is
240 maps, **53 for, 15 against, sign p=4.1e-06** — unambiguous. The
configuration is therefore added as evaluator-only `strategic_deep`, and
deliberately not promoted.

**The gate declines evidence of 19,720:1.** The pre-registered
`strategic_r20` run at 400 maps is the cleanest statement of the problem
recorded in the 2026-07-26 interval entry: 46 map directions to 12, an
e-process at 1.97e4 that crossed at map 140, and still INCONCLUSIVE,
because the Wilson lower bound sits at 49.4%. At a 54.2% effect that bound
needs roughly 540 maps. The gate is not weighing the evidence and finding
it thin; it is applying a second, fixed-n criterion that the evidence
cannot influence. That run was pre-registered at n=400 with the decision
rule fixed in advance, and it is reported here as the failure it was.

What none of this changes: `strategic` is untouched, and no promotion has
been taken. What it should change is whoever owns the gate deciding whether
a criterion that rejects 19,720:1 is the one they want.
## 2026-07-26 — searching what the city builds makes it worse

Rollout search over victory lanes is the one search in this codebase that
measurably wins games, so the obvious next target was the decision that
compounds most: what a city builds. `ProductionSearchAi` commits a build by
projection before delegating to the scripted governor, which is possible
because the governor begins `if !g.cities[cid].queue.is_empty() { continue;
}` and leaves a filled queue alone. Searches are rate-limited to one city
every fifteen turns, putting the whole feature in the same cost class as
the lane search.

It is worse than the governor, at adequate power:

`ai_eval production advanced --pairs 120 --players 4 --seed 70000
--turns 200` → 108/240 games (45.0%), **9 mirrored maps for, 21 against**,
exact sign p=0.0428, significant *in the incumbent's favour*. Terminal
score share 49.7%.

The search was not inert — that was checked first, with the diagnostic that
should have existed before the doctrine axis was built. Candidate values
spread by 0.0057 at the median against a 0.002 commitment margin, with 1 of
6 sampled decisions degenerate, and the agent displaced the governor's pick
on a real fraction of its searches. It searched, it acted, and it lost.

**The likely cause is the horizon, and it generalizes.** A lane's effect is
visible in score share inside the projection: committing to Science changes
what the empire does immediately. A building's payoff compounds over the
century *after* it completes, while this rollout stops ten rounds past the
slowest candidate, capped at forty. Inside that window a cheap unit adding
score now beats infrastructure that pays later, so the search overrides the
governor toward the myopic choice and discards exactly the long-horizon
sequencing — reserved Spaceports, district families, wonder timing — that
the scripted layer is good at.

That is `PolicyAi`'s failure one level up. There the evaluator could not
see the action at all; here it can see the action but not its payoff.
Both are evaluator/decision mismatches, and the pattern across this
session is that every inert or harmful learned component has been one:

| component | mismatch |
|---|---|
| `PolicyAi` tactical | empire features cannot change under a unit move |
| `strategic` value net | blended estimate never flips a lane argmax |
| `Doctrine` axis | commitment margin exceeded the value spread |
| `ProductionSearchAi` | horizon shorter than the decision's payoff |

A future attempt at production search needs a terminal value that credits
unfinished compounding — a trained value net, or continuing the branch to a
real result the way `counterfactual_value_samples` already does — rather
than a longer fixed horizon, which only moves the cliff.

## 2026-07-26 — a value net over the same features is not a second opinion

The previous entry blamed the production search's loss on its horizon: a
building's payoff arrives after the projection ends, so the terminal value
cannot see it. The stated fix was a value function, which is precisely the
tool for estimating what happens beyond a horizon. `production_net` is that
agent with the trained net as its terminal evaluator instead of score
share, run on the same 120 maps:

| variant | terminal value | games | maps for | against | sign p |
|---|---|---|---|---|---|
| `production` | score share | 108/240 (45.0%) | 9 | 21 | 0.0428 |
| `production_net` | trained value net | 109/240 (45.4%) | 9 | 20 | 0.0614 |

**The hypothesis is wrong.** One game changed hands.

The reason is more useful than the fix would have been. The net's inputs
*are* `evolve::features` — the 25 empire aggregates — and Civilization
score is one of them, next to the yields and population that generate it.
A win probability regressed on those numbers is a re-weighting of what
score share already reports, not an independent judgement. It cannot tell
an empire whose infrastructure is about to compound from one with the same
current yields, because nothing in its input distinguishes them.
Substituting it therefore cannot repair an information problem, however
well calibrated it is — and it is well calibrated: grouped-holdout BCE
0.4058 against a 0.5636 constant baseline, ECE 0.0355.

That is the same root cause as `PolicyAi` and the inert lane-value blend,
now a third time, and it sharpens the table from the previous entry. Three
of those four rows are not four problems but one: **the learned evaluator
is a function of 25 empire scalars, and every decision it has been asked to
rank is invisible in them.** The fourth row, the doctrine axis, is the only
one that was a tuning error.

What this rules out, concretely: training a better net on
`evolve::features` cannot help any of these agents, so the training loop is
not the bottleneck and neither is data volume. What it points at is
`obs_tensor` — the 25 fog-honest spatial planes that already exist and that
no Rust agent can consume, because `tools/train_spatial.py` writes a
PyTorch checkpoint and nothing loads it. That gap, not calibration, is what
stands between this codebase and a learned component that changes a
decision.

The scripted governor remains the better production policy, and both
entrants stay eval-only.

## 2026-07-26 — features that respond to the decision being ranked

Three of this session's four failed learned components share one cause: the
evaluator's inputs do not change when the action is taken. That is a
measurable property, and nobody had measured it. Over 37,900 candidate
actions across three four-player games, comparing `evolve::features`
against a 34-wide extension (`decision_features`):

| action kind | n | `evolve::features` | extended |
|---|---|---|---|
| `attack` | 136 | 100.0% | 100.0% |
| `move` | 6,842 | **2.1%** | **69.2%** |
| `produce` | 11,869 | **11.6%** | **91.5%** |
| `fortify` | 1,170 | 0.0% | **100%** |
| `ranged` | 57 | 0.0% | **100%** |
| `city_strike` | 18 | 0.0% | **100%** |
| `research` | 316 | 0.0% | **100%** |
| `slot_policy` | 272 | 0.0% | 0.0% |
| **total** | **37,900** | **44.5%** | **86.1%** |

The nine added terms are cheap scalars, not a tensor: HP-weighted material
for both sides, wounded and fortified counts, adjacency count and mean gap
to the nearest enemy, building and district counts, and total queued
production cost. Each was added to close a specific measured hole —
`fortify` scored zero until fortification was represented, `ranged` until
enemy health was, and `produce` sat at 11.6% because the queue's *length*
rarely changes when one item replaces another, while its *cost* does.

Two design notes. The original vector is preserved as a prefix, so a model
trained on 25 and one trained on 34 differ in exactly one thing. And these
are scalars deliberately: `obs_tensor` already renders 25 fog-honest
spatial planes that no Rust agent can consume, because
`tools/train_spatial.py` writes a PyTorch checkpoint nothing loads, so a
plane-based evaluator is blocked on inference machinery that does not
exist. This is not.

**What this is not.** Visibility is necessary, not sufficient: a feature
that moves under an action does not thereby rank actions correctly. The
claim established is the stronger negative one — that the previous set made
correct ranking *impossible* for the decisions these agents were asked to
make. No agent uses this yet and no net is trained on it; `ValueNet`
hard-validates a 25-wide input, which has to be generalized first. Policy
cards and diplomacy remain invisible at 0%, since nothing here encodes slot
contents.

`the_decisions_that_were_invisible_are_visible` pins the measurement with
thresholds set well below what was observed and far above what
`evolve::features` reaches, so the property is a regression test rather
than a number in a document.

## 2026-07-26 — freeing the frozen armies changes the behaviour and not the result

`plan.threatened_city` is an empire-wide fact and `force_orders` sent every
force group to `Hold` whenever any city anywhere was under pressure. The
shipped census reported 61% of holds as threat-held, but it counted a hold
that way whenever the flag was *set*, including groups below the
local-superiority floor that would have held regardless. Attributed to the
disjunct that actually fired, the causal share is **34%**.

Measured over 6,568 force-group turns in eight six-player games (74x46, 250
turns, seeds 7000-7007), those holds were not a defensive posture at all:

- `Hold` was **56.4%** of all force-group turns
- **33.6% of holds** were groups that cleared the superiority floor and were
  stopped only by the global flag — **19.0% of all group turns**
- they stood a mean **13.2 hexes** from the emergency, and **83.1%** were
  outside the six-hex radius that defines the threat, so they could not have
  affected it whatever they did
- **4.4 units** frozen per order

Scoping the hold to groups that could plausibly arrive (the six-hex radius
plus three turns of march at the slowest member's pace) does exactly what it
was designed to do. Same eight seeds:

| | before | after |
|---|---|---|
| `Hold` | 56.4% | 50.7% |
| `Advance` | 17% | 23.5% |
| holds by a group strong enough to advance | 19.0% | **10.4%** |
| their mean distance from the emergency | 13.2 | **8.8** |

**And it bought nothing.** Pre-registered before the run — challenger,
control, settings, n and decision rule fixed in writing — `ai_eval
advanced_relief_scoped advanced --pairs 120 --players 4 --width 60 --height
38 --city-states 6 --turns 500 --seed 220000`:

- 118/240 games (49.2%) for the scoped hold, paired-map score 49.2%
- 14 map directions for, 16 against, exact sign **p=0.8555**
- Wilson **40.4%..58.0%**, Elo-equivalent **-6** (CI -68..+85)
- anytime e-process peak 1.00 for the treatment; not crossed
- **`promotion gate: INCONCLUSIVE`**

60x38 rather than the 24x16 default because the default map's maximum
separation is below the relief radius, which would have made treatment and
control nearly the same agent; 500 turns because a cap changes which agent
wins (#282/#285) and this is a military change that pays late.

The reading that survives is that **mobility was not the binding
constraint.** An army freed to march still arrives with 81% of its units
three or more eras stale and converts a spent garrison into a capture 22% of
the time. Freeing 9% of force-group turns to advance moves nothing while
what they advance *into* is unchanged.

So the behaviour stays behind `AdvancedAi::scoped_relief_hold`, off by
default, reachable as the `advanced_relief_scoped` entrant. It is worth
re-running, unchanged, once siege conversion moves — which is the point of
keeping it rather than reverting it. The census attribution fix lands
regardless: it was reporting 61% where the truth is 34%, and that number is
what made this look like the biggest available lever in the first place.


## 2026-07-26 — strategic_deep promoted on a pre-registered 300-map run

`strategic_r20h80` passed the gate once at 120 maps by 0.2 points of Wilson
bound and failed to pass again on a second seed set, so it was added as an
evaluator-only entrant and explicitly not promoted. The deciding run was
pre-registered before it started — challenger, incumbent, settings, sample
size and decision rule fixed in writing — because the gate's Wilson
interval is a fixed-n statistic and stopping when it happens to clear would
be optional stopping on a statistic that does not permit it. n=300 came
from a power calculation on the pooled effect, not from watching the run.

`ai_eval strategic_r20h80 strategic --pairs 300 --players 4 --seed 100000
--turns 200`:

- 339/600 games (56.5%), paired-map score 56.5%
- **56 mirrored maps for, 17 against**, exact sign p=0.0000
- anytime e-process **3.14e4**, crossing at map 127
- Wilson 50.8%..62.0% — clears parity
- Elo-equivalent **+45** (CI +6..+85)
- **`promotion gate: PASS`**, under the unmodified gate

Across all three disjoint seed sets: **540 independent maps, 109 map
directions to 32.** `strategic_deep` is therefore promoted from
evaluator-only to a builtin agent.

Two things it does not change. `strategic` keeps its original settings and
is not deprecated: it is the frozen control for measuring further search
work, the way `advanced_v1` is for `advanced`. And the promotion costs four
times the macro-search compute, so batch callers — soak, league, the
spectator fleet — should adopt it deliberately rather than inherit it.

For the record, alongside it: the pre-registered `strategic_r20` attempt at
n=400 **failed** under the same rules, reaching an e-process of 1.97e4 with
46 map directions to 12 and still reading INCONCLUSIVE, because its smaller
54.2% effect needs roughly 540 maps to move the Wilson bound. Both
pre-registrations are reported; only one succeeded.

## 2026-07-26 — the value net can now train on features that see the decision

`decision_features` measured 86.1% action visibility against
`evolve::features`' 44.5%, but nothing could train on it: `ValueNet`
hard-validated a 25-wide input and both the exporter and the Python trainer
had the width baked in. This removes that.

- `ValueNet::valid` now checks structure rather than a literal: four
  layers, a pinned `[..., 64, 32, 1]` hidden shape so the Rust evaluator
  and the trainer cannot disagree, and a free input width.
- `ValueNet::input_width` and `ValueNet::load_width(dir, width)` resolve
  the mismatch **at load time**. A directory can now hold either artifact,
  and an agent that evaluated the wrong one would return numbers rather
  than an error — the failure mode this codebase has spent the session
  removing. Every existing consumer now asks for `evolve::FEATURE_WIDTH`
  explicitly, so a 34-wide net dropped into `evolved/` is refused by the
  25-wide agents instead of being silently mis-evaluated. `eval` panics on
  a width mismatch, because reaching it means the load-time check was
  skipped.
- `civvis selfplay --decision-features` records the 34-wide vector;
  `tools/train_valuenet.py` reads the width from the corpus instead of
  assuming it, and warns if a file mixes widths.

Verified end to end: a three-game smoke wrote 36 columns (34 features, a
label, a game index) and the trainer reports the width it inferred.

This is plumbing, deliberately shipped without a strength claim. The
question it exists to answer — whether an evaluator that can see the
decision changes one — needs a matched corpus and a trained artifact, and
those are the next step, not this entry. Note in advance what a fair
comparison requires: the 25-wide and 34-wide corpora must come from the
same games and settings, or the comparison confounds representation with
sampling.

## 2026-07-26 — wider features do not predict better, and that is not the point

With the width plumbing in place, a 34-wide corpus was exported and trained,
and — because the earlier 25-wide corpus predates several rules PRs — a
matched 25-wide corpus was regenerated on the same commit, same seed, same
settings. Identical games, identical splits (15,578 rows, 144/48/48 games):

| features | test BCE | Brier | ECE |
|---|---|---|---|
| 25-wide `evolve::features` | **0.4529** | 0.1477 | 0.0164 |
| 34-wide `decision_features` | 0.4655 | 0.1475 | 0.0199 |

**The wider net predicts slightly worse.** Brier is a wash, calibration is a
wash, and cross-entropy is marginally against it — plausibly 34 inputs
fitting the same 64→32 hidden layers on the same 9,343 training rows.

Read carefully, because the obvious reading is wrong. Global predictive
accuracy is **not** what the extra terms are for, and improving it was never
the claim. The 25-wide vector returns *literally the same number* for 96% of
the candidate actions a tactical agent clones, so it cannot rank them at any
accuracy; the 34-wide vector moves for 69% of unit moves. A net can be a
marginally worse global predictor while being the only one of the two
capable of ordering the choices an agent actually faces.

That distinction is worth stating plainly because a reviewer checking BCE
alone would conclude the representation work failed, and a reviewer checking
only the visibility table would conclude it succeeded. Neither is the
question. The question is whether the agent plays better, and `policy_wide`
— `PolicyAi` scoring with `decision_features` and a net trained on it — is
the entrant that answers it.

Two defects were caught while wiring this, both by guards added earlier in
the session, and both worth recording because they are the failure modes
this work exists to prevent.

`ValueNet::eval`'s width assertion fired across six tests as soon as a
34-wide net was placed in `evolved/`: the previous change had shipped
`load_width` but wired only some of its callers, so `strategic` and
`production` still loaded any net they found and fed it 25 features. The
assertion turned a silent mis-evaluation into a loud failure, which is
exactly what it is for; all consumers now name the width they feed.

And `policy_wide` initially fell through to the provenance catch-all and
announced "scripted, no artifacts required" while quietly depending on a
34-wide net. That is the precise failure this repository's
provenance layer exists to prevent, so the coverage test now asserts that
only the four genuinely scripted agents may report no artifacts.

## 2026-07-26 — the blindness was protecting it

`policy_wide` is `PolicyAi` scoring with the 34-wide `decision_features`
and a net trained on it: the configuration the whole representation thread
was building toward, and the first in which the tactical evaluator can tell
its candidate actions apart.

`ai_eval policy_wide advanced --pairs 120 --players 4 --seed 70000
--turns 200`:

| entrant | games | maps for | against | Elo-equivalent |
|---|---|---|---|---|
| `policy` (25-wide, blind) | 108/240 (45.0%) | 9 | 21 | −35 |
| `policy_wide` (34-wide, sighted) | **34/240 (14.2%)** | **1** | **87** | **−313** |

Sight made it catastrophically worse, and the reason is the most useful
thing this line of work has produced.

With the 25-wide vector the computed gain is exactly zero on 96% of
candidates, so the agent cannot clear its commitment margin, declines to
act, and falls through to the scripted layer. That is why it measured near
parity: it was mostly *not playing*. The wider vector lets it distinguish
candidates, so it commits — and its ranking is far worse than the scripted
doctrine it displaces. **The blindness was not why it failed to win. It was
why it failed to lose.**

The mechanism is standard and worth naming rather than rediscovering. The
net is trained on states that `advanced` self-play visits. Greedily
maximising it one ply at a time walks immediately off that distribution,
into positions the training data never covered, where the estimate carries
no information — and the argmax is precisely the point where the estimate
is most likely to be wrong, because it selects for optimistic error. A
better-calibrated net does not fix this; the earlier measurement that the
34-wide net's BCE is *worse* than the 25-wide one's is irrelevant to it
either way.

What this rules in, for the next attempt:

- **Iterated self-play.** Retrain on states the agent itself visits, so the
  distribution follows the policy.
- **Staying near the data.** Constrain the learned policy to the
  neighbourhood of the scripted one that generated the corpus, rather than
  letting it maximise freely.
- **Search instead of a greedy argmax.** Rollouts evaluate a commitment by
  playing it out, which is why the macro search is the one search here that
  wins games — it never trusts a point estimate off-distribution.

What it rules out: widening the input further. Representation was a real
and necessary fix — 44.5% to 86.1% action visibility, measured — and it was
not the binding constraint on strength. Both facts are now established, and
this entry exists so the next attempt starts from the second one.

`policy` and `policy_wide` both remain eval-only; no default changed.

## 2026-07-26 — it maximises a symptom (correcting the previous mechanism)

The previous entry explained `policy_wide`'s collapse as an
off-distribution failure: a net trained on `advanced`-visited states,
greedily maximised, walks somewhere its estimate is meaningless. That
explanation is **wrong**, and testing it was much cheaper than the
retraining loop it recommended.

Two 8-game corpora on identical seeds, one walked by `advanced` and one by
`policy_wide`, scored against the same net:

| corpus | rows | BCE | Brier | ECE |
|---|---|---|---|---|
| expert-visited states | 320 | 0.3720 | 0.1146 | 0.1034 |
| learner-visited states | 320 | 0.3898 | 0.1222 | **0.0643** |

Both far beat the 0.5623 constant baseline, and the learner states are if
anything *better* calibrated. The estimate is fine where the agent goes.

The real mechanism is subtler and more general. The net is fit to outcomes,
so it encodes **correlation**, and an argmax over sibling actions optimises
whichever correlate is cheapest to move. Over 468 committed decisions:

| per-decision delta | chosen action | average legal candidate |
|---|---|---|
| adjacent enemies | **+0.13675** | +0.00176 |
| mean gap to nearest enemy | −0.00663 | +0.00123 |
| own HP-weighted material | **−0.00054** | — |

The agent drives units into contact at **seventy-eight times the rate of
the average legal move**, closing distance where the field opens it, and
loses material doing so. In games `advanced` wins, units stand in contact
because a strong empire is pressing an attack: contact is a *symptom* of
strength, not a cause of it. Maximising a symptom marches units into fights
they lose.

This is why accuracy was never going to help, and it explains the whole
shape of the session in one line: **a state-value function tells you how
good a position is, and says nothing about which action caused it.** The
macro search wins games because a rollout is counterfactual — it plays the
action out and observes the consequence. The one-ply value delta is
correlational, and correlational action selection is not merely weak, it is
actively harmful: `policy` at 45% was declining to act; `policy_wide` at
14.2% is acting on it.

Corrected recommendation, replacing the previous entry's. Iterated
self-play does not address this, because the correlation survives
retraining. What the learned route needs is **action-conditioned value** —
Q or advantage, trained on returns for actions actually taken — not a
state-value regression read greedily. The self-play loop is still required,
but as the thing that generates action-conditioned returns rather than as a
distribution fix.

## 2026-07-26 — freeze the symptom: a causal test, and a design rule

The previous entry blamed `policy_wide`'s collapse on the net's contact
terms: an argmax optimising a *symptom* of strength rather than a cause.
That was inferred from a correlation between chosen actions and one
feature, and this session has already retracted two mechanisms inferred
that way. So it was tested by denying the agent that specific symptom —
`policy_wide_frozen` holds the two contact terms at their pre-action values
while scoring candidates, so the net cannot reward an action for moving
them. Nothing else changes: same net, same features, same everything.

Same 120 maps as the collapse:

| variant | games | maps for | against | Elo-equivalent |
|---|---|---|---|---|
| `policy` (25-wide, blind) | 108/240 (45.0%) | 9 | 21 | −35 |
| `policy_wide` (contact free) | 34/240 (14.2%) | 1 | 87 | **−313** |
| `policy_wide_frozen` (contact frozen) | **120/240 (50.0%)** | 16 | 16 | **0** |

**Two features accounted for the entire collapse.** Denying them recovers
−313 Elo to exact parity, and the recovered agent is better than the blind
one it started as (50.0% against 45.0%) while genuinely acting — 16 maps
won against 9, on 32 maps that broke against 30.

That confirms the mechanism causally rather than by association, and it
yields a design rule the earlier entries missed:

> **A feature that makes a decision visible can simultaneously make it
> exploitable. In any feature set consumed by an argmax over actions, every
> feature must be one you would be content for the agent to maximise.**

The 34-wide vector was designed for *visibility* — measured, and correct on
its own terms: action visibility 44.5% → 86.1%. But visibility and safety
are different properties, and the terms divide cleanly along that line.
Material, HP, fortification and city fabric are **causal**: more of them is
genuinely better, and an agent that maximises them is doing something
sensible. Adjacency and gap are **correlational**: they are high in won
games because a strong empire presses attacks, and an agent that maximises
them charges into fights it loses. The visibility work needed both kinds;
the ranking work can only survive the first.

This is the cheapest available statement of what an action-conditioned
value would buy. Q or advantage learns the return of *taking* the action,
so a move into a losing fight is scored by what it costs, not by what it
resembles. Until that exists, a feature audited only for visibility is not
safe to hand to an argmax — and `policy_wide` is left in the tree as the
demonstration.

Both variants remain eval-only, and no default changed.

## 2026-07-26 — an audit for what the argmax exploits

The rule from the previous entry — that every feature handed to an argmax
must be one you would be content for the agent to maximise — is only useful
if someone remembers it. `PolicyAi::feature_pressure` makes it measurable:
for each feature it reports the mean change the *chosen* action makes,
beside the mean change an average legal candidate makes. A feature the
policy is exploiting shows a chosen-delta far above the field's.

That ratio is exactly how `policy_wide`'s collapse was found — contact at
0.137 against the field's 0.002, seventy-eight times, while the agent lost
material and 86% of its games. None of the instruments already in the
repository would have shown it: the win rate said "bad" without saying
why, the calibration check said the net was accurate, and the visibility
table said the features were working as designed. All three were correct.

`the_audit_detects_a_feature_the_argmax_exploits` pins it as a regression
check: with the contact terms free the audit must flag them, and with them
frozen it must not. A future feature set can be run through the same
measurement before it is wired to an argmax rather than after it loses 300
Elo.

The audit reports a question, not a verdict. Heavy pressure on a *causal*
feature is the policy working — an agent that pushes its own material up is
doing what it should. Heavy pressure on a *correlational* one is the
failure. Telling those apart still needs judgement about the game; what
this removes is the need to first suspect that something is wrong.

## 2026-07-26 — the measured agent ladder

`strategic_deep` was promoted on evidence gathered entirely against
`strategic`. The fleet's default major agent is `advanced`, so the
comparison an operator actually needs was missing. It is now measured, on
the same seed set and settings as everything else (four players, 200
turns, 120 mirrored maps, seed 70000):

| pairing | games | maps for | against | sign p | e | gate |
|---|---|---|---|---|---|---|
| `strategic_deep` > `advanced` | 136/240 (56.7%) | **22** | 6 | **0.0037** | 89 (crossed at 112) | INCONCLUSIVE |
| `strategic_deep` > `strategic` | 142/240 (59.2%) | 29 | 7 | 0.0003 | 335 (crossed at 91) | PASS |
| `strategic` > `advanced` | 130/240 (54.2%) | 20 | 10 | 0.0987 | 2.9 | INCONCLUSIVE |

Plus the pre-registered 300-map run that carried the promotion:
`strategic_deep` over `strategic`, 339/600 games, 56 maps to 17, sign
p=0.0000, e=3.14e4, `promotion gate: PASS`.

The ordering is consistent across every pairing: **`strategic_deep` >
`strategic` > `advanced`**, and the promoted agent beats the scripted
default by sign test and by anytime-valid evidence. The gate still reads
INCONCLUSIVE against `advanced` for the reason established earlier — at a
56.7% effect the Wilson bound needs roughly 300 maps, and this run is 120.

Two things worth taking from the table beyond the ranking.

**`strategic` has never been shown to beat `advanced` at adequate power.**
It leans ahead at 20 maps to 10, but p=0.0987 and an e-value of 2.9 are not
a result, and the 41.7%-at-four-players figure in older entries predates
every instrument fix made since. Anyone treating the existing search agent
as a known improvement over the scripted one is relying on numbers that
were never resolvable.

**Terminal score is 49.1% here too**, as it has been for every arm this
session. The promoted agent wins more games without out-scoring anyone,
which is exactly the signature of a change to victory-lane routing rather
than to economy — the same reading the direction-resolution line was added
to make legible.

## 2026-07-26 — action-conditioned value from expert logs inherits the confound

The corrected recommendation two entries ago was action-conditioned value —
Q or advantage on returns for actions actually taken — on the grounds that
it scores a move into a losing fight by what it costs rather than by what
it resembles. That reasoning has a hole, and this session's record on
unmeasured reasoning is three retractions, so it was measured before anyone
built on it.

A Q fit to logged play can only order actions the log contains. Over 480
expert decision points:

| quantity | value |
|---|---|
| legal move candidates offered per decision | 25.2 |
| of those, candidates that raise contact | 3.9 (15%) |
| expert turns that raised contact | **36%** of decisions |
| share of offered candidates that can appear as a taken action | **≤ 3.96%** |

Two things follow, and they point the same way.

**Coverage is about four percent.** Roughly 96% of the actions a learner
would consider never appear in the log at all, so Q has no signal for them
and must extrapolate — which is the situation that produced the −313 result
in the first place, moved from states to actions.

**The expert takes contact-raising moves at more than twice the base
rate** — 36% of its turns against 15% of candidates. So the log does not
say "contact is bad"; it says contact-taking accompanies good play, because
the scripted agent enters contact when the fight is favourable and declines
when it is not. A Q fit to that learns the same association the state-value
net learned. It is the *declined* moves — contact in unfavourable positions
— that carry the corrective signal, and by construction they are the ones
the log does not contain.

So action-conditioned value is not by itself the fix. What distinguishes a
good contact move from a bad one is absent from any purely observational
corpus of a competent agent, however it is labelled. Getting it requires
data that contains the bad version of the action: **on-policy exploration**
that actually takes the losing move and records the loss, or **search**
that plays the candidate out counterfactually instead of recognising it.

That is the same conclusion the whole session keeps arriving at from
different directions, and it is now measured rather than argued: the macro
search wins games because a rollout is an experiment, and every learned
component that failed was reading a correlation.

## 2026-07-26 — the horizon has a ceiling, and the promoted agent is near it

Doubling the search budget is the one change promoted this session, so the
obvious follow-up is to retest the null results at the larger budget — the
doctrine axis first, since a longer projection should let play styles
separate. A probe answered that before an evaluation was spent on it, and
the answer is no, for a reason that matters more than the retest would
have.

Spread between the four doctrine branches at one lane, three seeds, three
sampled reviews each, four-player 200-turn games:

| horizon | median spread | max spread | reviews where every branch is decided |
|---|---|---|---|
| 40 | 0.00451 | 0.01454 | 22% |
| 80 | **0.00000** | 0.06162 | **56%** |
| 120 | **0.00000** | 0.15994 | **89%** |

The median collapses to zero while the maximum grows tenfold. That is
saturation, not noise: a branch that reaches a decided game returns exactly
1.0 or 0.0, so once every branch resolves inside the horizon they agree **by
construction**, and the search is blind however good its evaluator is. At
horizon 120, 89% of reviews are in that state.

Three consequences.

**The doctrine axis should not be retested at the promoted budget.** It
would act less often, not more — the retest is answered.

**`strategic_deep` is already over half saturated.** It projects 80 rounds,
where 56% of these reviews are decided before the horizon ends. It wins
anyway, and the promotion stands on 540 maps, but the natural next move —
push the horizon further — is measured to be a dead end. The compute lever
that has paid all session has a ceiling, and it is close.

**The signal is bimodal, which suggests the fix.** Saturated reviews carry
nothing; unsaturated ones carry *more* at longer horizons, with the maximum
spread rising from 0.015 to 0.160. A fixed horizon averages those two
regimes together. An adaptive one — project until the branches separate or
the game decides, rather than for a fixed count — would spend rounds only
where they still buy discrimination. That is the first concrete improvement
to the macro search this session has identified that is not simply more
compute, and it is untested.

The threshold scales with how much game remains, so these percentages are
specific to 200-turn four-player games and will bite later in longer ones.

## 2026-07-26 — ★ PROMOTED: branches project from the plan in force

`StrategicAi` handed every branch of its macro search a **freshly
constructed** `AdvancedAi`. That discards the strategic plan, settler and
builder assignments, `major_war_since`, the `peace_until` cooldown and the
whole force-group table — so the projection answered *"what happens if I
restart my planner and commit to this lane"* while standing in for *"what
happens if I commit to this lane from here."*

`branch_agent` now clones the agent in force and applies the branch's
decision through the same three calls `take_turn` makes after a review
(`retarget` / `adapt` / `reweight`), and only when they would change
something. All three preserve campaign and unit-role memory by contract and
drop only the plan, so a branch re-assesses under its new lane without
amnesia. The counterfactual becomes an exact simulation of the decision being
considered.

**Pre-registered confirmation, fresh seed 132000, `--players 4 --pairs 500
--turns 200`:**

```
game-win share: strategic_warm 553/1000 (55.3%)  strategic 447/1000 (44.7%)
paired-map score: 55.3% (95% Wilson CI 50.9%..59.6%), Elo-equivalent +37 (CI +6..+68)
paired direction: warm 87, neutral 379, strategic 34; sign p=0.0000
anytime evidence: warm peak e=6.623e4, crossed at map 209; strategic peak e=1.000e0
terminal-score direction: warm 271, neutral 23, strategic 206; sign p=0.0033
promotion gate: PASS
```

Three disjoint seed sets:

| seed | maps | for | against | sign p |
|---|---|---|---|---|
| 130000 | 120 | 17 | 10 | 0.2478 |
| 131000 | 240 | 36 | 14 | 0.0026 |
| 132000 | 500 | 87 | 34 | 0.0000 |
| **pooled** | **860** | **140** | **58** | **5.2e-09** |

**On by default, unlike `strategic_deep`.** That one costs 4× the macro-search
compute on every game and therefore had to be opt-in; this costs one clone of
a small struct where there was one construction of it, so every caller gets
it — `strategic`, `strategic_deep`, `strategic_score`, soak, the fleet and the
exhibition. `strategic_cold` is the frozen control that keeps every published
pre-promotion `strategic` number reproducible. The league is unaffected: it
rates `Weights` genomes over `AdvancedAi` and never constructs a `StrategicAi`.

**What moved was routing, not the economy.** The promoted agent took 32
domination seats against the control's 51, and 148 religious against 112,
while terminal score moved 0.8 points. Domination converts at 3–8%, the worst
lane on the board. This also refutes the mechanism originally proposed for the
change — that a cold branch finds an empty force-group table and therefore
*under*-projects the militarised lanes, which predicted more domination, not
less. The run cannot separate which piece of retained state is responsible.

### Two negative results that bracket it

Both were pre-registered, both failed, and together they close a family of
ideas rather than one idea.

| change | what it does to a review | result |
|---|---|---|
| `strategic_h80` | 2× depth on **every** branch | 21 map directions to 5, p=0.0025 |
| `adaptive_horizon` | stop as soon as the branches separate | 39.2%, Elo −76, p=0.0000 |
| `focused_deepening` | same budget, concentrated on the leaders | 49.2%, p=0.8318 |

Depth on all branches wins; depth on the branches that look best does nothing;
stopping when they separate loses badly. **A shallow estimate is not
rank-preserving with respect to the deep one** — a lane behind the adaptive
baseline at depth 12 can be the best lane at depth 84 — so any within-review
pruning discards real signal. That retires rotation, adaptive stopping,
focused deepening, progressive widening and sequential halving together. The
only lever that has ever worked on this search is raising the total.

`focused_deepening`'s first run is worth recording separately because its
headline was misleading. It scored 46.2% while **terminal score was a dead
heat** (49.4%, 56 map directions to 57, p=1.0000): it built an equally good
empire and stopped committing — 58.4% of player-turns uncommitted against the
control's 44.9%, religious commitment 24.5% against 29.0%. Cause: a review is
a *maximum over the surviving lanes*, and a maximum over one draw clears a
fixed margin far less often than a maximum over six. **The commitment margin
is calibrated to the size of the candidate set the argmax ranges over**, so
any change to that set silently moves the decision threshold. That is also a
mechanism for `rotate_lanes` measuring null. Read the plan-commitment table
before the win rate whenever a treatment touches the candidate set.

See `docs/SUPERHUMAN.md` for the design reading these three runs support.

## 2026-07-26 — branch spread: the mechanism behind the promotion, and a population correction

The promotion above was argued from a mechanism that the numbers then
contradicted — a cold branch reaching for the army finds an empty force-group
table, so the militarised lanes should be *under*-projected and the promoted
agent should take *more* domination. It takes less. This is the measured
mechanism instead.

**Method.** Play a `StrategicAi` seat forward to a real mid-game position,
skip the ones a prior would answer, then call `rollout()` for the adaptive
baseline and every enabled lane exactly as `review` does, and record
`max − min` over those values. 16–18 positions per configuration.

| config | median spread | max | share above the 0.01 margin |
|---|---|---|---|
| 4p 24×16, cold branches | 0.0311 | 0.73 | 61% |
| 4p 24×16, **warm branches** | **0.0622** | 0.75 | **78%** |
| 3p 20×14, cold branches | 0.0491 | 0.62 | 94% |
| 3p 20×14, **warm branches** | **0.0850** | 0.72 | 94% |

**Projecting from the plan in force roughly doubles the spread between branch
values.** The counterfactual discriminates about twice as well between lanes,
because a cold branch's first act is to re-plan, which partially washes out
the very difference the branch exists to measure. That is a mechanism for +37
Elo that does not require any claim about which lane benefits.

> ### ⚠ RETRACTED the same day — the measurement above is unpaired
>
> The table compares two arms that **played different games**. A warm agent
> and a cold agent diverge from their first review, so they arrive at
> different positions, and the comparison confounds the treatment with the
> positions the treatment steers into. Re-measured properly by
> `search_probe`, which flips the flag on **one agent at one position** and so
> is paired:
>
> ```
> 57 of 120 positions reached the rollouts (4p 24x16, warmup 60)
>                       median      p90      max  >margin  decided  would commit
>   warm (stock)        0.0550   0.5838   0.7205      61%      43%          28%
>   cold (treatment)    0.0405   0.6077   0.7223      60%      46%          21%
> paired: cold spread higher on 17, lower on 20, identical on 20; sign p=0.7428
> ```
>
> **The spread difference is a coin flip.** The medians still differ in the
> same direction, which is exactly how an unpaired comparison misleads: a real
> per-position effect of zero can show a large difference in marginal medians.
>
> **What replaces it.** The two configurations **decide differently on 14 of
> 57 positions — one review in four** — while the dispersion distribution is
> unchanged. Fidelity does not sharpen the search's resolution; it moves its
> answer. The honest mechanism for +37 Elo is that a faithful counterfactual
> picks a different lane about a quarter of the time and those picks are
> better. No dispersion claim survives, and neither does the force-group story
> this entry was written to replace.
>
> The flip direction is 5 toward a lane against 9 toward adaptive (p=0.4240),
> so not even the *sign* of the routing change is resolved at this sample.
> Which lane benefits remains unexplained; only that the promotion is real
> (860 maps, sign p=5.2e-09) and that it works by changing decisions.

### Population correction: 0.0045 and 0.031 are both right

The horizon-saturation entry records a median branch spread of **0.00451** at
horizon 40 with a **maximum of 0.01454**. The median in the table above is
larger than that maximum, so the two cannot be measuring the same set.

A branch that reaches a decided game returns exactly 1.0 or 0.0, and the same
entry records that **22% of reviews are in that state at horizon 40**. The
0.0045 figure therefore describes the **undecided** subset, where every branch
is a live position judged by score share. Over all reviews the median is
0.031–0.085.

The distinction matters because a whole hypothesis was built on the smaller
number: that an ordinary review cannot clear the 0.01 commitment margin, so
only decided branches can act, and lowering the margin should therefore
convert search into action. `strategic_m002` (margin 0.002) tested it:

```
uncommitted player-turns: 47.3% -> 41.9%     lane switches/game: 2.33 -> 2.47
paired-map score: 50.0% over 60 maps, 2 map directions to 2, sign p=1.0000
```

The gate binds on a minority of reviews and moving it does a minority-sized
thing. Not promoted, not pursued further. **When quoting a spread, say which
population it is over** — the two differ by an order of magnitude and they
support opposite conclusions.

## 2026-07-26 — `search_probe`, and what screening three known knobs says

`search_probe` flips one flag on one agent at one position and reports the
branch values a review would compare and the lane it would choose. Paired by
construction, 48 seconds for 120 maps against about forty minutes for the
`ai_eval` run it triages for. It is a screen: a flat reading refutes, a moved
reading earns a pre-registered run and nothing more.

Calibrating it against three knobs whose evaluation outcomes are already
known, all at 57 paired positions, 4p 24×16, warmup 60, seeds 900..:

| knob | known eval outcome | spread, paired | would-commit | flip direction |
|---|---|---|---|---|
| `--horizon 80` | **won** 21–5, p=0.0025 | 26 up, 11 down, p=0.0201 | 28% → 12% | 3 lane / 12 adaptive |
| `--rotate` | **null** | 0 up, 32 down — *not comparable* | 28% → 7% | 0 lane / 12 adaptive |
| `--cold` | **lost** 34–87 | 17 up, 20 down, p=0.7428 | 28% → 21% | 5 lane / 9 adaptive |

Three things follow, and two of them retract earlier readings.

**Commitment rate does not predict strength, in either direction.** The knob
that won cuts it hardest but one (28% → 12%); the knob that measured null cuts
it hardest (28% → 7%); the knob that lost cuts it least (28% → 21%). Two losing
treatments happened to reduce it and that coincidence was written up as if it
were a law. It is a diagnostic that a treatment is doing *something*, not
evidence about what.

**Spread is only comparable between arms that project the same number of
branches.** Spread is `max − min` over the projected branches, so a treatment
that shrinks the candidate set lowers it mechanically. `--rotate` cuts seven
branches to about 2.7 — 224 projected branches against 85 — and duly reports
spread lower on 32 of 32 positions with no bearing on quality. This is the same
confound that makes the commitment margin a function of the candidate set. The
probe now detects it and prints `[NOT COMPARABLE]`.

**What the screen can honestly claim today is the negative half.** A treatment
that leaves every branch value identical cannot change a decision, and that has
happened here often enough to be worth two minutes (`INERT`, exit 3). Three
calibration points cannot establish that any positive reading predicts a win,
and this table should not be read as if they had: the winner raises spread and
the loser is flat, but a single null sits outside that ordering because its
spread reading is invalid. **Screen for inertness; decide with `ai_eval`.**

## 2026-07-26 — the priors make 3× the search's lane decisions, and removing the biggest is null

**Who decides the lane** (`search_probe --priors`, 200 four-player positions,
warmup 60, seeds 900..):

```
  prior                        n     agrees median spread    decided
  duel-religion                5         0%        0.0000       100%
  urgent-counter               2         0%        0.1911        57%
  irreversible-religion       92        15%        0.1106        32%

who decides the lane, over 200 sampled reviews:
  priors    answered   99 reviews and named a lane in   99 (100%)
  rollouts  answered  101 reviews and named a lane in   33 (33%)
  -> the priors make 3.0x as many lane decisions as the search does
```

A prior always names a lane; the rollouts name one only when a lane clears the
adaptive baseline by the commitment margin, and two times in three none does.
So the macro search picks roughly a **quarter** of the lanes this agent plays,
and `viable_religious_commitment` picks half of them alone — disagreeing with
the projection on 85% of the reviews it answers, always by taking Religion
where the search would stay adaptive.

Note this corrects an emphasis: `docs/AI_GAPS.md` names `urgent_counter` as the
dominant prior, measured over whole games. At the stage where lanes are chosen
it is not close — 2 against 92.

**Removing it is null.** `strategic_noprophet` (the prior disabled), 240
mirrored maps at fresh seed 160000:

```
game-win share: noprophet 238/480 (49.6%)  strategic 242/480 (50.4%)
paired-map score: 49.6% (95% Wilson CI 43.3%..55.9%), Elo-equivalent -3
paired direction: noprophet 12, neutral 214, strategic 14; sign p=0.8450
terminal-score direction: noprophet 58, neutral 137, strategic 45; p=0.2369
promotion gate: INCONCLUSIVE
```

Both pre-registered predictions fired, so this is a null and not a misfire:
search exposure **57% → 63%** with `irreversible-religion` priors **211 → 0**,
and religious commitment **30.3% → 26.8%**.

### Why a 3× decision share converts to a 0× strength share

**`adaptive` is not a lane.** Returning `None` hands the turn back to
`AdvancedAi`'s own victory planner, which frequently picks the same lane the
prior named. So 85% of those reviews changed their *label* while religious
victories moved only 171 → 164. The prior is largely **redundant with** the
behaviour underneath it rather than additional to it.

**A disagreement rate is an upper bound on behavioural impact, and a loose
one.** `search_probe` now says so in its own output. This is the third
confound of this shape recorded here — after the unpaired-trajectory
comparison and the unequal-branch-count spread — and they share a root: a
number computed at the decision layer does not measure the play layer.

### What this closes

| treatment | lane decisions changed | strength |
|---|---|---|
| `strategic_noprophet` | ~42% of all reviews | **0** (p=0.8450) |
| `focused_deepening`, rank-pruned | uncommitted turns 44.9% → 58.4% | **0** (p=0.8318 repaired) |
| warm branches (#413) | 1 review in 4 | **+37 Elo** |

Changing *more* lane decisions is uncorrelated with strength, and the
treatment that won changed the fewest. **Lane routing is not the lever.** The
macro search's only output is a lane, so what remains there is compute — which
works, and whose ceiling is already measured at horizon 80. Effort belongs on
a decision that repeats.

## 2026-07-26 — the production search is not blind, and its ranking is horizon-independent

`ProductionSearchAi` is a recorded negative result (9 map directions to 21,
p=0.0428) whose module note names a diagnosis it was never run against: *"if
every branch returns the same number the horizon is too short for the build to
land, and no win rate would say so."* It exposes `candidate_values` for exactly
that check. `search_probe --production` takes it.

```
production audit: 71 city decisions over 40 maps (4p 24x16, warmup 60)

  candidates projected per decision   5.0
  values the evaluator can separate   3.7
  median spread across candidates     0.015657
  p90 spread                          0.070006
  decisions where every candidate scores the SAME   3 of 71 (4%)
  same pick at horizon 200 as at the shipped ceiling  54 of 56 (96%)
```

**Both halves of the diagnosis are false.**

- **Not blind.** The evaluator separates 3.7 of 5.0 candidates; only 4% of
  decisions score every candidate alike. Contrast `PolicyAi`, whose computed
  gain was exactly zero on 96.4% of candidates — that is what blind looks like.
- **Not horizon-bound.** Raising the ceiling from 40 to 200, long enough for
  any build in the game to land and compound, leaves the chosen item unchanged
  on 96% of decisions. `ProductionSearchAi::max_horizon` exists so this stays
  re-measurable.

So the search sees the difference, keeps seeing the same difference once every
payoff has landed, and still loses to the scripted governor.

### What that leaves: the objective, not the window

**Score share is not win probability.** The lane search works because its
branches reach *decided* games and return exactly 1.0 or 0.0 — 22% of reviews
at horizon 40, 56% at 80. A production rollout starting mid-game and stopping
40 or 200 rounds later essentially never decides, so it ranks entirely by a
proxy, and on this decision the governor's hand-tuned sequencing beats that
proxy.

That also explains `production_net` cleanly: it swapped one function of the 25
empire aggregates for another, when the problem is that **no** function of them
is win probability.

### What this retires, before it was built

The obvious repair — make the rollout cheap enough (sealed per-city
simulation, frozen rivals) to afford a payoff-length horizon — is aimed at a
defect that is not there, and is **predicted-null**. It was on the roadmap as
M4 in `docs/SUPERHUMAN.md` and is retired there, on measurement rather than on
a run.

The surviving route is the one the module already named: branches continued to
a **real result**. A full continuation per candidate is roughly seventy times
the cost of a game, so it is an offline labeller feeding a distilled policy —
`docs/SUPERHUMAN.md` M6 then M5 — not an online agent.

## 2026-07-26 — what an outcome label would actually be worth

Two closed lines both ended at the same sentence — score share is not win
probability — and both pointed at the same repair: continue each candidate to
a **real result** and label it with the outcome. Before building that,
`search_probe --outcome` measures the label it would produce. Every candidate
of a city decision is continued to a natural end at the stock 500-turn budget.

```
outcome audit: 51 city decisions, every candidate continued to a real result
               (4p 24x16, warmup 60, 500-turn budget)

  candidates continued per decision              5.0
  decisions where the label DISCRIMINATES        14 of 51 (27%)
  ...of those, proxy pick == outcome pick         3 of 14 (21%)
  ...of those, the proxy's pick WON its game      6 of 14 (43%)
```

**On 73% of decisions every candidate leads to the same outcome.** The label
carries no signal there, whatever it costs to produce — and it costs about
seventy times a game per decision. Where it does discriminate, score share
picks the winning candidate 43% of the time, near chance, so the headroom is
real; it just exists on about a quarter of decisions.

**27% is an upper bound, not an estimate.** The engine is deterministic, so
each continuation is a single sample and a build that "wins" may win for
reasons unrelated to it. Chaotic divergence and causal effect are
indistinguishable in this design, and — because the same position and the same
agents always produce the same game — the label cannot be denoised by
repeating it.

**What that implies for the design.** An outcome-labelled corpus built from
single continuations would train on noise for three quarters of its rows. The
fix is replication across *opponents*, not more decisions: continue each
candidate against several distinct rival policies and label with the resulting
win rate. `data/league` already maintains a rated pool of distinct strategies,
which is the natural source. That is a larger job than the labeller as
originally scoped, and it is the honest version of it.

## 2026-07-27 — replicating the outcome label across opponents: the noise floor

The previous entry said a single continuation cannot be denoised — the engine is
deterministic — and that the fix is replication across *opponents*.
`search_probe --outcome --replicas K` does that: the searching seat keeps the
stock agent while the rivals are drawn from a pool of five distinct-but-sane
policies (the four `Doctrine` perturbations of the stock genome, each clamped by
evolution's own per-gene bounds, plus the frozen legacy planner). 850 full games
at the stock 500-turn budget, 12 minutes wall.

```
outcome audit: 34 city decisions, 5 candidates, 5 opponent replicas each

  decisions where the label DISCRIMINATES         8 of 34 (24%)
  candidates whose replicas DISAGREED            48 of 170 (28%)
  median win-rate spread across candidates       0.20
  decisions separating by >= 0.4 win rate         9 of 34 (26%)
```

**Replication does denoise, and it reveals the label is under its own noise
floor.** 28% of candidates changed outcome when only the opponents changed, so a
single continuation is genuinely noise for about a quarter of them. But a win
rate over K replicas carries a standard error of `sqrt(p(1-p)/K)` — **0.224 at
K=5** — and the median spread *between* candidates is **0.20**. The signal is
smaller than the error on each measurement of it.

| replicas | SE | resolves a true gap of |
|---|---|---|
| 5 | 0.224 | 0.89 |
| 20 | 0.112 | 0.45 |
| 50 | 0.071 | 0.28 |
| **100** | **0.050** | **0.20** |

So resolving the effect that is actually there needs about **100 replicas per
candidate, twenty times this run**: 4 hours for these 34 decisions, and roughly
**1200 hours** for a 10,000-decision corpus at 873 ms per game. A pilot is
affordable; the corpus is not, on this machine.

### The general criterion, which is the durable part

> **Search pays on a decision whose effect exceeds the outcome noise floor.**

That is why the lane search works and the production search does not, and it is
measurable in advance for either. A lane branch reaches a decided game 22% of the
time at horizon 40 and 56% at 80, returning exactly 1.0 or 0.0 — an effect of 1.0
against a floor near zero. A build choice moves the win rate by about 0.20 against
a floor of 0.224 at affordable replica counts.

**Measure the ratio before building search for any decision.**
`search_probe --outcome --replicas K` prints it, including the replica count that
would be required. This is the same discipline that retired the sealed-rollout
family and the commitment-margin hypothesis, applied one level up: not "does the
treatment fire" but "is there anything here to find".

## 2026-07-27 — ★ SHIPPED: the first evolved genome, and what it replaces

Every strategic agent in this repository resolves its genome through
`evolve::load_champion("evolved").unwrap_or_default()`. `evolved/` was
gitignored and **no `best.json` existed on this machine or in a fresh clone**,
so `advanced`, `advanced_evolved`, `strategic`, `strategic_deep`, the
exhibition and the fleet all silently played `Weights::default()`. The
hand-written defaults were never the intended agent — they were what the
loader returned when the intended one was missing. **Every strength number
published before today was measured on top of that.**

`data/evolved/best.json` is now a committed champion, on the same arrangement
as `data/league`: `.gitignore` anchors `/evolved/` to the repo root only, and
`load_champion` prefers a local `evolved/` so an in-progress run is never
shadowed by the snapshot.

### How it was earned

`civvis evolve --pop 24 --generations 25 --games 96 --players 4 --width 24
--height 16 --turns 500 --seed 7`, promoted at generation 2 by the unmodified
gate: SPRT **22–32 = 40.7%** against the incumbent where parity at a
four-player table is 25% (Fishtest-style, H1 0.40, LLR bounds ±2.94), holdout
no regression.

Then evaluated against the defaults it replaces, pre-registered, at the stock
500-turn budget:

```
ai_eval advanced_evolved advanced --players 4 --pairs 1300 --seed 172000 --turns 500

game-win share: advanced_evolved 1419/2600 (54.6%)  advanced 1181/2600 (45.4%)
paired-map score: 54.6% (95% Wilson CI 51.9%..57.3%), Elo-equivalent +32 (CI +13..+51)
paired direction: evolved 253, neutral 913, advanced 134; sign p=0.0000
anytime evidence: evolved peak e=1.601e7, crossed at map 746
terminal-score direction: evolved 836, neutral 2, advanced 462; sign p=0.0000
promotion gate: PASS
```

| seed | maps | for | against | sign p |
|---|---|---|---|---|
| 170000 | 240 | 46 | 27 | 0.0344 |
| 171000 | 500 | 80 | 51 | 0.0141 |
| 172000 | 1300 | 253 | 134 | 0.0000 |
| **pooled, disjoint** | **2040** | **379** | **212** | **~1e-13** |

Two earlier runs were reported here as unpromoted positives because they read
INCONCLUSIVE on the fixed-*n* Wilson bound. At 54.6% that bound needs about
1200 maps; 1300 were run so the criterion could be met rather than loosened.

### What is and is not established

- **Established:** the committed genome beats `Weights::default()` for
  `AdvancedAi` at four players and the stock budget, on three disjoint seed
  sets.
- **Not established:** transfer to `StrategicAi`. `strategic` and
  `strategic_deep` load the same champion and wrap an `AdvancedAi`, so they
  inherit it, but no paired run has measured them under it. #466 is testing a
  league-selected genome under `strategic_deep`; this one wants the same
  treatment before any claim is made about those entrants.
- **Not established:** transfer to other player counts or map sizes. The
  genome was evolved at 4p on 24×16 and evaluated there. `auto_dimension`
  gives 60×38 for four players in ordinary play, and that is a different
  distribution.
- **Superseded numbers:** any published comparison whose entrants resolve
  through `load_champion` now differs from its recorded value, because the
  loader no longer returns `None`. Re-measure rather than compare across the
  boundary.

## 2026-07-27 — a better champion, and carrying it where it is actually used

Two corrections to the entry above, both measured.

### 1. Generation 14 is the champion, not generation 2

The GA kept running. Generation 14 promoted through the same unmodified gate
(SPRT 36–62 = 36.7% against the incumbent where parity is 25%, holdout no
regression) with the run's best validation, 141.6 against 125.6.

Evaluated against the **same control, the same 1300 maps and the same seed**
as generation 2's gate run, so the two are directly comparable:

| champion | paired score | Wilson | Elo | map directions | sign p |
|---|---|---|---|---|---|
| gen 2 | 54.6% | 51.9–57.3% | **+32** (+13..+51) | 253 – 134 | 0.0000 |
| **gen 14** | **57.0%** | **54.3–59.7%** | **+49** (+30..+68) | **304 – 122** | 5.2e-19 |

Both pass. Generation 14 is +17 Elo above generation 2 with barely overlapping
intervals, so it replaces it in `data/evolved/best.json`.

### 2. The committed file alone would never have reached a game

`load_champion`'s `data/` fallback resolves against the **current working
directory**. `tools/spectator_supervisor.py` builds from `origin/main` in a
private worktree, `promote_binary()` copies only the binary, and the server
runs with the *deployment* checkout as its cwd — 548 commits behind
`origin/main` when this was written. The exhibition, the fleet, and any copied
binary would have gone on loading `Weights::default()` while the repository
contained a champion.

The genome is now compiled in with `include_str!`, exactly as `rules.rs`
already embeds all 29 ruleset files. Resolution order where a tree exists is
unchanged — local `evolved/`, then `data/evolved/`, then the built-in — so an
in-progress run still overrides the snapshot and a genome can still be swapped
without rebuilding. The built-in answers only for the canonical `evolved`
directory, because `--artifact-dir` asks about *that* directory and
`--require-artifacts` depends on the answer being honest.

**The general shape, which has now cost this session twice:** a number true in
one context read as true in another. The genome was present in the repository
and absent in the process; the fallback was correct in a checkout and wrong in
deployment. Ship-and-verify means verifying where the code runs, not where it
was written.
