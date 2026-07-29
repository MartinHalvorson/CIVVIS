# Post-conquest Domination commitment

Status: **preregistered before implementation or focal data**. This branch is
evaluator-only and changes no shipped AI behavior.

Pre-implementation reproducibility clarification, still before any simulation:
the first compile audit made the frozen difficulty explicit as Prince and
clarified that non-focal majors use the champion while city-states, Free Cities,
and barbarians use Basic. A subsequent source audit also preserved the declared
defeated-major population when it found that elimination clears
`occupied_from`: the evaluator records a focal `KeepCity` plus that city's exact
pre-turn owner before the field can be erased. These corrections change no
treatment, population, outcome, seed, or decision gate.

## Why this experiment exists

The production archive shows plenty of war without a civilization converting
that war into a Domination race. In the 100 most recent completed eight-major
Continents/Planet/Online games available at preregistration time
(`2026-07-29T03:28:03Z` through `2026-07-29T19:24:05Z`), the engine recorded:

- 93 Science and 7 Culture victories, with no Domination victory;
- 2,619 wars and 1,166 city-capture events (26.19 and 11.66 per game);
- 78 foreign Original Capitals in non-founder hands at the terminal snapshot;
  but
- no civilization ever held more than two of the seven foreign Original
  Capitals (43 games peaked at one holder, four at two, and none above two).

This is not evidence that the tactical planner ignores capitals. Static review
finds a large Conquest capital bonus, extra tactical objective value, a bypass
of the loyalty deferral when a capture completes Domination, and an explicit
capital-ordering fixture. `City::is_capital` also remains the Original-Capital
marker after conquest; Palace relocation is a separate concept.

The missing edge is higher in the hierarchy. An untargeted `AdvancedAi` gives
Conquest a fixed `victory_focus` progress of zero while Science starts at 25 and
rises with the technology tree. Conquest can still be selected by an active
war, an emergency, or a transient power advantage, which is why the frozen
strategy census in #566 observed the champion in Conquest for 27.1% of midgame
turns. That same census found Conquest-to-Recovery plus Recovery-to-Conquest to
be the largest transition pair. A civilization can therefore win a city, end
or recover from the local war, and return to the Science race without ever
treating the conquest as the first move of a Domination campaign.

#579 tests the upstream question: can the AI deliberately create one prepared
midgame war? This experiment is downstream and conditional on a real conquest:
**after paying the ordinary costs to conquer and keep one foreign major's city,
does a persistent Domination commitment turn dispersed captures into capital
concentration without making the agent weaker?** It neither changes war timing
nor grants a unit, city, resource, yield, or score.

## Frozen treatment

`post_conquest_domination` wraps the exact deployed generation-14 champion in
both arms. The weights are compiled from `data/evolved/best.json`; the evaluator
asserts generation `14` and FNV-1a `40b1fbb2a5b88bc6`. It never consults the
working directory, `league/`, `evolved/`, or `valuenet.json`.

Each map contributes two independent focal-seat cells, seats 0 and 7. For each
cell the evaluator constructs one base world, clones it, and runs:

- **control:** the ordinary adaptive champion; and
- **treatment:** the same stateful adaptive champion until the first qualifying
  start-of-turn boundary, then the same champion permanently retargeted with
  `AdvancedAi::retarget(VictoryTarget::Domination)`.

All seven non-focal majors use the same compiled champion controller in both
arms; every city-state, Free City, and barbarian uses the same stock Basic
controller in both arms. A qualifying boundary exists only when the focal seat
currently owns and has kept a city satisfying all of:

1. exact conquest provenance names another living or defeated **major**
   civilization;
2. `original_owner != focal`;
3. the city is still owned by the focal seat; and
4. Domination is enabled.

`occupied_from` is written only by `transfer_city(..., conquest = true)` and
survives `KeepCity` in the ordinary case. The engine clears that field when the
previous owner is eliminated, however, so the observer also records the exact
pre-turn owner of a city named by a focal `KeepCity` action. It retains that
evidence only while the focal seat continuously owns the city. Thus trade,
peaceful transfer, founding, Envoy/Suzerain control, loyalty accession, and a
recapture of the focal seat's own city cannot trigger the treatment. Capturing
a Free City does not qualify because the immediate conquered owner is not a
major. The trigger deliberately does not require an Original Capital: the
hypothesis is that the first *successful major conquest* should make subsequent
wars aim at capitals.

