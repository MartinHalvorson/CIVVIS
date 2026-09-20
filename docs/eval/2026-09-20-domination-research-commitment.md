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
lanes. Recovery and an appointed war do not gain this protection; local
siege defense runs before it. Ordinary commitment rules may still retain
invested work after the catch-up condition ends. Finishing research buildings
can delay other production, including expansion and reinforcements.

## Validation

The Library reservation and fresh University both fail before the change:
the ordinary governor replaces each with a Settler. Unrelated-building review
and local-defense controls pass. Final focused, full-suite, and frozen replay
results will be recorded before integration. AI-only queue policy; no engine
rules or native driver changes, so an engine crash soak is not applicable.
