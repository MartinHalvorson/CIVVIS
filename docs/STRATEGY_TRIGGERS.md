# Grand-strategy assessment-trigger census

Status: **implementation checkpoint frozen; no trigger data inspected**

## Why this measurement comes next

The 2026-07-29 adaptive-strategy census established two facts on 60 mirrored
production-profile maps. First, the embedded `advanced_evolved` champion made
2.69 midgame strategy switches per seat-game without a simultaneous change in
major-war status, threatened-city status, or city deficit. Those 1,292 changes
were 61.4% of its 2,105 midgame switches, so commitment instability is common
enough to study. Second, the champion was not more stable than stock
`advanced`: its unanchored switch rate was 14% higher and its paired-map score
was 47.5%. Genome stability therefore does not explain strength.

That observer-level result is not yet a license to add a generic cooldown.
`AdvancedAi::assess` already attaches one of a small number of static reasons
to every chosen strategy, but the reason is written only to the reasoning
journal. Relative military power, rival victory pressure, lane progress, and
settlement availability can therefore move the plan while all three exported
booleans stay constant. Delaying one of those responses blindly could suppress
an urgent defense or counter-victory plan.

This task exports the reason that the live decision already computed and asks
which exact assessment boundary accounts for the residual. It changes no
gameplay behavior.

## Frozen observer contract

`PlanReport` will expose the static assessment reason beside `strategy`. The
advanced agent will retain the reason produced by the same `assess` call that
produced its current plan; the evaluator must not reimplement, infer, or parse
the reason from the journal. Agents without a reported plan use `unreported`.

The reason vocabulary is the existing branch order in `assess`:

1. `at war and losing ground at home`
2. `an emergency objective is standing`
3. `Tagma timing is live`
4. `a neighbour is inside the ancient window and cannot wall in time`
5. `the religion lane still needs a religion`
6. `the assigned lane can still afford to expand first`
7. `following the assigned victory lane`
8. `countering a rival close to winning`
9. `already at war`
10. `a Prophet is a finite race worth entering now`
11. `strong enough to take what a neighbour has`
12. `already well down its best victory lane`
13. `short of cities with land still open`
14. `its best available victory lane`

If implementation rebases over a gameplay change that adds, removes, or
reorders an assessment arm, this document must be amended and pushed before
any census run. The exact instrumented source commit will be recorded here
before primary data are generated.

`ai_eval` will retain all current target, rush, context-boundary, strategy,
outcome, and promotion calculations. Its separate assessment-trigger trace
will record, for each observed player-turn:

- the reason label and its share of midgame observations;
- every ordered `previous reason -> current reason` transition that
  accompanies a midgame strategy switch;
- the subset of those transitions on previously defined **unanchored**
  switches, where major-war, threatened-city, and city-deficit booleans all
  remain unchanged;
- unanchored strategy switches whose reason label did not change, grouped by
  `previous strategy -> current strategy`; and
- for each unanchored reason transition, both occurrence count and the number
  of seat-games in which it occurred at least once.

For focus selection, the two directions of a reason transition are also
combined into an **unordered reason-pair family**. A same-reason strategy
switch is its own family. Pair canonicalization is lexical and output ordering
is count descending, then label ascending, so parallel execution cannot alter
the report.

The evaluator, rather than the analyst, will apply the frozen family decision:
select the globally dominant family with the same deterministic tie break,
check its exact elective vocabulary, and print the concentration, occurrences
per seat-game, seat coverage, and `NOMINATE`/`REJECT` verdict. The documented
thresholds are constants in that calculation; displayed rounding is never used
for the comparison.

The primary interval is unchanged: Standard turn 60 inclusive through
Standard turn 180 exclusive, speed-normalized by `Game::standard_duration`.
A switch belongs to the interval when its new observation is in the interval,
matching the shipped strategy census. The first observation for each seat can
never be a switch.

Tests must establish that:

1. a reported reason comes from the assessment that produced the reported
   plan;
2. reason changes, same-reason strategy changes, unanchored filtering,
   unordered pairing, and per-seat coverage are counted independently;
3. overlapping visible boundaries remain one boundary-accompanied switch;
4. deterministic ranking breaks count ties by label; and
5. adding the trace does not change winner, victory type, turn, or existing
   target/rush/strategy trace values for a fixed seeded game.

