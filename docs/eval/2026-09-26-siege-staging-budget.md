# Siege allocation versus staging strength — 2026-09-26

Native King / Gran Colombia / four-player Tiny Pangaea run
`civvis-20260926T173411Z` declared on Byzantium at turn 75, then took no
cities before losing to Science at turn 235. At turn 76 the objective board
reported Constantinople's siege supplied by two units against a 50-strength
requirement, with no Siege requisition. The tactical siege controller reported
55 staged strength against a 79-strength requirement and stayed in Stage.

The campaign requirement discounts defenders and city strength by the tech
edge, while the siege train's advance gate reads an undiscounted local bill.
If the allocator stops at its smaller requirement, the executor cannot cross
its own staging gate even when reserve bodies could supply the difference.

## Validation plan

Reproduce the disagreement in a world fixture, exercise real force allocation
and the siege state machine, then require the allocation budget to cover the
existing staging requirement whenever the siege doctrine is active. Preserve
the strategic budget when it is larger and when the doctrine is off. Do not
lower the tactical advance threshold.

Before the paired pilot, freeze an unmodified-policy binary from this task's
base. Compare before/after on four full fixed-profile pairs, seeds
37140000–37140003, using `early-conquest-opening` as the evaluator's existing
toggle. Each source version completes both toggle arms; compare matching arms
between source versions. No early stopping and no inference of a win benefit
from a unit test or score increase. Four pairs are a diagnostic pilot only.
Both versions use identical setup and live-policy bundles. The simulator's
opponents are CIVVIS controllers, not Firaxis AI.

Results pending.
