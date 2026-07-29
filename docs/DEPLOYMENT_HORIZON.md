# Deployment horizon calibration

Status: **preregistered; no focal seed has been read**.

## Prospective deployment-population amendment

This amendment was frozen before either focal seed was run or read. A
pre-execution audit of the next queued production-profile experiment found that
this study's original fixed eight-player Continents/Planet cell was not the
unattended deployment population. The merged supervisor redraws three axes
independently and uniformly for every unattended world: player count from 4
through 10, map script from its nine supported scripts, and Flat/Planet
topology. A result from one of those 126 joint profiles cannot establish the
prevalence of production censoring across all of them.

The fixed batches therefore use the same deterministic space-filling cycle as
the independently preregistered Spaceport study (#567). For zero-based map
offset `i`:

- players are `[4, 6, 8, 10, 5, 7, 9][i mod 7]`;
- scripts are `[land_only, water_world, continents, true_start_earth, lakes,
  inland_sea, pangaea, small_continents, islands][i mod 9]`; and
- topology is Flat at even `i` and Planet at odd `i`.

The three periods are pairwise coprime, so all 126 joint profiles occur once
before repetition. `MapSize::for_players` supplies requested dimensions and
city-state counts; the evaluator must not copy that table. The 12-map screen
contains six maps per topology, one or two per player count, and one or two per
script. The 48-map confirmation contains 24 maps per topology, six or seven per
player count, and five or six per script. Each phase restarts at offset zero and
uses its already frozen, disjoint game seeds.

This changes only the prospectively sampled deployment population. The
published `strategic_deep` controllers, nominal and external horizons, seeds,
12/48 map counts, endpoints, thresholds, stop rule, and six-job resource cap
are unchanged. Axis-specific summaries are descriptive only; no stratum may
promote, extend, or rescue a failing pooled gate.

## Why `--turns 320` is not the live policy

The production spectator starts Online games with `--turns 250` and enables
Science, Culture, and Domination victories, not Score. `Game.max_turns`
therefore remains 250 and all AI planning code that reads the game horizon
continues to price decisions against 250. With Score disabled, the server does
not award a turn-250 winner; it keeps stepping the same game until an enabled
victory occurs.

A read-only audit made before this preregistration found that 19 of the latest
24 archived production games finished after turn 250. Mean finish was turn
279.1 (range 229--343), with 23 Science victories and one Culture victory.
That is a mixed-map prevalence observation, not the focal experiment below.
It establishes why the distinction is practical:

- an evaluator that stops observing at 250 right-censors games production
  continues; but
- an evaluator that sets `Game.max_turns = 320` changes expansion deadlines,
  payback calculations, late production values, and other policy inputs.

The production-faithful construction keeps `Game.max_turns = 250` and extends
only the outer simulation loop. This task measures the resulting censoring. It
does not change gameplay or retroactively reinterpret a frozen experiment.

## Frozen instrument

Add `deployment_horizon`, an observer-only binary. Every major civilization
uses the published `strategic_deep` controller and must resolve without a
degraded artifact fallback; city-states and barbarians use `basic`. A game is
created with nominal turn limit 250. The runner records a snapshot immediately
after the last completed turn at or below that limit, then continues the same
agents and game state until a winner or external observation turn 320. It must
never rewrite `game.max_turns` after construction.

For each independent map report:

- whether an enabled victory existed by the nominal boundary;
- whether a game censored at 250 resolved during turns 251--320;
- the resolution turn and victory type;
- the eventual winner's rank by terminal score at the nominal snapshot;
- living-major city count at the nominal snapshot and resolution; and
- unresolved status at the external bound.

The aggregate reports nominal completions, late completions, still-censored
games, finish-turn distribution, victory types, and how often the eventual
winner already led score at 250. None of these observations enters an AI
rating. Unit tests must prove that the nominal snapshot is taken exactly once,
that continuation can pass turn 250 while `max_turns` remains 250, and that a
winner before the boundary is not classified as late.

## Amended implementation checkpoint

The deployment-population amendment was committed and pushed as
`6b01ca5f6e6703a70a8d765ef6073c2d80106d33` before its implementation. The
completed runner is frozen at
`250f1c1f8011e4cb063a6fd5bef165d2febca61a`. It derives every size row through
`MapSize::for_players`, rejects fixed-profile flags under `--deployment-mix`,
caps execution at six jobs, reports every map's requested and realized
geometry, and emits player-count, script, and topology summaries without
letting them enter the pooled gate.

All four focused CI-profile tests passed, including the 126-cell uniqueness and
exact 12/48-map marginal balances. The standalone binary rejected a conflicting
`--deployment-mix --players 8` invocation before simulation. A one-map,
one-turn diagnostic at seed 9,986,999 exercised the derived 4-player Land
Only/Flat profile, retained `Game.max_turns = 1`, and printed no focal gate. No
map from seed 9,986,000 or 9,987,000 has been run or read.

A final pre-flight audit before either focal seed found that the CLI accepted a
diagnostic `--difficulty` override but omitted difficulty from the predicate
that labels an invocation as the exact frozen profile. The runner now also
requires the production-default Prince difficulty before printing either gate.
The frozen command already omitted the flag and therefore already selected
Prince; this is prospective validation hardening only, with no change to the
command, population, controller, seed, endpoint, threshold, or resource rule.

## Prospective fail-closed invocation amendment

This second pre-flight correction was frozen before either focal seed was run
or read. A static command-path audit found that a supplied numeric option with
no parseable value silently fell back to its default. The exact-profile
predicate also accepted duplicated/default-substituted flags, did not bind
`--jobs 6`, and compared phase map counts and seeds only after parsing rather
than to their canonical raw command values. The earlier difficulty correction
bound resolved Prince difficulty but still allowed an explicit diagnostic
`--difficulty prince` even though the frozen command intentionally omits that
flag. No focal output exposed these bookkeeping defects; both registered seed
ranges remain unopened.

The runner must now exit 2 before constructing a game whenever a supplied
numeric or text option lacks a usable value. Formal recognition requires each
common command option exactly once with its canonical raw value:
`--deployment-mix`, `--turns 250`, `--observe-through 320`, `--speed online`,
`--poles poles`, `--randomize-civs`,
`--victories science,culture,domination`, and `--jobs 6`. The screen must bind
`--maps 12 --seed 9986000`; confirmation must bind
`--maps 48 --seed 9987000`. `--difficulty` must remain absent so the frozen
production-default Prince selection is made exactly as preregistered. Defaults,
noncanonical numeric spellings, duplicated flags, and diagnostic overrides may
run only under the diagnostic label and cannot receive either gate.

This prospective integrity correction changes no controller, sampled world,
horizon, seed, map count, endpoint, threshold, stop rule, or resource cap.

## Fixed deployment-population screen

The amended frozen command is:

```text
deployment_horizon --deployment-mix --maps 12 --turns 250 \
  --observe-through 320 --speed online --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9986000 --jobs 6
```

The runner must print every derived player/script/topology balance and each
profile's requested and realized geometry. The screen establishes material
production censoring only if at least 3/12 games have no winner at the nominal
boundary and then resolve to an enabled victory by turn 320. Games still
unresolved at 320 are reported but do not satisfy that term. A weaker result
stops without a larger run.

## Fixed confirmation and decision

A passing screen earns exactly one 48-map confirmation at seed 9987000, with
`--maps 48` and every other argument unchanged. Confirmation requires both:

1. at least 10/48 games resolve after the nominal boundary and by turn 320;
2. the two-sided 95% Wilson lower bound on that late-completion share exceeds
   10%.

There is no seed retry, pooled rescue, longer observation bound, controller
substitution, or threshold change. The six-job run must wait until no other
simulator batch is using six or more cores.

A confirmation pass changes evaluation language, not AI behavior: a study
claiming production-terminal fidelity must preserve `max_turns = 250` and use
an external continuation/time-to-event endpoint, or explicitly call itself a
turn-250 truncation. It does not invalidate prospectively frozen 250-turn
tests; their estimand remains policy strength at that boundary. A failure
retains the existing convention while leaving the archived prevalence
observation descriptive.
