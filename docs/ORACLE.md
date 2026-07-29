# Oracle ablation

`ablate` asks a deliberately stronger question than a policy comparison: if a
seat is handed a perfect, free version of one capability, does it win games it
otherwise loses? The matched gap to the same seat on the same map is an upper
bound on honest work inside that subsystem.

Two checks make a null interpretable:

- `none` must reproduce the untreated game exactly; otherwise the wrapper or
  harness is changing play.
- `treasury` must resolve as an advantage on the same profile; otherwise the
  batch has not demonstrated enough sensitivity to read another grant's null.

The number of times a grant fires is a third, separate check. A grant that
never changes the position has measured the stock agent under another name.

## 2026-07-29 preregistration: test the profile, not only the grant

Every oracle result recorded so far used `AdvancedAi` at Standard speed. The
exhibition profile is six players on 74x46 with nine city-states, 250 Online
turns, and the strongest measured controller is `strategic_deep`. Neither
transfer may be assumed: this repository has already measured both genomes and
learned move heads reversing across controller or speed profiles.

Before this entry, `ablate` could name neither dimension. This experiment adds
`--speed`, a grant-mode `--ai`, explicit agent provenance, and comma-separated
grants so multiple treatments can share one control. Both defaults remain the
historical values (`standard`, `advanced`). The best-lane oracle remains an
`AdvancedAi`-specific experiment; `--speed` applies there, but `--ai` does not.

### Hypotheses

1. The oracle's positive calibration transfers to the deployment profile:
   `treasury` helps more matched Online/Advanced cells than it hurts, fires at
   least once, and reaches two-sided McNemar `p < 0.05`.
2. Perfect territorial acquisition remains below useful resolution on that
   profile. `ground` is the focal structural grant because it is the newest
   completed Standard bound at preregistration and it acts in the
   resource-producing layer the positive calibration points toward.
3. `treasury` also remains a positive calibration when the wrapped controller
   is `strategic_deep`. This is a mechanism check for future strongest-agent
   oracle work, not evidence that free resources are a playable policy.

### Fixed screen

The primary run is fixed before implementation and uses untouched seeds:

```text
ablate --grant treasury,ground --ai advanced --pairs 12 --players 6 \
  --width 74 --height 46 --city-states 9 --turns 250 --speed online \
  --seed 993000 --jobs 4
```

That is 24 matched cells and one shared control. It is a screen, not a powered
null: `ground` advances only if it produces at least eight discordant cells and
at least a 2:1 helped:hurt direction. If it does, a fresh fixed 50-map run at
seed 994000 decides the subsystem. Otherwise the result is reported as the
profile-sized bound it is; the Standard result is not silently promoted to an
Online conclusion.

After the Online/Advanced null and treasury checks pass, the architecture
calibration is:

```text
ablate --grant treasury --ai strategic_deep --pairs 6 --players 6 \
  --width 74 --height 46 --city-states 9 --turns 250 --speed online \
  --seed 995000 --jobs 4
```

Twelve cells are sufficient only for the intentionally enormous calibration:
it passes on at least eight discordant cells, a positive direction, and
two-sided McNemar `p < 0.05`. Anything weaker is recorded as an inconclusive
calibration and no structural `strategic_deep` oracle result may be interpreted
from this batch.

No gameplay entrant is promoted by this experiment. Its output decides which
structural axis deserves a powered policy experiment and makes future oracle
claims name the controller and game speed they actually measured.

### Online/Advanced result

The fixed screen completed all 24 cells. The command block accidentally omitted
the explicit `none` arm required by the prose above, so that arm was replayed on
the same fixed profile and seed before interpreting either treatment. It was
exactly deterministic. The primary run's shared control and the replayed null
control both won 0/24 focal cells.

| grant | control won | granted won | helped / hurt / unchanged | exact p | fires | preregistered decision |
|---|---:|---:|---:|---:|---:|---|
| `none` | 0/24 | 0/24 | 0 / 0 / 24 | 1.0000 | 0 | sanity check passes |
| `treasury` | 0/24 | 17/24 | 17 / 0 / 7 | 0.000015 | 3,883 (161.8/game) | calibration passes |
| `ground` | 0/24 | 5/24 | 5 / 0 / 19 | 0.0625 | 4,028 (167.8/game) | escalation gate fails |

