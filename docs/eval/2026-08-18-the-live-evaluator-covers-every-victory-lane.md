# The live evaluator covers every victory lane

_2026-08-18 · `2bc57db4`_

## What was asked

Can the evaluator construct every explicit `civvis_orders --victory` mode with
the same live bridge that the binary deploys, rather than modeling either an
adaptive bridge (`live`) or an untargeted lane (`advanced_target_*`) alone?
Does a bare binary invocation follow the high-level launch chain's centralized
default rather than retaining a stale local fallback?

## How it was measured

At `2bc57db4`, a single `LIVE_TARGET_LANES` table was checked against all six
`VictoryTarget::ALL` variants. For each row, the contract test constructed the
targeted controller, enabled the live bridge, verified representative bridge
repairs, resolved the corresponding `live_target_<lane>` arm, and verified
that its tag list is exactly the lane axis followed by the complete bridge
list. It also checked that the arm differs from adaptive `live` by that lane
axis alone.

The Rust binary and the three Python launchers were statically compared for
their named defaults and imports. This was a registry/configuration check: no
games, map seeds, Elo calculation, or win-rate gate ran in this round.

## What it measured

The registry now has six target-pinned bridge arms:
`live_target_science`, `live_target_culture`, `live_target_religious`,
`live_target_diplomatic`, `live_target_domination`, and `live_target_score`.
Each carries one `victory-lane-*` tag plus all 74 live-bridge treatments.
Adaptive `live` remains the `--victory civvis` configuration and has not been
redefined.

The initial structural check at `2bc57db4` aligned the direct fallback to the
then-central Science default. Before this PR merged, #1960 moved that central
default to Diplomacy on its own deployment-shaped evidence; the final
integration updates `civvis_orders` to follow that current value. The two
intermediate launchers import it rather than declaring copies. Explicit
choices, including the automated batch's explicit `civvis`, are unchanged.
There are no outcome numbers or intervals from this round because it measured
coverage, not performance.

## What was decided

Ship the evaluator/default contract. This structural result does not promote a
victory lane; #1960 owns the evidence and decision that selected Diplomacy. A
future deployment-shaped run must compare the relevant `live_target_<lane>`
arm with adaptive `live` (or another declared control), using actual maps and
seeds, before changing a launcher or deployment profile.
