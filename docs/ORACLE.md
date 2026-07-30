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
