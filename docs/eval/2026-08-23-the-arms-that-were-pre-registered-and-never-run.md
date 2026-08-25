# The arms that were pre-registered and never run

_2026-08-23 · `1510b482`_

## What was asked

`docs/ROADMAP.md` objective 3 asks the shipped bundle to be priced by
withholding *before the next effect hides inside a composite the way
`city_target_floor` did*. Removing that floor measured **+41 Elo** on the
deployment profile (#1504 — `advanced_without_city_target_floor advanced
--matrix --pairs 400 --seed 8600000`, 55.9%, CI 51.0..60.7%, p=0.0000, against
−1 and p=0.9248 on the compact board), which is the same statement as the floor
costing that much for as long as it shipped unpriced inside the 2026-08-01
composite.

⚠ **That figure is gate-selected and is quoted here as the top of a band, not
as a point.** The run passed its gate, so `docs/EVAL_INTEGRITY.md` §4 applies to
it exactly as to any promoted number. `docs/EVAL.md`'s own entry gives the
replication: *"Five runs now agree: +30, +29, +34, +41 on deployment shapes,
flat on the compact board."* The honest reading of what a composite hid is
roughly +29 to +41, and the brief's "−41" is the largest of the five.

Several arms were registered, pre-priced, and never run. This round runs the
load-bearing one and records the disposition of the rest, so the open ends stop
being invisible.

Four things were asked for. One was run, one **cannot** be run, one turns out
to be **already answered by a different instrument** — and the counter that was
tracking it now reads zero for a reason that is not the reason it looks like —
and one is pre-registered here with its control problem named.

---

# 1. `advanced_governor_victory_lanes`, seed 29000000 — the open end

`docs/eval/2026-08-18-pricing-the-governor-s-routing-and-the-settling-asymmetry.md`
closed with this arm named as "the open end". `advanced_every_lane` measured
−95 at the deployment profile, the Expansion half measured −20 compact / −16
deployment and was RETAINED, so **by subtraction the four victory lanes should
carry roughly −70 to −80**.

Two later lines, from instruments that share nothing with that subtraction,
point the same way — which is what makes the third leg worth buying:

| line | reading |
|---|---|
| `docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md` | `governor-victory-lanes` **−4.73 pp, win z −15.37** — worst of 99 genes, while the gene ships default ON |
| `docs/gene_ledger.json` | `verdict: unresolved`, **`conflict: true`**, `helps * · share HURTS **`: win z **+2.46** against share z **−15.918** |
| the sibling half | `governor-every-lane` reads share z −16.933 and is correctly OFF |

## ⚠ The question was resolved while this arm was in flight, and that changes what it is

This round launched at 06:39 UTC with `governor-victory-lanes` shipping default
ON and `verdict: unresolved`. At merge time it does not. **#2344 turned the gene
off**: a pre-registered single-gene *direct* arm on 3,600 matched seat
comparisons, seeds 150000000–150000599 and disjoint from the whole-genome
screen, read **−4.778 pp at win z −6.112** with score share −2.732 pp at z
−23.757. The ledger now carries `verdict: hurts`, `default_on: false`,
`family_wise: true`, and the genome went 31 → 30
(`docs/eval/2026-08-23-governor-victory-lanes-direct-confirmation.md`).

That round names this arm as the reason it was run: *"a decomposition predicted
it, and its confirming arm was never run… It named
`advanced_governor_victory_lanes` (seed 29000000) as 'the open end'. That arm
had not run."*

**So this is no longer the open question — it is the fourth instrument on a
closed one, and it is the only one of the four that is an Elo measurement.**
The three that resolved it all count seat wins with the gene toggled inside a
genome (p10, the standard screen, and #2344's direct arm). This one plays 100
mirrored maps of `advanced` + the lane routing against plain `advanced` and
scores the pairing. Agreement across that gap is worth more than a fourth
seat-count would have been; disagreement would localise to the baseline, since
this arm's control is bare `advanced` and theirs is the deployment genome.

Its verdict is reported below against the 2026-08-18 round's own conventions —
compact and deployment point estimates with intervals — and explicitly against
the −70 to −80 that round predicted, which no seat-share reading can address
because it is an Elo claim.

## ⚠ The two screens disagree on the *sign*, and the difference is the map

This was not visible from either screen alone and it decides how to read
everything below. The two most recent whole-genome screens price this gene at
opposite signs, both far outside noise:

| screen | shape | win Δpp | win z |
|---|---|---:|---:|
| `2026-08-22-p10-native-6p-allseats-17574-pairs` | 60×38 **pangaea**, 6 city-states, online 250 | **+0.91** | **+2.46** |
| `2026-08-22-standard-gene-screen-23622-paired-seats` | 74×46 **continents**, 9 city-states, online 250 | **−4.73** | **−15.37** |

Same six players, same speed, same turn limit, same all-seats foldover, same
victory set. The map and the city-state count are the whole difference, and
they move this gene by **5.6 percentage points of win rate and eleven sigma**.
The ledger's `conflict: true` records the win-vs-share disagreement *inside*
p10; this is a second, larger conflict *between* screens that the ledger does
not carry, because p10 is its source and the standard screen is result-only.

It matters here because `--matrix`'s `deployment-online` profile is 6p, 74×46
continents, 9 city-states, online 250, all six victories — **the standard
screen's shape exactly**. So this round's deployment column is an independent
instrument on the same world, and its compact column is a third world again.

## PRE-REGISTRATION (written and committed before the run: `4067247f`)

**Arm.** `advanced_governor_victory_lanes` vs `advanced`.

⚠ **It is an *enable* arm, not a withhold arm**, and the round that registered
it is explicit about why. `AdvancedAi::new()` has `governor_victory_lanes:
false`; the arm calls `enable_governor_victory_lanes()`. Its sibling
`advanced_governor_expansion_lane` has the same shape, and that is what makes
the subtraction legal: all three readings (`advanced_every_lane` −95, the
Expansion half −16, this arm) are the same *kind* of delta measured against the
same `advanced` control. A withhold arm would not subtract against them.

**Design.** `ai_eval advanced_governor_victory_lanes advanced --matrix --pairs
100 --jobs 4 --seed 29000000`. Fixed N, no `--stop-when-decisive`: the betting
interval is anytime-valid, but an early-stopped point estimate is selected on
having crossed, and `docs/EVAL_INTEGRITY.md` R3 is precisely the rule against
quoting a decision procedure as an estimator.

**Seed streams.** `--matrix` strides its profiles by `MATRIX_PROFILE_SEED_STRIDE`
= 1,000,000, so the registered seed 29000000 resolves to compact
[29000000, 29000099], deployment-online [30000000, 30000099],
deployment-contested [31000000, 31000099].

**Pairs: 100 intended, against the 400 the sibling arms ran — a declared
deviation, not a stopping point discovered later.** Measured cost on this
machine at 2026-08-23 00:54–01:24, load average 72, twelve sibling agents
resident: a 3-profile matrix at 8 pairs took **23m21s wall and 2200 CPU-s**
(≈46 CPU-s/game) and returned only **157% CPU against 4 requested jobs**. That
is 2.92 wall-minutes per matrix pair, so the registered 400 pairs is ~19.4
hours of wall clock here and would not land. 100 pairs is ~4.9 hours and does.

**What 100 pairs can and cannot see.** The sibling arm resolved −20 compact at
p=0.0089 on 400 pairs, which implies SE ≈ 7.6 Elo there and therefore
SE ≈ 15 Elo at 100. Against a true −75 that is z ≈ −5, so this round can
comfortably separate "the victory lanes are the carrier" from "near zero" and
from the Expansion half's −16. It **cannot** put a tight interval around
−70 vs −80. The run reports its own smallest-resolvable-edge line and that
line, not this paragraph, is the record.

**And the target it is being measured against is itself wide.** The −70 to −80
prediction is a point-estimate subtraction that carries neither parent's
interval. `docs/PRODUCTION_VALUE.md` gives the composite as **−95 Elo, CI
−131..−60** (seed 27000000, deployment-online); the Expansion half's intervals
were not preserved (the 2026-08-18 round says so). Propagating just the
composite's own interval puts the victory lanes somewhere near −115..−44
before this round adds any error of its own. A ±30 reading is therefore about
as sharp as the question it is answering, which is the honest defence of N=100
— not that 100 pairs is good, but that 400 would have been spent narrowing one
term of a difference whose other term is unknown.

**Third profile declared.** The 2026-08-18 round ran two profiles because
`deployment-contested` was added to `PROMOTION_PROFILES` the same day
(`ba5515d0`, #2042). Running `--matrix` today therefore produces a profile the
sibling arms never had. The verdict is called on **compact and
deployment-online**, the round's own two columns; the contested profile is
reported beside them and is not comparable to either.

## Results

Ran exactly as registered: 100 fixed pairs per profile, no early stopping, on
the registered seed and its recorded strides. `arms differ on:
governor-victory-lanes` on all three.

| profile | seed prefix | paired score | **Elo** | 95% betting CI | sign test | gate |
|---|---|---:|---:|---|---|---|
| compact-standard | 29000000..=29000099 | 45.8% | **−30** | −68..+1 | p=0.0161 **for `advanced`** | INCONCLUSIVE → ACCEPT |
| deployment-online | 30000000..=30000099 | 43.5% | **−45** | −104..+21 | p=0.0470 **for `advanced`** | INCONCLUSIVE → **REJECT** |
| deployment-contested | 31000000..=31000099 | 59.8% | **+69** | +31..+114 | p=0.0000 for the treatment | **PASS** → ACCEPT |

`multi-profile promotion gate: RETAIN advanced — cleared 2/3 required
profiles.` Both fieldless Elo figures are reported by the run as *"not
gate-selected — the gate did not fire, so this estimate is not conditioned on
being large"*; the +69 is reported as a **DISCOVERY ESTIMATE**, selected on
passing and explicitly *"not quotable until confirmed on a disjoint seed"*.

### ⚠ First, the correction this round owes its own pre-registration

The registration estimated that 100 pairs would resolve ±30 Elo, extrapolated
from the sibling arm's 400-pair p-value. **That was about 2.5× too optimistic.**
Each run states its own power, and they say +83 (compact), +76 (deployment) and
+85 (contested) at 80%. So the two INCONCLUSIVE verdicts are a statement about
*this run's length*, not about the treatment: an effect of −45 was never going
to fire a gate that needs about +76. The point estimates and their intervals
carry the information here, and the gate verdicts carry almost none.

### The census reproduces the composite's own fingerprint

At the deployment profile the treated seat is smaller on **every** development
column, which is what #1955 measured for the whole `advanced_every_lane`
composite (*"the empire is uniformly ~30% smaller"*) — over 100 maps, 200 games:

| deployment-online | seat-win% | score | cities | pop | districts | buildings | gold |
|---|---:|---:|---:|---:|---:|---:|---:|
| `advanced_governor_victory_lanes` | 14.5% | 481.5 | 5.66 | 54.4 | 17.8 | 58.2 | 146.3 |
| `advanced` | 18.8% | 611.2 | 6.22 | 67.0 | 21.4 | 74.6 | 218.4 |

Terminal score is the sharper reading, because every map breaks on it: **10
favored vs 90 against, p=0.0000** at deployment and 29 vs 71, p=0.0000 on
compact. The victory-lane governor makes a materially poorer empire.

Its win mix says how: the treated seat collapses into a religion monoculture
and loses the score lane. Deployment victories, treated
`{religious 46, score 31, culture 10}` against `advanced`
`{score 67, culture 22, religious 13, science 9, diplomatic 2}`.

### And the seat-win delta agrees with the screens to within half a point

The three instruments that resolved this gene count seat wins with the gene
toggled inside a genome. This arm is an Elo pairing against bare `advanced`, a
different baseline entirely — yet the same column lands in the same place:

| instrument | basis | seat-win Δ |
|---|---|---:|
| standard whole-genome screen (#2323) | 23,622 matched seat comparisons | −4.73 pp |
| direct single-gene arm (#2344) | 3,600 pairs, seeds 150000000–150000599 | −4.78 pp |
| **this arm, deployment-online** | 100 maps / 200 games, seed 30000000 | **−4.3 pp** |

Four instruments, three baselines, four disjoint seed windows, one answer.

### ⚠ The third profile flips the sign, and the census says why

On `deployment-contested` — the same 74×46 board, but four of six chairs seated
with `live_target_diplomatic` and `live_target_culture` instead of the entrants
— the treatment wins **36.5% of seats against 17.0%**, and its census is nearly
identical to the control's (score 561.5 vs 567.6, cities 6.39 vs 6.55,
districts 17.6 vs 18.6). The smaller empire is gone; what remains is
conversion, `{religious 66, score 7}` against `{religious 18, score 12,
culture 3, diplomatic 1}`.

So the same commitment that starves the empire in fieldless self-play is what
beats a field chasing diplomacy and culture. This is the profile whose own
definition in `src/bin/ai_eval.rs` is headed *"★★★★★ THIS IS WHY THE MATRIX WAS
BLIND TO TWO THIRDS OF WHAT KILLS US"* — and every instrument that resolved
this gene (p10, the standard screen, #2344's direct arm) is fieldless.

**This is not a reason to reverse #2344 and this round does not ask for one.**
It is a DISCOVERY ESTIMATE by the tool's own label, on a `NoRegression` profile
whose comment warns its numbers *"are not comparable to `deployment-online`'s"*
because two entrants hold two chairs instead of six.

### PRE-REGISTRATION: the confirmation the run itself prescribes

Written and committed before the run. A discovery estimate that is left
unconfirmed is precisely the failure `docs/EVAL_INTEGRITY.md` §4 records — *"the
+45 lived in `docs/GENOME.md` as a bare promoted figure; the refutation reached
a PR body and never reached the document"* — so rather than hand this one on as
a striking observation, this round buys the replication. The machine was at
load average 5 on 18 cores with one job left running.

**Design.** Exactly the `deployment-contested` profile arguments, fixed N = 100,
no early stopping, on a disjoint prefix declared to the tool:

```
ai_eval advanced_governor_victory_lanes advanced --pairs 100 --jobs 4 \
  --seed 39000000 --confirm 31000000 \
  --players 6 --width 74 --height 46 --city-states 9 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,religious,diplomatic,domination,score \
  --difficulty prince --deployment-comparison \
  --field live_target_diplomatic,live_target_culture
```

`--confirm 31000000` makes the disjointness mechanical rather than a
convention: `ai_eval` compares the full inclusive prefixes and refuses any
overlap. **Registered prediction: the direction replicates and the size falls.**
That is what this ledger's winner's-curse record says happens (+207 → +86,
+92 → +61, +45 → −8), and stating it before the run is the only way the round
can be wrong about it.

#### Result: it replicated, and the registered prediction was right on both counts

100 maps, 200 games, seeds **39000000..=39000099**, declared to the tool as
`--confirm 31000000` so the disjointness was checked rather than asserted.

| | discovery (seed 31000000) | **confirmation (seed 39000000)** |
|---|---:|---:|
| game-win share | 73/200 (36.5%) vs 34/200 | **75/200 (37.5%) vs 41/200** |
| paired-map score | 59.8% | **58.5%** (95% betting CI 54.0%..65.6%) |
| Elo-equivalent | +69 | **+60 (CI +28..+114→+112)** |
| sign test | p=0.0000 | **p=0.0021** |
| gate | PASS | **PASS** |
| terminal score | 47·0·53, p=0.6173 | 52·0·48, p=0.7644 |

The run's own label: **`effect size: +60 (CONFIRMED — measured on seed
39000000, disjoint from the discovery seed 31000000; quotable, and quote this
estimate rather than the discovery one)`.** So +60 is the number, not +69.

**The registered prediction held exactly: the direction replicated and the size
fell**, +69 → +60, which is this ledger's own pattern (+207 → +86, +92 → +61).

The mechanism replicated too, which is what makes it a finding rather than two
lucky seed windows. Both arms field near-identical empires — confirmation score
559.9 vs 575.1, cities 6.36 vs 6.38, districts 17.2 vs 18.6 — and the win comes
entirely from conversion: `{religious 65, score 9, science 1}` against
`{religious 32, score 7, culture 2}`. Terminal score is null in both windows.
The victory-lane governor is not building a better empire on this board; it is
converting one, against a field that is chasing diplomacy and culture.

⚠ **What this is and is not.** It is a confirmed, quotable +60 on
`deployment-contested`. It is **not** comparable to the −45 on
`deployment-online`: that profile's own definition warns its numbers are not,
because two entrants hold two chairs here instead of six. And the field is four
`live_target_*` agents, which is a *model* of the lanes Firaxis' AI pursues, not
Firaxis' AI. This does not overturn #2344 and this round does not ask for it to.

What it does establish is narrower and still worth having: **every instrument
that resolved this gene off — p10, the standard screen, and #2344's direct arm —
is fieldless, and on the one board in this repository built to model who the
live seat actually plays, the same gene is worth +60 Elo across two disjoint
seed windows.** `src/bin/ai_eval.rs` heads that profile *"★★★★★ THIS IS WHY THE
MATRIX WAS BLIND TO TWO THIRDS OF WHAT KILLS US"*. Recommended follow-up, for
somebody who owns the ledger: a contested-field gene screen before this gene is
treated as settled.

## Verdict: RETAIN `advanced` — and the −70 to −80 prediction is **half right**

**Direction: CONFIRMED, on both of the round's own profiles.** The four victory
lanes are the carrier, exactly as the 2026-08-18 subtraction claimed. Negative
on compact and on deployment, sign-significant on both (p=0.0161, p=0.0470),
and worse on terminal score at p=0.0000 on both, with the composite's own
census fingerprint.

**Magnitude: NOT reproduced at the predicted size.** Doing the subtraction the
2026-08-18 round actually did — composite minus Expansion half — predicts
compact −42 and deployment −79:

| | composite | Expansion half | predicted victory half | **measured here** |
|---|---:|---:|---:|---:|
| compact | −62 | −20 | −42 | **−30** (CI −68..+1) |
| deployment | −95 | −16 | −79 | **−45** (CI −104..+21) |

The prediction is not excluded — −79 sits inside the deployment interval — but
the point estimate is 34 Elo short of it, and the compact estimate is short
too. **The honest verdict is that the decomposition got the direction and the
carrier right and the size wrong, in the direction this ledger always fails:
downward.**

**And the halves do not add up.** −45 and −16 sum to −61 against a composite
measured at −95; on compact, −30 and −20 sum to −50 against −62. Both gaps
point the same way — the composite is worse than its parts — which is what
*"a composite gate licenses the composite and never its parts"* predicts and
this is the first time this decomposition has had a number for it. ⚠ With
intervals this wide the interaction is suggestive and not established: −95's
own interval is −131..−60, so −61 is not far outside it.

No default changes here. The gene was already turned off by #2344 before this
merged, and this round's fieldless profiles agree with that decision from a
fourth direction.

**But the round does not close the gene.** Its third profile, confirmed on a
disjoint seed and quotable, reads **+60 Elo (CI +28..+112, 100 pairs, seed
39000000, p=0.0021)** on the one board built to model the lanes the live seat
actually loses to. A gene that is −45 fieldless and +60 contested is not
"resolved"; it is **field-dependent**, and every instrument that resolved it
was fieldless.

---

# 2. `advanced_wonder_reach`, seed 32000000 — the arm no longer exists

**Disposition: UNRUNNABLE. The gate cannot ever speak, and the round that
registered it still promises that it will.**

`docs/eval/2026-08-18-the-wonder-a-city-cannot-start-pays-its-prerequisites.md`
closes: *"The strength question is pre-registered and untouched: `ai_eval
advanced_wonder_reach advanced --matrix --pairs 400 --seed 32000000`, one run,
decided by the matrix gate and nothing else. The flag stays off in production
until that gate speaks."*

It cannot. On 2026-08-21, #2266 (`77332750`, *"Ten genes leave the code"*)
removed `wonder-prereq-reach` **with its arm**. `docs/GENE_SCREEN.md` records
it: the cull took the ten genes' `live_without_*` arms "and the
`advanced_holy_lane`, `advanced_holy_lane_v0` and `advanced_wonder_reach` arms
that set their fields". Verified against the tree at `1510b482`:

- `advanced_wonder_reach` is absent from `EVAL_ONLY_AIS` — `ai_eval` rejects
  the name outright;
- `wonder_prereq_reach`, `wonder_reach_credit` and `wonder_missing_prerequisites`
  are absent from `src/` entirely.

There is nothing left to seat, so seed 32000000 stays unspent.

## ⚠ And the number it was culled on has since been replaced by one of the opposite sign

This is the `holy-lane-parity` failure mode again (#2299,
`docs/gene_ranking_notes.md`): **a screen already in flight re-prices a gene
after its code is gone.** The cull ordered `wonder-prereq-reach` out at
**−26** wins/10k. The p10 screen, whose binary predates the cull, priced it
afterwards at **+29** — and `GENE_HEURISTIC_RANKING.md`'s *Removed from the
code* table now publishes that +29 as the gene's last tracked measurement.
`suzerain-cards` from the same cull moved further, to **+42**.

**This does not say the cull was wrong, and the round should not be read that
way.** p10 puts `wonder-prereq-reach` at +0.580 pp with `win_se_pp` 0.370, so
**z = +1.57** against a family-wise bar of 3.40 — inside noise, exactly as the
−26 was. #2266's own body called these eight "directive removals, not measured
harms". The honest statement is narrower and worse: *the gene was never
resolved either way, the pre-registered instrument that would have resolved it
has been deleted along with the code, and the only reading that survives it
points the other direction.* Re-opening the question now costs a code
restoration, not a batch.

**Recommended:** the 2026-08-18 round's closing promise is stale and should
stop reading as an open commitment. This round is the record; see the note
added to that file.

---

# 3. The ten never-named live treatments — and the arm inventory behind them

**Disposition: the debt list is EMPTY, that is a measurement artifact, and
underneath it 26 of the 49 arms objective 3 tells the fleet to run are
byte-identical to their own control.**

## The counter reached zero without a single withholding run

`docs/EVAL_STATUS.md` at `1510b482` reads *Withholdable live treatments 49 ·
Named somewhere in the evidence 49 · **Never named in any round: 0***, and
`tools/eval_manifest.py --check` passes, so it is current rather than stale.
But the count went **10 → 0 in a single commit** — `1510b482` itself (#2323,
*"Publish 10,000-seat gene-screen results"*) — and the mechanism was not ten
rounds. All ten became "named" as one row each in the table in
`docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md`.

The generated block warns about exactly this: *"whether a treatment was
**priced** is a judgement… whether it has ever been **named** is mechanical…
act on the last one, which cannot be flattered."* The last number has now been
flattered, because one whole-genome screen names every gene at once. **After
2026-08-22, `never_named` cannot measure objective-3 debt at all**: any future
whole-genome round zeroes it on publication. It is a solved metric, not a
solved problem.

To be fair to what did land: the screen row is real evidence and a far larger
instrument than the 400-pair arm objective 3 asks for — 23,622 matched seat
comparisons each. All ten read null on it (largest |z| = 2.17, against the
screen's own family-wise bar), so a 100-pair withholding arm, which resolves
roughly ±30 Elo, could not add to those rows. This round declines to spend the
budget re-asking two hundred times more coarsely.

## ⚠ The finding underneath: over half the objective-3 arm inventory cannot fire

Chasing "which of the ten is cheapest to price" turned up something worse than
the counter. **`live_without_X` is not a withhold from the universe bundle — it
is a withhold from the deployment genome, and for a treatment the ledger
already holds off it withholds nothing at all.**

`src/elo.rs`'s derived factory builds every one of these arms as:

```rust
let mut ai = AdvancedAi::new();
ai.enable_live_bridge();   // = enable_live_bridge_universe() + apply_gene_ledger()
disable(&mut ai);          // withhold the one treatment
```

and `apply_gene_ledger` (`src/ai/advanced/gene_ledger.rs`) already runs, for
every tag with `ledger_default_on(tag) == Some(false)`, **the same `disable`
function**. Every `disable_*` is a plain `self.field = false`, so the second
call is a no-op and the arm is the control. Counted over the published 49:

| | count |
|---|---:|
| arm is a real withhold — ledger `default_on: true` | 20 |
| arm is a real withhold — tag absent from the ledger, so untouched and on | 3 |
| **arm is byte-identical to `live` — ledger `default_on: false`** | **26** |

The 26: `amenity-project-preemption`, `army-target-weighs-enemy`,
`barbarian-bargain`, `barbarian-hunt`, `barbarian-ranged-answer`,
`blind-objective-units`, `buildings-before-projects`, `civilian-rescue`,
`district-coverage`, `endgame-war-runway`, `garrison-under-fire`,
`governor-every-lane`, `home-defense`, `housing-districts`, `naval-recon`,
`recorded-tactical-step`, `score-horizon`, `settler-guard-holds`,
`siege-commitment`, `siege-is-progress`, `siege-tracks-wall`,
`slot-kind-tiebreak`, `war-economy`, `war-patience`, `war-reinforcement`,
`wonder-ring-settle-value`.

**Five of the original ten are in that list** (`amenity-project-preemption`,
`blind-objective-units`, `endgame-war-runway`, `siege-commitment`,
`wonder-ring-settle-value`), so half the debt roadmap was pointing at arms that
cannot measure. This is #2095's title — *"the never-named list names treatments
you cannot run"* — one level deeper: that PR fixed arms whose **names** were
wrong, and these arms have correct names, exist, and run.

### Confirmed on the board, with a positive control

Byte-identity is a proof from the source, so the run is a confirmation rather
than the evidence. 20 paired maps, seed 778001, 4p 24×16, 4 city-states, 120
turns, standard speed, identical seeds across all three:

| arm | ledger | maps broken on wins | maps broken on terminal score |
|---|---|---:|---:|
| `live_without_recorded_tactical_step` | OFF → predicted inert | **0 / 20** | **0 / 20** |
| `live_without_war_economy` | OFF → predicted inert | **0 / 20** | **0 / 20** |
| `live_without_whole_turn_backtrack_guard` | **ON** → predicted live | 1 / 20 | **11 / 20** |

Both predicted-inert arms returned `ai_eval`'s own *"nothing differed: all 20
maps were neutral on wins AND on terminal score… it did not fire on this
profile"*. `recorded-tactical-step` is the sharp case: it records every
tactical step, so if it were live it would perturb nearly every game — and the
positive control, which does perturb them, breaks 11 of the same 20 maps at
the same shape and seeds. (An earlier 6-pair attempt is not quoted: its
positive control did not fire either, so its null was not evidence.)

### And the one line a reader would check says the opposite

`ai_eval` prints `arms differ on: war-economy` for a pair that is one agent.
The arm **spec** carries `LIVE_BRIDGE_TREATMENTS` minus the tag, while the
constructed agent carries the ledger's genome minus nothing, so the sanity line
that exists to stop a comparison of two identical agents asserts the axis is
there. Nothing in CI relates the two.

**Recommended, not done here** (it is `src/elo.rs`, a declared hotspot, and a
material expansion of this task): a test that constructs each
`live_without_X` and its `live` control and fails when their flag state is
equal — the guard that runs in the change that adds it. `MINOR_DEPENDENT_ARMS`
already encodes this exact lesson for `advanced_price_suzerainty`
(*"with no minor seated the arm is byte-identical to its control"*); the ledger
produces the same failure 26 times and nothing lists it.

**What remains, stated so it is not lost:** 23 of the 49 can genuinely be
priced by withholding, and none of the 23 has been. The other 26 need the
ledger default flipped before any withholding arm means anything — which is a
gene-screen decision, not an evaluator one. The counter that tracked this reads
0.

## PRE-REGISTRATION: the first of the 23, run here

Written and committed before the run. Knowing which 23 arms are real made one
of them affordable, and at 07:56 UTC the machine's load average was **10** on
18 cores while this round's critical path was pinned to a single job — so the
headroom bought the first withholding price objective 3 has ever had.

**Arm.** `live_without_settler_site_agreement` vs `live`. `live` is
`AdvancedAi::new()` + `enable_live_bridge()` — the deployment genome itself —
and the treatment is `default_on: true` in the ledger, so this is a genuine
withhold and not one of the 26.

**Why this one.** Of the ten treatments §3 opened with, five are real
withholds; of those five `settler-site-agreement` carries the largest standard-
screen magnitude (−0.46 pp, z −1.47) and sits on the axis this repository
repeatedly measures as its binding constraint (`docs/AI_GAPS.md`, expansion;
opening tempo correlates r=+0.69 with the outcome). Cheapest-and-most-likely-
to-matter, as asked.

**Design.** The `deployment-online` profile, its exact recorded arguments,
fixed N = 100, no early stopping, seed **37000000** (unused anywhere in the
tree):

```
ai_eval live_without_settler_site_agreement live --pairs 100 --jobs 3 --seed 37000000 \
  --players 6 --width 74 --height 46 --city-states 9 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,religious,diplomatic,domination,score \
  --difficulty prince --deployment-comparison
```

`--city-states 9` is explicit; `ai_eval` seats none in direct mode without it,
and a settlement treatment screened on an empty board is a null by
construction.

**What it can see.** The same ±30 Elo as §1. The standard screen already bounds
this gene to |Δwin| < 0.5 pp *as a gene inside the genome*; this asks the
different question — what the **shipped bundle** pays for it — and a null here
is a real answer to a question nobody has asked, not a repeat of the screen.
`ai_eval`'s "nothing differed" line is the check that the arm fired at all.

## PRE-REGISTRATION: a second real withhold, from the other half of the bundle

Also written and committed before its run. At 09:00 UTC the machine's load
average was **4.5** on 18 cores with this round's critical path still pinned to
one job, so a second arm cost nothing on the wall clock.

**Arm.** `live_without_blind_objective_strength` vs `live`. Ledger
`default_on: true`, so it is a real withhold. Chosen as the war-side
counterpart to `settler-site-agreement`'s expansion side — between them the two
arms sample both halves of the bundle rather than two draws from one — and it
is the highest-ranked of the five real withholds among §3's original ten
(6th-decile of `GENE_HEURISTIC_RANKING.md` at +30 wins/10k seats). It also
carries the sharpest disagreement to test: the ranking's +30 against the
standard screen's −0.01 pp at z −0.03.

**Design.** Identical to the arm above in every respect but the treatment and
the seed: `deployment-online` arguments, fixed N = 100, no early stopping,
`--city-states 9`, seed **38000000** (unused anywhere in the tree).

### Results — `settler-site-agreement`

Ran as registered: 100 maps, 200 games, seeds 37000000..=37000099 inclusive,
average 235.5 turns, `arms differ on: settler-site-agreement`.

| reading | `live_without_settler_site_agreement` vs `live` |
|---|---|
| game-win share | 99/200 (49.5%) vs 101/200 (50.5%) |
| paired-map score | 49.5%, 95% betting CI 42.0%..56.1% |
| **Elo-equivalent** | **−3 (CI −56..+43)** |
| paired direction | 10 · 79 · 11, sign p=1.0000 |
| terminal-score direction | 49 · 0 · 51, sign p=0.9204 |
| promotion gate | INCONCLUSIVE; resolves about +56 at 80% power |
| maps that broke | wins 21/100, terminal score 100/100 |

**The arm fired.** 21 of 100 maps broke on wins and all 100 on terminal score,
and `ai_eval` printed no "nothing differed" warning — so this is a measurement
of the treatment and not one of the 26 inert pairings §3 opened with. That
matters more than the number: it is the check that separates a null from a
no-op, and the check §3 exists because 26 arms cannot pass.

**Verdict: RETAIN. A clean, well-controlled null.** Withholding
`settler-site-agreement` from the deployment genome moves nothing measurable at
the deployment shape, on either wins or terminal score, and the two readings
agree. The run bounds the treatment's contribution to roughly ±56 Elo — which
is the honest limit of one 100-pair round and is stated rather than implied.

⚠ **What this does and does not settle.** It is the first time any of the 49
withholdable treatments has been priced by withholding it from the bundle that
actually ships, which is what `docs/ROADMAP.md` objective 3 asks for. It is one
treatment of 23 runnable ones, on one seed window. It does not license the
bundle, and a null at ±56 Elo is not evidence that the treatment is worthless —
only that it is not large.

### Results — `blind-objective-strength`

Ran as registered: 100 maps, 200 games, seeds 38000000..=38000099 inclusive,
average 233.9 turns, `arms differ on: blind-objective-strength`.

| reading | `live_without_blind_objective_strength` vs `live` |
|---|---|
| game-win share | 102/200 (51.0%) vs 98/200 (49.0%) |
| paired-map score | 51.0%, 95% betting CI 45.4%..57.8% |
| **Elo-equivalent** | **+7 (CI −32..+54)** |
| paired direction | 11 · 80 · 9, sign p=0.8238 |
| terminal-score direction | 49 · 0 · 51, sign p=0.9204 |
| promotion gate | INCONCLUSIVE; resolves about +55 at 80% power |
| maps that broke | wins 20/100, terminal score 100/100 |

**The arm fired** — 20 maps broke on wins, all 100 on terminal score, no
"nothing differed" warning — so this is the war half of the bundle genuinely
measured and not a second inert pairing.

**Verdict: RETAIN. A second clean, well-controlled null**, and the tightest of
the two: withholding the treatment is if anything very slightly *better*
(+7 Elo), with wins and terminal score agreeing at p=0.82 and p=0.92.

⚠ The disagreement this arm was picked to test resolves against the ranking.
`GENE_HEURISTIC_RANKING.md` still carries `blind-objective-strength` at **+30**
wins/10k from the legacy 60×38 pangaea p10 screen. Two independent standard-
shape instruments now say otherwise: the 23,622-seat screen at −0.01 pp
(z −0.03), and this withholding arm at +7 Elo (CI −32..+54, 100 pairs, seed
38000000). Neither reproduces the +30. That is the same legacy-vs-standard
shape gap §1 found for `governor-victory-lanes`, in a treatment where it
happens not to matter because both readings are null.

---

# 4. `advanced_coupled_expansion` — pre-registered here

Of the two candidates the brief offered for leftover budget, this is the one
worth the machine, for a reason neither entry states: **both are on
`docs/EVAL_STATUS.md`'s "Unreachable by any screen" list**
(`coupled_expansion`, `great_work_veto_by_district`), so the paired evaluator
is not one instrument among several for them — it is the only one there is.
That is strictly more valuable than re-asking the ten in §3, which the screen
has already bounded.

`docs/AI_GAPS.md` ranks it #3: *"the modelling is done; the disjoint gameplay
screen that would promote or reject it has not run"*, and asks for "a
genome-matched deployment comparison".

## PRE-REGISTRATION (written and committed before the run)

**Arm.** `advanced_coupled_expansion` (= `AdvancedAi::coupled_expansion()`,
off in production) vs `advanced`. One axis; not on `MINOR_DEPENDENT_ARMS` and
not on `DEGENERATE_CONTROLS`.

**Design.** The `deployment-online` profile only, run directly with that
profile's exact recorded arguments rather than through `--matrix`:

```
ai_eval advanced_coupled_expansion advanced --pairs 100 --jobs 2 --seed 36000000 \
  --players 6 --width 74 --height 46 --city-states 9 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,religious,diplomatic,domination,score \
  --difficulty prince --deployment-comparison
```

Fixed N = 100, no early stopping. **`--city-states 9` is explicit**: `ai_eval`
seats no city-states in direct mode without it, and this treatment prices
settlement sites.

**Why one profile and not the matrix.** AI_GAPS asks for a *deployment
comparison*, which is this profile. Seed 36000000 is unused anywhere in the
tree.

⚠ **Scheduling amendment, recorded rather than made silently.** This was
registered to start only in the window that opens when §1's compact and
deployment-online children exit, so the machine would never carry more than
the four jobs the operator allowed while load was high. At 07:24 UTC the
machine's load average fell from 72 to **17** on 18 cores — the sibling agents
had finished — so it was started immediately at `--jobs 3` instead of waiting
about an hour for that window. The operator's cap was conditioned on load, and
this changes only when the games are played, not which games: N, seed, shape
and the fixed-N rule are exactly as registered above, and nothing about the
schedule can bias a paired estimate.

## Results

Ran as registered: **100 maps, 200 games, seed prefix 36000000..=36000099
inclusive**, 6 players, 74×46 continents, 9 city-states, Online, 250 turns,
average 218.3 turns, `arms differ on: coupled-expansion`. Fixed N, no early
stopping.

| reading | `advanced_coupled_expansion` |
|---|---|
| game-win share | 89/200 (44.5%) vs `advanced` 111/200 (55.5%) |
| paired-map score | 44.5%, 95% betting CI 37.6%..52.3% |
| **Elo-equivalent** | **−38 (CI −88..+16)** |
| paired direction | 9 favored · 71 neutral · 20 against, sign p=0.0614 |
| promotion gate | **INCONCLUSIVE** |
| terminal-score diagnostic | 48.9%; direction 33 · 0 · 67, sign **p=0.0009** |
| maps that broke | wins 29/100, terminal score 100/100 |

The arm fired on this profile — 29 of 100 maps broke on wins and every map
broke on terminal score — so the reading is about the treatment, not about a
flag that never reached the board.

Two lines from the run matter as much as the estimate:

- **`resolution: … this gate promotes a true edge of about +67
  Elo-equivalent 80% of the time, and anything smaller reads as INCONCLUSIVE
  here whether or not it is real.`** This is the tool's own power statement
  and it is *worse* than the ±30 this round estimated from the sibling arm's
  400-pair p-value. The tool's number governs; the estimate in §1's
  pre-registration was optimistic and is corrected here rather than quietly
  left standing.
- **`effect size: −38 (not gate-selected — the gate did not fire, so this
  estimate is not conditioned on being large)`.** The winner's curse that
  `docs/EVAL_INTEGRITY.md` R3 names does not apply to this −38: nothing
  selected it. It is still one seed window.

## Verdict: RETAIN `advanced`. `coupled_expansion` is not promoted.

Both readings point the same way and one of them is significant. Wins fall
short of significance (p=0.0614) at a resolution that could only have caught
about +67; terminal score, which breaks on every map and therefore carries far
more information, is **worse at p=0.0009**. Nothing here supports promoting the
arm, and the direction is wrong for the hypothesis that pricing a Settler as a
bounded investment against a 90-turn payoff horizon buys anything at the
deployment shape.

`docs/AI_GAPS.md` ranked this #3 with the note *"the modelling is done; the
disjoint gameplay screen that would promote or reject it has not run."* It has
now run, on a seed window disjoint from everything in the tree, and it rejects.
The arm stays off in production, where it already was, and keeps its number.

⚠ **What this run cannot say.** Domination was enabled and never produced in
200 games, and diplomatic decided 1 game — so an expansion treatment acting
through conquest is invisible here. The lane it *should* act through, score, is
the largest at 80 of 200, so the main channel was present. And the strategic-
search half of AI_GAPS #3 is untouched by this: it prices the coupled
valuation, not the search.

## Why the great-work veto was *not* run, and what it needs first

`docs/AI_GAPS.md` marks it ★★★★★ and asks to "price the veto's
district-vs-slot key (`advanced_great_work_veto_by_district`) against stock on
the deployment profile". It needs a design decision before it needs compute,
and this round is not the place to make it silently:

- the arm is `AdvancedAi::targeting(Science)` + the flag, and
  `src/elo.rs`'s own test pins its control to `advanced_target_science` so the
  pair holds the Science target fixed — **not** to "stock" `advanced`;
- `advanced_target_science` is the sole entry in `ai_eval`'s
  `DEGENERATE_CONTROLS`: *"completes 0/16 at the deployment profile
  (victory_eval, 96 games, two disjoint streams)"*.

The pairing is symmetric, so the degenerate control does not break it the way
it broke the +669 Elo diplomatic reading — both arms carry the same floor. But
a lane that never finishes at this shape decides almost every game on the score
cap, and how much of the veto's effect survives that is unknown. Running it
without settling whether the deployment profile can host a Science-targeted
comparison at all would produce a number of unknown meaning, which is the
`EVAL_INTEGRITY.md` R1 family. **Recorded as blocked on a control decision, not
on budget.**

---

# What was decided

Six pre-registered runs, 600 map pairs, 1,200 games, six disjoint seed windows.
Every one ran to its registered N with no early stopping, and every
pre-registration was committed and pushed before its run started.

| # | gate | pre-registered | measured | **verdict** |
|---|---|---|---|---|
| 1 | `advanced_governor_victory_lanes` compact | 100 pairs, seed 29000000 | −30 Elo (CI −68..+1), sign p=0.0161 | **RETAIN `advanced`** |
| 1 | …deployment-online | 100 pairs, seed 30000000 | −45 Elo (CI −104..+21), sign p=0.0470 | **RETAIN `advanced`** |
| 1 | …deployment-contested | 100 pairs, seed 31000000 | +69 Elo, PASS, discovery | superseded by ↓ |
| 1c | …contested, confirmation | 100 pairs, seed 39000000, `--confirm 31000000` | **+60 Elo (CI +28..+112), p=0.0021, CONFIRMED** | **CHANGE — field-dependent** |
| 2 | `advanced_wonder_reach`, seed 32000000 | 400 pairs, matrix | — | **UNRUNNABLE — code culled (#2266)** |
| 3 | `live_without_settler_site_agreement` | 100 pairs, seed 37000000 | −3 Elo (CI −56..+43), p=1.0000 | **RETAIN — clean null** |
| 3 | `live_without_blind_objective_strength` | 100 pairs, seed 38000000 | +7 Elo (CI −32..+54), p=0.8238 | **RETAIN — clean null** |
| 4 | `advanced_coupled_expansion` | 100 pairs, seed 36000000 | −38 Elo (CI −88..+16); terminal score p=0.0009 | **RETAIN `advanced` — not promoted** |

**Nothing in this round changes a default.** Two arms measured null, two
measured negative, one is unrunnable, and the one positive result is on a
profile whose own definition forbids comparing it to the deployment gate.

## The headline, stated plainly

**The 2026-08-18 decomposition was right about the carrier and wrong about the
size.** The four victory lanes are what makes the governor composite bad —
direction confirmed on both of that round's profiles, sign-significant on both,
terminal score p=0.0000 on both, and the composite's own "uniformly smaller
empire" census reproduced. But it predicted −70 to −80 and this measured
**−45** at deployment and **−30** compact. The prediction survives inside the
interval and fails as a point estimate, downward, which is the direction this
ledger's effect sizes always fail.

## What this round found that it was not looking for

1. **26 of the 49 arms objective 3 tells the fleet to run are byte-identical to
   their own control** — `enable_live_bridge()` applies the ledger, which
   already calls the same `disable_*`, so withholding a ledger-off treatment
   withholds nothing. Proven from the source, confirmed on the board with a
   positive control. Over half the roadmap points at no-ops, and `ai_eval`
   prints `arms differ on: <tag>` for every one of them.
2. **`never_named` has stopped measuring objective-3 debt.** It went 10 → 0 in
   one commit because a whole-genome screen tabulated every gene at once. The
   counter is honest about its own definition and the definition no longer
   tracks the thing.
3. **`governor-victory-lanes` is field-dependent, and every instrument that
   resolved it is fieldless.** −45 with six entrant seats, **+60 confirmed** with
   four seats chasing diplomacy and culture.
4. **A pre-registered gate can be deleted before it runs.** #2266 removed
   `wonder-prereq-reach` on a −26 that the in-flight p10 screen then replaced
   with +29 — both inside noise — and took the arm that would have settled it.

## What remains

- **22 of the 23 runnable withholding arms are still unpriced.** Two are done
  here; the other 26 arms cannot be run at all until their ledger default
  flips.
- **The great-work veto is blocked on a control decision, not on budget** — its
  pinned control `advanced_target_science` is `ai_eval`'s sole
  `DEGENERATE_CONTROLS` entry. Detailed in §4.
- **A contested-field gene screen**, before `governor-victory-lanes` is treated
  as settled.
- **A test that refuses an inert `live_without_*` arm.** Recommended in §3 and
  deliberately not written here: it is `src/elo.rs`, a declared hotspot, and
  outside this round's claim.
