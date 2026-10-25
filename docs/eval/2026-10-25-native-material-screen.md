# Native material trial: input qualification

The eight-start trial registered in #4119 is still unarmed. Its intended
comparison remains the existing `army-upgrade-materials` buyer plus the new
stock reserve against that policy being off. All original inclusion,
continuation, outcome and interpretation rules remain in force.

## Why the recorder had not activated

At 2026-10-25 05:36 UTC the supported host policy contains
`CIVVIS_CAPTURE_DECIDER_INPUTS=1`, but the actual game driver for
`civvis-20261025T053159Z` lacks `--civvis-capture-inputs`. The live supervisor
is still the process launched from the older first-foothold worktree. Its
source has no input-capture selector. Updating the runtime checkout does not
replace this long-lived shell or its inherited environment. The earlier
unarmed game `civvis-20261025T051724Z` also has no input archive. These games
do not qualify the recorder and receive no trial arm.

That completed game lost to rival Culture at turn 203, with eleven cities,
six cities at turn 60, three combat captures, four city losses, 46 kills and
26 unit losses. It used frozen source `8810924ee`, before the stock reserve.
It is development evidence and does not measure the repair.

## Correction without restarting the host

The visual game launcher already reads the host policy for trade settings.
It now resolves input capture at the same invocation boundary, before any
attempt. An explicit CLI enable or disable wins; otherwise the current
policy wins over inherited supervisor settings, then the environment, then
the disabled default. Continuations keep the resolved setting. Invalid or
unreadable policy disables capture with a warning and does not stop play.
The file is parsed as data and never executed.

This replaces the earlier planned supervisor reload with a per-invocation
policy read. No host restart or process signal is required. The current
native seat uses one attempt per invocation. The setting remains fixed
inside an invocation containing multiple attempts.

After integration, verify an unarmed driver's actual capture flag and the
first complete captured request, reconstruct every referenced read through
the parent-linked archive and retain the resulting byte digests. Verify
the actual frozen binary and merged source too. Only after qualification
may the previously registered eight-start screen be armed. A failed or
missing qualification does not start that cohort, and an assigned game's
later capture failure does not remove or replace it.

## Validation

Eleven focused policy/runner tests and all 220 launcher tests pass. The new
checks exercise stale inherited zero and one settings, explicit CLI control,
invalid and unreadable policy, non-execution of shell-shaped values, and
the actual main-path resolution before launch. Existing checks cover the
actual runner commands, continuation forwarding and the brain option.
Full developer-tool and required Rust validation are pending. There is no
native capture qualification or strength result yet.
