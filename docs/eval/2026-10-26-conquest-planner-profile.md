# Current conquest planner CPU exploration

All **42 available fixed-board baseline calls** complete, retaining six unavailable assignments. All four preregistered sample processes and both explicitly post-selected follow-ups complete with twenty fresh repetitions each; each process requires identical complete world, actions, strategic plan and reasoning across its repetitions. All six one-second sampler calls succeed. No speed or strength improvement is claimed by this baseline.

The unmodified library source is `c871172d49375da2e635f2a4c001712a690ebc21`. Its 813 initially frozen Rust/data/Cargo/build inputs remain unchanged. Twenty additional literal compile-time includes match the original clean source commit exactly and remain unchanged; the actual depinfo’s 361 own-crate dependencies are also verified. Probe and library SHA-256 hashes, compiler identity, profile, commands and outputs are retained. The probe uses the optimized `ci` library and matching opt-level, debug-assertion and codegen settings.

The 48 assignments are the prior airfield-memo simulation boards at turns 50/100/150/200 in seeds 59002700/59002701. Forty-two full input files exist and reproduce their recorded hashes; six unavailable assignments remain unavailable. Apply the qualified Native baseline’s 501 treatment tags explicitly across the current gene registry, with forward-airfield off and unknown tags rejected. This is a synthetic model CPU workload using imported historical boards and an explicit treatment list. It does not reproduce the original simulation trajectory, stock model defaults or Native controller history.

## Baseline and selection

The single pass totals 6,055,294 process CPU ticks. These unpaired single readings are exploratory, not a comparison or a game-wide cost estimate. The selection and first four sample indices were published before execution. The initial four are seat zero at raw turns 150 and 200 in both seeds: indices 12,18,36,42.

After retaining every baseline and those four samples, a separate published follow-up plan selects the two largest single baseline readings: index 43 (turn 200, seat 1, 1,344,394 ticks) and index 20 (turn 200, seat 2, 932,749 ticks). This is post-selected exploration and supplies no independent performance estimate. No original case is replaced.

## Sample observations

The table sums sibling instruction offsets of named, nongeneric function nodes under `plan_observed_turn`, suppressing recursive ancestor duplicates. The summarizer checks all six planner-root totals against the raw traces. Generic and unresolved symbols are not fully classified. These are inclusive stack counts; columns overlap and must not be added or treated as independent time components.

| Index | Selection | Planner samples | city_yields | posted_shooters | air_resource_shortfall |
| ---: | --- | ---: | ---: | ---: | ---: |
| 12 | Preregistered | 648 | 108 | 9 | 28 |
| 18 | Preregistered | 693 | 145 | 250 | 2 |
| 36 | Preregistered | 607 | 105 | 178 | 2 |
| 42 | Preregistered | 698 | 125 | 3 | 130 |
| 43 | Post-selected | 827 | 342 | 162 | 6 |
| 20 | Post-selected | 805 | 301 | 235 | 2 |

Repeated city-yield/citizen-plan derivation appears beneath production choice in the larger boards. `src/ai.rs::BasicAi::pick_item` takes an immutable `&Game` and currently spans its production menu without one outer memo scope. The engine’s existing `Game::query_memo` is intended for such immutable candidate sweeps and clears the outer scope before a caller can change the board. This is an optimization hypothesis requiring full output equality and measured cost controls, not a proven gain.

Siege-post path searches also appear repeatedly under `assess_siege` and `posted_shooters`. Index 42 instead has substantial air-resource shortfall/readiness and attack-reach work during settlement scanning. The current shortfall helper computes readiness before some cheaper eligibility facts; reordering pure checks is another hypothesis. These observations do not justify changing siege positioning, readiness, settlement, production priorities or Native strategy.

## Validation, failure and retained evidence

The first published probe fails rust-quality because a new assertion is not rustfmt-formatted. The complete failed CI log remains retained. Formatting that assertion changes no logic; the corrected source is published before execution. The library is not rebuilt or relabeled for documentation-only probe changes. The runner captures every command, exit, stdout/stderr, PID, sample and error, and terminates only its own disposable probe if a uniform timeout occurs. No timeout or sampler failure occurs in these runs.

The summarizer requires all 48 assignments, all 42 successful baseline calls, six unavailable cases and all six successful twenty-repeat processes. All raw semantics, stack samples, timings, source/configuration identities, build receipts and the initial quality failure are sealed with per-file hashes and read times. Full original snapshot inputs are already published in `2026-10-25-airfield-slot-memo.sim-inputs.tar.xz`; their hashes and local reconstruction paths remain in the selection. This packet does not duplicate those snapshots.

No app source, live game, driver, default, ledger or trial policy changes in this profiling task.

The sealed packet has **238 byte-verified members, 6,848,372 bytes**, SHA-256 `0a0dbe9729e6ee6c12aa8852bcb7d20bfea5d29ed04adc5c6168ba7f7e327a8b`.
