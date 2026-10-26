# Fixed retrospective opening-Production census

Freeze the entire completed sixteen-start Slinger cohort from the companion
selection JSON before computing the metrics. Its seventeen attempts, including
the turn-41 continuation, all remain eligible. This is descriptive retrospective
analysis, not a randomized strategy result. Do not replace a game, sample point,
missing reading or unfavorable result. Preserve every source/profile/arm change.

Primary samples are exact turns 10, 20, 30, 40 and 50. For each original stem
and target, use the lowest continuation ordinal with a recorded actual decision
at frame zero, then the first such decision in that attempt. Keep repeated
same-frame decisions and alternative continuation readings as controls rather
than silently replacing them. No exact frame-zero decision means a missing
primary sample. Reconstruct all distinct successful captured reads; require
complete descriptors and last Native state at that exact turn/frame. Preserve
any differing read variants instead of treating a decision-wide guessed prefix
as its input. Record raw selected state, decision, prefix byte count/SHA and
source/binary/profile identity. Existing committed cohort archives remain the
raw source; avoid duplicating their full prefixes unnecessarily.

Primary metrics are own city count/population, host Production, Production per
city/population, food surplus, housing/growth/happiness, current queue quotes,
and worked terrain/specialist counts. The own roster replaces previous holdings.
The free city centre is not an extra citizen. A staffed district may appear in
both worked plots and specialists; exclude it from terrain-worker totals.
Missing or negative sentinel aggregate values remain unknown.

Use the first agent turn record at each target for all-major Production standing,
matching local-player metadata from Native seat/turn records. This map includes
unmet majors; do not use the maximum met-rival-only production as the global
leader. Report its timing separately from the pre-action frame-zero city state.
When that actual leader also has met-rival public stats, decompose the same-state
Production ratio into city-count ratio, population-per-city ratio and Production-
per-population ratio. Otherwise city/population decomposition is unknown; visible
rival cities are not a substitute for total city count.

As a fixed secondary check, replay the existing production_share_reading/run
helpers at share 1/3 over turns 50, 51 and 52, with the actual local seat. Use the
original source behavior and preserve unreadable readings. Repeated records in
one turn cannot create a three-turn run. This replay is a retrospective rule
probe, not an actual host defeat or a claim that the game would never recover.
It never changes the enabled shared rule.

For allocation bounds, reconstruct each complete actual captured input. Explicit
`tiles` records replace the complete record for that plot. Metadata-only deltas
and absent keys cannot renew ownership or yield information. Require an explicit
same-turn plot observation, visible locally, owned by the local player and that
city, passable, non-district, within three wrapped hexes. Exclude already worked
plots, the free centre, district/specialist plots and unknown legality fields.
Only finite native yield quotes are eligible. Unknown/stale plots remain unknown,
not zero opportunities. Do not borrow a later export or neighboring turn.

At most one terrain worker per city can be considered. Report (a) raw Production
increase at least one while every other five reported tile yields is preserved,
and (b) raw Production increase at least one with the observed Food surplus after
that one exchange at least two. The latter retains every other yield tradeoff.
These are quoted local terrain-yield bounds, not a native CanStartCommand quote,
actual citizen reallocation, prediction of city multipliers, free yields, or a
reason to issue a focus command. Existing locks/allocator behavior may prevent
the exchange. No allocation, gameplay, driver or live-trial changes are authorized
by this census.

Relevant primary source: CivvisControlAgent.lua:7046-7085 reads city yields and
population; :7374-7430 reads actual worked plots and specialists; :7794-7828 reads
host growth figures. Met-rival filtering begins at :8203. The all-major turn
Production map is emitted at :20730. Existing native Food-focus control quotes
Base/Assets/UI/Panels/CityPanel.lua:445-458 and verifies readback plus starvation
safety; its existence does not establish a Production-focus gain.

Before trusting output, run semantic controls for missing global leader versus
met-only data, free centre/specialist handling, replacement rather than stale
plot-field merging, freshness/ownership refusal and the one-worker food/yield
tradeoffs. Retain code, fixtures, failures, successful outputs and source hashes.
Publish the complete census and all unknown counts. No policy or ledger promotion
follows merely from an upper bound.

Source clarification before measurement: city `production` and the all-major
turn map use GetBuildQueue():GetProductionYield (the turn map can fall back to
City:GetYield). `city.yields.production` and public_stats.production use
City:GetYield directly. Keep queue Production and city-yield Production separate.
Factor only the same-state public city-yield ratio; do not present that product
as a decomposition of a different build-queue or post-action turn reading.
`turn.majors` counts alive OTHER majors; a complete reported map therefore needs
that number plus the local player. If a reported participant is unreadable or
missing, global standing remains unknown with the readable subset retained.
The retirement replay still preserves the existing helper's exact behavior.
