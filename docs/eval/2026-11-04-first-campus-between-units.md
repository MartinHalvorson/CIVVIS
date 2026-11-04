# Rejected first-Campus production prototype — 2026-11-04

The `first-campus-between-the-units` prototype was rejected because it did
not produce the intended Campus order. Its registration, policy, diagnostic
extension and draft tests are removed from the final proposal. No default,
strategy or live-screen setting changes in this report.

## Native development case

`civvis-20261104T061724Z` was native Standard-speed Immortal Gran Colombia,
on the Continents map, Small size, with six players. Writing was observed at
turn 56; the first Campus was placed at 163 and completed at 174. Those are
observations from the original game, not a treatment outcome.

The candidate allowed the existing free-slot Campus claim to interrupt a
fresh military build during major war with at least three cities, at least
two healthy non-recon land combat units per city, and no Campus standing or
queued. It retained earlier holds and the final defensive production pass.

At turn 106/frame 0 the native planner chose Heavy Chariot. The candidate
briefly claimed a Campus, then the final defense pass restored Heavy Chariot.
The source city had intact walls (100), health 200 and defense 24, but a visible
enemy Knight (50 strength) and Catapult (25) were nearby. Intact walls and six
healthy soldiers did not establish that an eleven-turn Campus was safe.
The final defense pass must remain authoritative.

## What the repeated comparisons actually establish

Historical candidate source: `efaced090` (production snapshots included).
Candidate executable SHA-256:
`ea4bb9a2d7789a92b2ac617b7f44455c184f31d06e9c080f8db606a3a702ea31`.

| Comparison | Repetitions | Result |
| --- | ---: | --- |
| Candidate OFF against original native history | 2 × 425 requests through 172/1 | Complete orders and decision payloads, and ordered input reads, match |
| Candidate ON against the same captured history | 2 × 384 requests | First internal decision difference 106/0; first raw order-list difference 157/0 |
| Persistent paired queries ending at 157/0 | 2 OFF/ON pairs | Actual EOF exit 0; corresponding payloads and input reads match the captured-prefix runs; only the named treatment and mirror path differ |
| Later native game `civvis-20261104T094622Z` | 2 × 346 OFF and 2 × 346 ON through turn 150 | OFF full controls match; ON order lists remain identical |

At 157/0, OFF lists capital Heavy Chariot before Guayaquil Catapult; ON lists
Guayaquil Catapult before capital Heavy Chariot. **No order is added or
removed. Per-city production is unchanged.** Earlier emitted order lists in
the tested prefix are identical. A raw list-inequality activation gate could
accept this permutation, but it cannot validate a Campus-production claim.
No new-gene firing artifact is submitted.

The independent verifier reconstructs every recorded read sequence, verifies
source receipt and archived source hashes, compares complete OFF payloads,
checks repeated ON payloads and successful-query receipts, checks the single
named treatment difference, and asserts that the first differing order lists
are permutations. Protocol verification passes; intended Campus effect fails.
The later game was examined after candidate design, with its outcome and early
city counts already observed for another screen. It is not an untouched holdout.

These are captured-history comparisons. No divergent native world was run,
and no science gain, saved city or win-rate improvement is established.

## Validation and next investigation

At historical prototype commit `ec2794355`, CI ran 6 new tests within 6,819
passing tests (53 skipped). Its overall gate failed because new-gene firing
evidence was absent. Passing policy-unit tests did not establish the emitted
Campus effect. A seventh defensive-handoff test was drafted at `bee9372a6`
but not executed; it is also removed from the final tree. The `efaced090`
native executable built successfully. Final report validation is separate
from those historical prototype checks.

A more promising question is the defender choice. At 106 the host offered a
Spearman for 33 production in three turns and Heavy Chariot for 33 in four.
`BasicAi::best_military` ranks legal candidates by raw strength with optional
unique-unit credit; it does not score the local matchup. Existing combat
rules give anti-cavalry +10 against qualifying cavalry, with War Cart immunity.
This makes matchup-aware emergency selection worth investigating while
preserving wall/repair priorities, legality, paid queues and completion time.
It is a hypothesis, not an implemented improvement or strength result.

## Evidence artifact

[`2026-11-04-first-campus-between-units.native.tar.xz`](2026-11-04-first-campus-between-units.native.tar.xz)
contains raw captured requests/replies, sealed input bytes and indexes,
successful query receipts, source snapshots, drivers, the independent verifier,
build/CI logs, and the later-case comparison. Executables and event FIFOs are
excluded. Original absolute paths are retained in receipts; relocation maps
identify the archived bytes. The verifier requires the CIVVIS parsing helpers
and reachable historical source commit (see archived README). The executable
hash records the built candidate; the executable is retained in the operator
archive rather than this Git artifact.

Archive: 5,293,532 bytes. SHA-256:
`d4fd940c6c9433167d5b4fe731524e2509d9d97ad9ad8f10fec9f7c1f30d50c2`.