`treasury` demonstrates that the batch can detect a deliberately enormous
advantage on this profile. `ground` is more interesting than a mechanical null:
it fired often and all five discordant cells favored the grant. Five is still
below the fixed minimum of eight, however, so the direction is unresolved and
the 50-map follow-up is not run. This result is neither evidence of no Online
territorial headroom nor a license to work on that subsystem. It is a correctly
stopped screen.

The zero raw control wins do not invalidate the matched comparison, but they do
make its scope important. `ablate` samples only seats 0 and 5 and leaves
`GameOptions::randomize_civs` false, so those seats are always Rome and Aztec
under the stock roster. The geometry, player count, city-state count, speed and
turn budget match deployment; the civilization roster does not sample the live
randomized distribution. This implementation also uses the normal Basic
controller for city-states and barbarians while every major uses the named
controller. The older Standard oracle used an `AdvancedAi` fleet for every
player, so the two point patterns cannot isolate a speed interaction.

### Online/strategic-deep result

The architecture calibration also completed all 12 fixed cells. The committed,
embedded `best.json` champion loaded; the optional `valuenet.json` did not, so
this is the published netless `strategic_deep` configuration rather than a
fallback to `advanced`.

| grant | control won | granted won | helped / hurt / unchanged | exact p | fires | preregistered decision |
|---|---:|---:|---:|---:|---:|---|
| `treasury` | 1/12 | 9/12 | 8 / 0 / 4 | 0.0078 | 1,847 (153.9/game) | calibration passes |

This lands exactly on the minimum eight discordant cells, all in the positive
direction, and clears the fixed significance threshold. Oracle sensitivity
therefore transfers to strongest-controller self-play on this two-seat Online
profile. It licenses a future structural oracle run under `strategic_deep`; it
does not make free resources a policy, promote an entrant, or alter the failed
`ground` escalation decision above.

The 24 strategic games took about 3 hours 2 minutes wall-clock at four jobs on
this host, including a period of fleet oversubscription; the control phase alone
consumed about 412 CPU-minutes. That cost is too high for a default screen. The
tool now reports each completed control and treatment cell so a long fixed batch
is observable without inspecting worker threads, but deployment-scale
`strategic_deep` oracle work should remain narrowly preregistered.

### Decision

Both positive calibrations transfer to the deployment geometry and Online
speed, including the strongest measured controller. Perfect territorial
acquisition produced a favorable but underpowered 5/0 screen and stopped at its
fixed gate, so no territorial policy experiment is justified by this batch.
The useful advance is methodological: future oracle claims can name their
controller and speed, share one matched control across treatments, refuse a
silent agent fallback, and expose progress during expensive batches. No
gameplay behavior changes here.

## 2026-07-29 preregistration: does the expansion ceiling reach the strongest agent?

The first Expansion result is the largest structural ceiling measured in this
repository: on the Standard/Advanced four-player profile the same focal seat
won 69/300 untreated cells and 157/300 granted cells, with 144 discordant cells
and exact `p = 0`. Its interpretation needs one correction before it guides
work. `Grant::Expansion` does not isolate settler price. At the start of a
granted turn it creates a free Settler whenever the empire has one to five
cities and no Settler already walking. It therefore bypasses all of the
ordinary production decision: the population floor, expansion window,
build-time site requirement, queue competition, production cost, and population
cost. It preserves the six-city ceiling, one-at-a-time serialization, transit,
site choice, and settlement. This is an upper bound on the bundled supply and
tempo of expansion, not evidence that any one bypassed conjunct is causal.

That distinction matters after #559's census. On 6p/74x46, a missing site never
blocked the sampled shortfall turns; the expansion window alone blocked 31.2%,
while 40.4% had no hard blocker and let the Settler compete on value and price.
Those are two different honest treatments. Before spending an evaluation on
either, this experiment asks whether the large ceiling survives the controller
upgrade from `advanced` to the strongest published `strategic_deep` agent.

### Profile and harness change

The exhibition does not have one map cell: after its bootstrap world it samples
4–10 players, nine map scripts, and both Flat and Planet topology. The focal
cell here is the same high-cost 8-player Continents/Planet cell used by the
contemporaneous rush and faith studies. It is one production-relevant cell, not
an exhibition-wide estimate. With the stock 84x54 size request, Planet resolves
to the globe's 105x44 storage rectangle (4,412 playable tiles); the evaluator
must print both requested and realized geometry so that conversion is visible.

