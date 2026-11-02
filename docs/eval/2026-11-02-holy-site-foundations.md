# Resume the founding site without multiplying foundations

`religion-seat-founds-first` could start another Holy Site after defense
removed the first one from the imported queue. Native paused construction
is retained as a tile foundation; the founding claim checked completed
districts and queued items alone. The existing opt-in now restricts its
legal candidates to owned paused foundations when any exist. An idle,
unthreatened city can resume its placed site. Busy queues, completed or
queued sites, founded religions, pending Prophets and the closed-race gates
keep their existing behavior. No registry or deployment default changes.

The motivating game, `civvis-20261102T203602Z`, is a configured 6p Small
Continents Gathering Storm Immortal Religious attempt. Its native journal
places Holy Sites in Puel Mapu at 17, Ngulu Mapu at 23 and Nag Mapu at 26;
defenders interrupt the first two. The host reports the Prophet class
exhausted at 39, before its first completed Holy Site at 42. Its first
Shrine appears at 130, after the race is lost. It hands the target to Science
at 57 and loses to a rival Technology victory at 211. These observations
do not establish that this repair would have won its Prophet race.

Five focused Rust tests pass under unoptimized CI profile. The paused-site
regression fails on the original implementation while the other three
tests pass. Final tests cover preserving an Archer queue, resuming the
same placed site, and allowing a legitimate new claim when the foundation
is foreign or belongs to another district. The local linker emitted its
large `__eh_frame` warning; the tests completed successfully.

Twenty disposable native queries compare the captured original executable
with the repaired one: four captured reads, each queried before/after with
the flag on, a repeated original control, and before/after with the flag
off. Every realized genome matches the observed source profile, except for
the deliberately disabled flag in off controls. Repeated controls are stable
and all off orders match exactly.

| Captured input | Observed query result |
|---|---|
| t12/frame0 | All orders unchanged |
| t23/frame0 | Stops the new capital Holy Site at (66,23); the fresh controller falls back to a Scout; all other orders match |
| t26/frame1 | All orders unchanged; the fresh query does not reproduce the persistent controller's third-site proposal |
| t28/frame0 | All orders unchanged, including resuming the already placed capital Holy Site at (66,23) |

The Scout fallback is a fresh-controller query result, not a prediction of
the persistent host's queue. These are proposals on captured inputs, not
native execution, saved turns or strength results. The t28 control guards
against suppressing the useful resume path while fixing duplicate starts.

The companion JSON binds the 144,372-byte raw archive, its source patch and
source-file hashes, both executable hashes, original captured bytes,
genomes, command lines, raw responses, red/green test logs and selected
native observation lines. All 64 archive member hashes verify. The original
binary is the attempt snapshot (SHA begins `bdb0125b67b2`); the diagnostic
candidate was built on the recorded task base with the archived patch.
The source manifest was checked against the final Rust files. No ledger
ingestion or win-rate claim is made.
