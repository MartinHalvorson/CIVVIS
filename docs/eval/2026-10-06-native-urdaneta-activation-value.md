# Urdaneta activation value in native Gran Colombia games

In Emperor run `civvis-20261006T192235Z`, all eight observed Comandantes were
consumed by verified activation within at most two turns of their first
observation. This is not evidence that every retirement was wrong: their
rewards differ, and several provide useful permanent bonuses or units.

Urdaneta is a narrower witness. At 194f1 he stood at (38,18) while a queued
walk was still completing. The native `gp` activation event records his actual
retirement at (35,17), 214ms after that snapshot. At that retirement plot the
only other observed friendly land unit within two tiles was Rocket Artillery
13565971 at (33,17), with all four movement points and one attack remaining.
There was no wounded adjacent Llanero. No event for that nearby Artillery
appears between the preceding snapshot and activation. This supports a wasted
local reset; it does not provide an activation-time per-unit movement ledger
or prove what preserving him would have done to the outcome.

Primary shipped sources:

- `DLC/GranColombia_Maya/Data/GranColombia_Maya_GreatPeople.xml`:
  `GREAT_PERSON_INDIVIDUAL_COMMANDANTE_RESET_UNIT_MOVES` uses
  `MODIFIER_PLAYER_UNITS_RESET_MOVES`, `SubjectRequirementSetId="AOE_LAND_REQUIREMENTS"`,
  `RunOnce="true"`, `Permanent="true"`.
- `DLC/GranColombia_Maya/Text/en_US/GranColombia_Maya_Units_Text.xml:135`:
  “All land combat units within 2 tiles regain all [ICON_Movement] Movement and attack capability.”
- Loaded native `DebugGameplay.sqlite`, `RequirementArguments` for
  `AOE_REQUIRES_OWNER_ADJACENCY`: `MinDistance=0`, `MaxDistance=2`.
- The separate `COMMANDANTE_HEAL_LLANERO_ON_RETIRE` modifier adjusts damage by
  -100 for `ADJACENT_LLANERO_UNIT_REQUIREMENTS`. This benefit must also count.
- The birth modifier grants `ABILITY_COMANDANTE_AOE_STRENGTH` through the same
  land/owner proximity requirement, with `COMANDANTE_AOE_STRENGTH Amount=5`.

The candidate restricts only Urdaneta. Retirement is useful near a friendly
land unit with spent movement, a combat unit with no attack remaining, or a
wounded Llanero. It excludes the Comandante's own movement as a justification
for consuming him. Native API uncertainty retains the existing behavior.
It filters exported activation targets and activation availability, checks
explicit activation requests at execution time, and applies the same rule in
the fallback driver. The fallback previously ran even after explicit bridge
orders, so changing the Rust order choice alone would not preserve a unit.
Movement retains the existing native highlighted-target and pathfinding gates.
It does not reserve every Comandante or add a general aura escort strategy.

The isolated snapshot probe reads all 44 native unit rows at 194f1. It refuses
to consume Urdaneta at the actual native retirement plot (35,17) and recognizes
the partially spent Spec Ops at (33,19) as a useful recipient elsewhere.
This is a predicate probe on frozen observations, not a whole-AI fidelity
replay or a candidate action executed in Civilization VI. A native trial and
win-rate comparison remain outstanding.

Validation: nineteen focused checks pass, including both activation paths,
spent movement/attacks, Llanero healing, domain/range boundaries, own-movement
exclusion, unknown API compatibility, unrelated-person control, and useful
movement/export targets. The initial thirteen-check gate on the original
controller had five passing controls and eight expected failures. All 82
Lua 5.1 suites pass; all 89 mod scripts compile under Lua 5.1. The agent uses
182 of the 200 top-level local slots. No Rust or simulated game rules change,
so Rust tests and headless soak games do not validate this native controller
policy and were not repeated for it.

Artifacts: `~/civvis-tactics-results/2026-10-06/native-urdaneta-activation-value/`
and the earlier `native-great-person-recruitment/next-urdaneta-retirement-audit.json`.
