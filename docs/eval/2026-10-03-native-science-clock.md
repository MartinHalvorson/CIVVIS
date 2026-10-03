# Native expedition clock in AI victory pressure

## Scope and decision

This repairs canonical `main`'s consumption of already-imported native Science
Victory Points. It is not a new denial strategy, a deployment-gene promotion,
or evidence of stronger native play. Both AI expedition calculations now use
`Game::science_victory_points` and `Game::science_victory_points_needed`, just as
the victory display already does. The existing launch gate, launch floor of
78, truncation, completion clamps, and simulator fallbacks remain unchanged.

The active experimental verification branch already corrected **rival**
distance before `civvis-20261003T040354Z`; its runtime source
`46241dd1fbcadb7a5c3ad81515e33fbb6152e2d3` contains the getter-based
`science_launch_progress`. The earlier assembly commit
`ad9e5a9bd6b58e4e49c2b83adbe90c8ec5e5e3b4` also contains that rival correction.
This change independently isolates that representation repair for canonical
`main` and corrects the **own-race** expression too. It does not explain why
that historical experimental campaign stayed on Rome or claim to prevent its
Science loss. No experimental campaign, early Science alarm, capital-handoff,
or policy-bundle changes are imported.

## Native basis and defect

Installed Gathering Storm
`DLC/Expansion2/UI/Replacements/WorldRankings_Expansion2.lua:373-374` reads:

```lua
local totalLightYears:number = g_LocalPlayer:GetStats():GetScienceVictoryPointsTotalNeeded();
local lightYears = g_LocalPlayer:GetStats():GetScienceVictoryPoints();
```

Lines 587-589 read the rival's points and per-turn rate alongside the native
target. The mirror already stores these observations separately from the
simulator-only `Player::exoplanet_distance`. That separation is intentional:
speculative simulated advancement must not alter a host measurement.
However, `lane_progress_table_uncached` and
`rival_victory_pressure_with_culture` still used the simulator field divided by
50 on canonical base `69ef4ea2f26ff257cd68e108c8a8ce5dc083b22a`.

On an observed Online expedition at 23/25, the raw simulator field can still
be zero. Canonical rival Science pressure then stays 78 rather than 98; own
Science progress stays at the four-project floor of 72 rather than 92.
Reusing the existing validated getters corrects both the numerator and the
game-speed denominator without copying native facts into speculative state.

## Regression evidence

Five tests cover rival ranking against a 90-point diplomatic clock, own-race
progress, native parsing/rebuild/sync and disappearance of observations,
simulator/invalid-observation fallbacks, and launch/completion guards. Four
fail first on the unchanged implementation; the fallback test already passes.
The native-sync regression keeps the simulator distance at zero while the
observed points change from 7/25 to 23/25, then disappear.

After repair, all five pass. Full `cargo test --profile ci --locked -j 4`:
4,459 passed, zero failed, 53 existing ignores. Seven CI-wiring tests pass;
changed-Rust quality and formatting pass. No engine rule changes, so an engine
soak is not applicable. This change does not rerun or claim a green broad
Python tooling suite; the previously documented roadmap-history failure is
outside this Rust repair.

## Recorded-state census, not a counterfactual game

`clock_census.rs` in the retained artifact directory reconstructs the last
recorded state frame of each selected turn with its matching tile boundary.
Both libraries receive the identical history and canonical 23-policy force
file. The AI is a fresh Domination controller per snapshot; it uses the actual
public `rival_pressure`, `denial_is_urgent`, and `denial_target` seams. This is
not a persistent-decider command replay or an execution in Firaxis.

For diagnosis only, a second cloned board enables Science alone so the exact
AI Science term can be read even when another lane ranks higher. It does not
drive the full-board denial decision. Every full-board census keeps the
recorded victory settings unchanged.

Source: `civvis-20261003T040354Z/events.jsonl`, SHA-256
`43a04f9a0a033f0dba0422cf465417a9382bf52d02029c89c055d1270269713e`.
The source run, active native game, its orders database, play pin, and policy
bundle are never modified.

Kongo's exact AI Science term and the full-board actionable counter:

| Turn/frame | Native points/target | Before Science term | After Science term | Counter before → after |
| --- | --- | --- | --- | --- |
| 195/2 | 0/25, Mars only | 65 | 65 | None → None |
| 198/3 | 0/25, expedition launched | 78 | 78 | Science → Science |
| 200/2 | 2/25 | 78 | 79 | Science → Science |
| 205/2 | 7/25 | 78 | 84 | Science → Science |
| 210/2 | 12/25 | 78 | 88 | Conquest → Science |
| 216/3 | 23/25 | 78 | 98 | Conquest → Science |

At 210/2 and 216/3 the strongest full-board pressure changes from Culture
78/85 to Science 88/98. The native measurements, simulator distance of zero,
and urgency labels are unchanged across all twelve rival samples. Six
snapshot decisions are retained, including both changed counter decisions.

Frozen libraries: baseline canonical claim `40a99c3c4d61ac004e48455df43dafe3fbb98785`
(production code unchanged from `69ef4ea2f`); candidate implementation
`c6d069b19` (later documentation-only updates do not change Rust).

| Artifact | SHA-256 |
| --- | --- |
| `libcivvis_baseline.rlib` | `e6c359a246fee8c96f8e11a5d36149803cc9109370f9400aa7c4b368e0e06f50` |
| `libcivvis_candidate.rlib` | `cbf69c833c4af51b1b789f4986349eced7faa428d44427943a6b125374324488` |
| `baseline-clock-census` | `3547765bab872deffc995bfc666122863c1e7dfaf7df529d4cdd48c24f3fbaed` |
| `candidate-clock-census` | `83dd7d819191630cbc306cfa988d28dcdfe652b797aedbdbda9462724789ea7b` |
| `force-on.txt` | `a79c0e54e708a92ccf8cd8b286da851fea43b643dff58cdedb8905ce74dfec3b` |

Build-provenance check caught a stale root `target/ci/libcivvis.rlib` after
`cargo test` replaced the actual dependency archive without refreshing that
root hardlink. The initial mislabeled candidate matched the baseline hash
and census. It is retained as `libcivvis_stale_root.rlib` and
`stale-root-clock-census*`, explicitly excluded from the comparison above.
The valid candidate links the completed build's actual
`target/ci/deps/libcivvis-b90f6ce0986a0c6d.rlib`, whose distinct hash is read
back before running its census. No stale-artifact null is presented as a result.

## Important policy limitation

Canonical `counter-in-lane` deliberately answers Science pressure with
Science, including for an assigned Domination controller. The existing
`score_counter_preserves_other_contracts_and_existing_pressure_gates` regression
explicitly enforces that behavior at Science pressure 97. This repair preserves
it rather than silently changing a promoted policy.

Consequently a corrected strongest-lane reading can change the chosen counter
from Conquest against Culture to Science against Science. That is a real
decision effect, **not** demonstrated tactical or win improvement. Whether an
assigned Domination army should instead answer the Science clock militarily
requires its own isolated policy trial. The experimental live branch already
has a separate military Science counter; none of its broader semantics is
backported here. Native superhuman performance remains unproven.

Raw evidence persists outside task worktrees at
`~/civvis-tactics-results/2026-10-03/native-science-clock-pr3872/`.
