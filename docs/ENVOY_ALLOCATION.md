# Conserved-stock envoy allocation census

Status: **preregistered before implementation and before any registered seed
is run or read**. This study is observer-only. It cannot change a shipped AI
policy or authorize gameplay integration.

Protocol amendment, still before any registered seed is run or read: the
initial routing bullets counted correlated focal seat-games and pooled all
checkpoints even though the paragraph below correctly named the map as the
independent unit. The frozen routing below instead pools the two focal seats
within each map and gives every map equal weight. This correction was made
from code/protocol review alone, without outcome data.

Second protocol amendment, also before any registered seed is run or read:
review removed no-immediate sends from the routing gate because a 1-to-2 setup
send or defensive margin can be optimal. That classification remains
descriptive, while only a positive conserved-stock gap may nominate allocation
work. The same review questioned the turn-320 rollover. Direct engine inspection
and a deterministic crossing test established that `do_end_turn` attempts the
score tiebreak after turn 250, but `set_winner` rejects it when score is disabled:
the unmodified world advances with `max_turns = 250`, no winner, and its normal
next-seat state. The original external turn-320 observation contract therefore
remains frozen.

## Question and correction

Perfect suzerainty may be valuable, but the current oracle cannot distinguish
earning more Envoys from allocating existing Envoys better. It writes new raw
Envoys into the focal player's table, and its Amani and turn-boundary contract
is unresolved. A later six-map print census also sampled immediately after
`advanced_envoys`, whose `while envoys_free > 0` loop spends the pool by
construction. An empty post-action pool proves that the loop ran; it does not
prove that the spent stock went to the right city-states.

This census asks two narrower questions without granting anything:

1. What immediate purpose did every stock `SendEnvoy` actually serve?
2. Holding the focal empire's realized raw Envoy stock fixed, could a different
   allocation across city-states it had met have controlled more states or
   crossed more 1/3/6 type-yield thresholds?

The hypotheses are prospectively separated:

- **allocation hypothesis:** a conserved-stock allocation can improve the
  realized control or threshold count at repeated checkpoints; send-level
  categories may explain a measured gap but cannot establish one;
- **acquisition hypothesis:** those allocation opportunities are rare, while
  the same realized stock often cannot control even one met state.

Both may be present. A descriptive result may nominate separate experiments;
it cannot combine them or declare either policy beneficial.

## Frozen controller and world population

Every major uses the exact committed `advanced_evolved` champion embedded at
`data/evolved/best.json`: generation 14, byte fingerprint
`fnv1a:40b1fbb2a5b88bc6`. The binary refuses to run if either pin changes and
requires the explicit flag
`--ai advanced_evolved`, prints the embedded generation, and rejects another
controller before simulation. This avoids both mutable live-league membership
and default-weight `AdvancedAi`, neither of which is the reproducible champion
a successful treatment would need to improve.

The study samples the unattended rollover population through the deterministic
126-profile cycle used by the other current production studies. At zero-based
map offset `i`:

- players are `[4, 6, 8, 10, 5, 7, 9][i mod 7]`;
- map scripts are `[land_only, water_world, continents, true_start_earth,
  lakes, inland_sea, pangaea, small_continents, islands][i mod 9]`; and
- topology is Flat for even `i` and Planet for odd `i`.

`MapSize::for_players` derives dimensions and city-state counts. Civilizations
are randomized. Poles, Online speed, and Science/Culture/Domination victories
are fixed. Every game retains `Game.max_turns = 250` while unchanged stateful
controllers are observed externally through turn 320 or an enabled victory.
Because score is disabled, the shipped `set_winner` gate vetoes the attempted
turn-limit tiebreak and the normal rollover proceeds; no cap or world transition
is edited. The binary asserts that the policy-visible horizon never changes.

Each map uses focal seats zero and the final major. Other seats keep the same
committed champion controller. The map, not the two seats or repeated turns,
is the independent reporting unit. Player-count, script, and topology summaries
are descriptive only.

## Exact action-boundary observer

For a focal turn, the observer clones the current `Game` and focal controller,
runs the complete champion turn, retains the resulting controller state, and
replays every successful action in order except the deferred final `EndTurn`.
Immediately before and after each successful `SendEnvoy`, it records:

- whether the target was met and legally observable before the action;
- raw and effective target counts, free Envoys, and the best rival's effective
  count;
- each crossed 1/3/6 type-yield threshold;
- whether focal suzerainty was acquired;
- whether an already-held suzerainty merely received a larger margin; and
- whether the send changed neither a threshold nor present control.

The observer never feeds a diagnostic into a controller decision. Sends to an
unmet target are reported separately and nominate a Civ VI legality/hidden-state
correction; they cannot count as evidence for a stronger allocation strategy.

