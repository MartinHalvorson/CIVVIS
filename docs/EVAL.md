# Evaluation baselines

> **Reading note (2026-07-30).** This is an append-only experiment log, so
> labels such as “deployment,” “strongest,” and “current” describe the dated
> run that contains them. They are not present-day rankings. The live
> exhibition now rotates through 4–10 seats and stock profiles; no learned
> model or search entrant is live. See `AI_GAPS.md` for the current assessment.

Recorded reference numbers so strength and health regressions are visible.
Re-run the battery after any AI or rules batch and compare against the most
recent entry; update this file (append, don't overwrite) when numbers move
for an understood reason. All commands are deterministic for a given build
and seed set.

```bash
civvis soak --games 12 --players 4 --turns 350 --start-seed 100
civvis tournament --ais advanced-20260730=advanced,advanced_v1,basic-20260730=basic,random-20260730=random \
  --games 40 --players 4 --quiet
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

`--require-artifacts` exits 3 instead of recording an untrained result. The
collapse warning is the load-bearing half: a missing artifact is only a
problem when it makes two entrants the same agent, and that is precisely
the case a win rate cannot show you.

⚠ **`--artifact-dir` used to point the check at a non-default champion
directory, and that is exactly what was wrong with it.** It moved the *report*
and never the run: `builtin_provenance(name, dir)` honours the directory, but
`builtin_ai(name, seed)` takes **no directory at all** — every arm resolves the
`ARTIFACT_DIR` constant, and `StrategicAi`/`PolicyAi`/`ProductionSearchAi` each
load their own net from that same constant. So the flag could print a net found
in one directory and then play the agent that read another, which is the one
failure this whole reporting path exists to prevent. `ai_eval` now exits **2**
with the reason rather than printing a provenance the run does not have. To
evaluate a different artifact, run from a working directory that holds it —
`evolved/valuenet.json` or `data/evolved/valuenet.json`, both of which
`ValueNet::load` now resolves. Threading a directory into construction is the
general fix and is ~70 call sites inside `elo::builtin_ai`.

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

## 2026-07-26 — three free military superpowers, all worth nothing

Oracle ablation: hand one seat a free, cheating version of one subsystem and
play it against stock agents on matched cells — same map, same seat, once with
the grant and once without — then run McNemar's exact test over the cells the
grant changed. The maximum over such grants is an upper bound on everything
any amount of honest work on that subsystem could ever be worth.

`ablate --grant all --pairs 50 --players 4 --turns 500 --seed 420000`, 60x38/6,
100 cells per grant:

| grant | fired/game | helped | hurt | McNemar p | verdict |
|---|---|---|---|---|---|
| `none` (control) | 0 | 0 | 0 | 1.0000 | **0 discordant of 100** |
| `modernity` — every unit free at the top of its upgrade chain, every turn | 92.4 | 16 | 16 | 1.0000 | no headroom |
| `taker` — a melee unit placed beside every enemy city | 476.8 | 15 | 13 | 0.8506 | no headroom |
| `attrition` — every unit starts each turn at full health | 111.5 | 11 | 14 | 0.6900 | no headroom |
| **`treasury`** — 200 Gold and 100 Faith per turn | 147.7 | **62** | **0** | **0.0000** | **headroom** |

**The calibration is what licenses reading the rest.** A null is otherwise
ambiguous between "this subsystem does not limit the agent" and "this
instrument cannot resolve anything", and those call for opposite next steps.
`treasury` is not a subsystem and is not realistic — it hands over more Gold in
ten turns than these empires accumulate in a game. At the same 100 cells where
the military grants produced 16/16, 15/13 and 11/14, it produced **62 to 0**.
The instrument detects an advantage. The military grants are not one.

The control returning **0 discordant cells out of 100** is the other half: the
same game played twice is bit-identical, so none of the above is harness noise.
It reproduced 26/100 across two independent runs.

**Military capability is not on the critical path in this simulator.** That
contradicts `AI_GAPS.md` item 4 — "the single biggest per-system win
available" — which its own re-sequencing admits had never been measured. It has
now been measured three ways, each an upper bound, each null. It is consistent
with everything else on record: 0 of 48 games ended in domination, 60% ended
diplomatic and 27% religious, while the agent spent 26% of its turns planning
Conquest and 21% in Recovery.

What this does **not** say. `treasury` grants a currency, not a decision, so it
shows the agent converts resources into wins efficiently — not that its
economic decisions are poor. What it relocates is the question: the leverage is
in the economy, and the open part is whether the *generation* of that economy
is what the agent is bad at. That is also where `ProductionSearchAi` failed
(9 maps to 21), diagnosed at the time as a horizon shorter than the decision's
payoff rather than as production not mattering. Those two findings now point the
same way.

Power: ~30 discordant cells resolves roughly a 70/30 split. A real 60/40 effect
would not have been detected, so these are "worth less than this run can
resolve", not "exactly zero". The calibration bounds how much that matters —
whatever the military grants are worth, it is a small fraction of what the
economy is worth.


## 2026-07-26 — the agent picks the wrong thing to play for, by 30 points

The three capability grants said this agent is not limited by what it can do.
The other half of the question is whether it is limited by what it chooses to
do. `ablate --mode best-lane` answers it: each cell is played once per victory
lane with the seat committed to that lane from turn one, and once adaptively.
The maximum over lanes is an oracle no agent could implement — it needs the
result before the decision — so the gap to the adaptive agent bounds the whole
of victory routing, search and priors included.

`--mode best-lane --pairs 25 --players 4 --turns 500 --seed 420000`, 50 cells,
350 games:

| policy | wins | share |
|---|---|---|
| adaptive (the shipped agent) | 14/50 | 28.0% |
| **committed Religion** | **29/50** | **58.0%** |
| committed Diplomacy | 20/50 | 40.0% |
| committed Science | 0/50 | 0.0% |
| committed Culture | 0/50 | 0.0% |
| committed Domination | 0/50 | 0.0% |
| committed Score | 0/50 | 0.0% |
| best lane per cell (oracle) | 38/50 | 76.0% |

25 cells where some lane won and adaptive lost, 1 the other way, McNemar exact
**p=0.0000**.

**The headline is not the oracle.** "Some lane won 76%" is a maximum over six
correlated runs and is optimistic by construction. The load-bearing number is a
single fixed policy: **committing to Religion from turn one wins 58% against
28% for adapting**, on the same 50 cells, at 25% parity. That is not a max over
anything. The agent gives up roughly thirty points by deciding for itself.

Read with the ablation, the picture is consistent and narrow: capability is
not the constraint (three free military superpowers, all null, against a
calibration that scored 62-0), and *routing* is, by a very large margin.

**Four of the six lanes never win.** Committed Science, Culture, Domination and
Score won 0 of 50 each. For Science that is arithmetic rather than weakness:
`docs/AI_GUIDE.md` records unassisted science victories landing on turns 1021
and 940, and these games stop at 500. Any agent that routes toward one of
those four at this player count and turn budget has already lost, which is
what the adaptive agent spends much of its time doing — 26% of its turns
planning Conquest across 48 games that produced no domination victory at all.

Caveats. Only one seat commits while three adapt, so 58% is "committing beats
adapting", not "religion is unconditionally strongest" — if every seat
committed they would contest the same prophet slots. The result is specific to
4 players, 500 turns, this map profile and all six conditions enabled. And a
single fixed lane is not the fix: it is the evidence that the routing decision
is worth an enormous amount and is currently being made badly.


## 2026-07-26 — the expansion ceiling is not what limits expansion

The oracle ablation put the leverage in the economy, and city count is the
largest single multiplier on one, so the obvious suspect was the hardcoded
`.min(6)` sitting on top of a `map_capacity` that reaches 9. Across 48
six-player games the agent finished on 6.4 cities per empire, apparently
resting on that ceiling.

`ai_eval advanced_wide advanced --pairs 120 --players 4 --width 60 --height 38
--city-states 6 --turns 500 --seed 520000`, where `advanced_wide` lets
`map_capacity` decide:

- 120/240 against 120/240, paired-map score 50.0%, Elo-equivalent +0
- **0 maps favoured either side, 120 neutral, sign p=1.0000**
- every diagnostic identical to the digit — cities 4.86 vs 4.86, score 519.4 vs
  519.4, settlers 0.18 vs 0.18

Identical diagnostics mean the same games were played. The treatment was inert,
and the arithmetic says why: `desired_cities` is `(3 + turn/90).min(capacity)
.min(ceiling)`, so the ceiling first binds past turn ~450, these games average
323 turns, and at 323 the target is 6 while the agent reaches **4.86**. It
never arrives at its own target, so raising the target cannot matter. The 6.4
that motivated this came from six-player games running to ~405 turns and did
not transfer to the four-player profile.

**What it relocates.** Expansion is limited by execution rather than by
ambition: the agent wants six cities and settles five. Whatever binds sits in
settler production, site availability, or the expansion window.

**And a process note worth more than the result.** Every grant in `oracle.rs`
carries a test asserting it actually fires, because a treatment that does
nothing produces a null for the wrong reason — two of those tests caught
exactly that during development. That discipline was not applied here, and a
240-game run bought what one line of arithmetic would have said first. A
treatment needs a fires-check before it needs an evaluation.


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

⚠ **That rationale survives; the flag it names does not.** `--artifact-dir` is
now refused for anything but the default (see the note above), so what keeps the
built-in honest is `--require-artifacts` alone. Restricting the embedded tier to
the canonical directory is still right, and for the reason given — an explicit
question about some other directory must not be answered with a built-in — it is
just no longer askable through this flag.

★ And the same defect was still live one artifact over. `ValueNet::load` had
none of this resolution: a single cwd-relative read, no `data/` tier, no
built-in, while nothing was tracked at `evolved/valuenet.json` anywhere in the
tree. So the learned evaluator resolved to `None` in every process on every
machine, and `strategic` has never once played as anything but
`strategic_score`. It now resolves local → `data/<dir>`, with a
present-but-unloadable artifact **stopping** resolution rather than falling
through to a net the experimenter did not place. Third instance of this class,
after the genome here and the league roster in #490 — which is the sharpest
available argument for reading the paragraph below as a standing rule rather
than a war story.

**The general shape, which has now cost this session twice:** a number true in
one context read as true in another. The genome was present in the repository
and absent in the process; the fallback was correct in a checkout and wrong in
deployment. Ship-and-verify means verifying where the code runs, not where it
was written.

## 2026-07-27 — PRE-REGISTRATION: the shipped genome may be tuned for the wrong map

Written before the runs report, so the prediction is on record rather than
fitted to the outcome.

`data/evolved/best.json` was evolved and validated entirely at **4 players on
24×16**. The exhibition runs **6 players on 74×46 with 9 city-states**
(`tools/spectator_supervisor.py`'s own command line).

```
validation : 24x16 =  384 tiles / 4 players =  96 tiles per player
deployment : 74x46 = 3404 tiles / 6 players = 567 tiles per player
             -> 5.9x more room per player where it actually runs
```

The champion's expansion genes moved sharply toward building tall:

| gene | default | champion | change |
|---|---|---|---|
| `city_target` | 4.000 | 2.408 | **−40%** |
| `settler_min_pop` | 2.000 | 4.457 | **+123%** |
| `settle_dist` | 0.400 | 0.692 | +73% |
| `min_city_dist` | 4.000 | 3.638 | −9% |
| `settler_stop_turn` | 150.0 | 143.2 | −5% |

It targets 40% fewer cities and demands a city more than twice as populous
before producing a settler. On 96 tiles per player that is correct — there is
nowhere to put more cities. On 567 tiles per player, land is nearly free and
under-expansion is a severe error.

**Prediction: the shipped genome under-expands at the deployment configuration
and may be weaker than `Weights::default()` there.** The eval prints city
counts, so the mechanism is checkable independently of the win rate.

**Runs in flight**, both pre-registered, fresh disjoint seeds, stock budget:

```
ai_eval advanced_evolved advanced --players 6 --width 74 --height 46 \
        --city-states 9 --pairs 200 --seed 190000 --turns 500
ai_eval strategic_deep strategic_deep_default --players 4 --pairs 500 \
        --seed 180000 --turns 500
```

**If the prediction holds**, the artifact needs scoping to small four-player
maps or re-evolving at the deployment configuration, and this file will say so.
**If it is refuted**, the genome transfers and the mechanism above is wrong.

### The process failure this exposes, which is not statistical

The genome carries a 1300-map validation, a 500-map confirmation and a 240-map
pilot. **All three used the configuration it was evolved on.** Three
independent seed sets cannot detect a mismatch between where a thing is
measured and where it is deployed — that is not a power problem, it is a
different error, and adding maps would never have found it.

It is the second instance of that error in two days. The first was the working
directory: `data/evolved/best.json` resolved in a checkout and not in the
process that serves games, until the genome was compiled into the binary.
**Both were invisible to every statistic and visible immediately on reading
what the deployment actually does.**

## 2026-07-27 — every result in this file was measured at one map density

The entry above pre-registers a mismatch for the shipped genome: evolved and
validated at 4 players on 24×16, deployed at 6 players on 74×46. Checking
whether that was one experiment's mistake or a property of the harness:

```
recorded `ai_eval` commands in this file          20
      ... that specify a map size                  1   (the deployment check itself)
ai_eval defaults                                  --width 24 --height 16 --players 2
```

**Nineteen of twenty ran at 24×16.** So every strength number in this
ledger — `strategic_r20`, `strategic_h80`, the `strategic_deep` promotion,
`policy_wide`'s −313, branch fidelity's +37, the genome's +49 — was measured at

```
     24x16 / 4 players =  96 tiles per player
     74x46 / 6 players = 567 tiles per player   (what the exhibition runs)
```

**about one sixth of the deployment density**, and none has been checked for
density sensitivity.

### This is a caveat, not a retraction

Mechanisms differ in how much density can matter to them, and the difference is
not rhetorical:

- **Expansion weights are directly coupled.** `city_target`, `settler_min_pop`
  and `settle_dist` answer "how many cities can I fit and when should I stop",
  which is a question *about* density. The shipped genome moved `city_target`
  −40% and `settler_min_pop` +123%, which is the correct answer at 96 tiles per
  player and plausibly the wrong one at 567. That one has a mechanism and a
  pre-registered test.
- **Branch fidelity has no obvious coupling.** Projecting a rollout from the
  planner in force rather than a fresh one is a construction fix; nothing in it
  refers to available land. Lower risk — but unchecked, which is a different
  thing from safe.
- **Macro-search depth and cadence are in between.** More room means longer
  games and different victory-lane dynamics, so the horizon that saturates at
  24×16 need not saturate at 74×46. The 22/56/89% saturation table is itself a
  24×16 measurement.

### What to do about it

Cheap and worth doing: **state the map size in every recorded command.** Nineteen
entries here do not, so a reader cannot tell what was measured without knowing
the binary's defaults.

Expensive and worth doing selectively: re-measure the promoted changes at
deployment density. A deployment-config game costs roughly 200× a 24×16 one, so
this is a per-result decision, not a sweep. Note that **matching density does
not require matching dimensions** — 47×47 at four players is 552 tiles per
player, close to deployment, at about 6× the cost of 24×16 rather than 200×.

The general form, which has now cost this session twice: **a number is true
inside the conditions that produced it.** The first instance was a working
directory, the second is a map density, and both were invisible to every
statistic computed on top of them.

## 2026-07-27 — REFUTED: the genome transfers to the deployment configuration

The pre-registration above predicted that the shipped genome would
**under-expand** at 6 players on 74×46, because it moved `city_target` −40% and
`settler_min_pop` +123% while being evolved at one sixth the density. The run
it named has reported.

```
ai_eval advanced_evolved advanced --players 6 --width 74 --height 46 \
        --city-states 9 --pairs 200 --seed 190000 --turns 500

mirrored head-to-head: 200 maps, 400 games, 6 players, average 388.3 turns
game-win share: advanced_evolved 217/400 (54.2%)  advanced 183/400 (45.8%)
paired-map score: 54.2% (95% Wilson CI 47.3%..61.0%), Elo-equivalent +30
paired direction: evolved 58, neutral 101, advanced 41; sign p=0.1074
terminal-score direction: evolved 131, neutral 0, advanced 69; sign p=0.0000
promotion gate: INCONCLUSIVE
```

**The prediction is wrong, and wrong on its own mechanism.**

| | cities | pop | score |
|---|---|---|---|
| `advanced_evolved` | **5.39** | 65.1 | 548.9 |
| `advanced` (defaults) | 4.76 | 59.5 | 501.3 |

The champion builds **13% more cities** at deployment density, not fewer. Its
paired score there (54.2%) is within noise of its score at the configuration it
was evolved on (54.6% over 1300 maps), and terminal score separates decisively
in its favour, 131–69 at p=0.0000. The gate reads INCONCLUSIVE only on the
fixed-*n* Wilson bound at 200 maps, which is a power statement, not a null.

### Why the reasoning failed

`city_target` is a *target*, not a rate. Reading one gene's direction and
inferring a behaviour ignored the rest of the genome it operates with —
`settle_dist` moved +73% in the same champion, and the settling decision is a
joint function of several weights plus the map. **A 40% cut to one parameter
produced 13% more cities.**

The general lesson is narrower than "check your deployment config", which the
pre-registration got right, and it is this: **genome parameters interact, so
behaviour cannot be read off a single weight's direction.** The mechanism was
invented from a table of deltas and it did not survive contact with a
measurement of the behaviour itself.

What made this a clean refutation rather than a story was pre-registering the
prediction, the seed and the size before the run, in a commit that predates it.

### What stands

- The genome is favourable at **both** densities measured: 54.6% at 4p/24×16
  over 1300 maps (gate PASS), 54.2% at 6p/74×46 over 200 maps (underpowered).
- `docs/EVAL.md`'s "not established" list loses the map-size entry and keeps
  the rest.
- The **harness-wide** caveat from the previous entry is untouched: 19 of 20
  recorded runs still used one density, and this is the first result checked
  across two. It came out well; that is one data point, not a general licence.


## 2026-07-28 — the agent changes its mind fourteen times a game, and spends a third of it on lanes that never win

`src/bin/plan_churn.rs`. The oracle ablation on PR #366 found that capability
is not what limits this agent and *routing* is: committing to Religion from
turn one won 29 of 50 matched cells where the shipped adaptive agent won 14,
McNemar exact p=0.0000. That is a thirty-point gap against a **fixed** policy,
so hindsight does not explain it away. A fixed policy can beat an adaptive one
for two reasons — the adaptive agent picks the wrong lane, or it picks the
right one and does not stay in it. This separates them.

`AdvancedAi::plan_stale` re-assesses every 5 turns and `assess()` recomputes
the strategy from scratch; **nothing in it reads the previous plan**. So a
long game contains up to a hundred independent re-decisions with hysteresis at
none of them. `plan_churn` records the grand strategy every turn, per major
seat, and collapses it into runs.

On the oracle's own profile — `--players 4 --width 60 --height 38
--city-states 6 --turns 500 --seed 420000`, 24 maps, 96 seats, mean game 329
turns:

| | |
|---|---|
| switches per seat | **14.2** |
| switches per 100 turns | 4.30 |
| mean run length | **21.7 turns** |
| longest run | 90.2 turns |
| runs under 10 turns | **37.7%** |
| distinct strategies visited | 4.74 of 7 |
| winning seats holding their final strategy | **32.8% of the game** |

**The allocation is the finding.** Share of all turns by strategy:

| strategy | share | won in the best-lane oracle |
|---|---|---|
| expansion | 23.5% | not a victory lane |
| recovery | 20.5% | not a victory lane |
| religion | 18.9% | **29/50** |
| conquest | 17.9% | 0/50 |
| science | 13.0% | 0/50 |
| culture | 4.0% | 0/50 |
| diplomacy | 2.2% | **20/50** |

**34.9% of the agent's turns go to the three lanes that won nothing, and 2.2%
to the second-best lane it has.** Religion and Diplomacy together — the only
two that ever convert at this player count and turn budget — take 21.1%.

Read with the ablation the chain is complete and mechanical: routing is the
constraint, the routing is myopic because it is re-derived from the board
every five turns with no memory of what the empire has already bought, and it
re-derives *toward* lanes that cannot finish because `victory_focus` ranks
lanes by **progress** and `lane_reachable` tests only Science. Progress is a
correlate, and this document's standing result is that no correlate of a
finished game is win probability.

**Map size changes the reading, which is itself part of the result.** The same
probe at 4p 24×16 (12 maps, 160-turn games) reports 4.5 switches per seat and
a 29.1-turn mean run, and its own branch calls that "intermediate". Longer
games contain more re-decisions, so the churn compounds with exactly the game
length the strength evaluations use. Do not quote the small-map numbers.

**What this does not establish.** That hysteresis would help. Committing
harder to a *badly chosen* lane is worse than churning, and 37.7% of runs
being under 10 turns is consistent both with thrash and with an agent
correctly reacting to shocks — 20.5% of turns in Recovery says shocks are
real. The next measurement is whether a switch is predicted by anything that
should predict it; the intervention worth testing is a viability filter on the
lane candidate set, not stickiness for its own sake.


## 2026-07-28 — the routing headroom is a property of the fallback genome, not of the agent

`src/bin/commit_curve.rs`, pre-registered at
`/Users/martin/civvis-commit-curve-preregistration.md` with a calibration gate
fixed before the run: *if committing at turn 0 is not at least 10 points above
the adaptive control, the oracle result does not reproduce on this harness and
nothing may be built on it.* **The gate fired.**

One focal seat is committed to Religion via `AdvancedAi::retarget` at turn T,
every other seat plays stock, and the same map is replayed for each T plus an
adaptive control. 40 maps, 4p 60×38, 6 city-states, 500 turns, seed 420000 —
the oracle's own profile and seed. The focal seat rotates with the map index.

**On the embedded gen-14 champion, the genome every deployed agent plays:**

| condition | wins | share | fired | mean end turn |
|---|---|---|---|---|
| commit at turn 0 | 6/40 | **15.0%** | 40/40 | 308 |
| commit at turn 60 | 12/40 | 30.0% | 40/40 | 303 |
| commit at turn 120 | 13/40 | 32.5% | 40/40 | 288 |
| commit at turn 180 | 12/40 | 30.0% | 35/40 | 304 |
| adaptive (control) | 11/40 | 27.5% | n/a | 294 |

McNemar against the control: turn 0 is **2 helped / 7 hurt, p=0.1797**. The
oracle reports the same nominal condition at 58% against a 28% control. The
control reproduces (27.5% against 28.0%); the *treatment* does not.

**Then the same run with only the genome changed:**

| genome | commit at turn 0 | adaptive control | paired | McNemar |
|---|---|---|---|---|
| `Weights::default()` — the fallback | **37.5%** | 15.0% | 12 helped / 3 hurt | **p=0.0352** |
| evolved champion — what ships | 15.0% | 27.5% | 2 helped / 7 hurt | p=0.1797 |

`ablate::play_lane` builds its field with `AdvancedAi::fleet(&game)`, which is
`AdvancedAi::new()`, which is `BasicAi::new()`, which is **`Weights::default()`**.
That is the value `load_champion("evolved").unwrap_or_default()` returns when
no artifact is present — the fallback, not the agent.

**So the largest claimed gap in this repository — "the agent picks the wrong
thing to play for, by 30 points" — is a measurement of the fallback.** On the
fallback the direction reproduces here and is significant. On the shipped
genome it is gone, and if anything inverted. Whatever the gen-14 GA found, part
of it was routing that a fixed religious commitment can no longer beat.

**What this does and does not establish.**

- It **does** establish that the oracle's routing headroom does not reproduce on
  the shipped genome. Reaching +30 points needs something like 15 helped against
  a handful hurt; this run measured 2 against 7.
- It **does not** establish that early commitment *hurts*. p=0.1797 is not a
  result, and 40 cells resolves only large effects.
- The two controls (15.0% and 27.5%) are **not** significantly different from
  each other or from the 25% parity this design implies — every seat plays the
  same genome, so the focal seat's expectation is 1-in-4 by construction. Do not
  read the control difference as the champion being twice as good; the clean
  comparison is the *paired* one within each genome.
- Harness differences remain and are not fully reconciled: `ablate` samples
  seats `[0, players-1]` over 25 maps and drives its loop from `game.current`,
  where this samples a rotating seat over 40 maps and iterates seats directly.
  The default-genome numbers here (37.5/15.0) do not match theirs (58/28)
  exactly. **The within-harness contrast — everything held constant except the
  genome — is the load-bearing comparison, not the cross-harness one.**

**A mechanism worth checking separately.** `assess()` sends a Religion-targeted
seat that has no religion yet straight to `GrandStrategy::Religion`
(`advanced.rs:1807`), bypassing the "the assigned lane can still afford to
expand first" arm at `:1809` that every *other* assigned target reaches. So
commitment at turn 0 plausibly suppresses the opening expansion entirely, which
would explain why turn 0 is the worst cell on the champion and why turns
60/120/180 — which expand first and commit after — all land near the control.
This has a consequence beyond the probe: `StrategicAi` projects its branches by
calling `retarget`, so a religion branch inherits the same expansion bypass and
the macro search may be systematically mis-projecting the lane that converts
best. `commit_curve` now reports mean cities per condition to settle it.

**The standing rule this is the third instance of.** `docs/SUPERHUMAN.md` §4:
*measure the artifact before the algorithm — ask what the process actually
loads, at the path it actually runs from.* The first instance was the champion
never being loaded at all; the second was `evolve::sprt_confirm` testing parity
at the wrong null. This is the third, and it is the most expensive, because the
conclusion it produced was about to redirect the whole AI programme.


## 2026-07-28 — a committed lane is not a routed agent, it is a crippled one

Follow-up to the entry above, same probe, same 40 maps at 4p 60×38 / 6
city-states / 500 turns / seed 420000, shipped champion genome.
`commit_curve` now reports the focal seat's final city count:

| condition | share | **mean cities** |
|---|---|---|
| commit at turn 0 | 15.0% | **1.68** |
| commit at turn 60 | 30.0% | 2.48 |
| adaptive (control) | 27.5% | **4.10** |

**`retarget(Religion)` does not produce an agent that plays religion well. It
produces an agent that never expands** — 1.68 cities against 4.10, a 59%
smaller empire.

This changes what the best-lane oracle measures. "Committed Religion wins 58%"
is not "what the agent could get by routing correctly"; it is "what a
1.7-city religious rush gets". The same applies to every other committed lane
in that table, and therefore to the "best lane per cell = 76%" maximum built
on top of them. **The oracle does not bound routing headroom.** It bounds the
value of one particular commitment implementation.

### The arm responsible, and how much of it it explains

`assess()` at `advanced.rs:1807`: an assigned-Religion seat with no religion
yet goes straight to `GrandStrategy::Religion`, skipping the "the assigned lane
can still afford to expand first" test at `:1809` that **every other assigned
target reaches**. `assigned_religion_may_expand` (off by default) makes that arm
ask the same question the others ask. Same 40 maps:

| condition | cities off → on | wins off → on |
|---|---|---|
| commit at turn 0 | 1.68 → **2.12** | 15.0% → 20.0% (4 helped / 7 hurt, p=0.5488) |
| commit at turn 60 | 2.48 → 2.75 | 30.0% → 27.5% (7 helped / 7 hurt, p=1.0000) |
| adaptive (control) | 4.10 | 27.5% |

**The fix is real and small: it recovers 0.44 of a 2.42-city deficit — about
18% — and is a null on wins at this power.** So the bypass is *a* cause and not
the main one. A targeted seat still finishes 1.98 cities short of an adaptive
one while being *allowed* to expand, and `desired_cities` is at least 3, so it
is not reaching its own target. Whatever else suppresses it is downstream of
the strategy label — production priorities, city dispositions, or settler
assignment under `GrandStrategy::Religion` — and is not in this arm.

Recorded, flag shipped off, because the cheap half of the diagnosis is done and
the expensive half is now well posed: **find what stops a targeted seat
expanding once the cascade lets it.**

### Why this matters beyond targeted play

`StrategicAi` projects every macro-search branch by calling `retarget`. So each
religion branch is simulated by a seat carrying whatever this defect is worth,
and religion is the lane this engine converts best. The macro search is the one
component in this repository that has ever won Elo, and the largest gain it has
had — `continue_from_plan`, +37 Elo — came from exactly this class of bug: the
counterfactual was simulating the wrong thing. **Measuring the projected city
count of a religion branch against the same branch played out is the obvious
next instrument**, and it is a fidelity question, not a routing one.


## 2026-07-28 — the search projects a religion branch that stops expanding, and it changes one review in four

`src/bin/lane_projection.rs`. `StrategicAi::branch_agent` builds every branch by
calling `retarget`, so a Religion branch inherits the `assess()` bypass measured
in the entry above: an assigned-Religion seat with no religion yet skips the
expansion test every other assigned lane reaches. The branch is therefore
projected by an empire that stops growing, while the adaptive branch it is
ranked against keeps growing.

The screen flips one flag on **one agent at one position** and reads the branch
values the review actually compares — paired by construction: same game, same
seat, same plan in force, same budget, one boolean apart.

**24 positions at turn 40, 4p 60×38, 6 city-states, shipped genome, seed 2100000:**

| | |
|---|---|
| religion branch value moved | **20 of 24** |
| direction | **10 up / 10 down** |
| mean change | −0.0037 against a branch spread of 0.060 |
| **argmax lane changed** | **6 of 24 — one review in four** |
| toward Religion / away | **3 / 3** |

**Not inert, and not the predicted bias.** The defect is real and moves the
search's decision on a quarter of reviews, but it does not systematically favour
or disfavour Religion — the direction is a coin flip. My prediction was that it
would bias the ranking *against* the lane this engine converts best. It does
not.

**Sampling turn matters and nearly hid this.** The same screen at turn 60 moved
only 2 of 8 positions and changed no decision. By turn 60 many seats already
have a religion, so the arm no longer applies. A screen that samples after the
window it is testing reads as null for the wrong reason.

**The signature is the one `continue_from_plan` had.** That treatment changed
the decision on 14 of 57 positions — one review in four — with the dispersion
distribution unchanged and no directional story for which lane benefited
(p=0.42), and it measured **+37 Elo**. This is the same shape. That justifies
spending one evaluation; it is **not** evidence of a gain, and the resemblance
is exactly the post-hoc pattern-matching this document has been burned by
before.

### The design error the screen caught

The flag was first written to apply to the **branches only**, on the theory that
this is a pure fidelity repair. That is backwards. Fidelity means the projection
matches what the agent *would actually do* — and the acting agent still stops
expanding when it commits to Religion. A branch that keeps expanding projects a
game that will never be played, which is *less* faithful, not more. The two must
move together, which is what `StrategicAi::set_religion_may_expand` now
enforces and what the `strategic_religion_expand` entrant uses.


## 2026-07-28 — where the other 82% of the expansion deficit probably is (HYPOTHESIS, not measured)

⚠ **This section is code-derived and has not been measured.** It is recorded as
a hypothesis with a named test so the next iteration does not re-derive it, and
it must not be cited as a result.

Repairing the `assess()` bypass recovered 0.44 of the 2.42-city gap between a
Religion-targeted seat (1.68 cities) and an adaptive one (4.10). The other ~2.0
cities are elsewhere. Reading the production path, the cap is *not* the
constraint and there are two plausible mechanisms, both keyed on the strategy
label rather than on any target:

1. **Settlers are out-competed, not forbidden.** `production_value`
   (`advanced.rs:7044`) gates a settler on
   `city_count + counts.settlers < plan.desired_cities`, and `desired_cities`
   is `(3 + turn/cadence).min(capacity).min(6)` — **strategy-independent, and
   at least 3**. So a seat stalling at 2.12 cities is not hitting its cap; the
   settler's flat `920.0 + site*4.0` is simply losing to something else. Under
   `GrandStrategy::Religion` a prophet project carries a **2.8×** affinity
   (`:2212`) while a religion is unfounded, and the Holy Site chain is the
   preferred district (`:3822`).

2. **The empire grows slower, so it qualifies later.** `yield_value` (`:2143`)
   weights food at **1.4 under Religion against 2.0 under Expansion**, and the
   settler gate also requires `city.pop >= 2`. Slower growth delays every
   city's eligibility to build a settler at all, which compounds.

**The test.** Add a per-condition settler-production and pop-trajectory count to
`commit_curve` and compare a Religion-targeted seat against the adaptive
control. Mechanism 1 predicts settlers *queued and displaced* — the gate passes
and the item loses on value. Mechanism 2 predicts the gate itself failing on
`city.pop >= 2`. They call for different repairs, and the counts separate them
directly.

**Why it may not be worth repairing at all.** A lane's whole point is to spend
differently, so some expansion loss is the *intent*, not a defect. What made
the `:1807` bypass a defect was that it skipped a test every other lane takes —
an inconsistency, not a preference. Nothing above is inconsistent in that way.
The reason to measure it is that the macro search projects branches through
this same path, so whatever it is worth, the search is already paying it.


## 2026-07-28 — ★ the expansion "bypass" is load-bearing: repairing it costs 53 Elo

Pre-registered at `/Users/martin/civvis-religion-expand-preregistration.md`,
prediction recorded as **NULL**, decision rule fixed in advance: anything short
of a gate PASS ships the flag off with the result recorded and no seed re-rolls.

```
ai_eval strategic_religion_expand strategic --players 4 --pairs 120 \
  --turns 500 --seed 2600000 --jobs 8
```

**120 maps, 240 games, average 162.9 turns:**

```
game-win share     102/240 (42.5%)  against  138/240 (57.5%)
paired-map score   42.5%   (95% Wilson CI 34.0%..51.4%)   Elo-equivalent -53
paired outcomes    6 sweeps / 90 neutral / 24 sweeps against
paired direction   exact two-sided sign p = 0.0014   SIGNIFICANT for the control
anytime-valid      control e = 2.169e2, crossed at map 63; treatment never crossed
promotion gate     INCONCLUSIVE (it has not cleared parity — it is below it)
terminal score     49.9%, direction 53/7/60, p = 0.5727 — a dead heat
```

**The prediction was wrong in the safe direction: I said null, and it is
significantly negative.**

### The repair worked mechanically, and that is why it lost

| diagnostic | treatment | control |
|---|---|---|
| **cities** | **2.59** | 2.50 |
| pop / tech / military | 16.3 / 14.5 / 175.6 | 15.5 / 13.9 / 162.3 |
| **faith** | **436.4** | **578.8** |
| religions founded | 0.45 | 0.61 |
| missionaries | 0.36 | 0.49 |
| **religious victories** | **79** | **117** |
| domination victories | 14 | 8 |
| turns planning religious | **29.3%** | **36.1%** |
| turns planning domination | **13.8%** | 10.1% |
| dominant-plan religious seats | 123, winning 22.8% | 164, winning **27.4%** |

The treatment expanded more, grew more, teched more and fielded more military —
**every empire aggregate moved in its favour, and it lost 53 Elo.** Terminal
score was a dead heat while wins moved significantly, which is this document's
oldest result restated: no functional of a finished empire is win probability.

### The mechanism, and why it inverts the intuition

Expansion costs value *inside the rollout horizon* — a settler is production
spent now for yields that arrive after the branch has already been scored. So
letting a religion branch expand **lowers that branch's projected value**
(`lane_projection` measured mean −0.0037), the search commits to religion less
(36.1% → 29.3% of turns, 164 → 123 dominant seats), and the freed reviews route
to domination (10.1% → 13.8%), which converts far worse.

> **The bypass is not a defect from the search's point of view. It is an
> accidental correctness.** Crippling a religion branch's expansion makes
> religion *look* better in projection, and religion is the lane this engine
> actually converts — so a bug that biases the projection toward the best lane
> was paying 53 Elo of rent.

**What this retires.** The `assigned_religion_may_expand` /
`branch_religion_may_expand` family, in both scopes. Both flags stay **off**,
so the repository ships zero behaviour change from this line. Do not re-open it
by proposing to apply the repair "only to the actor" or "only to the branches" —
the actor-only version was already measured null end-to-end (4 helped / 7 hurt,
p=0.5488), and the branch-only version is the incoherent one (it ranks a game
the agent will not play).

**What it establishes that is worth keeping.** A change that moves the branch
values by a *mean of 0.0037*, and flips the lane on one review in four, is worth
**53 Elo**. Victory routing in this engine is extremely sensitive — far more
than the null results on `refuse_unreachable_lanes` and lane rotation suggested.
That is a reason to keep working on routing, and a warning that the sign of any
routing change is not predictable from its mechanism. Both directions of this
axis have now cost real Elo: `focused_deepening` and `adaptive_horizon` by
pruning the candidate set, this by re-valuing one branch.

**A caution on the horizon interaction.** The finding depends on expansion being
under-valued at the rollout horizon, so it is a statement about the search's
*window*, not only about routing. An agent whose rollouts ran long enough for a
settler to pay back might well prefer the repaired branch. `strategic_deep`
(4× budget) is the natural place to check that, and it is a different
experiment, not a re-run of this one.


## 2026-07-28 — ⚠ CORRECTION: the horizon explanation for the −53 Elo result is refuted

The entry above attributed the loss to the rollout window: *"expansion costs
value inside the rollout horizon — a settler is production spent now for yields
that arrive after the branch has already been scored"*, and it closed by
proposing a `strategic_deep` re-run on the grounds that *"an agent whose
rollouts ran long enough for a settler to pay back might well prefer the
repaired branch."*

**That is wrong.** `lane_projection --horizon`, same 24 positions at turn 40,
seed 2100000, shipped genome — the only thing varying is the rollout window:

| horizon | mean change in the religion branch's value | lane changed |
|---|---|---|
| 40 | −0.0037 | 6 of 24 |
| 80 | −0.0053 | 4 of 24 |
| **160** | **−0.0428** | 3 of 24 |

The penalty **grows by an order of magnitude** as the window lengthens. If the
settler-payback story were right the sign would move the other way. It does not,
so the horizon is not what is doing this, and **the proposed `strategic_deep`
re-run is retired before it was run** — it would have cost about four hours to
confirm a mechanism a two-minute screen refutes.

### The reading that survives

A longer projection does not reveal the settler paying back; it reveals the lane
*failing*. Religion is gated on a **finite global race** — `max_religions()`
prophet slots, claimed first-come — and `religious_opening_viable` closes at
turn 120. Production diverted to settlers delays the Holy Site and the Prophet,
and a lost race is not recoverable later at any price. The longer the rollout
runs, the more of that failure it actually simulates.

So the `:1807` bypass is **not an accidental correctness and not a bug.** It
encodes a real constraint of the lane, and its own comment says so in as many
words: *"A Prophet is a finite global race, not an economic goal that can wait
until the generic city target is complete."* The code was right and the reading
that called it an inconsistency — mine — was wrong. What made it *look* like a
defect was that it is the only assigned lane skipping that test; the reason it
is the only one is that it is the only lane with a finite, first-come gate.

**Standing lesson, and this is the second time this loop.** Both of my mechanism
stories for this result were wrong, and the first was published here before it
was screened. The measurement (−53 Elo) was never in doubt; the *explanation*
was, twice. Attaching a plausible mechanism to a real number is not free — it
directs the next experiment, and here it was about to direct four hours at the
wrong one. **Screen the mechanism as hard as the result.**

### A caveat on this instrument, found while using it

`lane_projection`'s closing READING branches on the *count* of lane changes by
direction, and at 24 positions those counts are 3-versus-1 and 1-versus-2 — far
too noisy to carry a verdict. At horizon 80 it printed "the search was
under-selecting Religion" on a 3/1 split while the mean change was **negative**.
Read the mean change, which is stable across all three windows and monotone; do
not read the branch text at this sample size. The instrument overstates its
confidence and should be fixed or its verdict line dropped.


## 2026-07-28 — ★ expansion is not site-limited or gate-limited: the settler loses the production argument, 100% of the time

`src/bin/expansion_funnel.rs`. PR #366 measured the agent wanting six cities and
settling 4.86, and closed with an honest unknown: *"expansion is limited by
execution rather than by ambition. Whatever binds sits in settler production,
site availability, or the expansion window."* Those are different defects with
different repairs. This separates them.

`production_value`'s settler arm (`advanced.rs:7044`) refuses a settler unless
five conditions hold, and then the item still has to beat everything else in the
queue. The probe samples every major seat every turn and attributes each turn
the empire was short of its planned target to the **first** condition that
failed, in the order the code tests them. Conditions 1–4 are reproduced from the
agent's own formulas; condition 5 uses `AdvancedAi::any_settle_site`, added for
this and used by nothing in the decision path.

**12 maps, 4p 60×38, 6 city-states, 500 turns, seed 2400000, 48 seats, shipped
genome. Cities at end 4.02 against a planned target of 5.25:**

| bucket | share | seat-turns |
|---|---|---|
| already at target | 34.4% | 4530 |
| settler already walking (one-at-a-time rule) | 17.3% | 2280 |
| expansion window closed | 14.3% | 1881 |
| no city at pop 2 | 8.2% | 1078 |
| **no reachable site** | **0.0%** | **0** |
| **lost the production argument** | **25.8%** | **3391** |

**Zero.** Across 48 seats and 12 full games there is not one turn where the
empire wanted a city, was permitted one, and had nowhere to put it. **The map is
never the constraint.** Every one of those 3391 turns had a site and built
something else.

### What that indicts

A settler is worth `920.0 + site_value * 4.0` — and **920 is a hardcoded
constant.** It is not a gene, so no run of `evolve` has ever tuned it, and it is
not a doctrine lever, so the macro search has never perturbed it. One
un-searched literal decides a quarter of all expansion decisions in the
subsystem the oracle ablation identified as the only one with headroom.

Four independent lines now point at expansion valuation:

1. this funnel — 25.8% of seat-turns, site available, settler out-competed;
2. the agent misses **its own** target (4.02 against 5.25), so this is not a
   disagreement about how many cities to want;
3. `gene_leverage` measured **scrambling the expansion block as a *help*** at
   1.8 SE — random draws beating the shipped values, which points at values
   that are wrong rather than values that do not matter;
4. the oracle ablation put the leverage in the economy, and city count is its
   largest single multiplier.

### What it does not establish

That raising the settler's price wins. Two previous expansion results went the
other way — `settler_min_pop = 5` produced a 3.0 SE score-share gain that
converted to **exactly zero** win improvement, and raising the `.min(6)` ceiling
was inert because the agent never reaches its target. This funnel explains the
second of those (the target is not the binding gate) and is orthogonal to the
first (that moved *when* a settler is allowed, not what it is worth).

The magnitude is also unconstrained: the probe says the settler loses, not by
how much. Nothing here names 1.6× or 2× or any other number, and picking one by
trying several against the same maps is exactly the selection bias that cost
this repository a recorded coordinate-descent result. One pre-registered
magnitude, one run.


## 2026-07-28 — ⚠⚠ CORRECTION: the settler does not lose the argument, it is never asked

The entry above concluded that expansion is limited by *what a settler is
worth*, on the strength of a 25.8% residual bucket labelled "lost the production
argument". **That label was wrong, and the conclusion drawn from it was wrong.**

The treatment built on it — `settler_price`, a multiplier on the settler's
`920.0 + site*4.0` — was run as an upper bound at `settler_price = 100.0`, a
settler that outbids everything by two orders of magnitude:

```
ai_eval advanced_settler_first advanced --players 4 --pairs 120
  --turns 500 --seed 2800000
paired-map score 48.8%, Elo -9, sign p=0.4531, gate INCONCLUSIVE
```

**That null is not a result, because the treatment never fired.** The
fires-check — the same census re-run with the treatment applied, which is what
should have been done *before* the eval:

| bucket | price 1.0 | price 100.0 |
|---|---|---|
| lost the production argument | 28.4% | **28.8%** |
| cities at end | 2.21 | **2.27** |
| settlers started | 2.46 | **2.52** |

A settler worth 100× produced 0.06 more cities. The multiplier does not reach
the decision.

### Why, and what the residual actually is

`advanced_production` (`advanced.rs:6923`) opens with

```rust
if !g.cities[&cid].queue.is_empty() { continue; }
```

**A city that is already building something is skipped entirely.**
`production_value` is consulted only when a queue runs dry, so the settler is
not out-scored — it is never scored. Splitting the residual on whether *any*
city was free to choose:

| bucket | share | seat-turns |
|---|---|---|
| **every city mid-build** | **25.8%** | 1826 |
| lost on value, with a free city | **2.6%** | 186 |

So the genuine valuation loss is **2.6%**, not 25.8%, and the settler-price axis
is closed on an upper bound that was inert — which is to say, not closed at all,
merely irrelevant at the margin it was tested.

### The finding that replaces it

**This agent never reconsiders what a city is building.** Across 48 seats, on
25.8% of all seat-turns the empire was short of its own planned city target,
permitted a settler, had a reachable site, and every city was locked into an
existing build. The plan above it re-assesses every 5 turns
(`plan_stale`); the production queue underneath it re-assesses **never**.

That is the opposite defect to the one recorded earlier in this document for the
grand strategy, and the two sit one layer apart: the lane churns 14 times a game
while the build queue does not change its mind once.

**The treatment this licenses** is production preemption — let a city abandon a
part-built item when a candidate's value greatly exceeds it. Civ 6 banks
progress per item, so switching is close to free, and a strong human switches to
a settler routinely. **Its fires-check is already written**: the
`every city mid-build` bucket must collapse, and `expansion_funnel` reports it.

### The process failure, recorded plainly

I attributed a residual bucket to a mechanism without checking that the
mechanism could move it, wrote that attribution into this document as a finding,
and spent a 240-game evaluation on it. The repository's own standing rule —
*"a treatment needs a fires-check before it needs an evaluation"*, earned at the
cost of a 240-game expansion-ceiling run — is exactly the rule I broke, and it
cost another 240 games. **A probe's bucket labels are hypotheses about the code,
not observations.** The fix that caught it took four minutes: re-run the census
with the treatment on and check the bucket moves.


## 2026-07-28 — production preemption fires, changes the games, and does not buy a single city

The entry above named production preemption as the treatment the expansion
funnel licensed, and wrote down its fires-check: *"the `every city mid-build`
bucket must collapse."* Both halves of that turned out to be wrong, and the
fires-check caught it before an evaluation this time.

`preempt_margin` (1.0 = off, the shipped behaviour) lets a city abandon a
part-built item when a candidate scores more than `margin ×` the current item's
present value. Switching is close to free here: `City::production_progress`
banks a paused build by item key, which is the Civ 6 rule.

Same census, 48 seats, 4p 24×16, 500 turns, seed 2400000:

| | margin 1.0 (off) | margin 1.5 |
|---|---|---|
| **cities at end** | **2.21** | **2.21** |
| settlers started | 2.46 | 2.42 |
| every city mid-build | 1826 seat-turns | 1856 |
| no reachable site | 91 | 143 |

**The treatment fires** — the games visibly diverge, and the site bucket nearly
doubles because the empires end up somewhere different. **It buys no
expansion at all.**

### Two errors in the fires-check itself, both mine

1. **The named bucket could not have collapsed.** "Every city mid-build" counts
   turns where every queue is *non-empty*. Preemption changes what is *in* a
   queue, never whether one exists. The criterion was unfalsifiable in the
   helpful direction — it could only ever stay flat or rise. The bucket that
   actually answers the question is the outcome: cities at end, and settlers
   started. Both are flat.
2. **The diagnosis it came from was too quick.** A city being mid-build is not
   what stops the settler, because when that queue does empty the settler is
   scored then — and the genuine free-city valuation loss is only 2.6%. The
   large blocking bucket is **"no city at pop 2" at 23.8%**: the cities are too
   small to make settlers, which is a *growth* constraint and untouched by
   anything in the production ranking. #496 already went at the food ceiling
   that gates the first settler.

**So the expansion axis does not close on preemption, and preemption does not
open on expansion.** They are separate questions and this measurement separates
them for four minutes of compute.

### What survives about preemption

The structural observation is still true and still unusual: **the plan layer
re-assesses every 5 turns and the production queue re-assesses never.** That is
a real asymmetry, preemption is the mechanism that removes it, it demonstrably
changes play, and switching costs almost nothing in this engine. What it is
*not* is a fix for expansion, and it must not be evaluated under that banner —
the motivation an evaluation is run under is what its result gets recorded
against, and a treatment that wins for reasons unrelated to its stated mechanism
has produced two retractions in this document already.

Left in the tree at `preempt_margin = 1.0`, which is exactly the shipped
behaviour, with the fires-check recorded and no evaluation spent.


## 2026-07-28 — routing the opportunistic war behind the Prophet race: mechanism fires, wins do not move

Pre-registered at `/Users/martin/civvis-prophet-first-preregistration.md`.
`advanced_prophet_first` tests `religious_opening_viable` **before** the arm that
fires Conquest on a bare power ratio, instead of after. `at_war` keeps its
priority, so only the *opportunity* to start a war is deferred, never a war
already running.

```
ai_eval advanced_prophet_first advanced --players 4 --pairs 120
  --turns 500 --seed 3100000

paired-map score   50.8%  (95% Wilson CI 42.0%..59.6%)   Elo-equivalent +6
paired direction   9 for / 104 neutral / 7 against    sign p=0.8036
promotion gate     INCONCLUSIVE
terminal score     49.4%, direction 29/49/42, p=0.1539
```

**The mechanism fired, cleanly and in the direction specified:**

| diagnostic | treatment | control |
|---|---|---|
| faith | **246.5** | 209.5 |
| religions founded | **0.53** | 0.47 |
| religious victories | **108** | 102 |
| domination victories | **5** | 9 |
| military | 188.3 | 190.9 |
| gold | 428.1 | **557.2** |

So the reorder does what it says — the empire enters the prophet race more
often, founds more religions, and converts more of them, while starting fewer
wars. **None of it reaches the win rate**, and the evaluator's own note is the
right summary: wins favour the treatment while terminal score favours the
control, which separates victory routing from development rather than
contradicting itself.

**Recorded as a null and the flag ships off**, on the `advanced_lane_reachable`
precedent and on the pre-registered rule. No seed re-roll.

### What this does and does not license

The point estimate is +6 Elo with a 95% interval of −56..+68. **At 120 maps this
run cannot see anything smaller than about ±60 Elo**, so it excludes a large
gain and says nothing at all about a small one. It is not evidence that the
reorder is worthless; it is evidence that it is not large.

**What it does settle** is a question the `strategic_religion_expand` result
raised. That run measured moving commitment *away* from religion at −53 Elo, and
the natural inference — that moving it *toward* religion should be worth
something comparable — is now tested and **does not hold at that magnitude.**
The relationship is not symmetric: the shipped agent is at or near the point
where more religion stops paying, even though moving away from it is expensive.

That asymmetry is worth more than the null. It means the −53 Elo was **not**
"religion good, conquest bad" — it was the search being moved off a local
optimum it is already sitting on, in a direction that happens to be steeply
downhill. Any future routing treatment should be read that way: the question is
not which lane is best, it is which direction off the current point is less bad.


## 2026-07-28 — ★★★ the league roster has no entrant that plays the champion genome

An artifact audit, not a mechanism. `docs/SUPERHUMAN.md` §4: *"measure the
artifact before the algorithm — ask what the process actually loads, at the path
it actually runs from."* Both of this repository's shipped gains came from that,
and this is a third instance of the same defect, in a place the fix for the
first one did not reach.

### The chain

- `AdvancedAi::new()` → `BasicAi::new()` → **`w: Weights::default()`**. The
  hierarchical agent constructed with no arguments plays the *fallback* genome.
- `league::make_send_ai` resolves `StrategyKind::Builtin { ai }` by name, and
  loads the champion for exactly two: **`"advanced_evolved" | "evolved"`**.
  Everything else, including `"advanced"`, falls to `_ => AdvancedAi::new()`.
- The shipped roster `data/league/league.json` contains **three** builtin kinds:
  `advanced` (rating 1702.7, 331 games), `advanced_v1` (1754.6), `basic`
  (1490.2). **`advanced_evolved` is not among them.**
- `Session::ai_fleet` (`server.rs`) seats the roster's best-rated strategy per
  civ when it has one, and otherwise every major seat gets the entrant *named*
  `"advanced"`.

**So no seat in the exhibition, and no entrant in the league, can play the
gen-14 champion that `#471` embedded in the binary.** The genome is loadable —
`ai_eval` prints `advanced_evolved: loaded best.json` — but nothing in the
roster asks for it.

### What that is worth, measured

```
ai_eval advanced_evolved advanced --players 4 --pairs 300 --turns 500
  --seed 3700000

game-win share     350/600 (58.3%)  against  250/600 (41.7%)
paired-map score   58.3%  (95% Wilson CI 52.7%..63.8%)   Elo-equivalent +58
paired direction   80 for / 190 neutral / 30 against   sign p = 0.0000
anytime-valid      e = 7.834e4, crossed at map 141
terminal score     54.4%, 222 / 0 / 78, p = 0.0000
promotion gate     PASS
```

Confirmed first at 120 maps on a disjoint seed (3400000): 54.6%, Elo +32,
22 for / 11 against, terminal-score direction 85/35 at p=0.0000. Two disjoint
seeds, same direction, the larger one clearing the unmodified gate.

This also independently reproduces the genome's recorded worth — `#471` measured
57.0% and +49 Elo through `evolve`'s gate on 1300 maps — **in the deployed
`advanced` agent**, which is the thing the roster actually seats.

### The fix, and what it deliberately does not do

Added `advanced_evolved` to `data/league/league.json` as a **new entrant**, which
is the only change that does not damage something:

- Redefining `Builtin:advanced` to load the champion was rejected. It would
  silently reinterpret an entrant with **331 games of rating history**, and
  every published number naming `advanced` with it. This document exists partly
  because that class of silent redefinition is expensive.
- Seeded at `advanced`'s own rating (1702.7) with a **new entrant's RD of 350**,
  *not* at 1702.7 + 58. A head-to-head against one opponent is not a Glicko
  rating; the wide RD is how the league is told it does not yet know, and it
  will converge in a handful of rounds.

⚠ **This does not immediately make the exhibition stronger, and should not be
described as if it does.** `seat_by_civ_seeded` picks from the best-rated
available strategies, and a fresh 1702.7 entrant will not be in that set until
the league has rated it. What the change does is make the strongest known agent
*reachable* by the rating machinery at all, which it currently is not.

⚠ A second caution, unmeasured: when the exhibition *does* seat from the roster
it picks league-bred `Advanced(genome)` entries rated up to 1823 — and the
best-rated of those, `g20-21` at 1790.8, measured **−98 Elo** when transferred
into `strategic_deep` (`docs/LEAGUE_GENOME_CHALLENGER.md`). Whether the top of
this roster is genuinely stronger than the champion is an open question this run
does not answer, and it is the more valuable one.


## 2026-07-28 — ★★★ the roster's top-rated genome is 108 Elo weaker than the champion it outranks by 121

The entry above added an entrant that can play the champion and left the more
valuable question open: *when the exhibition seats from the roster it takes the
best-rated strategy — is the top of that roster actually strong, or only
well-rated?* This answers it.

`advanced_league_top` builds `AdvancedAi::with_weights(w)` from the
highest-rated **active** `Advanced { weights }` entry in the *shipped* roster,
so it is reproducible from the tree. That is **`g56-50`, rating 1823.3**
(rd 50.3, 21 games) — the strategy `Session::ai_fleet` would prefer.

```
ai_eval advanced_league_top advanced_evolved --players 4 --pairs 300
  --turns 500 --seed 4100000

game-win share     210/600 (35.0%)  against  390/600 (65.0%)
paired-map score   35.0%  (95% Wilson CI 29.8%..40.6%)   Elo-equivalent -108
paired direction   18 for / 174 neutral / 108 against    sign p = 0.0000
anytime-valid      advanced_evolved e = 1.233e15, crossed at map 32
terminal score     48.0%, 118 / 182, p = 0.0003
promotion gate     RETAIN advanced_evolved
```

**The roster ranks these two backwards by about 230 Elo.** g56-50 is rated 121
points *above* `advanced` (1823.3 against 1702.7); the champion, which this
document just measured at +58 over `advanced`, beats g56-50 by 108.

This is a stronger form of an already-recorded result.
`docs/LEAGUE_GENOME_CHALLENGER.md` found the second-ranked genome, `g20-21`
(1790.8, 216 games), losing 98 Elo when transferred into `strategic_deep` — but
that involved a transfer into a different search, which leaves room for the
transfer to be the problem. **This has no transfer.** Two `AdvancedAi`s, one
genome apart, is precisely the comparison the roster's numbers claim to rank,
and they get it backwards.

### Why, and what it does not mean

Both halves of the pipeline that produced these ratings select on proxies.
`evolve` breeds on `50·players·score_share + 12·players·combat_share` — a
continuous statistic with a 24% weight on combat, which this engine converts
almost never — and the league then rates the survivors under a Glicko-2 pool
whose confounded era is on record in `docs/RATING.md`. A genome bred on a proxy
and ranked by a rating fitted over proxy-bred peers is not measured against
winning at any point in that chain.

**It does not mean Glicko-2 is broken now.** `#282` re-measured the corrected
pool at +0.4743 nats/game and 42.2% accuracy on 6502 games; the rating machinery
works. What is stale is **this snapshot**, whose entries were bred and rated
before those fixes, and which no run has re-rated since.

**Two caveats that are part of the result.** g56-50 carries only 21 games at
rd 50.3, so its rating is unresolved as well as wrong — but it is the entry the
seating rule prefers, and the far better-established g20-21 (216 games, rd 31.0)
independently measured −98. And `seat_by_civ_seeded` actually ranks on
per-leader/civ `leader_elo`, not the global rating this entrant reads, so the
seated strategy for a given civ may differ; the global top is representative,
not identical.

### What follows

The entrant added in the previous entry is seeded at `advanced`'s 1702.7 with a
new entrant's rd of 350, and that seed is now **known to be too low** — two
gate-quality runs place the champion above both `advanced` and the roster's top.
It is deliberately left there. Importing a head-to-head margin into a Glicko
pool as a starting rating is not a rating, and rd 350 is the mechanism that
exists for exactly this: the league will move it faster than any number chosen
by hand, and it will do so on games rather than on assertion.

The real repair is to **re-rate the shipped snapshot**, which is a league run and
not a data edit, and is the owner's call. What this run supplies is the reason
to spend it: the exhibition currently prefers, by rating, an agent measured 108
Elo weaker than one the binary already carries.


## 2026-07-28 — the exhibition and the league build their agents from different factories, and one of them downgrades silently

Continuing the artifact audit. There are **two** factories turning a roster
entry into a playing agent, and they do not agree:

| caller | factory | `Builtin { ai }` resolution |
|---|---|---|
| the **league** (`league.rs:1554`) | `make_ai` | delegates to `elo::builtin_ai` — knows every name in `BUILTIN_AIS` |
| the **exhibition** (`server.rs`) | `make_send_ai` | its own match: `basic`, `advanced_v1`, `random`, `advanced_evolved`/`evolved`, then **`_ => AdvancedAi::new()`** |

`StrategyKind::Builtin`'s own doc comment says the field is *"One of
`elo::BUILTIN_AIS`"*, and that list contains **`strategic`, `strategic_deep`,
`policy`, `neural`**. None of them is handled by `make_send_ai`.

**So a roster entry naming a search agent would be rated by the league as that
agent and seated by the exhibition as a default-genome `AdvancedAi`** — the
ratings and the games would describe different players, with nothing reporting
the substitution. This is the same silent-fallback shape as
`load_champion(…).unwrap_or_default()`, which cost this repository its largest
single measured gain before anyone noticed.

⚠ **It is latent, not live.** The shipped roster contains only `advanced`,
`advanced_v1` and `basic`, all of which the two factories resolve identically,
and the `advanced_evolved` entry added earlier today is handled by both. Nothing
is currently mis-seated. The defect is that the roster is *documented* as
accepting names the exhibition cannot honour, and the failure mode is silent.

The minimal repair is for `make_send_ai` to stop swallowing unknown names —
either delegate to one shared factory or panic on a name it does not implement.
`src/league.rs` is claimed by **#273**, so this is written up rather than
applied, and raised there.

### `Send` is not the reason

The obvious excuse for the duplication is that `make_send_ai` needs
`Box<dyn Ai + Send>` and the search agents might not be `Send`. **They are.**
A probe arm constructing `StrategicAi::with_weights(load_champion(…))` inside
`make_send_ai` compiles clean under `cargo check --lib`. Whatever the second
factory exists for, the trait bound is not it.

### Cost is a real reason, and it does not exclude everything

Measured on this machine, 8 games at 4p 24×16, 300-turn cap, single job:

| agent | 8 games | relative | implied per game-turn, 4 seats |
|---|---|---|---|
| `advanced` | 8.1 s | 1.0× | ~6 ms |
| `advanced_evolved` | 8.5 s | 1.05× | ~6 ms |
| **`strategic`** | **118.6 s** | **14.6×** | **~83 ms** |
| `strategic_deep` | 358.2 s | 44.2× | ~250 ms |

The live exhibition budgets about a quarter-second per turn. `strategic_deep` at
~250 ms would consume all of it and is fairly excluded. **`strategic` at ~83 ms
is not** — it fits inside the existing budget with room, and it carries both the
champion genome and `continue_from_plan`, the +37 Elo fidelity fix.

So "the exhibition runs the scripted agent because search is too slow" is true
of the deep configuration and **not** of the promoted one. What that upgrade is
worth is a separate measurement and is running.

⚠ The 14.6× and 44.2× are wall-clock on 24×16 four-player games. The exhibition
runs 74×46 at six players, where rollout cost scales with map and seat count, so
these ratios are a lower bound on what it would cost there. Do not treat the
83 ms as a budget for the live profile without re-measuring on it.


## 2026-07-28 — ★★★ the promotion gate has never measured the game that ships

`ai_eval` had **no `--speed` flag**. Every result in this document — including
`continue_from_plan` at +37, `strategic_deep` at +45, the gen-14 genome at +49,
and the +58 measured earlier today — was produced at the **default** game speed.

The exhibition and the live league both run **Online** (`data/speeds.json`:
`turns 250`, `cost_pct 50`), which `docs/EXHIBITION`/#513 records as the one kind
of game it simulates. Nothing in this repository has ever checked that a gain
measured on one transfers to the other, because until now it could not.

`--speed` added to `ai_eval`, defaulting to the previous behaviour. The first
thing it was pointed at is the gain measured this morning:

| | Standard (what the gate measures) | **Online (what ships)** |
|---|---|---|
| paired-map score | 58.3% | **51.5%** |
| Wilson CI | 52.7%..63.8% | 45.9%..57.1% |
| Elo-equivalent | **+58** (+19..+98) | **+10** (−29..+50) |
| map directions | 80 for / 30 against | 60 for / 51 against |
| sign p | **0.0000** | **0.4478** |
| gate | **PASS** | **INCONCLUSIVE** |

300 maps each, same size, same seat count, same agents; only the speed differs.
Games average 152 turns at Standard and 112 at Online.

**The difference is significant, not an eyeball.** Conditioning on the maps that
broke — the only ones carrying information — the treatment's share of decisive
maps falls from 80/110 = 0.727 to 60/111 = 0.541, a difference of 0.187 at
**z = 2.94, two-sided p = 0.0033**.

> **A promoted gain is a gain on the game it was measured on.** The genome is
> not shown to be worthless at Online speed — the point estimate is still
> positive and the interval still contains +50 — but it does not clear the gate
> there, and the gate is the whole basis on which it was promoted.

**What this does not say.** It does not retract the +58: that number is correct
for Standard speed. It does not show the genome is useless where it runs. And it
is one axis on one pair of agents — whether `continue_from_plan`,
`strategic_deep` and the rest survive the same check is now answerable and
unanswered. **That is the obvious next work, and it is cheap.**

## 2026-07-28 — the live league rates the legacy agent above the current one, and speed does not explain it

The live pool the spectator writes to
(`/Users/martin/civvis-spectator-src/league/league.json`, round 2271, 2211
matches) rates its three builtin entries:

| entrant | rating | games |
|---|---|---|
| `advanced_v1` (the **frozen legacy control**) | **1712.8** | 1257 |
| `basic` | 1694.1 | 442 |
| `advanced` (the current hierarchical AI) | **1669.0** | 1692 |

`advanced` ranks **11th of 12** active strategies, below both the agent it
replaced and the simple scripted one.

Head-to-head, **at the league's own speed and turn budget**:

```
ai_eval advanced advanced_v1 --players 4 --pairs 300 --turns 250
  --speed online --seed 5500000

paired-map score   74.2%  (95% Wilson CI 68.9%..78.8%)   Elo-equivalent +183
paired direction   148 for / 149 neutral / 3 against    sign p = 0.0000
anytime-valid      e = 1.801e37, crossed at map 22
```

**+183 Elo, against a rating that places it 44 Elo lower — an inversion of about
227 Elo on entries with 1257 and 1692 games.** At Standard speed the same pair
measures +114, so the gap is *larger* at Online, not smaller.

**The speed hypothesis is refuted**, which was the obvious explanation and the
one I expected to confirm. Whatever produces this rating, it is not that the
league plays a faster game.

What is left, none of it measured here: the league mixes seat counts (2, 4, 6, 8
and 12 seats appear in `matches.csv`) and maps where this evaluation is 4p
24×16; ratings are per leader/civ (`leader_elo`) and civ assignment may not be
symmetric across entrants; and `docs/RATING.md` records an earlier era in which
this pool carried negative information. **A 227-Elo inversion on two
well-sampled entries is worth more attention than any AI change currently
proposed**, because `Session::ai_fleet` seats by this number.

⚠ Both of these are measurements of the artifacts, not of an idea. Neither
required a new mechanism, and both were found by asking what the deployed
process actually runs — which is the third and fourth time that has paid in this
document.


## 2026-07-28 — what the exhibition's default seat is giving up, at Standard speed (Online check pending)

`Session::ai_fleet` seats `AdvancedAi::new()` when it is not seating from the
roster — the scripted hierarchical agent on the *fallback* genome, with no macro
search. `strategic` is the promoted search agent on the champion genome.

```
ai_eval strategic advanced --players 4 --pairs 300 --turns 500 --seed 4500000

game-win share     397/600 (66.2%)  against  203/600 (33.8%)
paired-map score   66.2%  (95% Wilson CI 60.6%..71.3%)   Elo-equivalent +117
paired direction   113 for / 171 neutral / 16 against    sign p = 0.0000
anytime-valid      e = 2.308e17, crossed at map 46
terminal score     52.4%, 185 / 2 / 113, p = 0.0000
promotion gate     PASS
```

Read with the cost table measured earlier — `strategic` at 14.6× `advanced`, or
about 83 ms per game-turn across four seats against the exhibition's ~250 ms
budget — this looks like a large, affordable upgrade to the seat the exhibition
actually plays.

⚠ **Do not act on this number yet, and that caution is the point of the entry
above.** It was produced at Standard speed. The gain measured this morning at
+58 with a gate PASS fell to +10 and INCONCLUSIVE at Online, significantly
(p=0.0033). Whether +117 survives the same check is running at
`--speed online --turns 250`, seed 5900000, and **this entry will be wrong to
cite until that lands.**

The general form of the rule this loop arrived at:

> **Every number in this document that predates `--speed` describes Standard
> speed. The exhibition and the live league run Online. Re-check before
> deploying any of them.**


## 2026-07-28 — ★★★★ two promoted gains, both measured on the wrong game; and why the league was right

The `--speed` check has now been run on the two largest gains this document
carries, and both collapse on the speed the deployment plays.

| gain | Standard (the gate) | **Online (ships)** | difference |
|---|---|---|---|
| gen-14 genome (`advanced_evolved` v `advanced`) | 58.3%, **+58**, gate PASS | 51.5%, **+10**, INCONCLUSIVE | z=2.94, **p=0.0033** |
| macro search (`strategic` v `advanced`) | 66.2%, **+117**, gate PASS | 54.0%, **+28**, INCONCLUSIVE | z=4.38, **p=0.00001** |

300 maps per cell, same size, same seat count, same agents. The z-tests
condition on decisive maps, which are the only ones carrying information.

**Neither gain is refuted.** Both point estimates stay positive, and the search
still wins on direction at Online (60 for / 36 against, p=0.0184). What fails is
the *gate*: neither clears parity on the game that ships, and both were promoted
on the game that does not.

> **Agent strength in this engine is strongly speed-dependent, and every number
> in this document that predates `--speed` was measured at Standard.**

### The same lesson, arrived at from the league — which turns out not to be broken

The previous entry recorded the live pool rating `advanced_v1` (1712.8, 1257
games) above `advanced` (1669.0, 1692 games), against a head-to-head of +183
Elo for `advanced`, and called it an inversion worth more attention than any AI
change proposed. **That reading was wrong, and the league's own match log says
so.** Over its 2211 recorded matches:

| | |
|---|---|
| games containing both `advanced` and `advanced_v1` | 478 |
| `advanced` placed ahead | **251 — 52.5%** |
| outright win rate, `advanced` | 21.2% |
| outright win rate, `advanced_v1` | **23.8%** |

**In the games the league actually plays, the two are at parity**, and the
rating is a faithful summary of them. There is no rating bug. The disagreement
is entirely between *my* evaluation profile and the league's: `ai_eval` defaults
to **4 players on 24×16 with no city-states**, while the league mixes 2, 4, 6, 8
and 12 seats on large maps, and the live exhibition runs 74×46 at six to ten.

So the general statement is broader than speed:

> **Measured strength ordering in this engine is a function of the
> configuration — speed, seat count and map size all move it, and two agents
> 183 Elo apart at 4p/24×16 can be at parity at the league's mix.** `ai_eval`'s
> defaults are not the deployment, and no result should be described as "this
> agent is stronger" without naming the profile it was measured on.

### What follows, in order

1. **Re-check the remaining promoted gains at `--speed online`** —
   `continue_from_plan` (+37) and `strategic_deep` (+45) are untested there.
2. **Re-check at the deployment's seat count and map**, not only its speed. A
   confirmation running at 6p 74×46 is the first of these.
3. **Consider whether the gate's defaults should be the deployment profile.**
   That is a policy question for the repository, not a change to make quietly:
   it would reinterpret every historical number in this document, which is
   exactly the kind of silent redefinition that is refused elsewhere here.

⚠ Retraction, recorded plainly: the previous entry's claim of "a 227-Elo
inversion ... worth more attention than any AI change currently proposed" is
withdrawn. The measurement behind it was sound and the interpretation was not —
I compared a 4p 24×16 evaluation against a rating earned on a different
distribution of games, and the league's own placement log resolves it in the
league's favour. **Checking that took one query against data already on disk,
and I published the accusation before running it.**


## 2026-07-28 — ★★★★ the gate and the deployment measure different games: mirrored pairs against free-for-all

Two explanations for the `advanced` / `advanced_v1` disagreement were proposed
here and **both are now refuted by measurement**:

| explanation | test | verdict |
|---|---|---|
| the league plays a faster speed | `--speed online` at 4p 24×16 | **refuted** — the gap *grows*, +114 → +183 |
| the league plays bigger, fuller maps | 6p 74×46, 6 city-states, Online | **refuted** — the gap grows again, **+207** (76.7%, gate PASS) |

At the exhibition's own profile the head-to-head gap is the largest measured
anywhere. Yet in the league's own 478 games containing both, `advanced` places
ahead 52.5% of the time. The third difference is the one that was there all
along, and it is structural:

```
league game composition, 2211 matches (seats, distinct strategies):
   2 seats,  2 distinct :  426
   4 seats,  4 distinct :  605
   6 seats,  6 distinct : 1090
   8 seats,  8 distinct :   14
  12 seats, 12 distinct :   75

mirrored-style (2 distinct strategies over >2 seats):     0
free-for-all (every seat a different strategy):        1784
```

**The league plays free-for-all — every seat a different strategy, and not one
mirrored game in 2211. `ai_eval` plays mirrored pairs and nothing else**: at
six players it fields three seats of each agent.

> **These are different quantities.** Mirrored pairing asks *"if half the world
> played A and half played B, which half wins?"* Free-for-all placement asks
> *"dropped alone into a field of five different strategies, who finishes
> ahead?"* Pairwise dominance does not determine free-for-all placement — a
> multi-player game admits non-transitivity, kingmaking, and outcomes decided by
> which neighbour a seat happens to draw. For `advanced` against `advanced_v1`
> the two answers differ by roughly **200 Elo**.

**Every promotion decision in this document is a mirrored-pair result. The
exhibition seats by a free-for-all rating.** Nothing has ever checked that a
gain in one is a gain in the other, and this pair is a worked example of the two
disagreeing about as strongly as they could.

### What is measured and what is inferred

**Measured:** the league is 100% free-for-all or two-seat duel and 0% mirrored;
`ai_eval` is 100% mirrored; the two disagree by ~200 Elo on this pair; speed and
map/seat-count are both eliminated as the cause.

**Not separated:** game *type* and *field composition* are confounded here. The
league's free-for-all games seat four strong bred genomes alongside the pair,
while a mirrored `ai_eval` contains only the pair. Whether it is being alone, or
being alone *among strong opponents*, that closes the gap is untested — and
`ai_eval` cannot test it, because its design fills every seat from the two
entrants. Answering it needs a field evaluator, which `civvis league` already is.

### What follows

1. **Name the game type on every result.** "Stronger" is not a property of an
   agent here; it is a property of an agent in a pairing scheme, at a speed, at
   a seat count.
2. **A gain intended for the exhibition should be validated free-for-all**, in
   the league, against the field it will actually meet — not only through the
   mirrored gate.
3. This does **not** retract the mirrored numbers. `strategic` really is +117
   mirrored at Standard and +28 mirrored at Online; those are correct statements
   about mirrored play. It retracts the *unqualified* reading of them.

⚠ Recorded against myself: this is the second explanation I proposed for the
same observation and had refuted by my own next measurement, and the third
mechanism story this loop that did not survive contact. The measurements have
held up throughout; the stories attached to them have not. **The cheap move —
asking what the log actually contains — resolved in one query what two
600-game evaluations could not.**


## 2026-07-28 — ⚠⚠ correcting the correction: the live rating IS unreliable for this pair, and one summary statistic hid why

Two entries ago I claimed the live pool's rating of `advanced_v1` above
`advanced` was a ~227 Elo inversion. One entry ago I **retracted** that on a
single number — `advanced` places ahead in 52.5% of the 478 games containing
both — and concluded the league was faithful and I was wrong.

**That retraction was over-corrected, and the same match log says so once it is
broken down instead of averaged.**

| seats | entrant | games | win% | parity |
|---|---|---|---|---|
| 2 | `advanced` | 29 | 37.9% | 50.0% |
| 2 | **`advanced_v1`** | **253** | **56.1%** | 50.0% |
| 4 | **`advanced`** | 428 | **31.8%** | 25.0% |
| 4 | **`advanced_v1`** | 102 | **10.8%** | 25.0% |
| 6 | `advanced` | 820 | 16.3% | 16.7% |
| 6 | `advanced_v1` | 484 | 12.4% | 16.7% |
| 12 | `advanced` | 75 | 9.3% | 8.3% |
| 12 | `advanced_v1` | 75 | 6.7% | 8.3% |

**At four seats the two are not close.** `advanced` wins 31.8% against a 25%
parity; `advanced_v1` wins 10.8% — under half its share. That is a large gap in
the same direction as every mirrored evaluation here (+114 Standard, +183
Online, +207 at 6p 74×46) and as the controlled free-for-all below.

**Where `advanced_v1`'s rating comes from:** it played **253 two-seat duels and
won 56.1%** of them, while `advanced` played **29**. A ninefold schedule
imbalance on the format with the highest per-game rating swing is enough to
carry a pool position that the four-seat games contradict.

**Why the 52.5% hid it.** That statistic is dominated by the 308 six-seat games,
where both sit near the bottom of a field containing strong bred genomes and
"placed ahead" is nearly uninformative — at six seats they score 16.3% and 12.4%
against a 16.7% parity, i.e. both roughly at or below their share. Averaging a
weak-signal majority with a strong-signal minority produced a number that looked
like parity and was not.

> **A single summary over a heterogeneous schedule is not evidence about either
> half of it.** I retracted a correct finding on one, and the disaggregation
> that overturned the retraction was the same one query, one column further in.

### The controlled free-for-all, at the stock budget

Four builtin entrants, one seat each, every strategy in every game, evolution
disabled, fresh 1500/rd 350, **stock 500-turn budget** (mean 274 turns, 2 of 128
games at the cap). Halfway, 128 games each:

| entrant | rating | wins | win% | parity |
|---|---|---|---|---|
| `advanced_evolved` | **1517.0** | 39 | 30.5% | 25% |
| `strategic` | 1498.5 | **49** | **38.3%** | 25% |
| `advanced` | 1487.0 | 32 | 25.0% | 25% |
| `advanced_v1` | 1483.1 | 8 | **6.2%** | 25% |

Free-for-all reproduces the mirrored ordering: `advanced_v1` collapses to a
quarter of its parity share. **So the "mirrored versus free-for-all" explanation
offered in the previous entry does not survive either** — the format is not what
separated my evaluations from the live pool's rating; an unbalanced schedule is.

⚠ Note `strategic` has the **most outright wins (49) and the second rating**.
Glicko rates placement across the whole field, and an agent can win most often
while placing worse when it does not. Which of those the exhibition should seat
on is a real question and not one this run answers.

⚠ A truncation caught and fixed mid-experiment: the first attempt passed
`--turns 250` while the league plays `default_speed()` = **standard = 500**, so
9 of 16 games hit a half-length cap. That is the failure #282/#285 exists to
prevent. The cap-hit rate is the cheap tell — 56% there against 2 of 128 here.


## 2026-07-28 — ★★★★★ the league breeds and seats on PLACEMENT; the gate promotes on WINS; they disagree by 3.5x

The controlled free-for-all finished: four builtin entrants, **one seat each so
every strategy plays every game** — a perfectly balanced schedule — evolution
disabled, fresh 1500/rd 350, stock 500-turn budget, 240 games each.

| entrant | Glicko | wins | win% | **mean place** | 1st | 2nd | 3rd | 4th |
|---|---|---|---|---|---|---|---|---|
| `strategic` | **1510.3** | 90 | 37.5% | 2.375 | 90 | 35 | 50 | 65 |
| `advanced_evolved` | 1506.7 | 73 | 30.4% | 2.433 | 73 | 53 | 51 | 63 |
| **`advanced_v1`** | **1485.3** | **17** | **7.1%** | 2.600 | 17 | **104** | 77 | 42 |
| **`advanced`** | **1483.6** | **60** | **25.0%** | 2.592 | 60 | 48 | 62 | 70 |

**`advanced_v1` is rated above `advanced` while winning 3.5× less often**, on a
balanced schedule, at the stock budget, in a controlled pool. And Glicko is
**right**: their *mean placements* are 2.600 and 2.592 — indistinguishable.

`advanced_v1` takes **second place 104 times and last only 42**. It is a
consistent non-winner. `advanced` wins 60 and comes last 70. Averaged into a
placement, those are the same agent. Counted in victories, one is 3.5× the other.

> **Mean placement is not win probability.** An agent that almost never wins but
> rarely finishes last is indistinguishable, by the statistic this pool rates on,
> from one that wins three and a half times as often.

### Why that matters here, in code

| consumer | selects on |
|---|---|
| `evolve_league` → `conservative_order` → `lower_confidence` | the **rating** (placement) |
| `Session::ai_fleet` → `seat_by_civ_seeded` | the **rating** (placement) |
| `docs/EVAL.md` promotions → `ai_eval` | **wins** |

**This repository breeds its genomes and seats its exhibition on placement, and
promotes its agents on winning.** Nothing converts between the two, and the pair
above shows how far apart they can be.

It is also the most plausible account on the table for a result measured earlier
today: the roster's league-bred genomes lose badly to the champion on *wins*
(`g56-50` at **−108 Elo**) while ranking above `advanced` on *rating*. A breeder
selecting on placement will accumulate exactly that phenotype — safe, consistent,
rarely first. ⚠ Plausible account, not a demonstration: showing it would need a
breeding run selected on wins as a control, which this does not provide.

### Four explanations tested and discarded to get here

Every one of these was proposed in this document, and each was refuted by the
next measurement:

| explanation for the `advanced` / `advanced_v1` disagreement | how it died |
|---|---|
| the league plays a faster speed | gap *grows* at Online, +114 → +183 |
| the league plays bigger, fuller maps | grows again, **+207** at 6p 74×46 |
| the league plays free-for-all, the gate mirrored | FFA **reproduces** the mirrored ordering |
| the league's schedule is unbalanced (253 duels against 29) | a balanced schedule reproduces the inversion |

The statistic was the answer the whole time, and it was visible in one column of
the match log — the placement histogram — that nothing had printed.

### What follows

1. **Any selection intended to produce a winning agent must weight winning.**
   Rating on placement is a defensible choice for a ladder people watch; it is
   the wrong objective for breeding a maximally strong AI, and it is currently
   used for both.
2. **`docs/EVAL.md` and the league are not comparable instruments** and never
   were. A gain here is a gain in mirrored win rate; a rise there is a rise in
   free-for-all mean placement.
3. The cheapest next measurement is a league run whose parent selection is by
   win rate rather than `lower_confidence`, against this one as control.


## 2026-07-28 — ⚠ CORRECTION to the entry above: the inversion was a mid-run snapshot; the compression is the real result

The previous entry was written from **round 15 of 16** and led with
*"`advanced_v1` is rated above `advanced` while winning 3.5× less often."* The
run finished one round later and **that ordering resolved.** Final, 256 games
each:

| entrant | Glicko | wins | win% | mean place | 1st | 2nd | 3rd | 4th |
|---|---|---|---|---|---|---|---|---|
| `advanced_evolved` | **1508.5** | 78 | 30.5% | 2.430 | 78 | 57 | 54 | 67 |
| `strategic` | 1506.3 | **95** | **37.1%** | **2.387** | 95 | 37 | 54 | 70 |
| `advanced` | **1489.1** | 65 | 25.4% | 2.578 | 65 | 52 | 65 | 74 |
| `advanced_v1` | 1481.9 | 18 | **7.0%** | 2.605 | 18 | **110** | 83 | 45 |

`advanced` now sits above `advanced_v1`, correctly. **The headline claim of the
previous entry does not survive to the end of its own run, and is withdrawn.**

**What survives, and it is the stronger statement:**

| pair | win-rate ratio | Glicko gap | mean-place gap |
|---|---|---|---|
| `advanced` vs `advanced_v1` | **3.6×** (25.4% vs 7.0%) | **7.2 points** | 0.027 |
| `strategic` vs `advanced_v1` | **5.3×** (37.1% vs 7.0%) | 24.4 points | 0.218 |

> **Glicko placement compresses a 3.6× difference in winning into 7.2 rating
> points** — about half a percent of the scale, and well inside the rd of 30 the
> pool carries. A selection process reading this number is very nearly blind to
> a difference that dominates the objective.

That is a weaker claim than "the ordering is inverted" and a more robust one: it
does not depend on which side of a coin-flip the last round lands, and it holds
across all three pairs. The phenotype behind it is unchanged and stark —
`advanced_v1` finishes **second 110 times and first 18**.

⚠ Also note `advanced_evolved` rates 2.2 points above `strategic` while winning
17 fewer games *and* placing worse on average (2.430 against 2.387). At rd 30.1
that gap is noise, not an inversion, and it should not be reported as one.

### The process failure, recorded

I published a headline from a **mid-run snapshot of my own experiment** rather
than waiting for it to finish, and the headline did not survive the next round.
Nothing forced that — the run had a fixed, known length and was already in
progress. The correction cost nothing but the retraction; had anyone acted on it
first it would have cost more.

**Wait for the run you designed to finish before describing what it found.**


## 2026-07-28 — ⚠ CORRECTION: the search agent is 7x over the exhibition's time budget, not "comfortably inside" it

An earlier entry measured agent cost at 4p 24×16 and concluded:

> *"`strategic` at ~83 ms is not [excluded] — it fits inside the existing budget
> with room."*

with the caveat that those ratios were a lower bound for the live profile and
should not be used as a budget without re-measuring. **Re-measured, and the
caveat was the whole story.** At the exhibition's own profile — 6 players,
74×46, 6 city-states, Online speed:

| profile | seconds per game | ms per game-turn |
|---|---|---|
| 4p 24×16, Standard (the earlier estimate) | ~1.0 | ~83 |
| **6p 74×46, Online (the exhibition)** | **201.7** | **~1833** |

**Twenty-two times the estimate, and about 7× the ~250 ms the live exhibition
budgets per turn.** That measurement seats three `strategic` agents in a
mirrored six-player game; scaling by seat share, even a *single* strategic seat
lands near 600 ms — still ~2.4× over — and an all-strategic table would be ~15×.

Rollout cost scales with map area *and* seat count because every branch
simulates the whole world forward, so a 9× larger map with 1.5× the seats is
roughly two orders of magnitude per review. **`strategic` is not deployable to
the live exhibition at its current profile**, and the earlier entry should not
be read as saying otherwise.

### What that leaves, and it is smaller than it looked

| agent | cost vs `advanced` | FFA win% (256 games, parity 25%) | mirrored Elo v `advanced` |
|---|---|---|---|
| `advanced` | 1.0× | 25.4% | — |
| `advanced_evolved` | **1.05×** | 30.5% | +58 Standard / **+10 Online** |
| `strategic` | 14.6× at 4p, **~200×** at the exhibition profile | **37.1%** | +117 Standard / **+28 Online** |

- **`strategic` is the strongest agent measured and cannot be afforded** where it
  would matter. Offline consumers — the league, breeding, promotion runs — have
  no realtime budget and can use it freely; the live exhibition cannot.
- **`advanced_evolved` is essentially free (1.05×) and directionally better.**
  But its FFA edge over `advanced` is **5.1 points at 1.28 SE — not
  significant** at 256 games, and its mirrored edge at the shipped speed is +10,
  INCONCLUSIVE. The honest summary is *cheap, plausibly better, unproven at the
  deployment profile*.

**So the deployable recommendation from this loop is modest**: the roster entrant
that lets the champion be seated at all is worth adding (it costs nothing and
cannot be worse), and no claim of a large exhibition gain is supported. A large
gain would need either a cheaper search configuration than `strategic` or a
profile the current one can afford, and neither has been measured.

⚠ This is the second time in this document an estimate taken at 4p 24×16 has
failed to transfer to the deployment profile — the first was every win-rate
number, via `--speed`. **`ai_eval`'s defaults are not the deployment, for
strength or for cost.**


## 2026-07-28 — ★ the cost-performance frontier of the macro search, which nothing had measured

Every registered search variant moves the budget **up** — `review_every` 20 and
10, `horizon` 80, the 4× `strategic_deep`. Nothing has ever asked the opposite
question, because nothing was cost-bound until the entry above measured
`strategic` at **7× the exhibition's time budget**.

`strategic_cheap` cuts all three multiplicative knobs at once:

| knob | `strategic` | `strategic_cheap` |
|---|---|---|
| `review_every` | 40 | **80** (half the reviews) |
| `horizon` | 40 | **20** (half the rollout) |
| `rotate_lanes` | off (~7 branches) | **on** (~3 branches) |

`rotate_lanes` is a **recorded null for strength**, which is exactly what a
cost-bound deployment wants from a knob.

### Cost

| agent | 8 games, 4p 24×16 Online | vs `advanced` | ms/game-turn at the **exhibition profile** |
|---|---|---|---|
| `advanced` | 6.5 s | 1.0× | — |
| `strategic` | 73.3 s | 11.3× | **1833** |
| **`strategic_cheap`** | **6.9 s** | **1.06×** | **223** |

Against a **~250 ms** budget. `strategic_cheap` is 6× cheaper in situ than
`strategic` and lands just inside it with **three** of six seats searching; a
single searching seat would be nearer 75 ms.

### Strength, pre-registered (`/Users/martin/civvis-strategic-cheap-preregistration.md`)

```
ai_eval strategic_cheap advanced --players 4 --pairs 300 --turns 250
  --speed online --seed 6700000

paired-map score   52.3%  (95% Wilson CI 46.7%..57.9%)   Elo-equivalent +16
paired direction   54 for / 206 neutral / 40 against    sign p = 0.1797
promotion gate     INCONCLUSIVE
```

Reference on the identical profile: `strategic` measured **+28** (54.0%,
p=0.0184, also INCONCLUSIVE at the gate).

| agent | search cost | Online Elo v `advanced` |
|---|---|---|
| `advanced` | — | 0 |
| **`strategic_cheap`** | **~6% of `strategic`'s** | **+16** (−23..+55) |
| `strategic` | 1× | +28 (−12..+67) |

**I predicted a null and most of the +28 lost.** The point estimate retained
more than that. ⚠ But at 300 maps this resolves about ±40 Elo, so **+16 and +28
are not distinguishable**, and neither is distinguishable from zero. The honest
reading is *a search at 6% of the cost is not measurably worse than the full
one*, not *the frontier is flat*.

That is still the useful shape for a deployment: the expensive configuration
buys nothing this evaluation can see, and the cheap one is affordable where the
expensive one is not.

⚠ **No deployment claim yet.** The pre-registration requires confirmation at the
exhibition's own profile before one, because the last two estimates taken at
4p 24×16 — every win rate, via `--speed`, and the cost table — both failed to
transfer. That confirmation is running at 6p 74×46.


## 2026-07-28 — ★★★ the cheap search is +16 on the evaluator's map and −63 on the deployment's

The pre-registration for `strategic_cheap` required a confirmation at the
exhibition's own profile before any deployment claim, *because the last two
estimates taken at 4p 24×16 both failed to transfer*. It ran, and it reversed
the result.

| profile | score | Elo | direction | sign p | gate |
|---|---|---|---|---|---|
| 4p 24×16 Online (the evaluator's default) | 52.3% | **+16** | 54 / 40 | 0.1797 | INCONCLUSIVE |
| **6p 74×46 Online (the exhibition)** | **41.0%** | **−63** | 22 / 49 | **0.0018** | **RETAIN `advanced`** |

Terminal score agrees and is stronger: 45.4%, **23 maps for / 127 against**,
p=0.0000, resting on all 150 maps rather than the 71 that broke on wins.

**`strategic_cheap` is significantly worse than the scripted agent at the
profile that matters, and mildly better at the one the evaluator defaults to.**
Recommending it from the small-map number would have shipped a 63-Elo
regression.

### Why, as a hypothesis rather than a claim

`strategic_cheap` cuts the rollout `horizon` from 40 to 20 and reviews half as
often. Twenty projected rounds is a large fraction of a 4-player game on 384
tiles and a small one of a 6-player game on 3404. The same absolute budget buys
proportionally far less foresight as the world grows, so the configuration that
is merely *cheaper* on a small map may be *blind* on a large one.

⚠ Stated as a hypothesis deliberately. Four mechanism stories in this document
have been refuted by the next measurement, and this one is untested — separating
`horizon` from `review_every` from `rotate_lanes` would need three more runs.
**The measurement stands on its own without it.**

### The strategic consequence, which is uncomfortable

| | 4p 24×16 | 6p 74×46 (the exhibition) |
|---|---|---|
| `strategic` (full search) | +28 Elo | **~200× `advanced`, 7× over the time budget** |
| `strategic_cheap` | +16 Elo, 1.06× cost | **−63 Elo**, 223 ms/turn |

At the deployment's profile the search appears to be **more necessary and less
affordable at the same time**. The cheap configuration is the only one that fits
the budget and it is actively harmful there; the configuration that helps cannot
be run.

⚠ "More necessary" is an inference, not a measurement — `strategic` at full
budget has not been evaluated at 6p 74×46 because 300 games there would cost
about 17 hours. That is the measurement that would settle it, and it is the
obvious thing to spend an overnight run on.

### The rule, now three for three

| estimate taken at 4p 24×16 | at the deployment profile |
|---|---|
| champion genome, +58, gate PASS | +10, INCONCLUSIVE (speed alone) |
| `strategic` cost, ~83 ms/turn | **1833 ms/turn**, 22× the estimate |
| `strategic_cheap`, +16 | **−63, significant against** |

> **`ai_eval`'s defaults are not the deployment — not for strength, not for
> cost, and not even for the sign of an effect.** Every number in this document
> measured at 4p 24×16 should be read as a statement about 4p 24×16 until it is
> re-measured, and the three cases above are the reason.

`strategic_cheap` stays evaluator-only with the null-and-worse recorded, per the
pre-registered rule. No seed re-roll, no knob-tuning to rescue it.


## 2026-07-28 — ★★★ the blindness is not `ai_eval`'s alone: the champion was bred on the profile that does not transfer

Adding `--speed` to `ai_eval` fixed one instrument. Auditing the rest shows the
gap is systemic — **every other binary that builds a game does so through
`Game::new`, which pins `default_speed()` = `standard`:**

| layer | file | speed |
|---|---|---|
| the evaluator | `src/bin/ai_eval.rs` | **fixed today** (`--speed`, default unchanged) |
| **the breeder** | `src/evolve.rs:162,342` | `Game::new` → **standard** |
| the league | `src/league.rs:1535` | `Game::new` → **standard** |
| 21 other probes and evaluators | `src/bin/*.rs` | `Game::new` → **standard** |

Among the 21 are the instruments that *gate decisions*: `policy_eval`,
`genome_breed`, `policy_breed`, `victory_eval`, `search_probe`, `search_dose`,
`gene_leverage`, `gene_probe`, `order_ablate`, `proxy_fit`. **Every recorded
null and every recorded gain in this document was measured at Standard speed**,
and the exhibition runs Online.

### The chain that closes

`docs/SUPERHUMAN.md` records the recipe that produced the shipped champion:

```
civvis evolve --pop 24 --generations 25 --games 96 --players 4 \
  --width 24 --height 16 --turns 500 --seed 7
```

**4 players, 24×16, 500 turns, Standard speed** — the exact profile this
document has now shown three times does not transfer, once reversing an
effect's sign. And the champion's measured worth:

| profile | champion v `advanced` |
|---|---|
| 4p 24×16 Standard — **the profile it was bred on** | **+58, gate PASS** |
| 4p 24×16 Online — the shipped speed | **+10, INCONCLUSIVE** |

> **The genome that ships was optimised on a game the deployment does not
> play.** Its advantage is largest exactly where it was selected and largely
> gone where it runs.

⚠ **Consistent with, not proof of, specialisation.** Demonstrating that the GA
overfitted its profile needs a breeding run at Online speed compared against
this one on a common holdout, which nothing has done. An alternative reading —
that Online simply compresses *all* agent differences — fits the `strategic`
result too (+117 → +28) and is not excluded. **Both readings imply the same
next action**, which is why the distinction can wait.

### What follows

1. **Breed at the speed that ships**, or at minimum evaluate the champion there
   before promoting the next one. The machinery is one `GameOptions` field away
   in `evolve.rs`.
2. **`--speed` belongs on the gating evaluators**, not just `ai_eval` —
   `policy_eval`, `victory_eval`, `genome_breed` and the `search_probe` family
   at least. Mechanical: `Game::new(...)` → `Game::new_with(GameOptions { speed, .. })`.
3. Until then, **every number in this document is a statement about Standard
   speed at 4p 24×16**, and should be written that way.

⚠ One caveat on the live league, which muddies its own record: `civvis league`
builds games at Standard, while the spectator plays Online and records into the
same directory with `--league-record`. The live pool is therefore a **mixture of
two game types**, which is worth knowing before any rating in it is used as
evidence — including in the entries above where I used it as such.


## 2026-07-28 — the breeder accepted `--speed` and ignored it, so it bred truncated games

Found while auditing the toolchain for the speed blindness above. It is not
merely that `evolve` lacked the flag — **it half-had it, which is worse.**

`main.rs` resolves the breeder's turn budget through `stock_turns(&args)`, and
`stock_turns` *does* read `--speed`:

```rust
max_turns: arg(&args, "--turns", stock_turns(&args)) as u32,
```

But both game sites in `evolve.rs` called `Game::new(...)`, which pins
`default_speed()` = `standard`. So

```
civvis evolve --speed online ...
```

set a **250-turn budget** and then played **Standard games truncated at 250** —
exactly the half-length cap `#282`/`#285` exist to prevent, arriving silently
through a flag that looked supported. A short cap does not shorten a game, it
changes the answer: at 250 turns the cap named a different winner in 13 of 24
replays.

**Fixed.** `EvoCfg` gains a `speed` field defaulting to `game::default_speed()`,
plumbed from `--speed`, and both sites now build through
`GameOptions { speed, .. }`. The default reproduces every genome this repository
has bred; `--speed online` now plays Online.

```
civvis evolve --speed online --pop 4 --generations 1 --games 2 --players 4 --width 24 --height 16
  evolve: pop 4 · 2 games/genome · 24x16 4p 250t · 2 threads
```

Four other `EvoCfg` construction sites — `evolve_probe`, `search_probe`, and two
in `genome_gate` — were updated to the explicit default so the change is
behaviour-preserving everywhere. Suite green at **1139**.

**Why this matters beyond the bug.** `docs/EVAL.md` records the shipped champion
at **+58 Elo on the 4p 24×16 Standard profile it was bred on** and **+10,
inconclusive, at the Online speed the exhibition plays**. Breeding at the speed
that ships was one struct field away and the field did not exist. It does now,
and the run that would use it — a champion bred at Online, compared against the
shipped one on a common holdout — is the experiment that separates *the GA
overfitted its profile* from *Online compresses every difference*. Both readings
are live; this makes the discriminating experiment possible.

⚠ The 21 other probes and evaluators still build through `Game::new` and remain
Standard-only. This entry fixes the breeder and the two probe families that
share `EvoCfg`, not the whole toolchain.


## 2026-07-28 — ★★★★★ the champion's advantage is a property of the profile it was bred on

The entry above left two live readings of the champion's shrinking edge — *the
GA overfitted its breeding profile* against *Online compresses every difference*
— and said both implied the same next action so the distinction could wait. It
did not have to. **The data to separate them was already here.**

The champion, audited at the deployment's own profile:

```
ai_eval advanced_evolved advanced --players 6 --width 74 --height 46
  --city-states 6 --pairs 150 --turns 250 --speed online --seed 8700000

paired-map score   48.7%  (95% Wilson CI 40.8%..56.6%)   Elo-equivalent -9
paired direction   23 for / 100 neutral / 27 against    sign p = 0.6718
promotion gate     INCONCLUSIVE
```

| profile | champion v `advanced` |
|---|---|
| 4p 24×16 **Standard** — the profile it was **bred** on | **+58**, gate PASS |
| 4p 24×16 Online | +10, INCONCLUSIVE |
| **6p 74×46 Online — the deployment** | **−9** (CI −65..+46), INCONCLUSIVE |

**The deployment interval excludes +58.** The advantage the genome was promoted
for does not exist where the genome runs.

### Which reading that kills

"Online compresses every agent difference" is **refuted**, by a measurement
already in this document. At the *same* deployment profile:

| pair | Elo at 6p 74×46 Online |
|---|---|
| `advanced` v `advanced_v1` | **+207**, gate PASS, p=0.0000 |
| `advanced_evolved` v `advanced` | **−9**, INCONCLUSIVE |

A profile that resolves a 207-Elo difference is not insensitive. It is
specifically the *champion's* edge that vanishes there.

> **The gen-14 genome is specialised to 4 players on 24×16 at Standard speed.**
> That is the profile `civvis evolve` was run at, it is `ai_eval`'s default, and
> it is not the game the exhibition plays. The GA did its job; it was pointed at
> the wrong game.

⚠ What this does **not** say. It does not say the genome is *worse* — −9 with a
CI spanning ±55 is parity, not a regression. It does not say evolution cannot
work here; it says this evolution optimised a different game. And it is one
genome at one deployment profile — the honest generalisation is about the
method, not about gen-14 specifically.

### Correction to my own change, earlier today

I added `advanced_evolved` to `data/league/league.json` and described it as
worth **+58**, then revised that to **+10** after the speed check. **At the
deployment profile it is worth nothing measurable.** The entrant is still worth
having — it makes a genome the binary already carries *reachable*, which it was
not, and at parity it costs nothing — but **no strength claim survives**, and
the seeding at `advanced`'s own rating turns out to have been the right
conservative call for reasons better than the ones I gave.

### What this makes possible, and it is the whole point of the breeder fix

The previous entry gave `evolve` a working `--speed`. The experiment this result
demands is now one command:

```
civvis evolve --speed online --players 6 --width 74 --height 46 ...
```

Breed at the profile that ships, then compare against gen-14 **at that profile**.
If the specialisation reading is right, a genome bred there should beat gen-14
there by roughly what gen-14 beats the default by at 4p 24×16. That is the
overnight run this loop has been building toward, and it is the first one whose
target profile matches the deployment.


## 2026-07-28 — ★★★★★ SYNTHESIS: every promoted gain evaporates at the deployment profile; the one unpromoted difference does not

The last of the deployment-profile measurements, pre-registered at
`/Users/martin/civvis-strategic-exhibition-preregistration.md`:

```
ai_eval strategic advanced --players 6 --width 74 --height 46 --city-states 6
  --pairs 60 --turns 250 --speed online --seed 8300000

paired-map score   43.3%  (95% Wilson CI 31.6%..55.9%)   Elo-equivalent -47
paired direction   7 for / 38 neutral / 15 against    sign p = 0.1338
terminal score     44.8%,  7 for / 53 against,  p = 0.0000
promotion gate     INCONCLUSIVE
```

I predicted *inconclusive, with the point estimate closer to zero than +28*.
Inconclusive on wins is right; the estimate went **past** zero to −47, and the
terminal-score direction is unambiguous at 7 maps against 53.

### The whole table, one profile at a time

| comparison | 4p 24×16 Standard (the gate) | **6p 74×46 Online (ships)** |
|---|---|---|
| gen-14 genome v `advanced` | **+58**, gate PASS | **−9** (CI excludes +58) |
| `strategic` v `advanced` | **+117**, gate PASS | **−47**, score p=0.0000 |
| `strategic_cheap` v `advanced` | +16 (Online 4p) | **−63**, wins p=0.0018 |
| `advanced` v `advanced_v1` — **never promoted** | +114 | **+207**, gate PASS |

> **Every improvement this repository has promoted was measured at 4p 24×16 and
> is absent — or reversed — at the profile the exhibition plays. The one large
> difference that was never promoted, between the current agent and the legacy
> one it replaced, holds and grows there.**

That last row is what makes the rest interpretable. A profile which resolves
+207 at gate-PASS strength is not insensitive and is not compressing everything.
It is specifically the *promoted* differences that fail to appear.

### What is and is not established

**Established.** The genome's edge is bred-profile-specific (its deployment
interval excludes +58, and the same profile resolves +207). `strategic_cheap` is
significantly worse where it ships. Three separate 4p 24×16 estimates — a
strength, a cost, and a sign — failed to transfer.

**Not established.** `strategic` at −47 is **inconclusive on wins** (p=0.1338 at
60 maps); only its terminal score is significant, and terminal score is not a
promotion input. The pre-registration says a result like this "warrants the full
300-map overnight run before anything in `docs/SUPERHUMAN.md` is rewritten", and
that remains the correct next step. **Nothing here retracts `strategic_deep`'s
promotion**, which has not been measured at the deployment profile at all.

**Not attempted.** Why the promoted gains are profile-specific. The genome has an
account — it was bred there. `strategic` does not: its rollout budget is fixed in
turns, so a larger, slower world gets proportionally less foresight, which is the
same pressure that plausibly sank `strategic_cheap`. That is a hypothesis and
this document has refuted five of mine.

### The two runs that follow, in order

1. **`strategic` v `advanced` at 6p 74×46 Online, 300 maps.** ~17 h. Settles
   whether the flagship search agent is a regression where it ships.
2. **`civvis evolve --speed online --players 6 --width 74 --height 46`**, then
   gen-14 against the result *at that profile*. The breeder gained a working
   `--speed` earlier today specifically to make this possible, and it is the
   first breeding run in this repository aimed at the game that ships.

⚠ Neither was launched from this session: the machine is carrying load 36 on 18
cores with another agent's evaluation running, and a 17-hour job has no business
joining that queue. Both are pre-registered and ready.


## 2026-07-28 — the pooled confirmation was launched and did not finish; what stands, and what does not

The synthesis entry above named a pooled 300-map confirmation of `strategic` at
the deployment profile as the run that would settle whether the flagship search
agent is a regression where it ships. It was launched —

```
ai_eval strategic advanced --players 6 --width 74 --height 46 --city-states 6
  --pairs 240 --turns 250 --speed online --seed 9100000 --jobs 6
```

— and **stopped incomplete** when this PR was closed out, at 27 minutes of wall
against a projected ~6.4 hours (480 games at ~202 s each, ~4.2 effective cores).
**No result from it is recorded, because none exists.** The log at
`/Users/martin/strategic-exhibition-240.log` contains only its header.

**So the deployment-profile evidence on `strategic` remains exactly the 60-map
run**, and its limits are the ones already stated:

```
paired-map score   43.3%  (95% Wilson CI 31.6%..55.9%)   Elo-equivalent -47
paired direction   7 for / 38 neutral / 15 against    sign p = 0.1338
terminal score     44.8%,  7 for / 53 against,  p = 0.0000
promotion gate     INCONCLUSIVE
```

**`strategic` is not shown to be a regression at the deployment profile.** The
win-rate direction is inconclusive at 60 maps; only terminal score is
significant, and terminal score is not a promotion input. Anyone continuing this
should treat "the flagship search agent may be a regression where it ships" as
an **open question with one suggestive 60-map reading**, not as a finding, and
run the pooled confirmation before acting on it.

The three results that do **not** depend on that run are unaffected and stand:

| finding | status |
|---|---|
| the gen-14 genome's edge is bred-profile-specific (deployment CI **excludes +58**, same profile resolves +207) | **established** |
| `strategic_cheap` is significantly worse where it ships (−63, wins p=0.0018) | **established** |
| three 4p 24×16 estimates — a strength, a cost, and a sign — failed to transfer | **established** |

## 2026-07-29 — ★★★ what the action encoding can and cannot rank

`policy_wide` lost 313 Elo because a net fit to outcomes encodes correlation and
an argmax over siblings optimises whichever correlate is cheapest to move. The
prescription recorded then was Q or advantage on **returns for actions actually
taken**, and nothing in the repository emitted that data. `q_dataset` now does,
and `q_train` fits a ranker over it — *which action was taken*, not what the
outcome was, because a head asked that question has no correlate to chase.

The dataset is built by replaying `game.log` against a fresh game of the same
seed, so it records without disturbing what it records. That replay is checked
rather than assumed: 60 games, **0 rejected applications, 0 divergent score
lines**, 2.66M rows. A run that diverges exits without claiming the file.

Held-out top-1, split **by game**.

**Mixed-kind negatives** (125,623 held-out decisions, chance 20.0%):

| features | top-1 |
|---|---|
| state only (34) | **20.0%** — exactly chance |
| kind one-hot only (77) | 54.4% |
| geometry only (13) | 54.2% |
| all (124) | 54.0% |

The full vector is no better than the kind one-hot alone. What is learned here is
that the expert moves more often than it fortifies — true, and useless for
choosing *which* move. State landing on exactly chance is both the metric's
sanity check and `policy_wide`'s failure reproduced from the other side: every
candidate in a decision shares one state vector, so state can only tie.

**Same-kind negatives** (60,363 held-out decisions, chance 21.2%):

| features | top-1 |
|---|---|
| kind one-hot only | 22.6% — chance, as it must be when the one-hot is constant |
| **geometry only (13)** | **31.8%** |
| all (124) | 31.3% |

So the thirteen geometry terms *do* discriminate siblings, by half again over
chance, on unseen games. That is the signal a move-ordering prior needs — and it
is a **prior for a unit-action search that does not exist yet**, not a policy.
`StrategicAi` searches victory lanes, not unit actions, so there is nothing in
the tree today for this to order. Read as a greedy policy its ceiling is the
expert it imitates.

**Two corrections, each of which changed a conclusion:**

- The within-kind control was first run on stride-sampled data, where only **103
  of 531,892** decisions were same-kind by chance. It reported 22.1% and meant
  nothing. `--negatives-same-kind` exists because of that.
- Tie credit is `1/k`. `max_by` returns the *last* maximum, so a head that cannot
  separate candidates read **0.0%** where the honest answer is chance — which
  made structural blindness look like a trained anti-preference and inverted the
  first reading of which feature block carried the signal.

| claim | status |
|---|---|
| the replay-based emitter reproduces the games it records | **established** (0/60 divergent) |
| state features cannot rank siblings, by construction | **established** (exactly chance) |
| mixed-kind ranking is a kind prior, not action discrimination | **established** (kind alone matches the full vector) |
| ~~geometry discriminates same-kind siblings~~ | **REFUTED below** — same-kind negatives can belong to a different unit, and 39-42% of them did. With the actor held fixed the lift is +0.5 ± 0.2 pp. The head learned *which unit acts*, not *where it should go*. |
| any of this improves play | **unmeasured** — no agent or default changed |

> **Correction (2026-07-29).** The entry above called the same-kind result the
> signal a unit-action search could order, and marked it established. It was one
> control short: same *kind* is not same *actor*, and the geometry block carries
> the acting unit's HP, strength and movement. See "the geometry learned which
> unit acts, not where it should go" below, which ran that control and dissolved
> the effect. As a move-ordering prior — the exact use proposed here — the head
> is worth half a point.

## 2026-07-29 — ★★★★ the breeding proxy is aligned with winning; its combat term is not

`evolve` selects on `50*P*score_share + 12*P*combat_share` and promotes on wins.
That split has been called a defect (including by me, three times) on the grounds
that the search operator climbs one hill while the gate stands on another. It was
never measured, so `proxy_align` measures it: play whole games with the stock
fleet, rebuild `selection_value` exactly as `eval_game_observation` does, and ask
how often the seat the proxy would pick is the seat that won.

120 games, 4 players, 44×28, 200 turns; all 120 decided by victory.

| objective | leader is the winner | 95% CI |
|---|---|---|
| chance | 25.0% | — |
| `selection_value` (shipped) | 104/120 = **86.7%** | 79.4–91.6% |
| score share alone | 115/120 = **95.8%** | 90.6–98.2% |

Mean Spearman between the proxy's ordering of the table and the ordering by final
score: **0.917**.

**The objection does not stand.** At 86.7% against a 25% baseline the proxy is
strongly aligned with winning, and the dense-signal/sparse-gate split is sound
engineering rather than a misalignment.

**But the combat term is dragging it, significantly.** The two objectives are
read on the same games, so the comparison is paired: the combat term makes the
proxy right where score alone was wrong in **2** games, and wrong where score
alone was right in **13**. Fifteen discordant pairs, exact two-sided sign test
**p = 0.0074**.

So the 12-point combat share is not a free extra signal. It costs about nine
points of agreement with the gate, and dropping it would make selection propose
candidates the SPRT is more likely to accept.

**Two limits on that recommendation, stated rather than buried:**

- This measures the proxy *within a table*, on games between identical stock
  agents. `evolve` uses it to rank *different genomes* across paired games, which
  is not the same question. An objective that cannot pick the winner in front of
  it is a weaker instrument for ranking genomes, but this run does not measure
  the second thing directly.
- The combat term is plausibly the only thing giving the war genes anything to
  select on, and `gene_probe` already found much of that block inert. Removing it
  may buy alignment at the cost of making those genes fully dead. That side of
  the trade is **unmeasured**.

| claim | status |
|---|---|
| the breeding proxy is aligned with winning | **established** (86.7% vs 25%, n=120) |
| the split between dense selection and sparse gate is a defect | **refuted** |
| the combat term reduces agreement with the gate | **established** (p=0.0074, paired) |
| dropping it would improve evolution's throughput | **untested** — it follows, but it is not measured |
| dropping it would strand the war genes | **unmeasured** |

## 2026-07-29 — ★★★ the geometry learned which unit acts, not where it should go

The action-ranking entry above established 31.8% top-1 against 21.2% chance on
same-kind alternatives and called that the signal a unit-action search could
order. **That conclusion was one control short.** Two moves can have the same
kind and belong to different units. The thirteen geometry terms include the
acting unit's HP, strength and movement, so the head can identify *which unit*
`AdvancedAi` activates without learning anything about *which destination* is
best for that unit.

Two changes make that distinction measurable:

- `q_dataset --negatives-same-actor` restricts every negative to the chosen
  action's kind **and** unit. It implies the same-kind filter, so the kind prior
  cannot return through the side door.
- `q_train --eval-data` trains on one complete corpus and evaluates on another.
  Its per-kind uncertainty is macro-averaged by game; thousands of correlated
  decisions from one trajectory no longer manufacture a tiny error bar.

The emitter now also honors `--speed` on both the played game and its replay.
Before this, requesting the six-player Online profile still recorded Standard
games — the same profile mismatch that erased every promoted gain earlier in
this file.

### Pre-registered profile-transfer test

Training: eight 4-player, 44×28, 200-turn Standard games, seeds 920000–920007.
External evaluation: eight 6-player, 74×46, 250-turn Online games with six city
states, seeds 930000–930007. Both used four negatives per decision. All sixteen
games ended in victory; both replays had **zero rejected actions and zero score
divergences**.

With same-kind negatives, the earlier result transfers strongly to the unseen
deployment profile:

| chosen kind | decisions | unseen games | chance | top-1 | game-macro lift |
|---|---:|---:|---:|---:|---:|
| all | 294,027 | 8 | 21.8% | 33.3% | **+11.5 ± 0.3 pp** |
| move | 216,313 | 8 | 20.1% | 33.5% | **+13.5 ± 0.4 pp** |
| fortify | 13,047 | 8 | 32.0% | 64.7% | **+32.6 ± 1.6 pp** |
| ranged | 1,516 | 8 | 38.3% | 43.1% | +4.8 ± 2.4 pp |
| attack | 602 | 8 | 43.4% | 50.8% | +7.4 ± 3.5 pp |

This reproduces the aggregate finding and rules out Standard-to-Online domain
shift as its explanation.

Then hold the actor fixed. The number of emitted negatives falls from 222,294
to 134,846 in the Standard corpus and from 1,115,620 to 651,035 in the Online
corpus: **39–42% of the supposedly sibling alternatives belonged to another
unit.** On the controlled corpus the result disappears:

| chosen kind | decisions | unseen games | chance | top-1 | game-macro lift |
|---|---:|---:|---:|---:|---:|
| all | 209,525 | 8 | 26.8% | 27.3% | **+0.5 ± 0.2 pp** |
| move | 204,198 | 8 | 26.4% | 26.9% | **+0.5 ± 0.2 pp** |
| improve | 3,549 | 8 | 40.2% | 40.2% | 0.0 ± 0.0 pp |
| promote | 791 | 8 | 45.3% | 45.3% | 0.0 ± 0.0 pp |
| ranged | 741 | 8 | 44.3% | 39.9% | −4.4 ± 2.2 pp |
| attack | 246 | 8 | 47.2% | 44.3% | −2.8 ± 2.3 pp |

The state-only control lands at exact chance for every kind, confirming that
tie credit and the grouped evaluator are behaving as designed.

### What remains open

Training and holding out inside the Online corpus gives move only +1.0 points.
Attack reads +10.0 points, but on **80 decisions from two held-out games**; that
is a lead for a fresh disjoint corpus, not evidence. It cannot rescue the model
that failed on all eight external games.

The mechanism is visible in the encoder. Once actor and kind are fixed, HP,
strength, moves left, treasury and Faith are constants. Adjacent move targets
usually share the remaining ownership, enemy-presence and distance flags. The
vector contains no destination terrain, local force field, route progress or
plan-relative geometry, so it has almost nothing with which to choose one empty
neighbor over another.

> **Do not wire this ranker into gameplay.** It learned activation order, not
> destination or target order, and its honest ceiling is still the expert it
> imitates. The next representation must make a target's spatial neighborhood
> and progress toward the active objective visible, then pass this same-actor,
> external-profile gate before a unit-action search spends a rollout on it.

| claim | status |
|---|---|
| the Standard-trained ranker transfers to Online games | **established**, for choosing an actor |
| the 13 geometry terms rank moves for one unit | **refuted** (+0.5 ± 0.2 pp) |
| they rank attacks for one unit across profiles | **refuted in this sample** |
| an Online-trained attack head has a signal | **open** — only two held-out games |
| integrating the current artifact would improve play | **unsupported; rejected before gameplay A/B** |
## 2026-07-29 — what the combat term is made of: signal, not noise, pointing elsewhere

Follow-up to the entry above, which established that the 12-point combat share
costs ~9 points of agreement with the promotion gate (p=0.0074, paired). That
left an obvious cheap question unanswered: *is the term carrying anything at
all?* A weighted term that barely varies between seats would be noise however it
is weighted, and removing noise is free.

Same 120 games, describing the term rather than only scoring it:

| property of `combat_share` across seats | value |
|---|---|
| mean spread (max − min) | **0.295** |
| games where it is flat (spread < 0.10) | **1/120** |
| games where every seat is zero | **0/120** |
| games where its leader differs from the score leader | **56/120 (47%)** |

**It is not noise.** It varies substantially in 119 of 120 games, is never
degenerate, and disagrees with score share about who is ahead in nearly half of
them. It is a large, well-populated signal about combat achievement — which is
simply a different quantity from winning.

That cuts both ways and both should be recorded:

- **Removal is not free.** This is real variance, and it is the most plausible
  candidate for what the war-gene block selects on — a block `gene_probe` already
  found largely inert. Dropping the term may strand those genes completely.
- **Retention is not harmless.** The 50:12 weighting contains the disagreement
  most of the time, but when it does change the proxy's verdict it is wrong 13
  times in 15.

So the trade is now stated precisely rather than assumed in either direction, and
the decision still needs the gene-perturbation run: perturb each war gene on one
seat, play paired games on the same map against the same opponents, and compare
mean |Δ| in score-only value against |Δ| in full `selection_value`.

| claim | status |
|---|---|
| the combat term is degenerate or near-uniform | **refuted** (spread 0.295, flat 1/120) |
| the combat term carries information distinct from score | **established** (leaders differ 47%) |
| removing it is free | **refuted** — it is real variance |
| what that variance is worth to the GA | **unmeasured** — needs the gene perturbation |

## 2026-07-29 — ★★★★ destination context makes one unit's move learnable

The same-actor control above reduced the old geometry head to +0.5 points and
identified the missing information before a policy was allowed to use it. This
is the pre-registered follow-up: preserve the games, seeds, profiles, candidate
sampling, optimizer, external evaluation and game-macro uncertainty, and change
only what the action row can say about a destination.

The thirteen legacy scalars remain an exact prefix. Thirty-five appended terms
describe:

- the acting unit's role, so exposure and spacing can mean different things to
  a scout, vanguard, ranged unit, siege train, support, religious unit or
  civilian;
- target movement cost, defense, water/route/development state and base yields;
- nearby friendly support, visible hostile attack coverage and target exchange
  margin;
- progress toward known foreign cities, home, and unexplored frontier; and
- distance and progress toward the spatial objectives `AdvancedAi` already
  reports from its force groups, threatened city and campaign target.

The high-level objectives are captured once per AI turn and attached to every
candidate in that turn. They contain no chosen action, absolute coordinate,
movement direction or rank. The encoder selects the objective nearest the
acting unit *before* comparing destinations, so it cannot switch goals to make
one candidate look good. This is hierarchical context, not a copy of the
expert's final answer.

### Exact external-profile rerun

Training is the same eight 4-player, 44×28, 200-turn Standard games and seeds
920000–920007. External evaluation is the same eight 6-player, 74×46, 250-turn
Online games with six city states and seeds 930000–930007. All sixteen games
ended in victory. The emitter reproduced the previous corpus counts exactly:
67,032/134,846 Standard chosen/negative rows and 314,813/651,035 Online rows,
with **zero rejected replay actions and zero score divergences**. Explicit plan
context was present for 98.4% and 98.7% of chosen decisions respectively.

Every row below is trained on Standard and evaluated only on the eight unseen
Online games. Candidates have the same action kind and acting unit. Chance is
the exact mean `1 / candidates` for each game, not `1 / mean(candidates)`.

| visible feature block | move top-1 | chance | game-macro lift |
|---|---:|---:|---:|
| legacy 13 only | 26.7% | 26.4% | +0.3 ± 0.2 pp |
| explicit plan only | 35.8% | 26.4% | **+9.4 ± 1.0 pp** |
| destination, explicit plan blanked | 39.7% | 26.4% | **+13.3 ± 0.5 pp** |
| destination only | 41.2% | 26.4% | **+14.8 ± 0.7 pp** |
| legacy + destination | **41.7%** | 26.4% | **+15.2 ± 0.6 pp** |
| all state and action terms | 41.4% | 26.4% | +14.9 ± 0.6 pp |

The result is not just the explicit objective leaking through a side channel.
Plan progress by itself transfers, and the destination block with those three
terms blanked transfers independently. Combining them adds another 1.9 points.
Conversely, giving the model the shared state vector does not help: every
candidate in a decision sees the same state, and the additional nonlinear
context slightly reduces the external top-1.

This establishes movement representation, not combat or strength. The unseen
set has 204,198 move decisions but only 741 ranged and 246 attack decisions.
The destination-only attack lift is +2.8 ± 6.1 points and ranged is +3.9 ± 2.0;
neither supports a claim. `improve` and `promote` remain exactly at chance
because the action row still does not encode which improvement or promotion is
named.

> **The representation gate is now passed for one-unit movement. Do not turn
> the imitation head into a gameplay policy.** Its target is still the action
> the current expert took, so its ceiling is that expert and replacing the
> expert cannot establish an improvement. The supported next experiment is a
> counterfactual return emitter: branch the chosen move and same-actor
> alternatives from the identical state, continue each branch, train Q or
> advantage on those returns, then spend a gameplay A/B only if that head
> predicts held-out branch outcomes. Destination ranking can order those
> rollouts; outcome labels are what can make their result stronger than the
> script it imitates.

| claim | status |
|---|---|
| legacy geometry ranks destinations for one unit | **refuted again** (+0.3 ± 0.2 pp) |
| explicit strategic objectives transfer across profiles | **established** (+9.4 ± 1.0 pp) |
| target terrain/local-force geometry transfers without the explicit plan | **established** (+13.3 ± 0.5 pp) |
| the combined representation ranks same-unit moves | **established** (+15.2 ± 0.6 pp, n=204,198) |
| the same representation ranks attacks | **unmeasured at useful power** |
| the imitation artifact improves gameplay | **not claimed; wrong target for that question** |

## 2026-07-29 — ★★★ counterfactual moves expose real regret, but the first advantage head does not transfer

The destination result above answered whether the expert's choice is
representable. It did not answer whether another destination would have been
better. `q_counterfactual` now asks that causal question without changing the
trajectory it observes:

1. clone a real `AdvancedAi` turn and read the successful actions it took;
2. replay the prefix to the first `move` with another legal destination for the
   same unit;
3. branch the exact pre-action game into the chosen move and three evenly
   sampled same-unit alternatives; and
4. continue every candidate for 80 rounds with identical policy memory and four
   matched rotations of the bounded strategic doctrines.

A decided branch returns 1/0. An unresolved branch returns Civilization score
share among living majors, the same bounded terminal value the shipped
strategic search uses. Repeating an identical branch is not a replica—the
engine is deterministic—so one chosen branch is repeated as an integrity test,
while actual replicas vary opponent doctrine. The tool buffers all rows and
refuses to write the dataset if an observed prefix or candidate is rejected or
the repeated outcome differs in winner, turn, scores, or value.

### The causal label is real and replicates across profiles

The Standard corpus is 24 four-player 44×28 games, seeds 944000–944023, with
four sampled decisions per game at turns 50, 75, 100, and 125. The disjoint
deployment corpus is 12 six-player 74×46 Online games with six city states,
seeds 945000–945011, sampled at turns 70, 100, 130, and 160. Both use the same
80-round horizon, three alternatives, and four doctrine replicas.

| causal-label property | Standard | unseen Online |
|---|---:|---:|
| decisions / candidate rows / continuations | 94 / 360 / 1,440 | 48 / 183 / 732 |
| decisions with candidate spread > 0.005 | **50/94 (53.2%)** | **26/48 (54.2%)** |
| mean best-minus-worst return | **0.0418** | **0.0201** |
| expert chose a best-mean candidate | 52/94 (55.3%) | 23/48 (47.9%) |
| mean oracle regret of expert choice | **0.0182** | **0.0094** |
| a sibling beat the expert in every replica | 6/94 | 2/48 |
| mean-return winner tied/won doctrine replicas | 79.3% | 66.7% |
| branches resolved to victory | 546/1,440 | 153/732 |
| rejected branches / repeat mismatches / observation errors | **0 / 0 / 0** | **0 / 0 / 0** |

The preregistered emitter gate was 25% of decisions separating by more than
0.005. Both profiles clear 53%. The expert is not an oracle over its own legal
moves: it leaves measurable return on the table, and the effect survives the
larger Online profile. That is the opportunity a stronger move policy may
eventually capture.

### Fixed Standard-to-Online model gate

`q_advantage_train` fits a listwise distribution over measured returns, not an
expert-action label. The first model is deliberately linear and low capacity:
94 decisions do not justify a hidden net with thousands of parameters. All
Standard decisions train for a fixed 40 epochs at return temperature 0.01.
Online contributes no gradient or hyperparameter choice; each feature-block
ablation uses that identical fit and is scored only as external evaluation.
Regret and uncertainty are macro-averaged by game.

| visible terms | Online model regret | return lift vs chance | return lift vs expert |
|---|---:|---:|---:|
| destination (primary) | 0.0122 | **−0.0016 ± 0.0022** | **−0.0028 ± 0.0037** |
| destination, plan blanked | 0.0112 | −0.0006 ± 0.0021 | −0.0018 ± 0.0035 |
| explicit plan only | 0.0128 | −0.0022 ± 0.0021 | −0.0034 ± 0.0036 |
| legacy 13 only | **0.0096** | +0.0010 ± 0.0011 | −0.0002 ± 0.0018 |
| legacy + destination | 0.0114 | −0.0008 ± 0.0018 | −0.0020 ± 0.0033 |

The primary head reduces regret on its Standard training games from 0.0185 for
the expert to 0.0125, but that gain reverses out of profile. On Online it picks
a top-return move 43.8% of the time versus 46.9% chance and 47.9% for the
expert. Legacy terms are the least bad external block, but they merely tie the
expert within uncertainty; that is not a promotion.

One capacity hypothesis was tested without spending another external set. The
destination encoder explicitly says role should change how threat, cohesion,
and objective progress are interpreted, while a linear head cannot form those
interactions. On a Standard-only split of 14 training and 10 held-out games,
the plain model's held-out regret was 0.0058 and its lift over the expert was
+0.0051 ± 0.0045. Appending all 8×27 role-by-destination products **worsened**
regret to 0.0080 and reduced that lift to +0.0030 ± 0.0032. The capacity
hypothesis fails its selection test, so no fresh external maps were spent on
it.

> **Do not integrate either head or spend a gameplay A/B on it.** The causal
> emitter establishes that better moves exist; it does not establish that the
> present dataset/model can identify them on deployment games. A gameplay
> replacement would knowingly select lower-return moves in the only external
> profile measured.

The next justified learning experiment is more independent games and a target
that retains replica structure instead of collapsing doctrine outcomes to one
mean—e.g. a pairwise posterior over the probability that candidate A beats B,
with a no-override region when doctrine signs disagree. Only a model that
reduces regret on a fresh external profile earns the mirrored gameplay gate.

| claim | status |
|---|---|
| same-unit destinations have different causal continuation returns | **established on both profiles (53% clear the spread gate)** |
| the expert always chooses the best measured destination | **refuted (55% Standard, 48% Online)** |
| the fixed destination advantage head transfers Standard → Online | **refuted in this sample** |
| explicit role interactions repair the learner | **refuted on Standard holdout** |
| the current model should control gameplay | **rejected before A/B** |

## 2026-07-29 — PRE-REGISTRATION: preserve matched doctrine outcomes before overriding a move

The first causal move head averaged four matched doctrine continuations into
one return before learning. That makes a 2-2 split look like a confident small
effect, makes the target scale depend on the map profile, and forces the model
to replace the expert even when its margin is negligible. Its Standard gain
reversed on Online games. This experiment changes those three properties and
nothing about gameplay.

**Hypothesis.** A linear pairwise head trained on the sign of every matched
candidate comparison, with disagreement shrunk toward indifference, will
transfer better than the mean-return listwise head. Requiring a predicted
probability of at least **0.70** before replacing the expert will turn weak or
profile-specific preferences into abstentions instead of regressions.

The decision rule is exact: score every candidate, identify the highest-scored
non-expert sibling (enumeration order keeps a tie), and replace the recorded
expert only when `sigmoid(sibling_score - expert_score) >= 0.70`. A tied score
therefore retains the expert. The 5% gate below uses the game-macro mean
override rate, matching the return uncertainty unit.

The target for each unordered candidate pair is fixed before data collection.
Each of the four doctrine-matched return differences contributes a win, loss,
or half-win tie. A Jeffreys `Beta(0.5, 0.5)` posterior mean is the logistic
target, so 4-0, 3-1, and 2-2 evidence becomes 0.90, 0.70, and 0.50. All pairs
are kept; split evidence explicitly teaches a zero margin. The feature vector
is the difference of the already-validated 35 destination features. Training
uses 80 deterministic epochs, batch size 32, rate 0.05, and L2 0.0001. No
nonlinear terms or external-profile tuning are allowed.

The data and decision rule are also fixed:

- development: **64** four-player 44x28 Standard games, seeds
  946000-946063, no city-states, four observations at turns 50/75/100/125;
- untouched external test: **32** six-player 74x46 Online games, seeds
  947000-947031, six city-states, four observations at turns
  70/100/130/160;
- both: three same-unit alternatives, 80-round continuation horizon, four
  matched doctrine rotations, and zero tolerated integrity failures;
- the existing hash split holds out 25% of Standard games. The external run is
  spent only if the 0.70-gated policy has positive held-out return lift over
  the expert and overrides at least 5% of decisions;
- after that selection gate, the same fixed model is refit on all Standard
  games and evaluated once on Online. The existing 40-epoch, temperature-0.01
  listwise destination head is rerun on the same corpus as the target-control;
- external success requires positive lift over the expert with a game-macro
  95% lower confidence bound above zero and at least 5% overrides. Anything
  weaker is a rejection, not permission to tune the threshold on these maps.

The primary metrics are oracle regret and return lift versus the recorded
expert, macro-averaged by independent game. Override rate, the fraction of
overrides with positive mean return, and their matched-doctrine win/loss/tie
counts diagnose whether abstention worked. No gameplay code or model artifact
is promoted by this experiment; a passing external result earns a separate,
mirrored gameplay A/B.

### Result: the rank signal improves, but the preregistered abstainer is inert

All 64 Standard games were collected at the fixed seeds. They produced 247
decisions, 938 candidate rows, and 3,752 doctrine continuations; four scheduled
observations had no eligible move. There were **zero rejected branches, repeat
mismatches, or observation errors**. Of the 247 decisions, 127 (51.4%) had a
best-minus-worst mean-return spread above 0.005. Mean spread was 0.0339, mean
expert oracle regret was 0.0170, and a sibling beat the expert in all four
replicas at 16 decisions.

The game-hash split assigned 44 games / 171 decisions to training and 20 games
/ 76 decisions to the held-out selection set. The exact fixed fit produced:

| game-macro metric | train | held-out Standard |
|---|---:|---:|
| decisions | 171 | 76 |
| expert oracle regret | 0.0181 | 0.0133 |
| ungated model regret | **0.0121** | **0.0082** |
| ungated return lift vs expert | **+0.0060 ± 0.0039** | **+0.0051 ± 0.0039** |
| 0.70-gated model regret | 0.0181 | 0.0133 |
| 0.70-gated overrides | **0/171** | **0/76** |

This is a useful ranking signal but not a usable abstaining policy. The
held-out best-sibling probabilities had median 0.505, p90 0.517, p99 0.529,
and maximum 0.532. None even reached 0.55. Training was similarly compressed
(maximum 0.530), with weight L2 norm 0.1201 and largest absolute weight 0.0644.
The posterior target itself contains substantial indifference: 541/910 train
pairs and 260/435 held-out pairs have target 0.50. These diagnostics were
reported without changing the fit or decision rule.

The selection gate therefore **fails**: gated lift is zero only because the
model always retains the expert, and its game-macro override rate is 0%, below
the required 5%. The 0.70 threshold is not tuned after observing this result.
No Online games were generated, seeds 947000-947031 remain untouched, the
listwise external control was not run, and no gameplay A/B or integration was
spent.

> **Do not promote this head.** Replica-aware pairwise supervision preserves
> information that mean returns discard and its ungated ranking is promising,
> but the raw logistic margin is not a calibrated posterior suitable for a
> conservative override gate. In this fixed linear formulation, abstention is
> total rather than selective.

A justified successor must separate ranking from confidence without using the
untouched Online profile: cross-fit the pairwise scorer by independent game,
calibrate its out-of-fold margins against robust matched-doctrine superiority
on a fresh Standard calibration corpus, and preregister a risk/coverage rule
before one external evaluation. More doctrine replicas would also give that
calibrator a less quantized target. The fresh external set is spent only if a
separate Standard selection set shows positive return lift at nonzero
coverage; lowering 0.70 on this corpus is not evidence.

| claim | status |
|---|---|
| replica-aware destination ranking improves held-out Standard regret | **supported (+0.0051 ± 0.0039 lift)** |
| the raw pairwise sigmoid reaches a selective 0.70 override region | **refuted (maximum 0.532; 0/76 overrides)** |
| the fixed head transfers to Online | **not tested; external spend gate failed** |
| the current head should control gameplay | **rejected before external evaluation or A/B** |

## 2026-07-29 — PRE-REGISTRATION: calibrate ranking confidence on independent games

The replica-aware head above ranked held-out moves in the right direction, but
its raw score difference was not a calibrated probability: every prediction
stayed between approximately 0.47 and 0.53. Lowering the override threshold on
those observed games would spend the selection set twice. This experiment
instead freezes that ranker and learns only a two-parameter probability map on
new games before opening another new Standard selection set.

**Hypothesis.** Independent Platt calibration of the frozen pairwise margin
will identify a nonempty high-confidence tail whose 0.70-gated choices improve
counterfactual return over the expert on fresh Standard games. If calibration
only magnifies noise, held-out Brier score or return lift will reject it before
the untouched Online profile is generated.

The ranker is exactly the destination-only model selected above: seeds
946000-946063, the existing 25% game-hash holdout, 171 training decisions, 80
epochs, batch 32, rate 0.05, and L2 0.0001. It is regenerated deterministically
from that command and then frozen; neither new Standard split contributes a
ranking gradient. At each decision it selects the highest-scored non-expert
sibling, retaining enumeration order on a score tie. Its raw margin is sibling
score minus expert score.

The regenerated frozen artifact is 1,896 bytes with SHA-256
`2c93f4456b72d1acf548f1994c9ce49569fe158c7b8eb18f4c903b606ce1c463`.
This pins the actual coefficients, not only their training recipe, before any
calibration game is generated.

Calibration uses one standardized scalar. The mean and population standard
deviation of the frozen margins are learned on calibration games only, with a
1e-6 standard-deviation floor. The target is the same Jeffreys posterior mean
for that selected sibling against the expert: four matched return differences
contribute wins, losses, or half-win exact ties to `Beta(0.5, 0.5)`. A
monotone map `sigmoid(a * standardized_margin + b)` is fitted with full-batch
gradient descent for 4,000 fixed steps at rate 0.05, L2 0.01 on `a` only, and
`a` projected to [0, 20]. Every decision receives inverse game decision-count
weight so one independent game is one calibration unit. There is no threshold
search: the expert is replaced only at calibrated probability at least 0.70.

The new data are fixed before collection:

- calibration: **32** four-player 44x28 Standard games, seeds
  948000-948031, no city-states, observations at turns 50/75/100/125;
- blind Standard selection: **32** otherwise identical games, seeds
  948032-948063, generated only after the calibrator implementation and
  parameters are frozen;
- untouched external test, generated only after a selection pass: **32**
  six-player 74x46 Online games, seeds 947000-947031, six city-states,
  observations at turns 70/100/130/160;
- every corpus uses three same-unit alternatives, an 80-round horizon, the
  four distinct doctrine rotations, and zero tolerated rejected branches,
  repeat mismatches, or observation errors. Additional replicas are not
  repetitions: the emitter has exactly four distinct doctrines.

Selection passes only if all three preregistered conditions hold on seeds
948032-948063: calibrated Brier score is lower than the frozen raw sigmoid,
0.70-gated game-macro return lift over the expert is positive, and game-macro
override coverage is at least 5%. The calibrator and ranker then remain frozen
for one Online evaluation. External success additionally requires the
game-macro 95% lower confidence bound on return lift to exceed zero, at least
5% coverage, and calibrated Brier score below raw. Report oracle regret,
ungated and gated lift, Brier and log loss, probability quantiles, override
mean-return signs, and matched-doctrine wins/ties/losses.

No model enters gameplay from this experiment. A selection failure leaves the
Online seeds untouched. An external failure ends the line. External success
would earn a separate mirrored gameplay A/B; it would not itself authorize a
default policy change.

### Result: rank order survives, but margin magnitude carries no positive confidence signal

The 32 calibration games at seeds 948000-948031 produced 125 decisions, 485
candidate rows, and 1,940 doctrine continuations. Three scheduled observations
had no eligible move. There were **zero rejected branches, repeated-branch
mismatches, or observation errors**. Of the decisions, 71/125 (56.8%) had
mean-return spread above 0.005, mean spread was 0.0224, mean expert oracle
regret was 0.0120, and a sibling beat the expert in every doctrine at 13/125.

The frozen ranker again had directional value: its ungated game-macro return
lift over the expert was +0.0054 ± 0.0045 and regret fell from 0.0119 to 0.0065.
Its margin magnitude did not carry confidence, however. The preregistered
monotone fit converged to:

| frozen calibration term | value |
|---|---:|
| margin mean / population standard deviation | +0.024259 / 0.035254 |
| game-weighted margin / target correlation | **-0.0385** |
| nonnegative Platt slope | **0.0000** |
| intercept | +0.0292 |
| calibrated probability, every decision | **0.507** |
| raw / calibrated Brier | 0.02169 ± 0.00354 / 0.02151 ± 0.00357 |
| raw / calibrated log loss | 0.69340 / 0.69304 |
| 0.70 overrides | **0/125** |

Projection to zero is the preregistered monotonicity constraint doing its job:
on these independent games, larger positive rank margins covary slightly with
*lower*, not higher, matched-doctrine superiority targets. The intercept learns
only the base rate. The frozen artifact is 1,169 bytes with SHA-256
`aa6efe782232907dc01c25c0ad02c136ad7d5c7ebc008eb248bfcc6956eeb134`.

This is a structural preselection failure. A constant probability of 0.507 can
never clear 0.70, so it has exactly 0% coverage for any possible selection
outcomes and cannot satisfy the preregistered 5% condition. Generating the
blind selection set cannot alter a frozen prediction. Seeds 948032-948063
therefore remain untouched, as do Online seeds 947000-947031. No selection,
external evaluation, gameplay A/B, or integration was spent.

> **Do not lower the threshold or use margin size as confidence.** The head's
> ordering continues to find better moves on average, but how far apart its
> linear scores land is not an out-of-sample measure of reliability. Platt
> scaling can rescale a monotone signal; it cannot manufacture one.

The next justified learner must predict override reliability from information
other than the rank margin: candidate state/destination context, the expert
versus sibling feature difference, and a target for consistent doctrine
superiority. The completed calibration corpus may serve as development data,
while seeds 948032-948063 remain a genuinely blind Standard selection set.

| claim | status |
|---|---|
| the frozen ranker still improves mean return on new Standard games | **supported (+0.0054 ± 0.0045)** |
| larger frozen margins imply more reliable superiority | **refuted (monotone slope 0.0000)** |
| independent Platt scaling creates a selective 0.70 region | **refuted (constant 0.507; zero coverage)** |
| selection or Online evaluation is warranted | **rejected without spending either corpus** |

## 2026-07-29 — PRE-REGISTRATION: predict robust overrides from decision context

Two independent Standard samples now show that the frozen pairwise ranker
orders moves usefully on average, while its score magnitude contains no
positive confidence signal. A scalar recalibration cannot repair that. The
next test asks a different question: can the state and the absolute expert and
sibling destinations identify *where* the frozen ordering is reliable?

**Hypothesis.** A low-capacity logistic reliability head trained directly on
matched-doctrine superiority will identify a nonempty 0.70 region with lower
out-of-fold Brier score than both the raw rank margin and a training-fold base
rate, while its gated counterfactual return exceeds the expert at at least 5%
coverage. Absolute context can distinguish, for example, a safe formation move
from an exposed advance even when the frozen rank margins have the same size.

The destination ranker remains byte-for-byte fixed at SHA-256
`2c93f4456b72d1acf548f1994c9ce49569fe158c7b8eb18f4c903b606ce1c463`.
For every decision it names the highest-scored non-expert sibling. The
reliability row is fixed at 105 terms: 34 shared decision-state features, the
35 destination terms for the expert, the same 35 terms for the named sibling,
and their frozen raw rank margin. Kind one-hot and legacy geometry are omitted;
every sampled candidate is already a same-unit move.

The target is the Jeffreys posterior mean that the named sibling beats the
expert under the four matched doctrine rotations, with exact return ties worth
half a win. Each feature is standardized from training games only with a 1e-6
standard-deviation floor. Logistic cross-entropy is optimized by deterministic
full-batch descent for 6,000 steps at rate 0.05, L2 0.02 on non-intercept
weights, and inverse decision-count weighting within each game. The threshold
is fixed at 0.70; neither model capacity nor threshold is selected from an
evaluation split.

Development deliberately excludes every game that trained the ranker:

- take only the existing 25% game-hash holdout from Standard seeds
  946000-946063 (20 games / 76 decisions), then add all independent calibration
  seeds 948000-948031 (32 games / 125 decisions);
- assign those 52 games to five deterministic hash folds. For each fold,
  standardization, the 105 reliability weights, and the constant base-rate
  control are learned on the other four folds. Concatenate predictions only
  for games omitted from that fit;
- the out-of-fold gate passes only if reliability Brier is strictly below both
  the raw `sigmoid(margin)` and fold-trained constant Brier, gated game-macro
  lift over the expert is positive, and game-macro coverage is at least 5%;
- only after an out-of-fold pass, fit the identical head on all 52 development
  games and generate the still-blind Standard selection games at seeds
  948032-948063 using the already-fixed 4p 44x28, turns 50/75/100/125, three
  alternatives, four doctrines, and 80-round horizon protocol;
- selection requires the same three conditions against the full-development
  constant. Only a pass generates the still-untouched 32-game Online profile
  at seeds 947000-947031. External success additionally requires the
  game-macro 95% lower confidence bound on lift above zero.

All loaders require exactly the preregistered game ranges, current 159-wide
counterfactual schema, four distinct replicas, declared means matching replica
means, and finite values. Report oracle regret, ungated and gated lift,
raw/constant/reliability Brier and log loss, probability quantiles, override
mean-return signs, and doctrine wins/ties/losses. Any development failure keeps
both blind corpora untouched. Even external success earns a separate mirrored
gameplay A/B; this experiment changes no game policy.

### Result: absolute context overfits and its confident tail is wrong

The fixed development set contains 201 decisions from 52 independent games:
the 76 decisions in the prior ranker holdout plus the 125 independent context
decisions. Five-fold game grouping held out 8/10/14/10/10 games. Every fold
learned its own normalization, reliability head, and base rate from the other
games; the held-out base rates ranged from 0.498 to 0.521.

The frozen ranker remains directionally useful across the combined set:
ungated regret falls from 0.0124 for the expert to 0.0071, a game-macro return
lift of **+0.0053 ± 0.0031**. The reliability head does not identify that gain:

| out-of-fold metric | raw margin | fold constant | context reliability |
|---|---:|---:|---:|
| Brier | 0.02306 | **0.02331** | **0.03014** |
| log loss | 0.69343 | **0.69394** | **0.70852** |
| p50 / p90 / p99 / maximum | — | — | 0.515 / 0.636 / 0.735 / 0.762 |
| 0.70 overrides | 0 | 0 | **4/201 (1.9% game-macro)** |
| gated return lift vs expert | 0 | 0 | **-0.0003 ± 0.0003** |

All four supposedly confident overrides were nonpositive by mean return: three
ties and one loss. Across their 16 matched doctrines they recorded zero wins,
13 ties, and three losses. This is not a threshold near-miss: coverage misses
the required 5%, Brier is 29% worse than the fold constant, and the direction
of gated return is negative.

The failure is present in both sources rather than coming from one shifted
corpus. Reliability Brier is 0.03225 versus a 0.02532 constant on the 76 prior
holdout decisions, and 0.02882 versus 0.02205 on the 125 independent context
decisions. The two confident overrides in the latter include the only
mean-return loss, producing -0.0004 ± 0.0004 lift there. Meanwhile ungated
ranker lift remains +0.0051 ± 0.0039 and +0.0054 ± 0.0045 respectively.

The full-development fit reduces its own regularized loss from 0.69315 to
0.67951, with weight L2 norm 0.4624, while out-of-fold log loss rises to
0.70852. That gap is direct evidence of finite-sample overfit, not a reason to
increase capacity. The frozen 6,891-byte artifact has SHA-256
`f4a1361f778ba937e44421046ace48f0be59a07933889301dc391d2c420348b5`.

The out-of-fold gate therefore **fails every condition**. The tool never opened
a selection file. Standard seeds 948032-948063 and Online seeds
947000-947031 remain untouched, and no gameplay A/B or integration was spent.

> **Stop tuning this move-override corpus.** Better destinations measurably
> exist and the frozen ranker finds some of their average value, but 52 games
> do not support a trustworthy selective residual policy over 105 context
> terms. Threshold lowering, a hidden layer, or feature selection on these same
> folds would convert a clean rejection into adaptive overfitting.

A future move learner needs materially more independent labeled games, a
pretrained or much lower-dimensional representation, and nested selection
before the preserved seeds are opened. Until then, gameplay work should pivot
to strategic mechanisms whose hypotheses can be tested directly in mirrored
full games rather than promoting this unresolved local surrogate.

| claim | status |
|---|---|
| frozen destination ordering retains average value across 52 games | **supported (+0.0053 ± 0.0031)** |
| absolute state/destination context predicts reliable overrides | **refuted out of fold (Brier 0.03014 vs 0.02331)** |
| the 0.70 tail is safe and sufficiently broad | **refuted (1.9% coverage; 0 wins, 1 loss)** |
| blind Standard selection or Online evaluation is warranted | **rejected without spending either corpus** |

## 2026-07-29 — preregistration: what actually changes the adaptive strategy?

The route-connected ancient-rush test (#557) closes that opening-policy line:
the treatment made larger empires and armies but lost the paired outcome screen,
and its frozen route test selected 94.2% of seat-games. Military work therefore
returns to the midgame, but the evaluator currently hides the state that chooses
midgame behavior. `ai_eval` traces only `PlanReport.victory_target`; an adaptive
`AdvancedAi` has no assigned target, so it is printed as `adaptive` on every
turn with zero switches even while `PlanReport.strategy` moves among Expansion,
Recovery, Conquest, and the enabled victory lanes.

That missing observation matters because the existing churn result cannot by
itself justify hysteresis. The same 2026-07-28 evidence wave later established
that its headline routing oracle used the fallback genome, not the embedded
champion, and that hard commitment suppressed expansion. Before changing a
decision, this experiment asks whether the champion's midgame switches are
ordinary responses to visible state boundaries or strategy changes with no
such boundary.

### Frozen measurement

`PlanTrace` will retain its existing assigned-target and ancient-rush metrics
unchanged and add a separate observer-only trace of `PlanReport.strategy`.
For every observed player-turn it will record:

- the strategy label;
- whether the seat is at war with a living major civilization;
- whether the reported plan has a threatened city; and
- whether the seat has fewer cities than the plan's reported `desired_cities`.

A **strategy switch** is a change of strategy label between adjacent observed
turns for one seat. A switch is **boundary-accompanied** when at least one of
those three booleans changes on the same observation; the three components are
also counted separately and may overlap. “Unanchored” means only “none of these
plan-visible boundaries changed,” not “irrational”: rival victory denial,
military power, or victory progress can still explain it and are deliberately
not guessed from the label.

The primary interval is the speed-normalized midgame from Standard turn 60
(inclusive) through Standard turn 180 (exclusive), which is Online turns
40–119 under the shipped duration table. The output will report all-game and
midgame strategy shares, switches per seat-game, unanchored midgame switches,
boundary counts, and the midgame transition matrix. Existing outcome,
promotion, target, and rush calculations must remain byte-for-byte invariant
for a fixed run.

The frozen run is:

```text
ai_eval advanced_evolved advanced --players 8 --width 84 --height 54 \
  --city-states 12 --pairs 60 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9981000 --jobs 6
```

This compares the embedded gen-14 champion with the stock-weight anchor on the
production spectator's exact game profile. Mirroring balances seats and the 60
map seeds are fresh. Strategy diagnostics are observational and do not enter
the promotion verdict.

### Frozen reading

Top-level commitment instability remains a live intervention candidate only if
the `advanced_evolved` seats satisfy **both** conditions:

1. at least 1.0 unanchored midgame strategy switch per seat-game; and
2. at least 50% of their midgame switches are unanchored.

Otherwise, do not add generic hysteresis: the next military experiment must
operate inside a stable Conquest/Recovery episode (campaign routing, staging,
or post-war conversion), or first expose a missing trigger if the switch table
shows one dominant unobserved transition.

A narrower mechanism claim — that routing stability helps explain any champion
strength — requires **all** of: the champion's unanchored midgame switch rate is
at least 25% below stock `advanced`; its paired-map score is at least 52%; and
champion-favored map directions exceed stock-favored directions. If any term
fails, the genome comparison is descriptive and no outcome attribution is made.
There is no holdout and no gameplay promotion in this task: it adds the missing
measurement, records the frozen screen once, and selects the next falsifiable
midgame mechanism.

### Result: the champion churns too, and routing stability does not explain it

The frozen command completed all 60 map pairs: 120 games, 480 seat-games per
entrant, 116,293 observed champion player-turns and 116,636 stock player-turns.
Games averaged 247.6 turns. The new trace exposes what the old target report
could not: both entrants remained `adaptive` for 100% of assigned-target
observations with zero target switches, while their live grand strategies
changed about twelve times per seat-game.

| midgame strategy diagnostic | `advanced_evolved` | `advanced` |
|---|---:|---:|
| switches per seat-game | 4.39 | 4.08 |
| unanchored per seat-game | **2.69** | 2.36 |
| unanchored share | **1,292 / 2,105 (61.4%)** | 1,131 / 1,959 (57.7%) |
| boundary-accompanied | 813 / 2,105 (38.6%) | 828 / 1,959 (42.3%) |
| all-game switches per 100 turns | 4.96 | 4.91 |

The champion passes both preregistered commitment-instability eligibility
terms: 2.69 is above 1.0 unanchored switch per seat-game and 61.4% is above the
50% residual-share floor. This revives commitment as a mechanism worth
isolating on the agent that ships; it does **not** promote hysteresis by itself.
“Unanchored” deliberately omits public victory denial, relative military
power, and victory-progress changes, any of which can be a sound reason to
switch.

The exposure and transition table localize where the next instrument belongs.
Champion midgame turns were 27.1% Conquest and 19.7% Recovery, and
Conquest→Recovery (353) plus Recovery→Conquest (278) made up 631 of 2,105
switches (30.0%). The visible boundary totals were threat 591, war 225, and
city deficit 164; components overlap. That aggregate cannot say which of the
1,292 residual switches came from denial, opportunistic power, or best-lane
progress. Adding a generic cooldown now could therefore suppress an urgent
counter-plan while claiming to cure churn. The next falsifiable routing
instrument is the already-computed `assess()` trigger (`because`) beside the
strategy report, followed by trigger-scoped rather than generic hysteresis if
one elective trigger dominates the residual.

The narrower genome mechanism is refuted on every frozen term:

- unanchored churn was **14% higher**, not at least 25% lower (2.69 versus
  2.36 per seat-game);
- paired-map score was **47.5%**, not at least 52% (95% CI 35.4%–59.9%,
  −17 Elo, CI −104–+70); and
- map directions were **4 champion / 46 neutral / 10 stock**, not champion
  favorable (exact sign p=0.1796).

The outcome is `INCONCLUSIVE`, not evidence that the embedded champion is
weaker. Terminal score was 49.3%, with directions 26–34 (p=0.3663). The exact
profile produced 8 champion wins (all Science) and 14 stock wins (13 Science,
1 Culture). What is established is narrower and useful: **routing stability
does not explain champion strength, because the champion is not more stable.**
No holdout is run and no gameplay behavior changes.

## 2026-07-29 — the expansion axis: one oracle with headroom, six treatments, and what replicated

`Grant::Expansion` (#554) is **the only subsystem grant this harness has ever
returned HEADROOM for**. A free Settler while the seat is short of its city
target, one at a time, stopping at six:

| run | granted | control | McNemar | discordant | fired |
|---|---|---|---|---|---|
| `none` (control) | 24/100 | 24/100 | p=1.0000 | 0 | 0 |
| `treasury` (calibration) | 89/100 | 24/100 | p=0.0000 | 65 | 166.7/game |
| **expansion, 50 pairs, seed 450000** | **46/100** | 24/100 | **p=0.0007** | 40 | 5.4/game |
| **expansion, 150 pairs, seed 460000** | **157/300 = 52.3%** | 69/300 = 23.0% | **p=0.0000** | **144** | 5.6/game |

Two disjoint seeds, 400 maps, 184 discordant cells. It carries a seat from well
below parity (23.0%) to above it (52.3%) where parity is 25%. Every other
subsystem grant — `ground`, `siting`, `taker`, `modernity`, `attrition` — is
null.

### What the grant does *not* mean, and my own retraction

#554 claimed it "stops at six — the agent's own `desired_cities` target",
concluding *the AI cannot afford the empire it already wants*. **That conclusion
was wrong.** `EXPANSION_TARGET` is a hardcoded 6 in `oracle.rs`;
`production_allocation_census` (#569) measures `desired_cities` at **3.83** on
4p/24×16 and **5.00** on 6p/74×46. The grant pushed the empire *above* its own
plan, so what it measured is that **the target is too low**, not that the target
is unaffordable.

`production_allocation_census`, 6p/74×46, 75,474 production over 6 maps:

```
building 26.4%  district 21.7%  civilian 18.0%  project 13.8%
wonder 11.1%    military 6.1%   settler 2.8%    idle 0.2%
cities held 4.83 against a target of 5.00
the missing cities would have cost 67 production — 0.5% of everything made
```

Affordability is not the constraint at either scale (0.5% deployment, 14.3%
eval).

### Six treatments, and why five failed for the same reason

| treatment | result |
|---|---|
| expansion window → payback (#562) | wins null; **terminal score significant, twice** (below) |
| production preemption (`preempt_margin`) | cities at end **2.21 vs 2.21**; settlers started 2.46 → 2.42 |
| settler-specific preemption (#571) | **inert** — settlers 16 vs 16, 23 vs 23 |
| the settler's own valuation | free-city loss only **2.6%**; it already wins when asked |
| capital growth (`OPENINGS.md` §12) | every city after the first arrives **later**, monotone in dose |
| city-target ramp floor (#571) | fires at deployment; **NULL on confirmation** (below) |

**Five of the six worked to deliver the plan's target faster, and the target was
the thing that was wrong.** That is the single sentence this axis produced.

### `expansion_pays_back` — replicated on economy, never on wins

`expansion_window_open` reserves a flat `standard_duration(50)` for every city
regardless of what it can build. `expansion_pays_back_for` asks instead whether
there is time to build the settler at *this city's* production rate, walk it,
and hold the ground long enough to return its cost.

| run | wins | terminal score | wins resolution |
|---|---|---|---|
| 6p/74×46, 120 pairs, seed 490000, 250t | 50.0%, Elo +0 | 56–24, **p=0.0005** | 2 of 120 |
| 6p/74×46, 120 pairs, seed 491000, **500t** | 51.2%, Elo +9 | 58–30, **p=0.0037** | 23 of 120 |

**Read the resolution column.** At 250 turns only two maps produced a win
direction, so 50.0% there means *unmeasured*, not *equal*; the 500-turn run
resolves 23. Across both, the economic gain replicates at p<0.005 and the win
rate is positive twice and significant neither time. Left off by default.

### A gene sweep that measured a path a live agent never takes

`GENOME.md` records "`city_target` saturates above six", swept at 20 mirrored
maps a point, with 6/8/10/12 identical to four decimal places, concluding *the
agent never gets that many cities anyway*.

`city_target` is a **gene**, reached only through `unwrap_or_else` when there is
no plan. A live `AdvancedAi` reads `plan.desired_cities` and never consults it.
The sweep measured a fallback path, which is why every value above six was
byte-identical. The live target is
`(3 + turn / cadence).min(map_capacity).min(6)` in `assess`.

**Before trusting a gene sweep, check the gene is on the live path.**

### The map scale inverted a reading four times on this axis

| reading | eval 4p 24×16 | deployment 6p 74×46 |
|---|---|---|
| settler blocked by "no site" | **44.0%** (sole blocker 401×) | **0.0%** |
| expansion window shut | 8.3% | **31.2%** |
| cities held / target | 1.83 / 3.83 | 4.83 / 5.00 |
| `city_target_floor` 3→6 | **inert** (cities 1.83 → 1.83) | cities 4.83 → **5.33** |

24×16 at four players is 96 tiles per player; 74×46 at six is 567. Every
expansion instrument added here reports both scales for that reason.

## 2026-07-29 — ★★★★ the macro search chooses its two axes in the wrong order

> **Correction, 2026-07-30:** this diagnostic reconstructed the first step
> with the doctrine then in force. The shipped lane pass actually projects the
> unchanged evolved base genome (`self.weights`, `Doctrine::Incumbent`) and
> consults the active doctrine only in the second-step commitment comparison.
> The 64.6% disagreement and 22.3% actionable-miss figures below therefore
> describe the wrong comparator and are retracted. The corrected implementation
> is pinned by a regression test and the deployment-profile paired run later in
> this log replaces the gameplay conclusion.

`StrategicAi` chooses a victory **lane** and a **doctrine**, one after the other.
`lane_values` projects each lane using `self.weights` — the doctrine currently in
force — and `doctrine_values` then projects each doctrine under the lane that
won. That is coordinate descent, and it reaches the joint optimum only when the
axes do not interact.

They interact. `joint_axes` builds the whole `lane × doctrine` value matrix out
of the shipped public API (`doctrine_values(g, pid, lane)` *is* one column of
it), so this measures the search that ships rather than a copy of it. A
self-check asserts `lane_values` reproduces the matrix row it should: **max
deviation 0.00e0**.

30 games, 4 players, 44×28, 200 turns, 175 review points.

| quantity | value |
|---|---|
| best doctrine depends on the lane | **140/175 (80.0%)** |
| joint argmax differs from the sequential pick | **113/175 (64.6%)** |
| — of which the lane differs | 76 (43.4%) |
| — of which the doctrine differs | 103 (58.9%) |
| gap clears `DOCTRINE_COMMITMENT_MARGIN` (0.002) | **39/175 (22.3%)** |
| gap clears `TARGET_COMMITMENT_MARGIN` (0.01) | **20/175 (11.4%)** |
| value left on the table | mean 0.0039, **max 0.2068** |

**Robust to the starting doctrine**, which is the result that makes the rest
trustworthy. Swept over all four incumbents: disagreement 62.9–64.6%, clears the
doctrine margin 33–39, clears the lane margin 14–20. A real agent's doctrine
drifts, so a probe frozen in one of them measures itself; the sweep costs no
extra rollouts because the matrix is already built.

So in roughly **one review in five**, joint search would find a cell worth more
than the margin the agent requires before it will move — and the worst cases are
twenty times the lane margin. That is an actionable gap for about 2.5× the
rollouts (28 branches against 11), which is inside the band already measured as
productive here: doubling the search wins at p=0.0023, quadrupling adds nothing.

**Two false starts, recorded because each would have given the wrong answer:**

- The verdict first keyed on the **disagreement rate**. A pilot disagreed on 100%
  of reviews while the mean gap was 0.0009 — below the doctrine margin and far
  below the lane margin — so joint search would have found a better cell every
  review and committed to almost none of them. Rate is the wrong headline; the
  actionable share is the decision.
- The probe sat in `Doctrine::Incumbent` forever, because a probe that never
  reviews never drifts. I predicted that would *inflate* the gap. It **deflated**
  it about sevenfold. The direction of a bias is not worth reasoning about when
  measuring it is free.

**Three limits, before anyone treats this as a win:**

- Trajectories are `AdvancedAi`'s, not `StrategicAi`'s — the probe rides along
  rather than steering, so the sample is not self-selected. `StrategicAi` is
  `AdvancedAi` plus lane commitment, so its positions would be *more*
  lane-specialised, which would plausibly raise interaction rather than lower it.
- This is the evaluator's own value, and being right about a ranking is a
  separate question from that ranking winning games. `policy_wide` is the
  standing reminder: a calibrated evaluator ranked actions catastrophically.
- 4 players on 44×28 Standard is **not the deployment profile**. Every strength
  claim in this file that failed to transfer failed exactly here, so the
  implementation must be validated at 6 players on 74×46 Online before any
  default changes.

| claim | status |
|---|---|
| the two search axes interact | **established** (80.0%, n=175) |
| coordinate descent misses the joint optimum | **established** (64.6%) |
| the miss is large enough for the agent to act on | **established** (22.3% over the doctrine margin) |
| the result depends on the starting doctrine | **refuted** (stable across all four) |
| joint search wins games | **unmeasured** — needs a deployment-profile paired run |

### `city_target_floor` — the ramp is null, and the first reading was a third false positive

`assess` computes `desired_cities = (3 + turn / cadence).min(map_capacity).min(6)`,
so the empire wants three cities at the opening. `city_target_floor` starts that
ramp at six instead. It fires cleanly at deployment scale and does nothing at
the eval default, where `map_capacity = (2 + land / 55)` caps the target first:

```
[eval 4p 24x16]        floor=3  cities 1.83  score 106
[eval 4p 24x16]        floor=6  cities 1.83  score 106
[deployment 6p 74x46]  floor=3  cities 4.83  score 356
[deployment 6p 74x46]  floor=6  cities 5.33  score 367
```

| run | paired score | direction | terminal score | wins resolution |
|---|---|---|---|---|
| 6p/74×46, 120 pairs, seed 500000 | 53.3%, Elo +23 | 18–10, p=0.1849 | 61–58, p=0.8546 | 28 of 120 |
| **6p/74×46, 240 pairs, seed 510000** | **49.6%, Elo −3** | 31–33, **p=0.9007** | 116–117, **p=1.0000** | **64 of 240** |

**The 53.3% did not reproduce.** That is the third apparent gain on this line of
work to evaporate when confirmed on a seed it was not found on, after a +23 and
a +20 earlier in the same session. The confirmation has better win resolution
(64 maps against 28) and its terminal-score direction is 116–117, which is as
flat as that statistic gets. Left off by default as a recorded entrant.

### The follow-up was fires-checked and declined without an evaluation

The remaining untested cell was `city_target_floor = 6` together with
`parallel_settlers`. `advanced_parallel_settlers` measured near-inert on its own,
and its doc explains why — the clause beside it already caps cities-plus-settlers
at `desired_cities`, so a seat wanting three cities never wants a second settler.
With the target raised and room on the map, that reasoning no longer applies, and
six cities serialized through one settler at ~9 turns of production plus a walk
each is a rate limit no valuation can beat.

```
[deployment 6p 74x46] floor=3 parallel=false  cities 4.83  score 356
[deployment 6p 74x46] floor=6 parallel=false  cities 5.33  score 367
[deployment 6p 74x46] floor=6 parallel=true   cities 5.67  score 368
```

It fires, and it is **+0.34 cities on top of a +0.50 that measured exactly null
over 240 maps**. A fires-check exists to decide whether an evaluation is worth
its compute, and here it says no: the predecessor's larger effect on the same
metric did not convert, so this one has no path to converting either. Recorded
rather than run.

### What the axis amounts to

One oracle with large, replicated headroom — free settlers take a seat from
23.0% to 52.3% — and **seven treatments, none of which reaches it on wins**. The
oracle removes the settler's cost; every treatment redistributes production the
empire already had. The two things that did move were economic and did not
convert: `expansion_pays_back` (terminal score p=0.0005 and p=0.0037 on disjoint
seeds) and this one (+0.50 cities, null).

That is worth stating plainly for whoever picks this up: **the expansion gap is
real and is not currently reachable by changing a decision.** Anything further
here should either change what a settler costs, or stop and go elsewhere.

## 2026-07-29 — ★★★★ what a searching turn costs, and why the first answer was wrong

The entry above established that no searching agent is seated in the deployed
league and that breeding cannot produce one. The obvious explanation is cost.
`turn_cost` measures it at the deployment profile: 6 players, 74×46, 9
city-states, interleaved on the same seeds so both fleets meet the same
contention on a shared box.

| fleet | ms a game-turn | ratio |
|---|---|---|
| all `AdvancedAi` | 13.3 | 1× |
| **one searching seat among five** | **76.7** | **6.4×** |
| all `StrategicAi` | 410.0 | 29.2× |

**The first version of this probe measured only the all-searching fleet and
would have given the wrong recommendation.** It reported 25–31× and concluded
"too expensive to seat as it stands — make the search cheaper rather than
better." But nothing seats that way. A league entry is *one* strategy among five
opponents, so the cost of admitting search is `(5a + s) / 6a`, not `s / a`. The
configuration that would actually ship costs **6.4×**, which is a deliberate
trade rather than an impossibility — and the difference between those two
conclusions is the difference between abandoning the search line and investing in
it.

That is a general trap worth naming: **measure the configuration that would
ship, not the one that is convenient to construct.** It is the same failure as
evaluating at 4p 24×16 and deploying at 6p 74×46, one level down.

**What follows.** Seating a searching agent in the league is affordable. A round
would take several times as long, which is a real cost for an offline rating
system that runs continuously, but 76.7 ms a game-turn is about thirteen turns a
second — the live exhibition paces turns for viewers and reports
`frames_missed: 0`, so a searching seat is very unlikely to be what a spectator
would notice. The highest-value change available is therefore to anchor one, so
the self-improvement loop can rate search against the bred genomes at all.

**Limits.** Three seeds, 100 turns, on a box at load 30–50. The ratio is robust
to that because the runs are interleaved, but the absolutes are not, and
early-game turns are cheaper than late ones for both fleets, so a 250-turn game
would move both numbers up. The category — single digits, not tens — is what
this establishes.

| claim | status |
|---|---|
| an all-searching fleet costs ~29× a scripted one | **established** (n=3) |
| **seating one searching entry costs ~6.4×** | **established** (n=3) |
| search is too expensive to seat | **refuted** — that conclusion came from measuring a fleet nobody runs |
| a searching seat would break the exhibition's frame-per-turn guarantee | **unmeasured**, and unlikely at 13 turns/sec |

## 2026-07-30 — ★★★★ the first replayable, anchored Elo baseline

The tracked Elo file was not a baseline. It was a schema-2, 1000-centred toy
ledger from one game in which `advanced` and `basic` each controlled two seats.
Those cloned seats were counted as independent evidence, every
player/civilization combination started over, and no map, speed, turn limit, K,
or raw result survived. Appending to it could not produce a longitudinal
measurement.

Schema 3 now makes the experiment explicit and replayable:

- the primary row is one player identity across every civilization draw;
- exact player/leader/civilization rows are diagnostics and inherit that
  player's current prior;
- a protocol records the ordered controller roles, table size, dimensions,
  full turn limit, city-states, speed, map script/shape/poles, active mods, K,
  and a fixed rating anchor;
- mutable controllers enter under dated identities while frozen
  `advanced_v1` remains the connected control;
- every scored table is retained under a deterministic event id, sorted into
  one canonical update order, and replayed on load to verify the aggregates;
- rerunning a seed is idempotent, a changed result under the same id is an
  error, concurrent checkpoint arrival cannot change the final table, and
  keyed events cannot mix K values or unkeyed history;
- persistent tournaments reject duplicate identities, effective-controller
  aliases, degraded learned entrants, cloned seats, a missing controller role,
  malformed numeric flags, and any profile mismatch.

The anchor is literal rather than documentary. After every Elo update all rows
move by the same translation that returns `advanced_v1` to 1500. Pairwise gaps
and expected scores are unchanged. What changes is cross-generation meaning:
repeatedly introducing a fresh weak controller at 1500 can no longer inflate a
new challenger while leaving an older inactive challenger on another scale.

Canonical command, protocol v1:

```bash
civvis tournament \
  --ais advanced-20260730=advanced,advanced_v1,basic-20260730=basic,random-20260730=random \
  --games 40 --players 4 --seed 0 --quiet
civvis tournament --standings
```

Profile: 4 players, 60×38, Standard 500 turns, six city-states, Pangaea,
flat/poles, stock rules fingerprint `fnv1a64:3423bd46da2b8cd7`, K=24; ordered
controller roles are `advanced`, `advanced_v1`, `basic`, `random`. Every
entrant drew each of Rome, Greece, Egypt, and China ten times. The fingerprint
is computed from the fully merged JSON, so changing stock data or editing a
mod without renaming it now forces a different ledger; engine/scoring changes
still require an explicit protocol bump.

| immutable player | anchored Elo | direct Elo vs anchor | games | wins |
|---|---:|---:|---:|---:|
| `advanced-20260730` | **1588.5** | **1708.2** (95% 1588.7–1841.0; pair 31/40, 62.5–87.7%) | 40 | 29 |
| `advanced_v1` (fixed anchor) | **1500.0** | — | 40 | 8 |
| `basic-20260730` | 1431.4 | 1314.8 (95% 1187.3–1431.0; pair 10/40, 14.2–40.2%) | 40 | 3 |
| `random-20260730` | 1174.1 | 736.6 (95% −∞–1093.0; pair 0/40, 0.0–8.8%) | 40 | 0 |

This establishes a reproducible +88.5 Elo gap for the July 30 advanced
controller over the frozen legacy control on one named multiplayer profile. It
also gives an order-independent +208.2 direct performance gap from their 31/40
pair score; its 95% Wilson interval is 62.5–87.7%, which transforms
monotonically to 1588.7–1841.0 Elo. The difference is
expected because the incremental K=24 path has only 40 updates and is not a
batch maximum-likelihood fit. The direct-anchor result at an equal game count
is therefore the longitudinal baseline; the incremental number remains useful
for continuously updated ordering. Neither turns 40 correlated multiplayer
worlds into a narrow confidence interval, and both remain an internal CIVVIS
scale rather than a human or Firaxis calibration. Future controllers need a
new identity and the same protocol; a material rules or scoring change needs a
new protocol rather than being mixed into this file.

## 2026-07-30 — the contextual rating update was overconfident

The staged Gaussian updater summed each observation's *posterior mean shift*.
That applies the prior variance once for every placement stage, and repeatedly
marginalizes the same civilization uncertainty as though it were a fresh draw.
The fix accumulates likelihood natural score and precision for each
player/civilization seat, collapses the repeated stages, and applies the final
posterior variance once. Closed-form tests now cover a plain repeated
observation, a shared context, order invariance, and diminishing evidence.

Strictly out-of-sample replay on the available live history (822 games through
round 697; last 70% scored):

| seats | evaluated games | Glicko-2 info/game | corrected staged + civ |
|---:|---:|---:|---:|
| 4 | 127 | **+0.1612** | +0.1505 |
| 6 | 37 | +0.2074 | **+0.2477** |
| 8 | 365 | −0.0141 | **+0.0104** |
| mixed, mean 7.0 | 576 | +0.0521 | **+0.0608** |

The old update scored +0.0632 nats/game on that same mixed replay. Correcting
it removes 0.0024 nats/game of optimistic movement while retaining a small
+0.0087 advantage over Glicko on the mixed slice. The four-seat slice favors
Glicko, the six-seat slice is small, and neither system extracts much from the
eight-seat history. The honest conclusion is profile dependence, not that one
estimator universally replaced the other.

## 2026-07-30 — ★★★ the league now breeds winners, not safe placers

The league needs two legitimate but different numbers. Its public Glicko ladder
ranks full placement; evolution is supposed to produce the agent most likely to
win. Until this change, one `conservative_order` drove parent choice, niche
strength, niche elites, and retirement from `rating ± 1.96·rd`. The `wins`
counter printed beside it did not affect breeding at all.

That is not a theoretical distinction in this roster. A completed controlled
run recorded above found the placement leader `g56-50` **−108 win-Elo** against
the embedded champion. Another found a 3.6× win-rate difference compressed to
7.2 placement-rating points. Selecting harder on the same placement estimate
cannot recover an objective the estimate does not measure.

Evolution now uses the lower 95% Wilson bound on outright win rate for parents,
best-niche choice, and protected niche elites. Retirement uses the upper bound,
so an uncertain newcomer remains protected; placement Glicko breaks only an
exact win-bound tie. Two wins in two games do not outrank a settled 70% winner
in the regression test.

Applied read-only to the committed roster, the parent order changes as follows:

| win-selected rank | entrant | wins/games | lower 95% win bound | placement Elo |
|---:|---|---:|---:|---:|
| 1 | `g20-21` | 82/216 | **31.8%** | 1790.8 |
| 2 | `g28-28` | 61/170 | **29.1%** | 1766.0 |
| 3 | `advanced_v1` | 111/331 | **28.7%** | 1754.6 |
| 4 | `advanced` | 91/331 | 23.0% | 1702.7 |
| 5 | `g44-41` | 26/84 | 22.1% | 1753.2 |
| 8 | `g56-50` | 4/21 | **7.7%** | **1823.3** |

The highest placement point estimate therefore stops being the second breeding
choice on four wins of evidence; the two well-sampled winning genomes become
the first two. This does not prove their children will improve, and raw win
rate assumes the league keeps entrants on the same table-size distribution.
It does align the self-improvement loop's selection pressure with its stated
goal. Glicko remains unchanged for prediction, seating, and the viewer-facing
ladder rather than being forced to serve two incompatible objectives. The
`strategic_deep_league` transfer entrant now consumes this same win-first
ordering; it no longer quietly switches back to placement Glicko when choosing
which generalist genome to put under macro search.

## 2026-07-30 — live league games no longer invent a strict finish order

Distributed league results already retained the engine winner and competition
ranks, so equal scores were Glicko draws. The spectator recorder instead sorted
the same terminal state and passed only `(strategy, civilization)` rows to a
legacy adapter. That adapter assigned ranks `0, 1, 2, ...`: every live score tie
became an arbitrary pairwise win for the lower player id, and the leader was
re-derived later rather than retained from the game that actually ran.

Batch and live ingestion now share one competition-ranking function. The live
API carries strategy, exact leader, civilization, rank, and declared-winner
status into the same `Outcome`; malformed ranks fail before the roster lock is
mutated. A regression pins a declared low-score winner above two equal-score
losers, verifies that the tied players receive identical Glicko movement, and
checks the exact `player@leader@civ@rank` audit row. The compatibility adapter
remains for callers that truly have only a strict list, but the engine no longer
uses it.

## 2026-07-30 — corrected joint macro search is neutral at deployment scale

The July 29 interaction probe reconstructed sequential search incorrectly: it
chose the lane under the doctrine currently in force. The live policy has
always chosen its lane under the unchanged evolved base genome and considered
the active doctrine only in the second step. `choose_joint_axes` now reproduces
that exact order, with a regression in which a prior doctrine switch would
make the old comparator choose a different lane. The full-matrix treatment is
therefore a test of joint optimization against the policy that actually ships.

Pre-registered deployment-profile command:

```bash
cargo run --profile ci --locked --bin ai_eval -- \
  strategic_joint strategic_doctrine --pairs 20 --players 6 \
  --width 74 --height 46 --turns 250 --city-states 9 \
  --speed online --seed 7320000 --jobs 10
```

The 20 maps produced 40 games averaging 213.3 turns. Every mirrored map split
one game each: **20/40 wins per arm, 50.0% paired score, +0 Elo-equivalent**
(95% Wilson **−148 to +148**), zero favorable directions either way, and
two-sided sign p=1.0000. Terminal score leaned only 50.4%; five maps favored
joint, one favored sequential, and fourteen tied (p=0.2188). No promotion
evidence crossed at any point.

The mechanism did fire, but rarely. Joint search reached 268 rollout reviews
and replaced the exact sequential choice in **10 (3.7%)**; all ten changed the
lane and nine also changed doctrine. Only 42% of all reviews reached rollouts,
because urgent counters and irreversible religious commitments correctly
answered the others before macro search. A joint review evaluates 28
lane × doctrine branches instead of the sequential policy's 11. The feature
therefore spends roughly 2.5× the branch work at the reviews where it can act,
for no resolved win gain in this sample.

This is not proof of exact parity—the interval is wide—but it is enough to
reject promotion. `joint_axis_search` remains off in every production entrant;
`strategic_joint` stays as an evaluator-only control so a larger future run can
accumulate evidence without rebuilding the experiment. The corrected result
also replaces the retracted 64.6%/22.3% diagnostic above: interaction in an
internal value matrix is not gameplay strength, and the exact shipped
comparator matters before either number is interpretable.

## 2026-07-30 — the deployment audit now measures the deployment

`audit` previously constructed every game through `Game::new`, so a command
that said `--turns 250` was still a truncated **Standard-speed** game. Its war
checks also hard-coded ten raw turns, falsely reporting legal Online peace
after the rules had scaled the ten-Standard-turn duration to eight. The harness
now accepts `--speed`, derives the default turn limit from that speed, constructs
`GameOptions`, and applies `Game::standard_duration` to duration invariants.
Invalid nonpositive counts and undersized maps are rejected or bounded rather
than wrapping through integer casts.

The motion census is now split by controller role. That makes a rated major's
rate comparable across maps with different city-state and barbarian
populations, whose intentionally bounded jobs have very different idle shapes.
Exact rerun:

```bash
cargo run --profile ci --locked --bin audit -- \
  --games 2 --players 6 --width 74 --height 46 --turns 250 \
  --city-states 9 --speed online --start-seed 7330000 --quiet
```

| controller role | unit-turns | livelock | idle field | fortified picket |
|---|---:|---:|---:|---:|
| rated majors | 53,100 | **0.49%** | 18.96% | 7.75% |
| city-states | 19,315 | 0.38% | 26.05% | 28.01% |
| barbarians | 17,625 | **3.31%** | 6.10% | 75.26% |
| all | 90,040 | 1.02% | 17.97% | 25.31% |

Both games completed with **zero rule violations**. The attribution changes the
gameplay reading: most remaining circling belongs to barbarians, while the
rated controllers spend only one unit-turn in 204 in a confirmed loop. Raising
the global anti-livelock penalty from the blended 1.02% headline would optimize
the wrong population and risk damaging productive major movement. No tactical
constant changed. Future AI work should use the `major` line as its baseline
and treat the other roles as separate controllers.

Attribution also localized all 16 “city builds nothing” reports to
city-states. Each had reached its bounded garrison and exposed only general
districts or repeatable projects that its deliberately specialized governor
does not choose. The auditor had equated “engine-legal” with “on this policy's
action surface.” It now treats only a repair, building, repair project, or the
city-state's own district family as actionable infrastructure; the exact rerun
removes all 16 false symptoms without changing gameplay.

## 2026-07-30 — ★★★★ win selection is now conditioned on table size

The first win-selected league fix still compared one mixed lifetime win rate.
That is not a stable objective when the history contains different table
sizes: parity is 50% in a duel, 25% with four players, and 12.5% with eight.
An entrant could therefore become a “proven winner” merely by having more of
its evidence come from smaller tables. The current round's manifest already
fixes its player count, but selection discarded that identity after rating.

Every newly rated seat now checkpoints `(games, wins)` under its exact table
size. Parent ordering, strongest-niche choice, niche-elite protection, and
retirement use the Wilson bound for the **current manifest's player count**;
the placement rating remains only the exact tie-break. The manifest, rather
than a worker's local flags, also supplies the selection player count during
distributed finalization.

Long-lived rosters are migrated from retained `matches.csv` rows before the
next idle update. The migration accepts both historical `player@civ` rows and
the current `player@leader@civ@rank` form, rejects an ambiguous or malformed
log rather than guessing, and labels the standings as retained evidence.
Older aggregate games that predate the log remain in the public totals but are
not assigned a fictional table size. If no retained row exists for the current
size, selection explicitly falls back to that legacy aggregate until direct
profile evidence arrives.

Read-only replay of the production roster's 822 retained games changes the
conservative order materially. These are the active evolvable parents; bounds
are lower 95% Wilson bounds on outright wins:

| entrant | mixed all-history evidence | mixed lower | retained 6p evidence | 6p lower |
|---|---:|---:|---:|---:|
| `advanced_evolved` | 33/99 | **24.8%** | 4/18 | 9.0% |
| `advanced` | 162/714 | 19.8% | 7/27 | 13.2% |
| `g28-28` | 141/748 | 16.2% | **12/33** | **22.2%** |
| `g48-44` | 106/605 | 14.7% | 4/20 | 8.1% |
| `g676-58` | 7/57 | 6.1% | **5/12** | **19.3%** |

The mixed statistic would breed `advanced_evolved`, then `advanced`, then
`g28-28`; a six-player round instead has direct evidence for `g28-28`, then
the diplomatic `g676-58`, then `advanced`. This is not a claim that 12 games
settle the second entry—the Wilson penalty is why its lower bound is 19.3%
rather than its 41.7% point rate. It is a demonstration that the discarded
context changed the decision the breeder makes. A four-player round reads its
separate four-player evidence and can make a different, reproducible choice.

This still does not combine different maps, speeds, or opponent pools into a
causal head-to-head estimate. It fixes the largest mathematical mismatch in
raw win probability and makes the remaining context visible in the checkpoint
instead of irreversibly blending it. The anchored tournament remains the
instrument for a fully fixed longitudinal Elo profile.

## 2026-07-30 — ★★★ the live exhibition now selects winners, not placers

After evolution moved to same-table-size outright wins, the exhibition still
sampled each civilization's top three **placement** ratings. That left the
system optimizing one objective and deploying another. It also made a
low-placement outright winner almost unable to collect further live evidence:
the result recorder could rate it, but the seating policy would not let it play.

Seeded live seating now ranks the available roster by the lower 95% Wilson win
bound for the full table size, uses the exact leader/civilization placement
rating only as a tie-break, and keeps the existing 3:2:1 rank-weighted sample
and no-repeat rule. The full table size is passed explicitly; a game with one
human and five AI seats must read six-player evidence, not pretend it is a
five-player contest. If fewer than the requested top three have retained exact
evidence, the whole candidate pool stays on the previous placement policy.
One migrated row therefore cannot switch half a lineup onto a different
objective, and the embedded legacy snapshot remains behaviorally compatible
until comparable evidence exists.

Read-only replay of the production log gives these first-seat winner pools:

| live table | conservative top entries (wins/games; lower 95% bound) |
|---:|---|
| 4p | `advanced_evolved` 25/61 (29.5%), `advanced` 25/61 (29.5%), `g28-28` 19/52 (24.8%) |
| 6p | `g28-28` 12/33 (22.2%), `g676-58` 5/12 (19.3%), `advanced` 7/27 (13.2%) |
| 8p | `advanced_evolved` 2/3 (20.8%), `g48-44` 60/422 (11.2%), `advanced` 36/254 (10.4%) |

The six-player case is the material correction. Placement puts `g676-58` near
the bottom at 1548 and would normally omit it; direct six-player tables record
five wins in twelve, enough for its conservative bound to rank second even
after the small-sample penalty. Conversely placement leader `g48-44` has only
4/20 six-player wins (8.1% lower bound) and leaves the first pool. Later seats
repeat the calculation after removing already used entrants, preserving
lineup diversity without reaching past known winners while comparable choices
remain.

This is an objective-alignment change, not a randomized gameplay A/B. The
retained games directly answer which named controllers won at each table size,
but their opponent pools and maps are not fully controlled, so the new lineup
is better supported rather than causally guaranteed stronger. The display
ladder intentionally remains placement Glicko; a spectator can still see who
usually finishes well without that statistic silently deciding which AI is
supposed to win the exhibition.

## 2026-07-30 — live winner pools remain win-selected after seat one

The first live implementation checked whether the **currently unused** pool
still contained three exact profiles before every seat. With exactly three
profiled entrants, seat one correctly sampled them, but removing its pick left
only two; seat two then reverted to placement and could admit an unprofiled
high placer before the two remaining known winners. That contradicted both the
selection objective and the stated “while comparable choices remain” contract.

Readiness is now decided once from the full live roster. A ready table samples
only exact-profile entrants until they are exhausted, shrinking 3:2:1 to 2:1
and then 1 as no-repeat seating consumes them. Only an unavoidable later seat
may fall back to unprofiled placement. A roster with fewer than three profiles
at the start still uses the complete legacy placement policy. The regression
sweeps 128 seeds with exactly three profiled strategies plus a higher-rated
unprofiled strategy: the first three seats must always be the three winners,
and the fallback must appear fourth.

## 2026-07-30 — persistent Elo now pins the implicit lobby too

The schema-3 tournament profile already pinned rules content, map geometry,
speed, turn limit, city-state count, controller roles, mods, and K. The harness
also inherited outcome-affecting defaults from `GameOptions`: Civ6 rules, an
Ancient start, Prince difficulty, barbarians, disaster intensity 2, no modes,
the Civ6 leader pool, deterministic stock civilization fill, no human seats,
free-for-all teams, and every victory type. A later default change could
therefore have appended a different experiment to the same ledger without a
profile mismatch.

The profile now carries a readable `setup_contract` derived from the same
`GameOptions` defaults used to construct each tournament game. Profile equality
rejects any contract change, and a pinned regression forces deliberate protocol
review when one of those defaults moves. Existing schema-3 files deserialize to
the known historical contract for compatibility; the canonical 40-game ledger
writes it explicitly. Raw-game replay continues to reproduce the same aggregate
and anchored ratings, so this closes an identity gap without changing the
baseline experiment.

## 2026-07-31 — AI-strength audit: causal controls and a profile-matrix gate

The largest replicated oracle ceiling is still city-state control: perfect
suzerainty measured 56.7% against 22.7% control over 400 maps. New censuses
located the reachable bottleneck before changing play. The agent held 0.00
unspent envoys, but on the 6p 74×46 sample controlled only 7% of met
city-states; it built a Diplomatic Quarter in 1/6 games and never built a
Consulate or Chancery. Allocation is not the problem. Income is.

The implementation therefore added independent evaluator arms for the live
policy deck, explicit influence valuation, influence infrastructure, and the
combined economy. Infrastructure converts `influence_points` into expected
envoys using the active government's threshold and payout, the remaining turn
horizon, and only met city-states not already controlled. It also lets the
first Diplomatic Quarter see a discounted Consulate stream. Zero-city-state,
unmet, already-controlled, and expired-horizon cases receive exactly zero new
value.

The screens correctly refused the initial causal story:

| comparison | profile/maps | paired score | terminal score | envoy diagnostic |
|---|---:|---:|---:|---|
| infrastructure vs stock | 4p Online, 12 | 50.0% (+0) | 49.8% | 6.6 vs 5.9 |
| influence weight vs live-deck control | 4p Online, 12 | 43.8% (−44) | 50.8% | 5.0 vs 4.0 |
| combined vs stock | 4p Online, 12 | 52.1% (+14) | 50.4% | 9.0 vs 7.6; suzerainty 0.40 vs 0.21 |
| combined vs stock | 6p deployment, 12 | 54.2% (+29) | 50.5% | 17.6 vs 15.0; suzerainty 0.67 vs 0.56 |
| live-deck control vs stock | 6p deployment, 8 | 53.1% (+22) | 51.4% | 13.5 vs 12.3 |
| influence weight vs live-deck control | 6p deployment, 8 | 50.0% (+0) | 50.6% | 16.6 vs 16.9 |
| infrastructure vs stock | 6p deployment, 8 | 53.1% (+22) | 50.4% | 14.4 vs 14.1 |

Those component rows use disjoint seeds and are not additive. They show that
the combined deployment direction cannot be attributed to influence income:
the influence-only component was flat and infrastructure did not materially
move envoys on its deployment sample. The pre-existing live deck accounts for
at least as much of the direction. All four arms remain reproducible; none of
the new envoy behavior is enabled in `advanced`.

The second direct treatment addressed strategy churn. A 40-Standard-turn
(20 Online turns) commitment applies only to soft best-lane, lane-progress,
and opportunistic-war changes. Wars, threats, emergencies, assigned lanes,
city deficits, Prophet races, and exits from Recovery remain immediate. Final
review caught an earlier version that could still linger in Recovery; its −73
deployment screen is not evidence about the corrected treatment.

The corrected arm completed a 20-map matrix. Compact midgame switches fell
from 3.74 to 2.90 per seat-game and unanchored switches from 1.98 to 1.44; it
scored 53.8% (+26) with 50.6% terminal score. Deployment switches fell 2.99 to
2.69 and unanchored switches 1.79 to 1.66; the arm scored 55.0% (+35), with an
8–3 direction (p=0.2266) and flat 50.1% terminal score. Both verdicts were
INCONCLUSIVE. Compact therefore passed the safety requirement, deployment did
not meet the strength requirement, and the matrix retained `advanced`. The
treatment remains evaluator-only: reduced churn is demonstrated, strength is
not.

### The matrix gate

`ai_eval --matrix` now runs both required profiles concurrently with disjoint
seeds and a shared total job budget:

```sh
cargo run --release --bin ai_eval -- challenger advanced \
  --matrix --pairs 120 --jobs 12 --seed 12000000
```

The compact safety profile is 4p 24×16, four city-states, Standard speed and
500 turns. The strength profile is the 6p 74×46, nine-city-state, Online
250-turn deployment. Both use Continents, Planet topology, poles, randomized
civilizations, and science/culture/domination victories. Deployment must earn
an ordinary `promotion gate: PASS`; compact must have at least 20 maps and
must not earn `RETAIN` for the incumbent. Any failed child process, insufficient
sample, compact regression, or deployment result short of PASS retains the
incumbent and exits non-zero. Profile-shaping flags are rejected in matrix mode
so a caller cannot silently rename a different experiment as the gate.

The first 20-map matrix application tested `advanced_policy_live_control`
against stock `advanced`. Compact Standard scored 47.5% (−17 Elo direction),
with 52.4% terminal score; its verdict was INCONCLUSIVE, so it satisfied the
no-regression safety requirement. Deployment Online scored 53.8% (+26), won
14 games against 11, and took 52.1% terminal score; terminal direction was
15–5 (sign p=0.0414), but the win-based promotion verdict remained
INCONCLUSIVE. The matrix therefore accepted 1/2 profiles, exited non-zero, and
retained `advanced`. That is exactly the intended distinction between a
promising follow-up and a production-strength claim.

An independent 40-map check then compared the retained controller with the
source-pinned `advanced_v1` anchor on the matrix's deployment profile (nine
city-states and randomized civilizations, seed 10042000). `advanced` scored
47.5% (−17 Elo-equivalent, 95% CI −124..+89), with 7 maps favoring it, 10
favoring the anchor, and 23 neutral (sign p=0.6291). Terminal score was 49.0%
(14–26 direction, p=0.0807). The result is **INCONCLUSIVE**, not a reversal of
the earlier +207/PASS result on the six-city-state fixed-roster profile and not
evidence that `advanced` is universally strongest. It establishes the narrower
claim needed here: none of the new treatments displaced the incumbent, and the
incumbent/anchor ordering itself must remain profile-qualified.

## 2026-07-31 — full-prefix resolution and evaluator integrity

The initial screens above were followed by fixed, non-optional prefixes. No
treatment cleared both sides of the promotion matrix, so this work does **not**
change the production `advanced` controller. `advanced_v1` remains the frozen
source-pinned anchor; it is not renamed as the production winner either.

Before extending the experiments, the evaluator's identity path was hardened.
Netless `policy`, `policy_wide`, and `policy_frozen` arms that consume the
embedded champion are now canonically identified as `advanced_evolved`, while
netless `neural` is `basic_evolved`. Loaded aliases print the same canonical
identity. A shared alias table drives both provenance and construction, so the
factory cannot silently build one controller while provenance reports another.
Direct evaluation now refuses missing definitional artifacts unless the caller
explicitly supplies `--allow-degraded`; matrix promotion never permits that
override. True effective self-play is an error rather than a warning.

The matrix runner also received two reproducibility/latency fixes. Each profile
seed now uses a constant 1,000,000-seed stride, independent of `--pairs`, so
extending a prefix cannot move the other profile onto different maps. The total
worker budget is split approximately one third to compact and two thirds to
deployment, with every worker retained; result windows contain four games per
worker to reduce scheduler-tail idle time without material buffering.

The fixed-prefix outcomes were:

| challenger | maps/profile | compact safety | deployment strength | matrix decision |
|---|---:|---|---|---|
| `advanced_strategic_commitment` | 120 | 51.0%, +7 | 46.5%, −25; direction 18–34, p=0.0365 | retain stock |
| `advanced_evolved_commitment` | 20 | 52.5%, +17 | 40.0%, −70; direction 5–15, p=0.0414 | retain stock |
| `advanced_evolved` | 120 | 57.3%, +51; direction 44–16, p=0.0004 | 45.6%, −30; direction 24–40 | retain stock |
| `advanced_envoy_priority` | 120 | 48.3%, −12 | 54.4%, +30; direction 38–19, p=0.0163 | inconclusive; retain stock |
| `advanced_policy_live_control` | 300 | 52.3%, +16; 95% Wilson 46.7–57.9 | 54.3%, +30; 95% Wilson 48.7–59.9 | inconclusive; retain stock |

The 300-map live-policy confirmation used a new, disjoint prefix:

```sh
target/ci/ai_eval advanced_policy_live_control advanced \
  --matrix --pairs 300 --jobs 8 --seed 22051000
```

Its deployment direction was 92–51 (p=0.0008), and its anytime evidence crossed
the directional threshold at map 98, but the ordinary paired-score Wilson lower
bound remained below 50%. The pre-declared gate therefore stayed INCONCLUSIVE.
This is evidence for another targeted treatment, not permission to weaken the
gate after seeing the result.

Two independent anchor checks kept the conclusion profile-qualified. The live
policy control versus `advanced_v1` over 300 fresh maps scored 50.5% (+3) on
compact and 53.7% (+26) on deployment; both matrix verdicts were inconclusive.
Stock `advanced` versus `advanced_v1` over a separate 120-map prefix scored
45.6% (−30) on compact and 48.8% (−9) on deployment; that matrix likewise did
not establish a replacement. These do not contradict older PASS results on a
different six-city-state, fixed-roster profile. They demonstrate why controller
rankings must name the profile and decision rule.

The direct envoy arm did change the intended mechanism. Its deployment averages
were 19.3 versus 14.3 envoys and 0.70 versus 0.41 suzerainty share, with higher
science, culture, and economy diagnostics. The new production prepass constructs
the empire-unique Diplomatic Quarter → Consulate → Chancery chain only when a
met, contestable city-state and enough remaining envoy stream exist. It never
overwrites an occupied queue and yields to Recovery, local danger, rushing, and
major war. Those behaviors remain default-off because their opportunity cost has
not cleared the promotion rule.

### Verification of the integrated tree

The rebased implementation passed the complete optimized test suite:

```sh
cargo test --profile ci --locked
# library: 1291 passed, 0 failed, 20 intentionally ignored
# all binary, integration, and doc-test suites: 0 failed

cargo build --release --locked --bin civvis --bin ai_eval
```

A fresh deployment-scale health soak also completed 12/12 games without a
panic, no-tech-progress flag, minor winner, or unexplained no-winner result:

```sh
target/release/civvis soak --games 12 --players 6 \
  --start-seed 26051000 --jobs 8 --width 74 --height 46 \
  --city-states 9 --speed online --turns 250 \
  --map continents --shape planet --poles poles
```

The outcomes included science, culture, religion, and turn-cap score victories,
with live declarations, unit losses, city captures, and three capital captures.
Release-mode CLI checks independently confirmed exit 2 for effective self-play,
exit 3 for a missing definitional artifact, and exit 2 when
`--allow-degraded` is attempted in matrix mode.

## 2026-07-31 — typed arm identity and strict evaluator construction

This is an evaluator-integrity change, not a policy treatment. It adds no
strength result and changes no production `advanced` behavior.

The evaluator now receives an agent, effective provenance, and typed
architecture/weights/evaluator/treatment specification from one fallible arm
boundary. Collapse checks compare the resolved specs. Comparisons that differ
on multiple behavior-defining axes refuse to run without
`--deployment-comparison`; promotion-matrix children state that flag because
the matrix asks whether the whole challenger should replace the incumbent.

Artifact resolution is also end-to-end. `--artifact-dir` is threaded into the
actual genome, policy-net, strategic-net, wide-net, and production-net
constructors and into matrix children. Missing definitional artifacts fail by
default; `--allow-degraded` is the explicit diagnostic path and remains
forbidden in matrix mode.

Independent checks exercised the runtime boundary, not only its display text:

- all 83 registered names resolved and constructed in production and empty
  artifact directories;
- every reported alias matched its effective arm byte-for-byte after a seeded
  short game;
- distinct stock/genome controls produced distinct specs and game states;
- a modified custom champion matched direct construction from that file and
  differed from the production champion;
- the 46 focused Elo tests and all 27 `ai_eval` tests passed.

CLI probes additionally confirmed: a three-axis `production`/`advanced`
comparison exits 2 without the deployment flag and completes with it; a
single-axis `basic`/`advanced` comparison completes using a custom empty
artifact directory; default degraded construction exits 3; and an explicitly
degraded effective self-comparison exits 2.

The complete integrated tree then passed `cargo test --profile ci --locked
--no-fail-fast`: 1,297 library assertions passed, 20 censuses/benchmarks were
intentionally ignored, and every binary, integration, and doc-test suite
completed with zero failures.

## 2026-07-31 — preregistration: live-policy/direct-envoy composite

The next strength hypothesis is fixed before reading a result. The challenger
`advanced_envoy_composite` differs from stock `advanced` in exactly one named
treatment bundle: it enables the existing `PolicyDeck::Live` and the existing
direct, horizon-gated Diplomatic Quarter → Consulate → Chancery production
path. It makes no other change. In particular, `pol_influence` remains zero
because the isolated influence-weight treatment was flat.

The fixed command and seed prefixes are:

```sh
target/ci/ai_eval advanced_envoy_composite advanced \
  --matrix --pairs 300 --jobs 8 --seed 32051000
```

Compact consumes seeds 32,051,000–32,051,299 and deployment consumes
33,051,000–33,051,299. They are disjoint from every prefix used to choose or
extend either component. Reading the same prefix at 20 or 120 maps is an
interim diagnostic only; the committed result is the full 300-map prefix and
no parameter or mechanism may change inside it.

The decision rule is unchanged: deployment must earn an ordinary win-based
`promotion gate: PASS`, compact must contain at least 20 maps and must not
retain the incumbent, and both children must complete. Only that matrix PASS
permits changing the production default. Any other result retains stock
`advanced` and leaves the composite evaluator-only.

Mechanism and opportunity-cost diagnostics are also fixed in advance. The run
will report envoys and suzerainties, completed Diplomatic Quarters, Consulates,
and Chanceries, any terminal chain stage still queued and its remaining
production cost, plus cities, population, research, yields, military, and
ordinary queue cost. The primary decision remains completed-game wins; none of
these diagnostics can substitute for it.

### Execution incident and frozen engine repair

The first execution completed the deployment child but aborted the compact
child on seed 32,051,026. A cloned forcing-reply line captured a civilization's
last Settler; elimination then removed a foreign Battering Ram that was still
linked to the attacking Pike and Shot, and `enter_tile` dereferenced that
cached formation partner. The invalid cross-owner link originated when levy
control transferred only the military member of a city-state escort
formation.

No challenger or control behavior was adjusted. Before restarting the fixed
prefix, the engine was changed so every unit-ownership transfer atomically
unlinks both formation members, linked leaders require same-owner symmetric
co-location, and tile entry revalidates its cached partner after capture and
elimination callbacks. Targeted regressions cover both levy directions and the
exact elimination shape; seed 32,051,026 then completed. The full matrix must
be rerun from seed 32,051,000 under the original command and decision rule;
the already observed deployment result is not reused as the promotion result.

### Complete fixed-prefix result and promotion

The repaired executable reran the exact command over both complete prefixes.
The matrix passed:

| profile | paired score | 95% Wilson CI | Elo-equivalent | map direction | gate |
|---|---:|---:|---:|---:|---|
| compact Standard | 50.4% | 44.8%–56.0% | +3 (−36..+42) | 68–59, 173 neutral | ACCEPT / INCONCLUSIVE |
| deployment Online | 58.9% | 53.3%–64.3% | +63 (+23..+102) | 119–40, 141 neutral | PASS |

Deployment's exact sign test rounded below 0.0001 and its anytime-valid
e-process crossed at map 72, peaking at `7.153e8`. The challenger won 232 of
600 deployment games versus 125 for the incumbent. Compact completed all 300
maps without an established regression: 161 challenger wins versus 156 and an
exact directional `p=0.4779`. The parent therefore reported
`multi-profile promotion gate: PASS`.

The mechanism moved exactly as intended. At deployment, mean Envoys rose
14.0→20.1 and suzerainties 0.38→0.72. Completed Diplomatic Quarters,
Consulates, and Chanceries rose from 0.33/0.21/0.17 to 0.94/0.93/0.92, while
terminal chain cost stayed essentially exhausted (0.1 Production). The gain
did not come at an observed development loss: cities rose 5.86→6.18,
Population 74.9→80.4, technologies 58.7→61.7, civics 44.8→46.5, Production
328.1→364.5, Science 175.7→204.2, and military power 1028.1→1182.0. Science
wins rose 105→200.

The exact bundle is now production `AdvancedAi::new()` and therefore reaches
server autoplay, CLI fleets, default fallbacks, and the `advanced` builtin
through one constructor. `advanced_envoy_composite` is retained as a canonical
alias of `advanced`; `advanced_pre_envoy_composite` freezes the old controller
for historical comparisons. Every older evaluator-only treatment continues to
construct from that frozen baseline, so promotion does not retroactively
change what its recorded name means.

Post-promotion verification used the production constructors rather than the
experimental comparison alone. `cargo test --profile ci --locked
--no-fail-fast` passed all 1,300 library tests (20 deliberate censuses and
benchmarks ignored) and every binary, integration, and doc-test target.
`cargo build --release --locked --bin civvis --bin ai_eval` completed, and the
release deployment-scale health command below completed 12/12 fresh-seed games
without a panic or incomplete result:

```sh
target/release/civvis soak --games 12 --players 6 \
  --start-seed 34051000 --jobs 8 --width 74 --height 46 \
  --city-states 9 --speed online --turns 250 \
  --map continents --shape planet --poles poles
```

Finally, the release evaluator refused `advanced_envoy_composite advanced` as
an effective self-comparison with exit code 2, confirming that the promoted
alias cannot accidentally manufacture new evidence against itself.

## 2026-07-31 — preregistration: fog-honest city pressure

`advanced_fog_pressure` starts from the promoted live-policy/direct-envoy
controller and changes one information channel. Friendly units and city
defenses remain current facts. Hostile contributions to Recovery and Bastion
pressure instead come only from units observed through the shared visibility
contract: current sightings use their observed effective HP, and last-known
strength fades linearly to zero over six turns. Current hidden position and HP
are never read by this channel. The incumbent retains the old omniscient scan.

The fixed strength command is:

```sh
target/ci/ai_eval advanced_fog_pressure advanced \
  --matrix --pairs 300 --jobs 8 --seed 34061000
```

Compact consumes seeds 34,061,000–34,061,299 and deployment consumes
35,061,000–35,061,299. Neither prefix was used to select the treatment,
memory horizon, or decay. The complete prefix must be read once; interim
results cannot alter the mechanism.

This is an information-integrity change rather than an optional value tweak,
so its rule is non-inferiority: promote only if both 300-map children complete
and neither child retains the incumbent. An ordinary deployment superiority
PASS is stronger evidence but is not required. Any retained regression leaves
the arm evaluator-only and sends the implementation back for repair rather
than knowingly trading substantial strength for the first attempted policy.

Independent integrity checks are fixed before the strength result. Unit tests
must prove that byte-identical observation tensors with different hidden
current position/HP produce identical pressure and Recovery selection; that a
new visible sighting can change pressure; that stale memory decays to zero;
that own current HP remains exact; and that serial, one-worker, and four-worker
pressure vectors are bit-identical. The general `fog_census` is also run on
the same probe prefix with and without `--fog-honest-pressure`. Because it
perturbs other controller inputs too, only removal of pressure-caused
divergences is attributable to this treatment; unrelated residual witnesses
must remain reported rather than hidden.

### Complete fixed-prefix result and promotion

Both 300-map children completed. The ordinary strength parent retained
`advanced` because deployment superiority was inconclusive, but the
preregistered integrity/non-regression rule passed: neither profile retained
the omniscient incumbent.

| profile | paired score | 95% Wilson CI | Elo-equivalent | map direction | fixed integrity rule |
|---|---:|---:|---:|---:|---|
| compact Standard | 48.2% | 42.7%–53.9% | −12 (−51..+27) | 50–68, 182 neutral | ACCEPT / no retained regression |
| deployment Online | 51.4% | 45.8%–57.0% | +10 (−29..+49) | 67–59, 174 neutral | ACCEPT / no retained regression |

Compact's exact sign test was `p=0.1172`; terminal score was exactly 50.0%
with directions 146–150. Deployment's sign test was `p=0.5331`; terminal
score was 50.1% with directions 161–139. The treatment won 202 deployment
games versus 185 for the incumbent, while compact recorded 150 versus 171.
Neither anytime-valid process crossed in either direction.

The mechanism diagnostic moved strongly in the intended channel. On compact,
threat-accompanied plan boundaries fell 1,548→1,049; on deployment they fell
2,165→1,448. Recovery exposure fell 9.8%→9.4% compact and 15.5%→14.5%
deployment. The main economy stayed close: deployment cities were 6.03 versus
6.00, Population 77.6 versus 77.4, Science 193.7 versus 191.6, and military
power 1130 versus 1120. Production was 348.4 versus 350.4. This is evidence
that hidden contacts had been generating alarms, not an ordinary superiority
claim.

The full fresh-prefix integrity census then ran both controllers:

```sh
target/ci/fog_census --maps 64 --probes 12 --players 4 \
  --width 44 --height 28 --turns 200 --seed 34060000 --jobs 8
target/ci/fog_census --maps 64 --probes 12 --players 4 \
  --width 44 --height 28 --turns 200 --seed 34060000 --jobs 8 \
  --fog-honest-pressure
```

All 1,407 treatment-tensor, save/load-tensor, and null-controller checks
matched. Controlled hidden-fact divergences fell from 57/709 (8.0%) on the
frozen incumbent trajectory to 16/698 (2.3%) on the treated trajectory;
plan-report divergences fell 41→7 and first-action divergences 2→0. Different
policies reach different later states, so that aggregate change is a
trajectory-level diagnostic rather than a paired effect estimate. The unit
tests remain the causal proof for this pressure channel. The 16 residual
witnesses are explicitly retained as work for other omniscient inputs.

`AdvancedAi::new()` now enables the fog-honest pressure path. The exact former
production controller is `advanced_pre_fog_pressure`; historical
`advanced_envoy_composite` resolves to that frozen identity, while
`advanced_fog_pressure` is a canonical alias of production `advanced`.

Post-promotion verification exercised the production construction boundary.
`cargo test --profile ci --locked --no-fail-fast` passed all 1,304 library
tests (20 long censuses/benchmarks intentionally ignored) and every binary,
integration, and doc-test target. The release build of `civvis`, `ai_eval`,
and `fog_census` completed. Release evaluator probes independently refused
both promoted aliases as effective self-comparisons: `advanced_fog_pressure`
versus `advanced` and `advanced_envoy_composite` versus
`advanced_pre_fog_pressure` each exited 2.

A fresh deployment-scale production soak also completed 12/12 games:

```sh
target/release/civvis soak --games 12 --players 6 \
  --start-seed 36051000 --jobs 8 --width 74 --height 46 \
  --city-states 9 --speed online --turns 250 \
  --map continents --shape planet --poles poles
```

Every game reached a recorded result without a panic. The sample included
science, culture, religion, and score outcomes, 146 wars, and 40 city captures
(four of them capitals), so the health check exercised rather than avoided
the strategic pressure and combat paths changed in this loop.

## 2026-07-31 — PRE-REGISTRATION: qualified low-dimensional move override

The prior 105-term reliability head overfit 52 development games and wrote a
diagnostic v1 artifact before its gate failed. This successor repairs both
problems before reading a new result. A runtime artifact now includes its
qualification evidence and is loadable only when grouped development, blind
Standard selection, and untouched Online deployment all pass. A missing,
malformed, stale, failed, or merely diagnostic file executes the promoted
scripted `advanced` expert exactly; the strict evaluator refuses to call that
fallback `advanced_q_override`.

The frozen destination ranker remains the deterministic
`civvis-q-pairwise-v1` model trained on Standard seeds 946000–946063 with 80
epochs, batch 32, rate 0.05, L2 0.0001, four doctrine replicas, destination
features only, and raw threshold 0.70. Regeneration must reproduce its recorded
SHA-256 `2c93f4456b72d1acf548f1994c9ce49569fe158c7b8eb18f4c903b606ce1c463`.
It is frozen before any new reliability game is opened.

The reliability representation is fixed at 13 terms rather than 105: raw rank
margin, then sibling-minus-expert objective progress, hostile threat, hostile
coverage, adjacent friendly strength, adjacent friend count, attack margin,
ranged support, terrain defense, exits, frontier exposure, hostile progress,
and home progress. It excludes empire aggregates, duplicated absolute
destinations, and actor-role flags. Normalization remains training-fold-only;
the deterministic five-fold logistic fit remains 6,000 full-batch steps at
rate 0.05 with L2 0.02 and inverse decision-count game weights. The override
threshold remains 0.70.

New data are materially wider in independent games and fixed before collection:

- development: 192 four-player 44x28 Standard games, seeds
  1240000–1240191, no city-states, observations at turns 50 and 100;
- blind selection: 96 otherwise identical games, seeds 1240192–1240287,
  generated only after the out-of-fold development gate passes;
- untouched deployment: 96 six-player 74x46 Online games, seeds
  1241000–1241095, nine city-states, observations at turns 70 and 130,
  generated only after Standard selection passes;
- every observation uses three same-unit alternatives, four distinct doctrine
  rotations, an 80-round horizon, and zero tolerated rejected branches,
  repeat mismatches, or observation errors.

The exact collection commands are:

```sh
target/release/q_counterfactual --games 192 --players 4 \
  --width 44 --height 28 --turns 200 --city-states 0 --speed standard \
  --warmup 50 --spacing 50 --decisions-per-game 2 \
  --alternatives 3 --horizon 80 --replicas 4 \
  --seed 1240000 --jobs 8 --out /tmp/q-override-development.csv

target/release/q_counterfactual --games 96 --players 4 \
  --width 44 --height 28 --turns 200 --city-states 0 --speed standard \
  --warmup 50 --spacing 50 --decisions-per-game 2 \
  --alternatives 3 --horizon 80 --replicas 4 \
  --seed 1240192 --jobs 8 --out /tmp/q-override-selection.csv

target/release/q_counterfactual --games 96 --players 6 \
  --width 74 --height 46 --turns 250 --city-states 9 --speed online \
  --warmup 70 --spacing 60 --decisions-per-game 2 \
  --alternatives 3 --horizon 80 --replicas 4 \
  --seed 1241000 --jobs 8 --out /tmp/q-override-deployment.csv
```

Development and selection each require reliability Brier strictly below both
the raw margin and fold/full-development constant, positive game-macro return
lift over the expert, and at least 5% game-macro coverage. Deployment requires
the same conditions plus a positive 95% lower confidence bound on lift. Only
all three passes atomically write `civvis-q-override-qualified-v2`; otherwise
no loadable artifact is written and no gameplay A/B is authorized. A qualified
artifact earns a separate 300-map-per-profile mirrored gameplay matrix against
`advanced`; it does not itself authorize production promotion.
