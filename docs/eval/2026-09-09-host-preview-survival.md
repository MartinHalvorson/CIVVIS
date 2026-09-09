# Selected strike survival against the host preview

Run `civvis-20260909T010354Z`, turn 140: embarked Scout 2359303 approached
31,43 and attacked the 2 HP barbarian Scout 6225962 at 31,42. The host
preview reported 1 damage dealt and 100 taken. Actual combat did exactly
that and killed our 100 HP scout. On the reconstructed observed board,
CIVVIS instead computed a 0-versus-0 strength exchange, killed the enemy,
and left our scout at 73 HP. Both the default and selected
`doomed-blow-veto-2` native replays ordered the fatal attack.

The native/host discrepancy is confirmed; its arithmetic cause is not.
This change does not invent a new native damage rule. When either existing
`doomed-blow-veto` version is selected, the bridge includes one
`combat_policy / DOOMED_BLOW_VETO` directive in a batch containing melee or
ranged unit strikes. It uses the existing order transport. The controller
consumes that metadata and attaches the policy to the attack rows, including
rows queued behind approach moves. Before requesting a strike it consults
the current host preview and refuses a blow whose predicted incoming damage
is at least the unit's remaining HP. Unavailable previews or HP readings
leave the native decision in place. No host policy persists between batches.

The host API is the shipped `Base/Assets/UI/Panels/UnitPanel.lua:3924`:
`m_combatResults = CombatManager.SimulateAttackInto( attacker, eCombatType, locX, locY );`
Lines 1773–1776 read `pAttacker:GetDamage()`, `pAttacker:GetMaxDamage()`, and
`m_combatResults[CombatResultParameters.ATTACKER][CombatResultParameters.DAMAGE_TO]`
to display the attacker's resulting health. The guard uses those same readings.

The controller acknowledges consumption with `combat_policy_applied`; the
bridge verifies that event rather than assuming the policy arrived.
A refusal emits `strike_survival_refused` with unit, target, HP and preview,
and the ordinary refusal reason `lethal_host_preview`, which the bridge
postcondition checker also reads. It does not request
combat or record a strike as issued. Unit rows keep their ordinary `ATTACK`
or `RANGE_ATTACK` verbs. Unselected behavior and gene defaults are unchanged.

## Validation

The new Lua controls first failed on the lethal opening strike, wounded HP
threshold, and a queued strike after its approach. After the repair, all
controls pass: lethal and surviving attacks, unavailable preview, current and
maximum HP, delayed issue, ordinary ranged strike, and policy isolation across
batches. All 38 discovered controller Lua test files pass under Lua 5.1
(via the local lupa runtime). The first broad run also exposed an order-list
identity regression; consuming the metadata in the original list preserves
the controller's inserted escort rows and restores those tests.

The existing full bridge decision test now also verifies that the emitted
JSON carries the selected policy exactly once. All 156 bridge tests pass,
including host acknowledgment, stale acknowledgment and refusal attribution
controls. After merging current main, the full Rust suite passes: 3,237 tests, zero
failures, 50 ignored.

The historical t140 replay uses one binary for default, v1 and v2. All three
native outputs still propose Scout 2359303's ATTACK at 31,42, retaining the
known model mismatch. The outputs pass unchanged through `civ6_brain`'s
five-field adapter and `write_turn` into SQLite, then the actual Lua controller
runs those scout rows and policy metadata against a fake host returning the
recorded 1-dealt/100-taken preview. Default requests the fatal strike once;
v1 and v2 request it zero times, acknowledge the policy and name the refusal.

Replay binary SHA-256: `39847e443fea4d18f4c78b998cf8796b7f558b3ba60629d9d5f691468773b94b`. The source is based on
`4b500a6db376542fd68d68aa104cf3815010e1ce` with the subsequently tested acknowledgment/refusal-checker
changes; exact source hashes and all native JSON, transport rows and controller
results are retained under the local evidence directory
`010354-turn140-host-preview-survival/`. No running host database was modified.

These are deterministic regression controls against recorded host evidence,
not a counterfactual host execution or a measured win-rate improvement. The
underlying native amphibious damage mismatch remains unresolved.
