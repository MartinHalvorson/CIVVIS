# Bomber bases and imminent Loyalty revolt

This change preserves Bomber-class aircraft, including Jet Bombers, when their
owned city center or Aerodrome will revolt on the next publicly observed
Loyalty tick. It runs before the joint battle planner, immediate-kill pass,
and cavalry/Bomber city assault. A legal rebase reserves the aircraft's turn.
The destination must survive arrival and one later operating tick at its
current public rate. Ordinary rebasing uses the same destination restriction.

The existing air-surge opt-in and explicit Domination target gate both paths.
No field, gene, default, engine rule, native action, or live runtime changes.
Carrier and Airstrip positions do not inherit destruction from a nearby city.
If no legal safe slot exists, ordinary aircraft decisions remain available.
The forecast does not predict future changes in population, governors, or
military ownership, and does not protect against combat capture of a base.

## Defect and controlled validation

The actual-dispatch regression failed before the policy change at "evacuated
Bomber must survive". Its canonical population pressure, rather than an
observed-rate override, causes a real city revolt during normal turn
advancement. A profitable immediate kill consumes the aircraft first. The
same fixture confirms a legal safe rebase exists and that a stationary
aircraft is destroyed by the revolt.

Twelve focused tests now pass. They cover both attack prepasses, actual Jet
Bomber/Aerodrome survival, owned public rate with the pressure city hidden,
stable-base and non-Domination controls, destination deadlines, capacity
reservation, full bases, spent movement, unknown rates, and non-city bases.
The initial regression source is preserved in commit `916b086d6`.

## Campaign protocol and evidence limits

Protocol registered at `2026-09-27T01:31:46Z`, before policy editing:
parent `a0ca7b2e0dbb01df3286b3115a82356165194c83`; fresh seeds
37970000–37970003; Prince players and barbarians; four majors and six minors;
Simón/Gran Colombia focal explicit Domination; Tiny 60×38 Pangaea; Online 250;
all victories enabled. Both source libraries use the same frozen evaluator
from `9dda6319d413cb11bc5fdda55dfa487a6c1f4c11` and the same 21 forced rows
(SHA-256 `439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`).
Compare the same strike-reach-on rows across sources and retain all off rows.
Only the focal seat has an explicit target; rival controllers are adaptive
and the new explicit-target gate leaves their behavior unchanged.

The separate known seed 37930001 is diagnostic only. Its older completed
boundary trace (parent `57850ab0`, review `d3d2f2df`) shows healthy aircraft
disappearing in intervals when their bases revolt. Coincident boundary events
alone do not exclude interception on an earlier sortie. A previous count
missed upgrades: at observed turn 237 that older reviewed source still has
three healthy Jet Bombers, Aluminum stock 5, and no shortages. The older
parent has two healthy Jets. Do not characterize this as an already destroyed
wing or a zero-Aluminum checkpoint, or mix that history with the new sources.

The observer records complete applied actions, reasoning loss counters,
Bomber promotion class including Jets, aircraft health and positions, city
Loyalty/rates and ownership, supply, and city/capital retention. Candidate
source and artifacts freeze before results; no seed substitution, partial
outcome inspection, or within-block tuning.

## Completed frozen block

Candidate `ff5963e697b34ee896f7d6c50c67728f6d2fcf94` and parent `a0ca7b2e`
completed all registered pairs, including both strike-reach arms, before any
outcomes were read. All eight fresh action streams are exactly equal across
sources, and every observer action sequence is contiguous and matches the
final applied-action count. The primary on-arm results are identical:

| Seed | Result | End | Focal score | Foreign capitals held |
| --- | --- | --- | --- | --- |
| 37970000 | Science loss | 228 | 987 | 0 |
| 37970001 | Culture loss | 203 | 505 | 0 |
| 37970002 | Culture loss | 185 | 650 | 0 |
| 37970003 | Science win | 245 | 1170 | 1 |

Zero fresh Domination wins and no fresh behavioral contrast. The off-arm
results are retained as well, including a Science win and Score win; these
do not count as Domination wins.

The new known-seed primary on arm is also identical: Religious loss at 184,
score 924, five Bomber-class aircraft alive. The separate known off arm fires
the safety behavior twice: aircraft 582 leaves Edo (Loyalty 10.7783, rate -14)
before observed turn 160, and aircraft 604 leaves Kamakura (Loyalty 5.1322,
rate -10.9189) before observed turn 167. Both survive to the candidate's end
as Jets; the parent's same aircraft disappear when those bases transfer.
Observed aircraft losses fall 4→1 and coincident base-transfer losses 3→0.
Foreign major cities ever held rise 4→9, foreign cities held at end 5→10,
and foreign capitals at end 0→1. Both games remain Science losses (232→237).
This known alternate-arm diagnostic is not fresh evidence of general strength.

Local validation: 12 focused tests; `cargo test --profile ci --locked`,
4,369 passed, zero failed, 53 ignored; changed-lines Rust quality passes;
explicit library and `civvis_orders` build passes. No campaign source changes
or substitutions. This is a conditional aircraft-safety fix, with native
adoption and consistent native Domination wins still unproven.

Artifacts: `civvis-tactics-results/2026-09-27/air-base-loyalty/` on the owning
MacBook Pro. Engine soak is not applicable to this AI-only change. The active
native game remains under its owner's control and is untouched by this task.