Before the run, grant mode in `ablate` gains the profile axes it cannot
currently express:

- `--map`, `--shape`, and `--poles`;
- `--randomize-civs`; and
- `--victories`.

Historical defaults remain Pangaea, Flat, poles, fixed stock civilizations, and
all victory conditions. Unknown values must fail instead of falling back. The
best-lane mode is outside this experiment and retains its existing interface.

### Fixed screen

The untouched primary seed and exact command are:

```text
ablate --grant none,expansion --ai strategic_deep --pairs 6 --players 8 \
  --width 84 --height 54 --city-states 12 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9990000 --jobs 6
```

Six maps sampled from seats 0 and 7 produce 12 matched cells. The shared control
is played once, then the exact null and Expansion treatments are played against
it. The null must change 0/12 outcomes. Expansion advances only if it fires,
produces at least six discordant cells, helps more cells than it hurts, and
reaches two-sided McNemar `p < 0.05`. With only six discordances, that requires
6/0; the significance test, rather than a raw win-rate threshold, continues to
govern larger discordant sets.

This narrow screen reuses the already completed strongest-controller positive
calibration (Treasury helped 8/12 and hurt 0/12 at `p = 0.0078` on the 6p Online
cell) instead of spending another 12 strategic games to remeasure a deliberately
enormous resource advantage. Consequently a failed Expansion screen is a stop,
not a powered negative bound on this different profile. There is no seed retry
and no threshold adjustment.

### Prospective clustered-inference amendment

At 2026-07-29 16:28 UTC, after the fixed screen had completed 10/12 control
games but before it printed a control summary or began either the null or
Expansion arm, a preflight audit identified that the two focal seat-cells on
one map share a generated world. Treating all 12 cells as mutually independent
would therefore make the screen's McNemar value anti-conservative whenever the
two seats move together. No control aggregate or treatment outcome had been
printed or inspected when this amendment was written.

The already-running screen remains byte-for-byte unchanged at frozen binary
head `8ed75b4`. Its original cell-level gate remains only a deliberately cheap
resource-allocation screen: passage can earn the disjoint confirmation, but the
screen's cell-level `p` is not itself a population-level transfer claim. Before
any confirmation, `ablate` additionally collapses each map's two seats into one
direction: helped when treatment wins more of the two seats than control, hurt
when it wins fewer, and unchanged on a tie. It reports an exact two-sided sign
test across discordant maps, restoring the independently generated map as the
inference unit. This changes no world, seed, seat, controller, treatment,
endpoint, or resource rule.

This also narrows the older calibration language. The Online/Advanced
Treasury result had 17 helped and zero hurt cells, so even worst-case pairing
places those changes on at least nine wholly positive maps (`p <= 0.0039`);
its sensitivity conclusion survives clustering. The 8/0 `strategic_deep`
Treasury aggregate can occupy only four to six positive maps, however, whose
two-sided map-level values range from 0.1250 to 0.03125. Because the old log
did not retain cell identities, that architecture calibration is now treated
as sensitivity rationale rather than confirmed map-level inference. No
playable policy was promoted from it, and a focal Expansion transfer claim now
requires the disjoint clustered confirmation below.

### Gated confirmation and decision

Passing every screen term earns one disjoint confirmation with only the focal
treatment:

```text
ablate --grant expansion --ai strategic_deep --pairs 20 --players 8 \
  --width 84 --height 54 --city-states 12 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9991000 --jobs 6
```

The confirmation supports transfer only if the grant fires, helps more cells
than it hurts, independently reaches cell-level two-sided McNemar `p < 0.05`,
helps more maps than it hurts, and independently reaches map-level two-sided
sign-test `p < 0.05`. The map-level condition is additive and cannot rescue a
failed original condition. A pass does not promote a playable agent: the grant
is cheating and deliberately bundled. It prioritizes separate preregistered
tests of the two measured honest causes—late expansion eligibility and settler
production value—without combining them. Failure stops this line on the focal
cell and is reported as either underresolved, null, harmful, or invalid
according to the failed term. Any claim about the exhibition mixture would
still require a separately fixed stratified sample across its varying player
counts, scripts, and topologies.
