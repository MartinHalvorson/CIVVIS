# Keep Domination research catch-up construction

## Native evidence

Native King/Gran Colombia run `civvis-20260920T143728Z` pinned controller
`44a39496100fbc9adaa4715f8128a6820d01f7dc`, binary SHA-256
`dcc2395cfd65eda4962544754202ff5ab966e012c0f00f7038b55907bb68f7e6`.
Through turn 150, 21 recorded decisions select a Library or University and
then replace it with another item in the same decision. At 52/0 Bogotá's
native actions are `Produce Library`, then `Produce Settler`; only the
Settler is exported. At 58/0 the replacement is a Builder, 61/0 a Settler,
and 73/0 an Archer. This is a planner conflict before host execution, not
a host refusal to build the requested science building.

At turn 150 Bogotá still has a Campus and no Library. The empire has ten
cities and 69.5 Science per turn. These observations establish repeatedly
canceled research investment; they do not establish how much earlier any
particular military technology could have completed.

## Change and limits

When explicitly targeting Domination, with a research-building-catchup
variant enabled, preserve legal queued Campus buildings with positive
Science yield while our public Science output is below 70% of the best
contacted living major's output. This is the existing catch-up planner's
threshold, including observed yield corrections. New queues are protected
before their first production turn, which the ordinary invested-production
commitment cannot do.

The check runs once per production review. It does not queue more buildings,
change research selection, reserve Campus districts, or affect other victory
lanes. Recovery, an appointed war, and the plan's threatened city do not gain this
protection; local siege defense runs before it. Ordinary commitment rules may still retain
invested work after the catch-up condition ends. Finishing research buildings
can delay other production, including expansion and reinforcements.

## Validation

The Library reservation and fresh University both fail before the change:
the ordinary governor replaces each with a Settler. Unrelated-building review
and local-defense controls pass. All nine final regressions pass, including
parity, unmet/eliminated rivals, disabled catch-up, other lanes, illegal
construction, recovery equivalence, and the explicit threatened-city gate.

After merging `280b3b7` (#3634), `cargo test --profile ci --locked` passes
3,754 library tests plus 205 binary/integration tests, with 49 library and
four doc tests ignored. `cargo fmt --all -- --check` and `git diff --check`
pass. AI-only queue policy; no engine rules or native driver changes, so an
engine crash soak is not applicable.

## Frozen native replay

Freeze the event prefix through turn 150 and replay its 412 unique awaited
decision frames with identical Domination/Gran Colombia arguments and the
19 forced genes, using a fresh persistent brain per binary. The matched
baseline is `e16f35e56`; final candidate `87132e541` adds only this policy
and its tests/docs. The later #3634 integration is covered by the full suite.
Both processes exit 0: baseline 111.01 seconds, final candidate 93.08.
Timing is informational, not a controlled performance benchmark.

Ignoring `order_failed`, `order_verified`, and `turn_verified`, 96 frames
change exported orders and 155 change raw internal native actions. The first
change is 52/0: Bogotá exports `BUILDING_LIBRARY` instead of `UNIT_SETTLER`.
Same-decision science-building cancellations fall from 21 to zero. Immediate
Library requests rise from three to 23, and University requests remain three.
Later next-queue, unit movement, and policy-deck requests also change as
production commitments change. The added threatened-city exclusion produces
identical orders to the first candidate on this trace; its regression covers
the boundary that the trace does not exercise.

These are requests against frozen host observations, including retries and
repeated reservations. They are not 23 completed Libraries, executed unit
orders, an observed Science gain, an earlier technology, or a victory. The
recorded native game continues under its original controller. A fresh native
game must establish whether completing this investment improves the campaign.

Local artifacts: `/tmp/civvis-research-commitment-replay/`,
`/tmp/civvis-research-commitment-frame-replay.py`,
`/tmp/civvis-3635-native-clobbers.json`,
`/tmp/civvis-3635-final-comparison.json`,
`/tmp/civvis-3635-refinement-comparison.json`, and
`/tmp/civvis-3635-{red,focused,full,merged-full,fmt}.log`.
