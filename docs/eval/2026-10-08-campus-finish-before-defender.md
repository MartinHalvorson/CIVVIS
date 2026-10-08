# Finish a nearly complete Campus building before a slow defender

Native Emperor Gran Colombia run `civvis-20261007T131449Z`, state 117f0,
records Bogotá building its Library with 33/45 production and one turn left.
The city and its 200-point walls had no damage; the Campus was intact.
The build menu quoted six turns for Pike and Shot. The native planner paused
the Library for that defender. This is a posthoc queue-interruption witness,
not evidence that retaining the Library would have won that game.

The frozen events file has SHA-256
`0d0d7ad4d6c2a36db02ffb81becd3dad6243e3bf376c9b7551574da623ed53fc`.
Its preserved copy and the earlier private-source experiments are under
`~/civvis-tactics-results/2026-10-08/gran-colombia-l6-domination-20261008T0620/`.
The original private frontend matched the complete native orders and decision
at the first changed frame. Those private replay results do not establish
that the public frontend matches the same complete native history.

This change adapts the hypothesis independently to public main. In a
Domination city with intact walls and no damage, the existing emergency
selector retains an already legal Campus-family building when its observed
host completion quote is at most one turn and the selected military defender
requires more than two turns. Both quotes must be finite and positive.
The established imminent-attack predicate must be false. That predicate uses
visible attack reach and a strength threshold; it does not exclude every
weak or unseen attacker.

The retained item uses the existing production-claim mechanism, so a later
civilian queue writer cannot silently overwrite it during the same turn.
Another unsafe city's defensive replacement still has priority. Wall
construction, wall repairs, damaged cities, imminent attackers, non-Campus
items, unknown timings and other victory targets keep the existing selector.
The underlying emergency treatment must already be armed. Host quotes are
required, so this exception does not introduce an estimated timing policy
into ordinary headless games.

## Public validation

With production code unchanged from public `origin/main`, the initial ten
selector controls gave eight passes and two expected failures: the one-turn
Library and University were replaced by a Warrior. The successful controls
covered city and wall damage, imminent attack with spent enemy movement,
unwalled cities, other victory targets, a two-turn building, a fast defender,
and missing completion quotes.

Candidate validation adds production-claim restoration, another damaged
city's priority, and invalid host-quote controls. Results and commands are
recorded in this PR and under
`~/civvis-tactics-results/2026-10-08/public-campus-finish-before-defender/`.

No candidate Civilization VI game or native Library completion has yet been
observed. A native evaluation must record the actual decider revision,
binary hash, effective genome, difficulty, player count, victory target and
operator retirement policy. Completion outcomes must distinguish retirement
from natural defeat. Recorded-history route changes and green regressions
are not a native win-rate comparison.
