# An existing Artifact for an owned Great Person

`artifact-before-more-museums` is a new explicit opt-in, **default OFF**. Once
an Archaeological Museum stands, a physical native Great Person requiring an
existing Artifact should not send the activation planner around the empire
building more empty Museums. The original Museum gene remains intact.

The installed `Base/Assets/Gameplay/Data/GreatPeople_Scientists.xml:44` names
Mary Leakey's requirements: `ActionRequiresCompletedDistrictType="DISTRICT_THEATER"`
and `ActionRequiresCityGreatWorkObjectType="GREATWORKOBJECT_ARTIFACT"`. Native
`20261103T002240Z` exported the same prerequisite on its owned, unusable person.
It completed Archaeological Museums on turns 153 and 175, owned no Archaeologist,
and lost to rival Science at 182 with zero Artifacts. This historical loss is
context, not a counterfactual result.

The new path requires a confirmed zero Artifact count, the physical activation
need, a completed Museum, spare Artifact capacity, a nearby owned land dig, and
a legal Archaeologist quote no longer than twenty standard turns, with time
left to use it. A damaged Museum city or a hostile military unit within six
hexes blocks the digger. An existing or queued digger blocks another. Existing
queue protection and defensive redirects retain their precedence; a selected
digger survives routine rescoring while the physical need remains. Missing work
inventory does not become an assumed zero. The original capacity path remains
available when the new prerequisite conditions are absent.

## Fixed-board proposal checks

Twenty disposable queries used four selected native input snapshots from that
one game. Each board ran the original program, the candidate off twice, and the
candidate on twice. The original/off and both repeat comparisons matched the
full orders and decision payload. Realized treatment lists matched every arm.

| Native turn | Candidate proposal | Other changed production orders |
| --- | --- | --- |
| 153 | Szeged Archaeologist, native quote 8 turns | Another Museum request disappears; two cities' naval choices change |
| 160 | Szeged Archaeologist, native quote 8 turns | The same additional Museum and naval changes |
| 175 | Ur Archaeologist, native quote 6 turns | Replaces Thermal Bath |
| 176 | Szeged Archaeologist, native quote 5 turns | Replaces Entertainment Complex |

These are independent fresh-board proposals. They do not reproduce a persistent
native trajectory. In particular, the naval changes at 153 and 160 must not be
described as a single isolated order change. A nearby dig does not prove a legal
walking route, and no build, dig, Artifact gain, Mary Leakey activation or
win-rate improvement is established here. No native experiment is armed and
no deployment selection or strength ledger is changed.

## Evidence

The full `cargo test --profile ci --locked` run passed: 6,699 tests, 56 ignored.
Seven focused Rust tests passed, covering the reversible default-off option,
the new production choice and its commitment, duplicate diggers, authoritative
inventory, physical need, land ownership, legal unlock, deadline, protected unit
queues, damaged cities and known-full slots. Fourteen append-point tests and
21 manifest tests passed. Three gene-tool tests initially found the missing
generated unmeasured row; the evidence table was regenerated without changing
the ledger, and all three corrected checks passed. The initial fixture compile
error and the initial missing-fire proof are retained.

The [adjudication](2026-11-03-artifact-activation.json) binds the source commit,
frozen binary and exact order changes. The
[raw archive](2026-11-03-artifact-activation.raw.tar.xz) has 76 members,
2,888,580 bytes, SHA-256
`d9b5e2e9e9162485c796a218b692b2300b92a66707f1d763b84357165ec77cd1`.
Its manifest binds every other member. It retains input bytes, native decision
receipts, all query stdout/stderr and arguments, source, installed requirement
rows, controls, repeats and failed diagnostics. A separate packaging pass checked
the native receipt hashes, selected input bytes, actual output payloads and
realized genomes before archiving. The fire ratchet accepts the four paired
proposal records; that is activation evidence, not strength evidence.

The first offer census used an unsupported building name and found zero offers.
The corrected audit uses Firaxis's `BUILDING_MUSEUM_ARTIFACT`; both versions are
retained. The existing bridge translates digs to `EXCAVATE` and handles the
ChooseArtifact prompt, but this candidate has not exercised that path natively.
