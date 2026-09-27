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
boundary trace shows healthy aircraft disappearing in intervals when their
bases revolt. Coincident boundary events alone do not exclude interception
on an earlier sortie. A previous count missed upgrades: at observed turn 237
the reviewed source still has three healthy Jet Bombers, Aluminum stock 5,
and no shortages. The parent has two healthy Jets. Do not characterize this
as an already destroyed wing or a zero-Aluminum checkpoint.

The observer records complete applied actions, reasoning loss counters,
Bomber promotion class including Jets, aircraft health and positions, city
Loyalty/rates and ownership, supply, and city/capital retention. Candidate
source and artifacts freeze before results; no seed substitution, partial
outcome inspection, or within-block tuning. Full campaign results and native
verification remain pending. No consistent native Domination wins are proven.

Artifacts: `civvis-tactics-results/2026-09-27/air-base-loyalty/` on the owning
MacBook Pro. Engine soak is not applicable to this AI-only change. The active
native game remains under its owner's control and is untouched by this task.
