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
