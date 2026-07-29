# Deployment horizon calibration

Status: **preregistered; no focal seed has been read**.

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

## Fixed focal screen

The untouched command is:

```text
deployment_horizon --maps 12 --players 8 --width 84 --height 54 \
  --city-states 12 --turns 250 --observe-through 320 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9986000 --jobs 6
```

Requested 84x54 Planet geometry must be printed with its realized 105x44
storage rectangle. The screen establishes material production censoring only
if at least 3/12 games have no winner at the nominal boundary and then resolve
to an enabled victory by turn 320. Games still unresolved at 320 are reported
but do not satisfy that term. A weaker result stops without a larger run.

## Fixed confirmation and decision

A passing screen earns exactly one 48-map confirmation at seed 9987000 with
every other argument unchanged. Confirmation requires both:

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
retains the existing convention on this focal cell while leaving the mixed-map
archive observation descriptive.