## Instrumented checkpoint

The exact observer implementation is committed at
`034b1de627033f84fe2d1a8118bef8e25f9b2af2`: the live
`assess()` cascade returns its plan and selected static reason together, the
advanced agent retains both in the same reassessment step, and `PlanReport`
exports that retained reason. Retargeting, reweighting, or returning to the
adaptive planner clears both cached values. The evaluator reads only the
report field; it contains no second implementation of the cascade.

The merge through current `origin/main` did not add, remove, or reorder any of
the fourteen frozen assessment arms above. The exact plan/report invariant
test passed, as did all 30 `ai_eval` tests covering trace preservation,
aggregation, deterministic ranking, and the automatic gate. No primary census
command has started and no trigger counts have been read.

Immediately before the primary run, this branch will merge the then-current
main, pass the complete CI-profile suite, build `ai_eval` with
`cargo build --release --locked --bin ai_eval`, and record the resulting head
and absence of any concurrent high-core simulation. Those operational checks
cannot be asserted prospectively.

## Frozen run

The primary census replays the prior experiment's exact maps and profile so
the new diagnostic explains the already-observed switch population rather
than selecting a friendlier corpus:

```text
ai_eval advanced_evolved advanced --players 8 --width 84 --height 54 \
  --city-states 12 --pairs 60 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9981000 --jobs 6
```

There are 480 seat-games per entrant. `advanced_evolved` is primary because it
is the production champion; stock `advanced` is a descriptive anchor. The
reused seeds are intentional and cannot promote gameplay: this is an
observer-only localization of a population already selected by the previous
preregistration. No holdout is opened in this task.

Before starting the run, the implementation commit, command, build profile,
and absence of concurrent high-core simulations will be recorded. The run
must use the repository's resource coordinator and must not overtake an
already queued primary experiment.

## Frozen decision rule

Let the **dominant family** be the most frequent unordered reason-pair family
among all of the champion's unanchored midgame strategy switches. Count ties
break by the canonical family label. The dominant family is **eligible** only
when both of its reasons (or its one same-reason label) belong to this frozen
elective set:

- `strong enough to take what a neighbour has`
- `already well down its best victory lane`
- `short of cities with land still open`
- `its best available victory lane`

Every family containing defense, emergency, active war, victory denial,
assigned-lane routing, ancient-rush timing, Tagma timing, or the finite Prophet
race is ineligible. In particular, neither entry to nor release from urgent
defense or victory denial may be delayed on this evidence. If the globally
dominant family is ineligible, no lower-ranked elective family may be selected
after seeing the result.

An eligible dominant family nominates a single trigger-scoped gameplay
experiment only if all three conditions hold:

1. it accounts for at least **30%** of the champion's unanchored midgame
   switches;
2. it occurs at least **0.75 times per champion seat-game**; and
3. it occurs in at least **25%** of champion seat-games.

These gates require concentration, material rate, and breadth separately. A
large count from a few unstable games is not enough, and several weak families
cannot be pooled after seeing the data.

The intervention scope is frozen by the winning family:

- A same-reason family under `its best available victory lane` or `already well
  down its best victory lane` nominates margin-based lane hysteresis, because
  the broad assessment arm stayed fixed while its argmax changed.
- A family led by `strong enough to take what a neighbour has` nominates
  threshold hysteresis around opportunistic Conquest only; assigned lanes,
  emergencies, active wars, ancient rushes, and victory denial remain exempt.
- A family involving `short of cities with land still open` nominates only a
  settlement-availability or city-target margin around that branch. It may not
  hold an Expansion strategy after the reported city deficit closes.

If the dominant family is ineligible, or if it fails any of the three gates,
generic and trigger-scoped hysteresis are both rejected on this evidence. The
next military experiment must improve execution inside a stable
Conquest/Recovery episode rather than suppress plan changes.

Even when a family passes, this task promotes only a hypothesis, not gameplay.
The subsequent arm requires a fresh preregistration, a development screen,
fresh mirrored outcome seeds, and the normal paired promotion gates. Stock
differences and win outcomes in the replay are descriptive and cannot alter
the family selection.

## Result

Pending implementation and the queued primary experiments ahead of this run.
