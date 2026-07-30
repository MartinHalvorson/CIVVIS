# Oracle ablation

Status of the 2026-07-29 `strategic_deep` Expansion transfer study:
**interrupted and invalid; no retry; confirmation unopened**.

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

`--grant all` is the safe capability set, including its exact `none` sanity
arm. It deliberately excludes `idle_reserve`. That intervention confiscates
Gold rather than granting a capability, so its sign reads in reverse and it
must be requested explicitly as `--grant idle_reserve`. When confiscation wins
more matched outcomes, deleting the reserve helps; when it loses more, the
reserve is valuable. The evaluator labels those directions directly instead of
calling either one generic “headroom.”

The evaluator also treats a nonsignificant result as unresolved, not as proof
of a null. A powered null requires a preregistered sensitivity argument outside
the generic tool output. This applies equally to capability grants and the
best-lane oracle: a run that does not resolve a direction cannot establish that
perfecting a subsystem or choosing the winning lane is worthless.

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

The same reporting correction applies to best-lane mode, which uses the same
two-seat-per-map layout. Its published 25 favorable / 1 adverse cell result is
still directionally robust under every possible within-map pairing, but future
runs print the exact map-level sign test rather than asking readers to recover
that bound from aggregate cells.

#### Sensitivity audit of retained aggregate results

The old logs retained helped, hurt, and unchanged cell totals but not which two
cells shared a map. A finite sensitivity audit can still enumerate every
two-cell partition consistent with those totals. In the table below, the
“possible map `p`” interval spans all such partitions; it is not a confidence
interval and does not recover the missing identities.

| result | helped / hurt cells | possible map `p` | conclusion after clustering audit |
|---|---:|---:|---|
| Modernity, seed 420000 | 16 / 16 | 0.1516–1.0000 | null survives every partition |
| Taker, seed 420000 | 15 / 13 | 0.1153–1.0000 | null survives every partition |
| Attrition, seed 420000 | 11 / 14 | 0.0963–1.0000 | null survives every partition |
| Treasury, seed 420000 | 62 / 0 | <0.000001 throughout | positive calibration survives |
| best lane, seed 420000 | 25 / 1 | <0.0019 throughout | routing ceiling survives for the fallback agent |
| Expansion, seed 450000 | 31 / 9 | <0.000001–0.2296 | first screen alone is indeterminate |
| Expansion, seed 460000 | 116 / 28 | <0.000001–0.0017 | disjoint confirmation survives every partition |
| Online/Advanced Treasury | 17 / 0 | 0.0005–0.0039 | calibration survives |
| Online/Advanced Ground | 5 / 0 | 0.0625–0.2500 | stopped screen remains stopped |
| Online/`strategic_deep` Treasury | 8 / 0 | 0.03125–0.1250 | architecture calibration is indeterminate |
| IdleReserve, seed 450000 | 6 / 12 | 0.03125–1.0000 | aggregate cannot distinguish null from harm |
| IdleReserve, seed 460000 | 41 / 27 | 0.00013–1.0000 | aggregate can support either map direction |

This preserves the load-bearing Expansion finding because its 150-map
confirmation is positive under every admissible clustering, even though the
smaller first screen is not. It also preserves the military nulls: their most
favorable possible map partitions still miss 0.05. Conversely, the merged
IdleReserve label cannot be recovered from its aggregate cells. Its directions
reverse across seeds at the cell level, but without map identities neither run
supports a map-level null or effect claim. Treat that axis as unresolved unless
the original per-cell records are recovered or the preregistered comparison is
replayed with map-cluster output.

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

### Interrupted screen: invalid, no retry

The exact seed-9,990,000 command began at 2026-07-29 10:38:41 UTC from the
frozen release binary at `8ed75b4`. It completed all 12 shared controls and
printed a control aggregate of 0/12 focal wins. It then completed only 6/12
`none` replay cells before the process was deliberately terminated at
2026-07-29 21:20:15 UTC in response to an operator halt, after about 10 hours
41 minutes. The log ended at `none progress 6/12`.

No `none` aggregate, exact-null verdict, Expansion cell, Expansion aggregate,
or decision label was produced. The partial control value is not a treatment
comparison and cannot answer either frozen hypothesis. In particular, replay
identity was not established across all 12 cells, and the focal intervention
was never reached.

The protocol forbids a seed retry or replacement. Therefore this study is
permanently **INVALID / STOP** rather than null, harmful, favorable, or
inconclusive. Seed 9,991,000 and its entire confirmation range remain unopened.
No gameplay change or follow-up Expansion experiment is licensed by this
interrupted run. The evaluator improvements and historical clustering audit
remain reusable methodology, but they carry no new Expansion result.
## 2026-07-30 — a resource-matched control for suzerainty, and why its budget cannot be matched

