# CIVVIS

### Notes from Martin:

Some videos up on [@civvis YouTube channel](https://www.youtube.com/@civvis)

A good bit of vibecoding. Still a bit buggy, apologies for any slop. Continues to be a work in progress.

Quick demo:

[![Spectate mode: a whole AI-vs-AI game on a Planet world of canal-ringed islands — six civilizations settle a globe of hexagons, the camera turns the planet and drops onto the Grand Canals II shelves and channels, Babylon puts the first satellite up on turn 189 and lands on the Moon and Mars, and three expeditions race for another star until Babylon's arrives on turn 282 for the science victory](docs/exhibition.gif)](docs/exhibition.mp4)

### The rest of this doc (and project) is written and maintained by AI:

-----------------------------------------------------------------------------

<!-- BEGIN CIV6 LEADER STRATEGY RANKING -->
## Which strategy suits which civilization, where the evidence says so

League round **60**, over the canonical 50-civilization Civ VI roster. A pair is listed
only when the leading strategy's conservative outright-win bound clears the optimistic
bound of every rival — the same lower-1.96σ Wilson bound the league itself selects
parents, retirement and live seating on. **0 of 50 pairs meet that bar.** The rest are
reported as coverage rather than ranked, because on this evidence they have no best
strategy.

Refresh after the live league changes:

`python3 tools/update_readme_rankings.py --league league/league.json`

Add `--check` to verify without writing.

No leader/civilization pair separates at this round. Nothing is ranked.

### All 50 pairs: what the league has, and what it would take

These are unresolved, not tied. Where two or more strategies have been rated on a pair,
the leading one is shown with its actual record so the gap between the evidence and the
claim stays visible — none of them separates from its own runner-up, so naming one would
report who has been seated, not what suits the civilization.

| Civilization | Leader | Candidates | Games | Leading strategy | Its record |
|---|---|---:|---:|---|---:|
| China | Qin Shi Huang | 8 | 325 | `g20-21` | 32/70 |
| Egypt | Cleopatra | 9 | 321 | `g28-28` | 16/43 |
| Greece | Pericles | 9 | 316 | `g48-44` | 8/17 |
| Rome | Trajan | 8 | 301 | `g20-21` | 24/46 |

The remaining **46 of 50** pairs have no strategy with 5 games in this snapshot, so
there is nothing to rank and nothing to contest: America (Abraham Lincoln), Arabia
(Saladin), Australia (John Curtin), Aztec (Montezuma), Babylon (Hammurabi), Brazil
(Pedro II), Byzantium (Basil II), Canada (Wilfrid Laurier), Cree (Poundmaker), England
(Victoria), Ethiopia (Menelik II), France (Catherine de Medici), Gaul (Ambiorix),
Georgia (Tamar), Germany (Frederick Barbarossa), Gran Colombia (Simón Bolívar), Hungary
(Matthias Corvinus), Inca (Pachacuti), India (Gandhi), Indonesia (Gitarja), Japan (Hojo
Tokimune), Khmer (Jayavarman VII), Kongo (Mvemba a Nzinga), Korea (Seondeok), Macedon
(Alexander), Mali (Mansa Musa), Maori (Kupe), Mapuche (Lautaro), Maya (Lady Six Sky),
Mongolia (Genghis Khan), Netherlands (Wilhelmina), Norway (Harald Hardrada), Nubia
(Amanitore), Ottomans (Suleiman), Persia (Cyrus), Phoenicia (Dido), Poland (Jadwiga),
Portugal (João III), Russia (Peter), Scotland (Robert the Bruce), Scythia (Tomyris),
Spain (Philip II), Sumeria (Gilgamesh), Sweden (Kristina), Vietnam (Ba Trieu), Zulu
(Shaka).

A strategy needs at least 5 games with that exact pair to qualify. The league's Glicko
rating is deliberately not shown: it orders matchmaking, not strength, and ranking on it
named a different strategy in 23 of 50 pairs while separating in none of them. See
`docs/EVAL_INTEGRITY.md` §5.
<!-- END CIV6 LEADER STRATEGY RANKING -->

**Why so little is ranked.** Until 2026-07-31 this table printed all 50 rows
ordered by “Elo”, which is the league's *placement* Glicko. That was the wrong
statistic and it was not a close call. The league's own selection contract
abandoned placement — it orders parents, retirement and live seating by
conservative outright-win bounds — because placement compresses who actually
wins. Over the 14 active strategies with 400+ games the two orderings agree at
Spearman ρ = 0.31, and on the live round-3205 league they name a **different
strategy in 23 of the 50 pairs**.

One row showed the whole failure. For Cleopatra the placement table printed
`deck-legacy`, which had won **8 of 43** games with that pair (18.6%); the win
bound prints `g28-28`, which had won **230 of 625** (37.0%). The shipped table
named a strategy that wins at half the rate, on a twentieth of the evidence,
and called it Egypt's best. The strategy it ranked first overall, `winbred-1`,
had won 15.8% of its 530 league games against stock `advanced`'s 21.5% of 2049
— significantly *worse* (p = 0.004).

So the table now ranks on the bound the league selects on, and prints a row
only where that bound actually separates the leader from every rival. On the
committed round-60 snapshot nothing separates, which is the honest answer for
960 games spread across 50 pairs. On the live round-3205 league exactly **one**
pair separates: Rome/Trajan → `advanced_v1`, 161/314, bound 0.458 — and that is
a *baseline*, not one of the bred genomes. Everything else is reported as
coverage, with the leading strategy's real record beside it, so the distance
between the evidence and the claim stays visible.

That also makes the table reproducible from the repository for the first time.
It is generated from the committed `data/league/` snapshot, so
`python3 tools/update_readme_rankings.py --check` passes on a fresh clone;
previously `/league/` was gitignored and the refresh command exited 2.

Design and rationale: `docs/EVAL_INTEGRITY.md` §5.

## How CivVis uses AI

Every civilization in the clip is controlled locally by Rust code. There is no
runtime LLM, prompt, model API, or generated move commentary — the crate's
entire runtime dependency list is `serde` and `serde_json`, so nothing in the
binary can reach a hosted model. The plan and reasoning panels expose
deterministic records emitted by the same controllers that choose the moves;
`src/reasoning.rs` keeps that journal write-only, so a seat that records
nothing plays exactly the same game as one that records everything.

The agents are a set of production controllers, baselines, and experiments—not
a six-rung strength ladder:

| controller | actual role | state in a normal checkout |
|---|---|---|
| `RandomAi` | uniform legal-action baseline for tests and tournaments | runs |
| `BasicAi` | cheap deterministic controller for city-states and barbarians; also a fallback and explicit entrant | runs |
| `AdvancedAi` | stateful scripted controller for major civilizations; the stock default and the parent of most league strategies | runs |
| `NeuralAi` | experimental `BasicAi` whose war decision can use rollout endpoints scored by a value net | **never runs** — resolves to `BasicAi` |
| `StrategicAi` | experimental `AdvancedAi` wrapper that rolls out victory-lane commitments; it can run without a value net | runs, on score share; never seated live |
| `PolicyAi` | experimental one-ply value-net action selector with `AdvancedAi` fallback | **never runs** — resolves to `AdvancedAi` |
| `ProductionSearchAi` | evaluator-only production rollout retained as a negative result | runs |

`Oracle<Ai>` is a diagnostic wrapper that grants a subsystem for free to
measure headroom. It is deliberately impossible to select as a rated player.
The 78 further names `ai_eval` accepts are controls and treatments built from
these seven controllers, not more deployed AI architectures.

### What actually runs

- Without a league, major civilizations use stock `AdvancedAi`; city-states
  and barbarians use `BasicAi`.
- The supervised exhibition defaults to `--league auto`. For each
  leader/civilization it makes a rank-weighted choice among the top three
  live-eligible strategies, avoiding repeats while the roster allows, from a
  mutable copy of the shipped roster. Those strategies are `AdvancedAi`
  variants plus the scripted baseline entries. The roster's `strategic` search
  agent is an offline anchor marked `league_only`, so it is not offered to
  exhibition or auto-play seats.
- Human auto-play hands that seat to a selected live roster strategy, or to one
  of `advanced`, `advanced_evolved`, `advanced_v1`, and `basic` when no roster
  is available.
- The repository ships and embeds `data/evolved/best.json`, the current
  40-gene `AdvancedAi` champion. **No `valuenet.json` is tracked anywhere in
  the tree**, at either path the loader searches. Consequently `neural`
  resolves to champion-weight `BasicAi`, `policy` to champion-weight
  `AdvancedAi`, and `strategic` keeps its score-share rollouts with no learned
  evaluator. `ai_eval` prints that provenance on every run and exits 3 under
  `--require-artifacts`.

The Civilization VI integrations are separate systems. The grounding mod can
export the economic subset of a league genome into the real game, but Firaxis'
AI still handles tactics and everything the export does not cover. The newer
computer-control mod is an independent Lua heuristic controller, not the Rust
`AdvancedAi`; no rung of its difficulty ladder has been won, and the committed
`docs/CIV6_LADDER.md` records zero attempts.

### Audit, 2026-07-31

Every controller above was re-checked against the current tree, and every
feature was re-measured on seeds none of the published results were found on.
The build is clean and `cargo test --release` is green: **1404 passed, 0
failed, 21 ignored across 49 suites**. Four things the audit changes.

**1. Three of the seven controllers cannot do the thing they are named for.**
`NeuralAi`, `PolicyAi`, and the learned half of `StrategicAi` all require
`evolved/valuenet.json`. `git ls-files | grep valuenet` returns the module and
the trainer and no artifact, at either tier the loader searches. Every run in
every checkout therefore reports:

```
neural:    plays as basic (missing valuenet.json)
policy:    plays as advanced (missing valuenet.json)
strategic: plays as strategic_score (missing valuenet.json)
```

The loader's path bug is fixed — resolution now falls back from `<dir>` to
`data/<dir>` — but fixing the path did not produce a net. Nets have been
trained and staged by hand for experiments; none has ever shipped. The
*deployed* impact of every learned component in CIVVIS is therefore exactly
zero, and the experiments that did stage one are the negative results below.

**2. The only searching controller is structurally barred from live play.**
Loading a league force-marks `strategic` as `anchor` and `league_only`
(`src/league.rs`), and live seating filters `!league_only` (`src/server.rs`).
No searching agent has ever played an exhibition or auto-play seat. The
round-3143 live league confirms it: of 19 active strategies, 12 are
parameterised `AdvancedAi` genomes and 7 are `Builtin:advanced`,
`Builtin:advanced_v1` or `Builtin:basic`. Nothing else.

**3. The self-comparison guard is inverted, and it changes what past runs
meant.** `builtin_provenance` drops the genome from a degraded agent's
effective identity: a net-less `policy` is reported as `advanced` and a
net-less `neural` as `basic`, when both actually carry the champion `Weights`.
Both directions of the guard are therefore wrong, and both are reproducible:

- `ai_eval neural basic` prints *“both play as basic; this run measures basic
  against itself and says nothing about either name”* — and then reports
  68.8%, +137 Elo-equivalent, p < 0.0001, **promotion gate PASS**. A run
  cannot be a self-comparison and clear a promotion gate.
- `ai_eval policy advanced_evolved` prints no warning at all, yet the two are
  the same agent: 20 of 20 mirrored maps neutral, and every diagnostic column
  identical to the digit.

So `policy` and `neural` are not policy and neural results. They are the
shipped genome measured on `AdvancedAi` and on `BasicAi`, which is a real
result — just not the one the name implies.

**4. The champion genome rides along with most experimental arms, and it is
most of what several published numbers measured.** `elo::builtin_ai` builds 38
of its 78 arms from `load_champion("evolved")` — the whole `strategic*`
family, `policy*`, `neural`, `production*` — while the usual control,
`advanced`, is stock `AdvancedAi::new()`. `X vs advanced` is a fair
*deployment* question for those arms (“should this replace stock?”) but it is
not a measurement of X. Re-running each against a genome-matched control
separates the two.

Everything below is 120 mirrored, seat-swapped map pairs on fresh seeds, at
four players on 24×16 Standard (seed 77,200,000) unless the row says
otherwise:

| what actually differs between the arms | comparison | paired | Elo-equivalent | gate |
|---|---|---:|---:|---|
| the whole scripted stack | `advanced` vs `basic` | 88.8% | **+359** (+262..+456) | PASS |
| the scripted planning upgrade (both stock) | `advanced` vs `advanced_v1` | 63.7% | **+98** (+34..+162) | PASS |
| …the same, at the six-player 74×46 Online deployment profile | seed 77,100,000 | 62.1% | **+86** (+22..+149) | PASS |
| the champion `Weights`, on `AdvancedAi` | `advanced_evolved` vs `advanced` | 58.8% | +61 (−1..+124) | INCONCLUSIVE |
| the champion `Weights`, on `BasicAi` | `neural` vs `basic` | 68.8% | **+137** (+70..+204) | PASS |
| macro rollout search **plus** the genome | `strategic` vs `advanced` | 62.9% | +92 (+28..+156) | PASS |
| macro rollout search **alone** | `strategic` vs `advanced_evolved` | 58.8% | +61 (−1..+124) | INCONCLUSIVE |
| production rollout search **plus** the genome | `production` vs `advanced` | 60.8% | +76 (+13..+140) | PASS |
| production rollout search **alone** | `production` vs `advanced_evolved` | 45.8% | **−29** (−91..+33) | INCONCLUSIVE |
| the learned tactical policy | `policy` vs `advanced_evolved` | 50.0% | the same agent | — |

Read the two pairs of rows together, because they are the point:

- **Macro search is worth about +61, not +92.** A third of the headline number
  was the genome, and once the genome is held fixed the search no longer
  clears the promotion gate — its direction is still significant (p=0.0003),
  its effect size is not.
- **Production search reverses sign, and the confounded version passes the
  promotion gate.** Against stock `advanced` it reads **+76, gate PASS**;
  against a genome-matched control it is **45.8%, −29**, which reproduces the
  repository's own recorded 45.0% almost exactly. `ProductionSearchAi` is a
  retained *negative* result, and the wrong control is enough to promote it.
  That is the concrete cost of the confound, not a hypothetical one.

Two other things are worth stating plainly. The 40-gene champion is worth more
than twice as much bolted onto the weak agent (+137) as onto the strong one
(+61) — what a genome largely overridden by hand-written logic looks like. And
the `advanced_evolved` row reproduces the repository's own 300-pair result
(58.3%, +58, seed 3,700,000) on a disjoint seed, which is the check that
matters most here.

**The flagship deployment number does not replicate at its recorded size.**
`advanced` over `advanced_v1` at the six-player Online benchmark is recorded
at 76.7%, +207, gate PASS. At the same profile on a disjoint seed it measured
62.1%, **+86** — same direction, same significance, same gate verdict, but
76.7% sits outside the 95% interval of 53.2–70.3%. The improvement is real;
the recorded effect is more than twice the replicated one, so quote it as the
result of one run rather than as the size of the gap.

**On tidiness.** The code itself is in good order: it builds clean, the tests
pass, and every controller module carries a doc comment that states its own
negative results rather than its intentions — `policy.rs` opens by explaining
why its own argmax loses. What has accreted is
*experimental surface*. `builtin_ai` now has 78 arms behind those seven
controllers and `src/bin` holds 44 research binaries; 41 of the 78 arms have no
entry in `docs/EVAL.md` and 13 appear nowhere in `docs/` at all. Most name
axes the repository has already closed. None of it is load-bearing and none of
it costs anything at runtime, but a reader counting names will badly
overestimate how many distinct agents exist.

### Where it works well

`AdvancedAi` is the useful game-playing core. It keeps persistent victory,
campaign, force-group, settlement, builder, city-role, and threat state, then
coordinates research, civics, policies, diplomacy, production, spending,
religion, trade, envoys, Congress, and unit orders around that state. It is not
purely greedy: tactical attacks use cloned positions and a bounded forcing-reply
extension before the agent commits.

The best-established scripted regression result compares `advanced` with its
frozen predecessor: on the recorded six-player 74×46, six-city-state Online
benchmark, `advanced` measured +207 Elo-equivalent against `advanced_v1` and
passed the promotion gate. This audit re-ran that on a disjoint seed and got
**+86** at the same profile and **+98** at the compact one — the improvement
replicates in direction, significance and gate verdict, at under half the
recorded size. Exact end-to-end tests also make the agent complete every
victory type without injected progress. Those tests establish rules and
planning coverage; they are not evidence of human-level play.

**That head-to-head edge does not show up in league play.** Over the 3,681
games the two have played in the round-3143 live league, `advanced` wins
outright 21.5% of the time (440/2049) and `advanced_v1` wins 22.5%
(368/1632) — a difference of −1.1 points against a standard error of 1.4, so
the league is well powered to see a several-point gap and does not see one.
The committed round-60 snapshot leans the same way (27.5% against 33.5%,
p = 0.09). League seating is not randomised between the two, so this is
observational rather than a controlled comparison, and a duel advantage need
not convert into outright wins in a rotating 4-to-10-way race where any rival
can take the game. But it is the largest body of evidence CIVVIS has at the
profiles it actually runs, and it says the flagship improvement is worth
roughly nothing in outright victories there. A mirrored duel and an N-way
free-for-all are different questions, and only the first has been answered.

The evaluation machinery is another genuine strength. Runs are seeded,
seat-mirrored, profile-labelled, provenance-checked, and gated on wins. That
machinery has caught silent artifact fallbacks, evaluator blindness, proxy
optimisation, underpowered comparisons, and map-profile overfitting before the
experimental agents reached live play. Its one known hole is the
self-comparison guard described in the audit above: it compares effective
*names*, and a degraded learned agent's name omits the genome it still
carries, so the guard fires on two agents that differ and stays silent on two
that do not.

Rollout search is useful research machinery, and on the small four-player
Standard benchmark it is the lever with the most evidence behind it — though
held against a genome-matched control it is worth +61 rather than +92 and does
not clear the promotion gate on its own. Whether buying *more* search than
already ships pays is a further question, and the familiar “`strategic_deep`
beat `strategic` by about +45 Elo-equivalent” cannot settle it, because that
comparison is not a budget comparison. Run it and `ai_eval` says so itself:

```
strategic_deep: plays as strategic_deep with untrained defaults (missing valuenet.json)
strategic:      plays as strategic_score (missing valuenet.json)
```

One arm blends an untrained default net at 25%; the other falls back to score
share. They differ in evaluator as well as in search budget. `search_dose
--only STOCK` is the instrument that answers the budget question, because it
constructs both `StrategicAi` values in process with the same genome and the
same net status.

### Where it is failing

- **Fair information:** `BasicAi` and `AdvancedAi` inspect the full `Game`, so
  they see through fog. The HTTP observation and `obs_tensor` are fog-honest,
  but the production controllers do not use them.
- **Learned control:** no trained model ships, and no Rust controller consumes
  the spatial PyTorch checkpoint produced by `train_spatial.py`. A historical
  25-feature state-value net could predict outcomes, but that does not make it
  a causal action evaluator.
- **Tactical value policy:** the 25-feature policy could not see 96% of its
  candidates. A 34-feature version made actions visible, then fell to **14.2%**
  against `advanced` (about **−313 Elo**) because its argmax maximized a
  correlate of winning—enemy contact—rather than the consequence of the move.
  Freezing the two contact features restored exact parity, confirming the
  failure mechanism rather than producing a useful policy. That figure carries
  the same genome confound as the rows above, but here it only flatters the
  result: `policy_wide` had the champion `Weights` and still lost by 313.
- **Production search:** `ProductionSearchAi` scored **45.0%** against the
  scripted governor and lost significantly by paired-map direction; this audit
  measured **45.8%** against a genome-matched control on a fresh seed.
  Extending its horizon from 40 to 200 barely changed its choices; the problem
  is the proxy objective, not insufficient lookahead.
- **Generalization:** the embedded evolved genome measured +58 on the small
  four-player Standard profile and **−9, inconclusive**, on the recorded
  six-player 74×46, six-city-state Online benchmark. `strategic` moved from
  +117 on the former to **−47, inconclusive on wins**, on the latter; the
  completed 300-map confirmation does not exist. A cheaper search variant
  reversed from +16 to a significant **−63**. Search and evolution results
  must therefore travel with their exact profile — and, per the audit above,
  with their control: every `strategic` figure here is measured against stock
  `advanced` and so carries the champion genome inside it.
- **Optimization surface:** `Weights` has 40 active genes; policy appetites and
  policy-deck/dedication choices stored in the same struct are not genes. Several
  genes are bypassed on common `AdvancedAi` paths, and inexpensive score-share
  improvements have repeatedly failed to produce more wins.
- **External strength:** internal Elo/Glicko ratings compare CIVVIS strategies
  only. There is no completed human or Firaxis-AI calibration, so “best” means
  best inside a named CIVVIS test or league population.

The live exhibition now rotates through 4–10 major civilizations and stock map
sizes, so it has no single deployment profile and there is no defensible
profile-independent “strongest agent.” See [AI_GUIDE](docs/AI_GUIDE.md) for the
implementation map, [AI_GAPS](docs/AI_GAPS.md) for the current assessment,
[EVAL](docs/EVAL.md) for the chronological evidence, and
[LEAGUE](docs/LEAGUE.md) for seating and ratings. [GROUNDING](docs/GROUNDING.md)
and [CIV6_COMPUTER_CONTROL](docs/CIV6_COMPUTER_CONTROL.md) document the two
real-game bridges.
