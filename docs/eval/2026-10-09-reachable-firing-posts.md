# Reachable siege firing posts

A gun could be assigned a nearby firing pocket with a clear shot but no route
through the siege's reserved tiles, even when a farther firing tile was
reachable. Assignment now ranks candidates by the existing formation,
exposure, distance and position key and chooses its first destination accepted
by `siege_route_step`, the same route rules used by approach and melee-post
assignment. A gun with no reachable candidate gets no post. The existing valid
firing-position hold, occupancy, terrain and line-of-sight checks remain.

## Reproduction and behavior

The fixture provides a closer visible firing pocket whose only entrance crosses
the excluded inner city ring, and a farther firing tile with an open route.
The Catapult and Archer cases require assignment to the reachable tile,
physical arrival through recorded tactical movement and a legal wall-damaging
shot. The blocked case seals every approach and requires no assigned post.

The complete unchanged-production local baseline on `2e6a0f898` executes all
53 discovered siege cases: 50 existing controls pass and exactly the three
new assignment assertions fail, exit 101. All 525 registered source, test,
data and Cargo hashes remain unchanged. These failures occur before the
positive cases' physical arrival and shot assertions. Independent baseline
[CI 37913319750](https://github.com/MartinHalvorson/CIVVIS/actions/runs/37913319750)
reproduces all three failures and passes 27 scheduled existing siege controls;
fail-fast leaves later controls unscheduled. Neither baseline has a fixture
error. Production changes only after complete local baseline proof.

The original three test assertions remain unchanged in the candidate. Main
`5ebbeb493` is merged once as Computer-trailed `ecfbccb3f` before candidate
compilation, retaining the merged recovery-capture and Settler-escort modules.
The complete local candidate siege suite passes all 53 cases, including actual
arrival and wall damage, exit 0. All 526 registered source hashes are unchanged.

Complete local `cargo test --profile ci --locked -- --test-threads=1`
passes all 4,746 tests, zero failures and 54 existing ignores across all
Rust targets and documentation, exit 0. All 526 registered source hashes
remain unchanged. Both unchanged spectator timing fixtures pass. The full
log and receipt retain the exact six target summaries and source manifest.

Independent main-integrated
[CI 37919444139](https://github.com/MartinHalvorson/CIVVIS/actions/runs/37919444139)
passes all 4,746 Rust cases, all 53 siege cases and all 35 preservation controls,
with 50 existing skips and four documentation ignores. The subsequent
71/21/22 overlapping regression controls and provenance/fidelity stages pass.
Quality, policy, overwrite, mod and publication checks also pass.

Paired [cost run 37919444017](https://github.com/MartinHalvorson/CIVVIS/actions/runs/37919444017)
passes the +8% budget. Its +0.28% median over five pairs is inside the ±1% noise
floor; IQR is 0.64pp and resolution is ±0.43%. This establishes neither a speed
gain nor a slowdown. No engine mechanic changes require an engine soak.

## Native motivation and limits

The completed external Gran Colombia four-player game
`civvis-20261009T081707Z` loses to Culture on turn 160 with no city captures.
It records six friendly ranged district attacks, including two on Whanganui.
The selected why-journal entries repeatedly show gun approaches stopping
short of their posts: 12 simulated movement refusals, ten out-of-movement
stops, nine no-further-route stops and 18 no-route/no-pass-through stops.
Repeated frames mean these are not 49 distinct native failures.

Private revision `5df81886b` has substantially different siege routing and
guards. Both investigated baselines lacked a firing-post route check, but the exact
cause of the native refusals is unproven. Source comparison, journal, baseline,
candidate and cost receipts remain under retained goal artifacts. No native
lane, pin, private source, installed mod or process is changed. These synthetic
results prove the public assignment and movement regression; they do not
establish native adoption, city pressure or a Domination win-rate improvement.
