# Public own strategic income readback

Follow-on correctness work after merged PR3801, not a new Domination policy.
The requested profile remains Prince, Gran Colombia/Simon, four-player Tiny
Pangaea, Domination, with all victory conditions enabled.

## Reproduction and public boundary

A standalone read-only fixture against PR3801's compiled library produced:

    world_income=2 observed_income=0 world_tiles=7 observed_tiles=1 observed_suzerain=Some(0)

The sole resource is a connected non-center Aluminum mine in a known city-state
whose Suzerain is the observer; Radio is researched. Public city redaction
correctly hides the foreign city's private owned-tile ledger, but strategic
income is recalculated from that incomplete ledger and loses the public +2.
The executable, source, exact library/probe hashes and command are preserved at
`~/civvis-war-evidence-20260926/strategic-readback-probe.md`.
No full campaign outcome or native runtime improvement is inferred from this
controlled fixture.

The shipped UI explicitly reads the local player's public gross resource
income: `DLC/Expansion2/UI/Replacements/TopPanel_Expansion2.lua:50` calls
`GetResourceAccumulationPerTurn`, lines51–52 read imports and bonuses, and
line64 sums them before displaying the tooltip. The surrounding lines25–85
were read from this machine's Firaxis installation. Unit and power demands
are read separately, so the repair must preserve gross income, not net stock
change or stockpile capacity.

## Intended repair and tests

Use the existing strategic-income correction seam, preserving the observer's
own public total after redaction without copying foreign tile ownership,
governors or rival private income corrections. Compute the correction against
the redacted raw model, not the already-clamped corrected value, so existing
negative native adjustments do not get counted incorrectly. Continue allowing
own improvement/pillage counterfactuals to change the modeled part.

Eight regression tests cover the reproduced non-center mine, all seven
strategic resources, repeat observation, existing positive/negative/clamped
host adjustments, world non-mutation and foreign privacy, fresh views after
pillage or Suzerainty loss, own-mine counterfactuals, and unrevealed resources.
Before implementation, seven tests failed and the unrevealed-resource control
passed. The failures include world2/view0 and native-adjusted world7/view5;
the private foreign correction was also still present in the redacted copy.
The correction at `0d67883d8` passes all eight focused tests:
`cargo test --profile ci --locked --lib strategic_income_tests`.
`cargo test --profile ci --locked` also passes: 4,389 passed, zero failed,
53 ignored across all targets and doctests. Changed-file quality passes with
`python3 tools/rust_quality.py --base origin/main --head HEAD`, as does
`git diff --check origin/main...`. Pre-ready synchronization found the branch
current with main `be8a36019`. Logs are retained under
`~/civvis-war-evidence-20260926/strategic-income-{red,green,full,quality}.log`.
CI cost and published-head validation remain pending. No additional campaign
soak is claimed for this bounded observed-state repair; the focused fixtures
and full suite establish correctness, not stronger Domination outcomes.

The correction is a current public reading. It does not claim omniscient
prediction of future hidden city-state infrastructure or arbitrary hypothetical
changes to foreign resource access. New authoritative observations must refresh
that reading. No stronger Domination result, policy promotion, or resumption of
the blocked real Firaxis game is claimed.
