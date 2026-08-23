# The arms that were pre-registered and never run

_2026-08-23 · `1510b482`_

## What was asked

`docs/ROADMAP.md` objective 3 asks the shipped bundle to be priced by
withholding *before the next effect hides inside a composite the way
`city_target_floor` did* — that one hid −41 Elo. Several arms were registered,
pre-priced, and never run. This round runs the load-bearing one and records the
disposition of the rest, so the open ends stop being invisible.

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

<!-- RESULTS-1 -->

## Verdict

<!-- VERDICT-1 -->

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
afterwards at **+29** — and `HEURISTIC_GENE_RANKING.md`'s *Removed from the
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
tree. It runs only in the window that opens when §1's compact and
deployment-online children exit (~09:25 UTC), so the machine never carries
more than the four jobs the operator allowed. If that window does not open,
this section records the registration and reports it unrun — which is the
failure mode this whole round exists to make visible.

## Results

<!-- RESULTS-4 -->

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

<!-- DECIDED -->
