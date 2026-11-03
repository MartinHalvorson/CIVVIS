# Hand over when the host reports the Prophet race exhausted

The existing `religion-lane-hands-over` opt-in counted founded religions and
pending rival Prophets, but ignored the native host's exhausted Prophet
class. In configured Immortal Religious game `civvis-20261102T203602Z`, the
host reported exhaustion at turn 39; the fourth founded religion appeared
at 57. The controller retained its Religious target through that interval.
The repair uses the existing `prophet_class_exhausted` helper alongside the
count, while preserving our own pending Prophet and founded religion. Missing,
empty or other-class telemetry does not close the race. No flag or deployment
default changes.

Six focused Rust tests pass, including the host-exhausted case with the flag
on and off, missing/other telemetry, and our pending/founded exceptions. The
five Holy Site foundation tests also pass against the integrated source, as
does `cargo check --tests --locked`.

Twenty disposable one-shot queries use four exact captured inputs from the
motivating game: original/candidate with the flag on, repeated original, and
original/candidate with it off. Realized treatment profiles are read back and
checked. Repeated original orders are stable and every off order matches.

| Captured input | Original target | Candidate target | Immediate orders |
|---|---|---|---|
| t38/frame0 | Religion | Religion | Identical |
| t39/frame0 | Religion | Science | Identical |
| t42/frame0 | Religion | Science | Identical |
| t57/frame0 | Science | Science | Identical |

These queries establish the target correction, not saved production or
research. A fresh controller does not reproduce the persistent controller's
Prayers choice at turn 42. The original Religion process context was not
captured, so this is not a qualified replay of that process. The candidate
includes the independently integrated Holy Site foundation repair from main.

A separate native Domination game, `civvis-20261102T215109Z` (Georgia), did
have its running executable, exact argv and cwd observed. Its original
executable first reproduced all 92 recorded decisions through turn 42 twice,
with matching frame identities and exact ordered input reads. The candidate,
built with the documented native fast release profile, then reproduced those
same 92 decisions twice with the same strict comparisons. This is regression
evidence on another lane, not evidence of Religion efficacy. The earlier
unoptimized candidate attempt matched 59 requests and failed the 60-second
deadline at turn 30/frame2; that failure is retained. The optimized passes do
not prove the cause of the earlier timeout.

The companion JSON hashes the 297,800-byte raw archive and all 93 members.
It includes the source patch and source manifest, executable hashes, captured
Religion inputs, realized genomes, commands, raw replies, focused test logs,
observed Domination process context, its original qualification, sealed read
bytes, both candidate regression runs, and the failed attempt. The two Python
scripts reproduce the one-shot and qualified-other-lane checks using matching
executables; neither publishes orders to a host. No win-rate claim or evidence
ledger ingestion is made. A future captured Religion process is needed to
measure whether the earlier target change alters persistent choices.
