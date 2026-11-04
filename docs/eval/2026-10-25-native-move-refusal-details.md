# Retain native reasons for stalled movement

The fixed eight-game material cohort has 11,552 movement no-ops classified
`cannot_start`. This classification is intentionally conservative: it does not
prove a mountain or permanent path obstruction. The current event retains
activity and native path facts but omits the host's operation failure text.
The locator retains all eight games and every reason count, not only selected
military examples. Full source events remain in #4120's cohort archives.

Reuse the existing exact-parameter `refusalReason` helper. Immediate refused
MOVE_TO events gain `operation_failure`; delayed `cannot_start` no-ops gain
`native.operation_failure`. Keep existing classifiers, fallback calls and
movement outcomes unchanged. Probe only failure paths, with the exact unit,
operation and destination. Existing helper text retains localized native
reasons, the answered call form, empty results and thrown-overload provenance.
A localized message is diagnostic evidence and never becomes a terrain fact.

The existing helper uses the parameterized hash-operation form and alternate
parameterless form. Installed `Base/Assets/UI/Panels/UnitPanel.lua:630` supplies
the FOUND_CITY result-form example. This change does not invent another API
signature, suppress native permission checks or claim that a refusal is fixed.
The helper stays nested to preserve the main chunk's Lua register budget; the
order handler exposes that same function for its delayed movement observer.

The new executable payload controls fail on unchanged production for the
missing reasons, then pass on the candidate. They verify the exact unit/target,
localized reasons and call-form provenance, alternate form, empty results,
thrown probes, unchanged classifier/fallback/outcome, and zero extra reason
queries for a different classifier. The final harness is also run against the
frozen unchanged Lua source and fails on that same missing field.

All 103 discovered Lua control suites pass in separate Lua 5.1 runtimes through
Lupa, including the complete host-board, queued-order, combat and register-budget
controls. The install and CI-wiring suites pass all 146 Python tests. No Rust
or planner sources change. Native operation reasons and runtime cost still need
delivery readback on a future game; these mocked controls are not native
execution, a movement improvement or a speed result.

The 115-member validation archive is 340,004 bytes,
SHA-256 `f2ccfc98a69a7196983b41e47a9803ac0be8bdf9deff52740ca872f6bd09ea0e`. Every member is read back against its
source, including both parent failures, all 103 per-suite logs, complete parent
and candidate Lua sources, focused controls, runner, receipts and locator.
