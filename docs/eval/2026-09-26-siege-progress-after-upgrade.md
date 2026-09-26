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


## Correction and completed checks

Store a damage reference that rises only when actual remaining health exceeds
it, while keeping the original milestone budget fixed. Initial missing health
still consumes its thresholds; a capacity increase alone grants no progress.
Cap credit at four quarters, retain spent milestones through repairs, and leave
the12-turn expiry and army/target/home-defense/outmatched-war guards intact.

Both new regressions failed before the correction; all12 milestone tests pass
afterward, including all10 existing safeguards. The full
`cargo test --profile ci --locked` run passed **4,330 tests**, zero failures,
with53 existing ignores. All9 evaluator profile tests and7 documentation command
tests passed. Changed-file Rust quality reports formatted, warning-free code.

Both history replays completed all476 frames and consumed byte-identical
snapshots (SHA-256
`a59204976c95771a1b435d803226328083772432a7edb29515f118d20cf4a9df`).
The baseline issues `MAKE_PEACE` to Egypt and Norway on167/frame0; the candidate
issues only Egypt's other-front peace order. Norway's stalled-war offer is gone.
The later archived host state already contains peace, so it still shuts down the
air surge. This demonstrates the corrected decision, not a counterfactual native
capture, survival, or win.


## Complete Prince pilot results

All four registered seeds and both toggle arms completed in each source version.
Every reported outcome below is unchanged between baseline and candidate. Neither
version won a game or held an original foreign capital. Seed37860002 observed
one foreign major city; its identical toggle legs do not count as two separate
successes. The pilot provides no evidence of an overall win-rate improvement.
It complements the specific native replay and regressions; it does not establish
that these seeds exercised the repaired wall-upgrade case.

| Seed | Toggle arm | Loss type | End turn | Focal score | Observed foreign major cities | Original foreign capitals |
| --- | --- | --- | --- | --- | --- | --- |
| 37860000 | off | culture | 198 | 552 | 0 | 0 |
| 37860000 | on | science | 217 | 359 | 0 | 0 |
| 37860001 | off | science | 229 | 567 | 0 | 0 |
| 37860001 | on | diplomatic | 205 | 454 | 0 | 0 |
| 37860002 | off | culture | 185 | 653 | 1 | 0 |
| 37860002 | on | culture | 185 | 653 | 1 | 0 |
| 37860003 | off | science | 241 | 973 | 0 | 0 |
| 37860003 | on | science | 241 | 973 | 0 | 0 |

The early-conquest toggle changed action histories on seeds37860000 and37860001,
and was identical on the other two seeds. All matching arms across source
versions are retained above; these are four paired seeds, not eight independent
samples. Complete profiles, raw outputs and comparison code are in
`pilot-before.jsonl`, `pilot-after.jsonl`, `pilot-provenance.json`,
`analyze-pilot.py`, and `pilot-comparison.json` in the evidence directory.

## Frozen sources and binaries

The preregistration and explicit-difficulty harness are committed before the
policy correction at `9dda6319d413cb11bc5fdda55dfa487a6c1f4c11`. The two new tests
then fail at `2291f778e`; the corrected source is
`bfdec9fc10dff4a91b59274812678abba757f2da`. Evaluators and clean orders binaries
were built with:

```sh
cargo build --profile ci --locked --features developer-tools --bin victory_eval --bin civvis_orders
```

| Binary | Baseline SHA-256 | Candidate SHA-256 |
| --- | --- | --- |
| `civvis_orders` | `60e5fffad95a0ff025606fa3df48792d9b53d974ae8d4e4df030f5ef2a215133` | `b9bc82abaf2f422d34ddd2df9fc292ec78432d5352a0620e4b406ce8ad0bb050` |
| `victory_eval` | `cc92f282442138bb820cd6b213b9a896fdc9ca47bbeeb639f131e6062552082e` | `4ae5a09b93e210898ed99caec13cb5f20c5afcc53bc86732c55f03014135642a` |

The diagnostic history uses the same baseline policy at claim `2f64a4ada` plus
local logging only; its exact patch, binary hash and arguments are retained in
`diagnostic.patch` and `diagnostic-history/provenance.json`. No diagnostic logging
is present in the shipped source. The candidate replay's provenance and
`replay-comparison.json` retain its exact source, arguments and changed peace
orders. None of these observations establishes a new native victory or permits
a difficulty promotion.