The commitment is seat-local and irreversible for the rest of that game. It
does not disappear if the triggering city is lost, the war ends, or the
five-turn strategic plan is reassessed. Existing urgent recovery, diplomacy,
target legality, combat, occupation, and city-disposition logic remain active
inside the targeted agent. This is a policy-bundle experiment: it tests the
existing complete Domination lane (research, production, diplomacy, and unit
routing), not a fitted scalar added to `victory_focus`.

The two arms must be byte-identical at every observed boundary before the
treatment's first qualifying trigger. Any earlier divergence invalidates the
cell. At the trigger, the evaluator records the exact serialized-world hash,
turn, triggering city, previous owner, current foreign-capital count, and both
agents' prior public plan report. A treatment may retarget exactly once.

## Outcomes and clustering

Games retain the deployment policy horizon of 250 Online turns and the shipped
victory set (`science,culture,domination`), but the observer continues an
otherwise unchanged undecided game through turn 320. Score victory stays
disabled. This gives a post-conquest campaign time to resolve without changing
when production, research, victory progress, or score-end pressure believe the
deployment horizon ends.

For every focal cell and arm the evaluator records:

- eligibility, trigger turn, and retarget count;
- peak and terminal foreign Original Capitals controlled;
- **post-trigger capital gain**: peak foreign capitals after the shared trigger
  minus foreign capitals controlled at that trigger;
- whether the focal seat ever controls at least two and at least three foreign
  Original Capitals;
- cities captured after trigger, wars declared after trigger, survival, and
  retention of its own Original Capital;
- winner and victory type; and
- terminal focal score and major-field score share.

The strength utility is fixed as

```text
0.80 * I(focal seat wins)
+ 0.20 * focal_score / sum(living-or-defeated-major scores)
```

and the paired strength share is `sum(treatment utility) / sum(both-arm
utility)`. The score term is an ordinary consequence of play, not an injected
reward. Raw wins, victory types, score, and capital outcomes are always printed
beside the composite so a passing label cannot hide a score-only effect.
Every map-seat-arm also emits a deterministic `raw` JSON row containing its
trigger provenance, exact-prefix status, all component outcomes, and utility,
so the aggregates can be reconstructed from the captured log.

The two seat cells are averaged within each map before inference, making the
map seed—not a seat or a game—the independent unit. A capital row averages its
qualifying seat differences and omits a map only when neither seat qualifies;
strength always averages both seats. Direction counts likewise compare one
map-row treatment mean with its control mean. Confidence intervals use a
deterministic 10,000-resample percentile bootstrap over those map rows with
bootstrap seed `0xAD0_107`; exact sign tests drop tied maps. No seat-level
pseudo-replication is permitted.

## Frozen phases and commands

The evaluator accepts exploratory runs, but prints a formal phase label only
when every option below occurs exactly once, has the canonical raw spelling,
and no unrecognized or duplicate argument exists. Missing, valueless,
malformed, default-substituted, or extra arguments are diagnostic-only. In
particular, a formal command requires explicit `--ai advanced_evolved`,
`--difficulty prince`, `--randomize-civs`, `--jobs 6`, both turn horizons, and
the entire map profile.

### 1. Serialized null

```text
adaptive_domination_eval --phase null --treatment none --ai advanced_evolved \
  --maps 4 --players 8 --width 84 --height 54 --city-states 12 \
  --deployment-turns 250 --observe-turns 320 --speed online \
  --difficulty prince \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --focal-seats 0,7 \
  --seed 10700000 --jobs 6
```

Both arms use the default-off wrapper. All eight focal cells must have identical
serialized worlds after every focal turn, identical terminal results, zero
retargets, and equal diagnostics. Failure stops the line.

### 2. One development screen

```text
adaptive_domination_eval --phase screen \
  --treatment post_conquest_domination --ai advanced_evolved \
  --maps 30 --players 8 --width 84 --height 54 --city-states 12 \
  --deployment-turns 250 --observe-turns 320 --speed online \
  --difficulty prince \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --focal-seats 0,7 \
  --seed 10710000 --jobs 6
```

The screen advances only if all terms pass:

