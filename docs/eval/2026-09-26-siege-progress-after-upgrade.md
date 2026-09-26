# Siege progress after wall upgrades

## Recorded failure and diagnosis

Historical King game `civvis-20260926T190901Z` offered Norway peace on turn167
because the war had stalled, while Kristiansand's walls fell on six consecutive
turns: 400 → 377 → 352 → 327 → 304 → 281 → 256 (turns161–167).
City health stayed200. Earlier, the city's wall capacity increased from300 to400.
The air campaign stood down after the host reported peace.

A persistent-agent, fresh-board replay of all476 observed frames through167
reproduces the Norway peace offer with the current policy. It reads archived
host states; its decisions do not create counterfactual future states or prove
what would have happened had the war continued. Diagnostics will inspect the
remembered milestone scale and the plan's target/home-defense gates before a
policy correction is selected.

The source event archive SHA-256 is
`2751e3a479dcc8e470bf347f4b1cd4721e3a0d741be55093155700816cdad94a`.
Full replay artifacts are retained at
`~/civvis-war-evidence-20260926/peace-replay-190901/` and instrumented diagnostics
at `~/civvis-war-evidence-20260926/siege-progress-replay/`.

## Preregistered validation

Before changing siege policy, freeze the evaluator with the explicit difficulty
option and register four seeds **37860000–37860003**. Compare identical seed and
policy-toggle arms across baseline/candidate source versions, retaining all games.
The user now requests **level4 Prince**, Simon Bolivar / Gran Colombia, Tiny
Pangaea, Domination. Historical King evidence retains its original label.

Both evaluator versions will run:

```sh
victory_eval --domination-pair early-conquest-opening --difficulty prince --games 4 --start-seed 37860000 --out <fresh-file>
```

Both player and barbarian difficulty must read back as Prince. The other profile
settings remain four players, 60×38 Pangaea, six city-states, Online speed,
barbarians, all victory conditions, the natural250-turn limit, and the recorded
compiled live-policy bundle. The CLI's omitted-difficulty behavior preserves the
historical King-player/Emperor-barbarian profile; explicit selections set both.

This is a four-seed simulator diagnostic. Rivals are CIVVIS controllers rather
than Firaxis AI, and all controllers use their respective source version.
Duplicate toggle legs are not independent samples. Record wins, domination wins,
loss type, ending turn, score, observed foreign major cities and original foreign
capitals. No stopping early or selecting only favorable seeds. This pilot cannot
establish a native win rate or justify difficulty progression.

Require a focused regression to fail before the correction, rerun the exact
observed-history replay, preserve existing bounded-fatigue tests, run the full
Rust suite and evaluator profile tests, and check changed-file quality before
shipping. No engine mechanics or live runtime settings are changed here.


## Diagnostic result

The instrumented baseline completed all476 frames. On turn167, the stored
`initial_maximum` was500, current maximum600, actual remaining health456,
`greatest_quarter`0 and `progressed_at` absent. The existing formula used
`(500 - 456) * 4 / 500`, so144 actual damage after the upgraded full-health
observation counted as44. Our army was present. The fatigue gate read Conquest,
target Norway, city `(23,23)`, no threatened city, and an active war.

The defect is the damage reference after an observed health increase, rather
than a missing army, target switch, home-defense condition or an insufficient
fixed milestone budget. A quarter of the original500HP budget is125; the
observed144HP drop exceeds that without lowering the threshold. Temporary
logging was removed after freezing its binary and patch in the evidence folder.