`Grant::Suzerain` measured 56.7% against a 22.7% control (p=0.0000, 400 maps,
two disjoint seeds, 150 map-directions to 18) — the largest headroom this
harness has found, larger than `expansion`. Every proposed cause for it has now
been eliminated, and the claim that survived does not follow from its evidence.

### The hole

`envoy_allocation_census` (#608, re-run on the shipping agent in #620) found the
free-envoy pool empty at **every** sample at both map scales and concluded
*"allocation is already perfect; the gap is income."*

An always-empty pool is evidence of **no slack**, not of good targeting. A
policy that spreads one envoy per city-state and a policy that concentrates
three into a suzerainty are indistinguishable when there is never more than one
envoy in hand. Allocation quality cannot be measured from a resource that is
never available to allocate.

The income reading no longer fits either. #624's census over 463 archived live
games shows the deployed population reaching **Ideology 88.1%**, **Gunboat
Diplomacy 64.4%** and a **Diplomatic Quarter 66.1%** — the sources #612/#620
nominated — while still holding **11.2%** of the city-states it meets against a
mean shortfall of **24.4** envoys. The six-game censuses that named those causes
were not representative of the population that plays.

### The control

`Grant::Envoys` hands over the envoys `Suzerain` would have created, into the
free pool `advanced_envoys` spends from, and lets the agent place them. It is
`Grant::Rebate` on the expansion axis — the control that proved that axis was
tempo rather than money — applied here.

Budget, not rate. A suzerainty switches its own trigger off; free envoys switch
nothing off, because the agent may spend them anywhere. A per-city-state ledger
tracks the **target level** reached and tops up only to the largest ever
required. Two earlier versions of that ledger were wrong and both were caught by
measuring rather than reasoning:

| ledger keyed on | raw envoys/game | against `suzerain`'s 54.5 |
|---|---|---|
| the deficit seen | 34.5 | 37% under |
| the deficit, first payment uncredited | 32.8 | over-pays where the seat already held envoys |
| **the target level, `max(paid, held)`** | **32.8** | correct per event — see below |

### ★★ The budgets cannot be matched, and that is the finding

3 pairs, 6 players, 74×46, Online, 250 turns, seed 999003:

| arm | fired/game | raw envoys/game | **per firing** |
|---|---|---|---|
| `suzerain` | 18.0 | 54.5 | **3.03** |
| `envoys` | 11.0 | 32.8 | **2.98** |

The two arms pay the **same amount each time they pay**. The entire aggregate
gap is in how often the gate re-opens — and it re-opens more for `Suzerain`
because a seat that actually *holds* every suzerainty provokes rivals into
pouring envoys back in, which raises the target and asks again. A seat merely
handed envoys does not provoke that response.

So `want` is a function of the arm. **No online rule can match the totals**, and
forcing it would be tuning to a number rather than a construction. This is
#584's "a shared condition is not a shared schedule" in a sharper form: there,
the schedule diverged because the treatment switched its own trigger off; here it
diverges because the treatment changes what the *opponents* do.

### Preregistered decision rule, fixed before the run

`ablate --grant none,suzerain,envoys --pairs 150 --players 4 --turns 500
--seed 460000` — #602's exact confirming configuration, so `suzerain` and the
shared `none` control reproduce known values inside the same batch.

Read `raw envoys granted` on both arms **before** the win rate. `Envoys` is a
**conservative** control at ~60% of the resource, which makes the outcomes
asymmetric:

| outcome | reading |
|---|---|
| `envoys` comparable to `suzerain` | **strong**: matching the full oracle on 60% of the budget means the gap is income, the agent converts envoys fine, and the work goes to `envoys_per_threshold`, the diplomatic buildings and the cards |
| `envoys` null while `suzerain` reproduces | **ambiguous** between "the agent cannot convert envoys even when handed them" and "it was underpaid by 40%". Named follow-up: a deliberately generous arm — not a conclusion |
| partial | a bundle; price future treatments against the residual, not against +34 |
| `suzerain` fails to reproduce | harness or seed moved; nothing in the batch is readable |

`Grant::None` must reproduce the control exactly, as it did in the fires-check
(`SANITY OK`, 0 discordant of 4).

### Instrument change

`ablate` now reports `raw envoys granted` per arm. A firing *count* cannot
express a budget: two grants can fire different numbers of times while moving
the same resource, or fire equally while moving very different amounts. Without
the quantity printed, the arms above would have looked matched at 18.0 against
11.0 firings and the 40% shortfall would have been invisible. **A control whose
budget is never printed is not a control.**
