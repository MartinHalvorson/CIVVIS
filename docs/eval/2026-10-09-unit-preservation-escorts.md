# Settler escorts during unit preservation

The preservation pass filters a proposed turn after the ordinary planner has
finished. It can remove a military unit's movement, retain a civilian's
movement, and append a retreat for the soldier. A Settler and its assigned
escort can therefore require joint reconciliation when their original orders
move them together. Preserving the soldier alone is insufficient to preserve
that escort contract.

The regression task checks executed positions after the returned orders are
applied. Its controls cover a recovering assigned guard, a threatened guard
that must reach a survivable retreat, an unchanged safe paired departure, and
an unrelated recovering soldier whose orders must not cancel the Settler's
route. The escape control requires the guard to move and remain safe; merely
holding both units is not an adequate replacement for an unsafe origin.

Independent test-only CI `37887813971` reproduced the recovering-guard
failure: the Settler reached axial `(6,6)` while its guard stayed at `(5,6)`,
native offsets `(9,6)` and `(8,6)`. The unchanged safe shared departure passed.
Fail-fast stopped the run after 2,038 passes and one failure, before the unsafe
retreat and unrelated-soldier controls were scheduled. Their baseline results
are not inferred from the first failure.

The candidate recognizes a shared departure only for an assigned land guard
and Settler that start stacked and propose the same movement endpoint. When
the guard is withheld, the civilian route and any dependent founding action
are withheld too. Retreat candidates are applied on a disposable board for
both units; only an exact arrival licenses the companion move. A survivable
joint retreat takes priority. If the civilian cannot reach any survivable
retreat, the guard may escape alone rather than being forced to die with it.
Unassigned civilians and separately planned routes retain their orders.

Nine new controls cover the original four scenarios, separately routed bound
units, ordinary single-step movement, an exhausted civilian, and release at
full health, and cancellation of a founding order after a withheld departure.
The original twelve preservation controls remain. Candidate
focused, full local, and independent CI results are pending; no pass is
claimed in this implementation checkpoint.

## Native observation and limits

The completed `civvis-20261009T043237Z` game used decider revision
`7918ddda5dc2839e52b58c65432ab95889f5fff2`, binary SHA-256
`63d5626cd0fdf2dc533d36b1e6a59a0380f775d756fd22f82ddd87bb67087367`.
Its `unit_preservation.rs` is byte-identical to public checkpoint
`c4f1c8f6afff15e25c8e990ac5747d43ca974488`, merged by PR #4008. The module's
SHA-256 is `fd3d54fdec3778abcfb629093900b0468f9e6b1ac154ba39eeeb8708c41da576`.
This establishes module identity; the complete private and public controllers
have other differences.

Turn 48 frame 0 orders Settler `720896` and Warrior `851970` to native offset
`(35,30)`. Turn 49 frame 0 orders the Settler to `(34,29)` and the Warrior to
`(36,30)`. The native ledger names `escort_cap_unresolved` with
`guard_has_order`. The journal also records a preservation retreat for a
Warrior at axial `(21,30)`, which is native offset `(36,30)`. A subsequent
native `unit_captured` event on turn 49 names the Settler and major captor 3.

The original proposed orders before filtering were not captured. These
records motivate the regression; they do not establish that preservation
caused this specific capture. The public Lua synchronization deliberately
preserves separately ordered units, as checked by `host_board_test.lua`.
Overriding unrelated military orders in that bridge would change its contract.

The read-only receipt `settler-escort-preservation-native-evidence.json`
retains selected state frames, orders, the capture event, journal entries,
module hashes, and hashes of all four original inputs. Inputs remained
byte-identical during the audit. No native lane, process, policy, pin, installed
mod, or private source changed. This game was retired by the production-rank
screen at turn 150; no new candidate game or win-rate improvement is credited.