At the end of every focal turn with at least one met, living, non-belligerent
city-state, the observer snapshots actual allocation and the conserved-stock
bound below. Seat-game summaries retain counts and checkpoint coverage so a
long-lived empire cannot masquerade as many independent observations. Routing
first pools both focal seats within a map, then averages map-level shares so a
long game or one seat cannot dominate the decision.

## Frozen conserved-stock bound

The resource budget is the focal player's realized, nonnegative **raw Envoy
stock already stored in its envoy table** at that checkpoint. Raw placements
on unmet states remain in the budget, but the counterfactual may allocate only
to currently met, living, non-belligerent city-states. This makes hidden sends
recoverable as allocation waste without inventing new Envoys. `envoys_free` is
reported separately and is not converted into raw stock because a future send
can create a nonlinear raw amount through Diplomatic League or a
different-government bonus.

For every eligible city-state and every raw assignment from zero through the
fixed budget, the optimizer clones only the focal raw envoy table and asks the
engine for the resulting effective `envoys_at` and `suzerain_of`. This preserves
Messenger, Puppeteer, rivals, and the strict-lead rule without reimplementing
them. It then uses exact dynamic programming to compute, independently:

- the maximum number of eligible suzerainties achievable with no more than the
  realized raw stock; and
- the maximum total number of achieved 1/3/6 type-yield thresholds with the
  same stock.

The actual counts use the same eligible set and engine methods. The bound may
redistribute past placements and is therefore a hindsight ceiling, not a
playable treatment. It conserves raw stock exactly and never interprets an
effective-count difference as a number of sends.

## Exact null

Before reading the census seed, four deployment-cycle maps at seed `10032999`
compare direct champion play with the replay observer disabled. For both focal
seats, all eight matched cells must reproduce the terminal result, serialized
`Game`, focal plan report, and lifetime strategy census exactly or the study
stops. The comparison includes crossing turn 250 and continuing to the frozen
external observation boundary.

```text
envoy_allocation_census --null --deployment-mix --maps 4 --turns 250 \
  --observe-through 320 --speed online --poles poles --randomize-civs \
  --victories science,culture,domination --ai advanced_evolved \
  --seed 10032999 --jobs 6
```

Only that exact profile may print the registered null PASS. Diagnostic runs
use disjoint seeds and cannot print a registered decision.

## Fixed census and prospective routing

After a clean null, the only registered census is 30 maps / 60 focal seat-games
at seed `10033000`:

```text
envoy_allocation_census --deployment-mix --maps 30 --turns 250 \
  --observe-through 320 --speed online --poles poles --randomize-civs \
  --victories science,culture,domination --ai advanced_evolved \
  --seed 10033000 --jobs 6
```

The result prints one of four prospectively defined routes. These are
experiment-nomination rules, never gameplay promotion gates:

- **ALLOCATION LEAD** when at least 10 of 30 maps contain a focal seat with a
  positive conserved-stock suzerainty or threshold gap at five or more
  checkpoints;
- **ACQUISITION LEAD** when the allocation condition is false, at least 20 of
  30 maps have eligible checkpoints, and the equal-weighted mean of each
  eligible map's fraction of checkpoints with a conserved-stock maximum of
  zero suzerainties is at least 25%;
- **MIXED** when both complete Boolean signals above hold: the 10-map
  allocation-gap condition and the 20-map/25% acquisition condition; and
- **NO MECHANISM** otherwise.

Any send to an unmet city-state independently prints **HIDDEN-STATE LEGALITY
BUG** and permits only a separate rules/visibility correction with deterministic
tests. It cannot promote an economic treatment.

No-immediate sends, secure extensions, thresholds, acquisitions, and raw versus
effective increments remain descriptive diagnostics. None can independently
nominate a treatment because setup and defensive-margin sends may be rational
even when they do not change control on that action.

An allocation lead permits a later preregistered, fog-honest treatment that
changes only future sends and never reallocates history. An acquisition lead
permits a separate study of Envoy-earning choices. Neither may change gameplay
from this census, and neither may use seed `10033000` to tune a policy.

## Validation and resource order

Before the exact null, focused tests must prove:

- all 126 deployment profiles are unique and the registered batches have the
  frozen marginal counts;
- official labels require the exact champion and profile;
- action replay preserves the complete stock world plus the controller's plan
  report and lifetime strategy census;
- met/unmet, threshold, acquisition, secure-extension, and no-immediate-effect
  classifications use true before/after states;
- fixed-stock optimization never exceeds the supplied raw budget, accounts for
  Amani through engine methods, and beats deliberately wasteful fixtures while
  tying already-optimal ones; and
- every routing rule rejects a missing conjunct.

The null and census use at most six jobs and remain behind the active Strategic
Expansion oracle and every older registered batch. No registered seed may run
while another process owns six or more simulator cores. Generated output is
never committed.
