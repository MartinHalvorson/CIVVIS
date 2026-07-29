# Terminal Faith opportunities

Status: **descriptive contract frozen before implementation**. This task reads
completed production saves. It starts no simulator, changes no controller, and
cannot promote gameplay.

## Observation

The latest 50 production saves available at preregistration ran from
`20260729T101619.443050Z-seed-5890025-turn-273-instance-20687` through
`20260729T161608.260324Z-seed-416573342-turn-302-instance-39983`. They are a
viewer-staged, mixed-revision sample rather than a randomized experiment.

Their 376 surviving major civilizations held mean 4,486.5 and median 3,835.0
Faith at the terminal snapshot. Of those seats, 258 held at least 2,000 Faith,
138 held at least 5,000, and 41 held at least 10,000. Non-winners averaged
4,286.2. Among the 138 seats above 5,000, 131 had reached Cold War and none had
a surviving Naturalist or Rock Band; the full sample had 18 such units.

Those balances are not an effect estimate. A terminal bank can be useful
reserve, can have no legal outlet, can be censored because another seat won
before its next turn, or can reflect a deliberately rejected purchase. The
census asks only which explanations remain possible.

## Prior causal result that this census cannot reopen

`docs/FAITH_CONVERSION.md` already tested the obvious conversion policy on the
same Science/Culture/Domination deployment profile. Removing the Culture-plan
gate produced 234 legal purchases (14 Naturalists and 220 Rock Bands), cut mean
terminal Faith from 2,770.7 to 1,029.5, and raised tourists from 29.1 to 39.8.
It changed only one of 60 matched seat outcomes, reached a 50.8% paired map win
score, and failed its preregistered 52.5% development gate. That line stopped:
no holdout and no gameplay integration.

Accordingly, Naturalist and Rock Band availability is reported only as a
replication diagnostic. No prevalence in this terminal sample may be used to
retry, tune, or revive that treatment.

## Frozen question and method

> After excluding the rejected Culture-asset policy, is terminal Faith merely
> unspendable, or do rich production seats retain a frequent legal conversion
> class that has not received its own causal test?

The command is:

```text
terminal_faith_census --dir target/spectator/results --latest 50
```

Files are selected by descending timestamped filename, not filesystem mtime.
Only `*.save.json` files are eligible. Each selected save is parsed as a
`Game`; parse failures are counted and make the command exit nonzero after the
readable files have been summarized.

For each living major, a private clone clears only the terminal `winner` and
`victory_type` fields and makes that civilization current. It does not advance
the world, change its treasury, complete a queue, restore movement, reveal a
tile, or run an AI. The clone then enumerates
`ActionFamilies::PURCHASES | ActionFamilies::EMPIRE`. This normalization asks
what the unchanged terminal position would legally offer if the game-over
guard and turn-owner guard were absent. A deterministic fixture must prove on
a live position that adding and then clearing those two guards reproduces the
same Faith-action set exactly.

If a pending city-capture disposition masks the ordinary legal-action set, the
seat is reported as capture-blocked and excluded from opportunity rates. The
census does not silently choose Keep, Raze, or Liberate on its behalf.

Faith actions are classified prospectively as:

- Naturalist or Rock Band purchases (the stopped prior treatment);
- religious-unit purchases;
- military-unit purchases;
- other-unit purchases;
- building purchases;
- district purchases;
- Great Person patronage; and
- Pantheon choice.

An action belongs only when its explicit currency is Faith, except
`ChoosePantheon`, whose legal offer is itself the engine's Faith-spending
contract. Duplicate city/formation offers count once for seat-level coverage;
the raw legal-action count is also reported. Great Person and unit subtypes are
reported so a broad label cannot hide one repeated item.

The output is fixed to include:

- selected filename bounds, readable games, parse failures, terminal victory
  mix, surviving majors, and capture-blocked seats;
- mean, median, and 90th-percentile Faith, plus counts at 2,000, 5,000, and
  10,000;
- for every class, raw offers and distinct seat coverage overall and within
  the 2,000- and 5,000-Faith groups;
- rich seats with no legal Faith action at all; and
- coverage by exact unit type and Great Person class.

## Reading and stop rule

This is a cross-sectional opportunity census, not a policy evaluation. It
cannot establish that the AI previously skipped an action, that buying would
beat waiting, or that any purchase improves wins. The terminal observation is
also right-censored by at most one table round when another seat wins.

A new causal preregistration is warranted only if one **previously untested**
class is legal for at least 10% of unblocked seats holding at least 2,000 Faith.
That threshold only licenses a separate prospective treatment with an exact
null, fixed development screen, disjoint holdout, outcome guardrails, and no
seed reuse. It does not license an AI change. Below 10%, the opportunity is too
sparse for the next shared simulator slot and this line stops.

The following do not qualify regardless of prevalence:

- Naturalists or Rock Bands, because their causal line already stopped;
- a class already consumed by an active queued experiment; or
- a category whose offer exists only after the census mutates anything beyond
  the two terminal guards described above.
