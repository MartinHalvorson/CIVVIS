# Native strategic income and bomber sustainment

The host exported strategic stockpiles but not their gross income. Consequently,
`air_surge_bomber_goal` priced bomber sustainment against a reconstructed map,
including only the city-state resources and improvements that reconstruction knew.
A stocked resource is not proof of continuing supply, and lowering the stockpile
bridge threshold would not repair the missing observation.

In `civvis-20260920T151707Z` (brain `280b3b7`), the campaign stood down at turn 141
with "no Aluminum for wing", while the host stockpile was 25 and Quito's first
Bomber was at 264/280 production. Stock increased from 20 at turn 126 to 24 at 127;
that does **not** establish income at turn 141. The old trace cannot establish
whether this particular stand-down was erroneous because it lacks this field.
The model's Online duration is 66%, so its zero-income two-bomber bridge costs
30 Aluminum (2 training + 2 per turn for 14 turns), exceeding that stockpile.
No threshold or victory setting is changed here.

## Host contract

Primary source: the installed Gathering Storm script
`Civ6.app/Contents/Assets/DLC/Expansion2/UI/Replacements/TopPanel_Expansion2.lua`:

- Lines 50–52 read `GetResourceAccumulationPerTurn(resource.ResourceType)`,
  `GetResourceImportPerTurn(resource.ResourceType)`, and
  `GetBonusResourcePerTurn(resource.ResourceType)`.
- Line 64: `local totalAccumulationPerTurn:number = accumulationPerTurn + importPerTurn + bonusPerTurn;`
- Lines 53–55 read unit and power demand separately.

The export therefore records **gross** income, never net stockpile changes.
Zero is an observation. A failed/unsupported API leaves that resource absent;
missing fields in older logs keep the existing modeled estimate. Resource type,
not database hash/index, is passed to these methods, matching the shipped script.

## Implementation

The bridge translates `strategic_resource_income` for the local host seat into
model player zero. At the end of each complete board rebuild or sync, it removes
its previous correction, measures modeled income, and stores host minus model.
This makes the current board agree with the host while preserving the income
delta of a hypothetical mine or policy on a planning clone. It avoids overriding
other players and does not add the same income twice on repeated frames.
Invalid/unknown entries are reported and retain modeled behavior.

The adjustment map round-trips through Game serialization with a default for old
saves. Simulated games leave it empty and retain the original rate calculation.
The existing bomber planner still subtracts other military consumers itself;
stockpiles, maintenance, and bomber thresholds are unchanged.

## Validation

- `cargo test --profile ci --locked`: 3,767 library and 205 binary/integration
  tests passed; 49 library and four documentation tests ignored.
- Five new Rust tests cover a sustainable four-bomber wing with no visible
  deposit, existing fighter demand, explicit zero versus a modeled deposit,
  repeat calibration, legacy fallback, counterfactual gains, serialization,
  invalid inputs, and full mirror construction/sync.
- All 52 control-mod `*_test.lua` files passed using `lupa.lua51.LuaRuntime`;
  the full controller also compiled with Lua 5.1. The new test executes the
  production export, including imported/bonus income and partial API failure.
- `cargo fmt --all -- --check`, `git diff --check origin/main...`, and all
  14 treatment append-point tests passed.
- `target/ci/civvis soak --players 4 --games 8 --start-seed 363800 --turns 180 --jobs 4`:
  8/8 games completed. This is a simulation crash check, not native victory evidence.

No native
capture, completed bomber campaign, or Domination victory is claimed by this
observation repair. The runtime owner adopts the exporter and brain together at
a normal fresh-game boundary; the running verification game is not restarted.