1. at least 18 of 60 focal cells qualify, every qualifying treatment retargets
   exactly once, and no arm diverges before its shared trigger;
2. treatment post-trigger capital gains exceed control by at least four in
   aggregate and treatment has at least two more qualifying cells that reach
   two foreign Original Capitals;
3. paired strength share is at least 52%, treatment-favorable map directions
   outnumber control-favorable directions, and paired terminal-score share is
   at least 50%; and
4. treatment survival and own-Original-Capital retention are each no more than
   two focal cells below control.

Failing exposure means `INERT`; failing any other term means `REJECT`. There is
no screen retry, seed shift, threshold sweep, or favorable-subgroup rescue.

### 3. Disjoint confirmation, only after a passing screen

```text
adaptive_domination_eval --phase holdout \
  --treatment post_conquest_domination --ai advanced_evolved \
  --maps 120 --players 8 --width 84 --height 54 --city-states 12 \
  --deployment-turns 250 --observe-turns 320 --speed online \
  --difficulty prince \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --focal-seats 0,7 \
  --seed 10720000 --jobs 6
```

The holdout passes only if all terms hold:

1. at least 60 of 240 focal cells qualify with exact-once retargeting and no
   pre-trigger divergence;
2. the paired mean of the within-map qualifying-seat post-trigger capital-gain
   differences is at least `+0.15`, its map-bootstrap 95% interval is wholly
   above zero, and treatment-favorable non-tied map directions pass a two-sided
   exact sign test at `p < 0.05`;
3. the fraction of qualifying cells that ever reach two foreign Original
   Capitals is at least 10 percentage points above control;
4. paired strength share is at least 52% with a map-bootstrap 95% lower bound
   above 50%, raw focal wins are not fewer, paired terminal-score share is at
   least 50%, and treatment-favorable strength directions outnumber adverse
   directions; and
5. survival and own-Original-Capital retention are each no more than five
   percentage points below control.

Domination victories are reported but are not a minimum-count gate: even 120
maps may be underpowered for a formerly zero-frequency endpoint. A pass says the
existing Domination lane causally concentrates capitals and improves overall
play after real conquest; it licenses a separate, reviewed gameplay-integration
PR. It does not change `AdvancedAi` on this branch. Any failed validity,
mechanism, strength, or harm term retains this evaluator as negative evidence
and leaves the shipped policy unchanged.

## Required implementation tests

Before any registered seed may run, focused tests must establish:

1. the compiled champion generation and FNV provenance;
2. peaceful transfer, founding, city-state capture, Free-City capture, and own-
   city recapture cannot trigger, while keeping a city conquered directly from
   a living or thereby-defeated major does;
3. treatment retargets exactly once and stays committed after peace, plan
   reassessment, and loss of the triggering city;
4. the control wrapper records the same eligibility boundary without changing
   its adaptive target;
5. treatment and control serialize identically through every pre-trigger turn;
6. foreign-capital peaks and post-trigger gains use Original Capitals, not the
   movable Palace/current-capital concept;
7. two focal seats fold to one independent map row and tied directions are
   excluded from the exact sign test;
8. the composite, raw outcomes, bootstrap, screen gates, and holdout gates match
   fixed hand-calculated fixtures; and
9. CLI/formal-label parsing fails closed on every missing, duplicate,
   malformed, valueless, noncanonical, or extra argument.

Run the focused CI-profile suite, `git diff --check origin/main...`, file-scoped
`rustfmt`, and `cargo test --profile ci --locked` before the null. This branch
must merge current `origin/main` through #579 immediately before registered
measurement. #579 changes the upstream war policy and therefore the eligible
population; mixing pre- and post-#579 baselines would make the estimand
ambiguous.

## Resource and chronology contract

- This preregistration is committed and pushed before evaluator implementation
  and before any seed in `10700000`, `10710000`, or `10720000` is read.
- No registered simulation starts while #561 or another six-core batch is
  active. Earlier already-preregistered shared-host evaluations keep their
  queue priority.
- No focal run starts before #579 lands and this branch merges that exact
  gameplay baseline once.
- A smoke may use only a clearly non-focal seed outside all three registered
  ranges and must print `DIAGNOSTIC ONLY`.
- No result from the live spectator archive is a treatment outcome; it only
  motivated the hypothesis above.
