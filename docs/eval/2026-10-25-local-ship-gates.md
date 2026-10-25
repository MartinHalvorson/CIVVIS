# Enforce local checks before enabling auto-merge

PR #4129 exposed a mismatch between the shipping helper's local contract and
the merge path. `REQUIRED_CHECKS` includes `paired-cost`, but `ship_task`
enabled GitHub auto-merge before evaluating that contract. The PR merged as
`84e58ddf97b554d698b7cb3d948c8875d3e30a77` at 10:11:49Z on October 25,
while the fresh cost workflow was still active. Its measurements completed
at 10:17:14Z. The later cost result passes within the budget and is inside
the noise floor; that later success does not repair the earlier ordering.

Move the existing auto-merge request into the local `success` branch. This
keeps auto-merge and explicit squash merging after every local required check
has passed, including a check the server might not itself enforce. Preserve
the existing same-head resume, bounded retry, conflict and staleness behavior.
Do not alter branch-protection settings or grant new permissions.

The regression harness models a server that merges immediately when auto-merge
is enabled despite a pending or failed local check. On unmodified production
code, both refusal cases fail because no `CommandError` is raised; the
all-success control passes. The first attempt had a mock callback arity error
and is retained separately; it is not behavioral RED. Corrected RED is on
checkpoint `821950a2d`, with unchanged source hashes: two intended failures,
one passing control. After the ordering fix, all 16 shipping/resume tests pass.

Run the complete collaboration tooling suite and normal repository CI before
shipping this change. This is orchestration code, so no Rust artifact or game
mechanic changes and a game soak is not applicable. Keep raw test output,
source receipts, the observed merge/cost chronology and all failed attempts.
