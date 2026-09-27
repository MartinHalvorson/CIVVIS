# Preserve the selected native recovery save

This process change preserves replay inputs; it makes no claim of stronger
Domination play or of fixing the native turn stalls.

## Observed gap

The four-player King Gran Colombia attempt `civvis-20260927T122456Z`
stalled at turn 135. Its first five continuations subsequently stopped at
turn 160, and the sixth stopped at turn 124. These are segments of one
unfinished game, not seven terminal games. The existing recovery loop
preserved native logs, but no `.Civ6Save` in the attempt's run directories.
The next fresh attempt, `civvis-20260927T132350Z`, cleared the old rolling
autosaves. The exact turn-160 input was therefore unavailable for a later
native baseline/candidate replay.

The turn-160 logs include a successful TopPanel controller wake before
activity stopped. This evidence does not establish a missing UI clock as
the cause, so this change does not alter controller callbacks.

## Behavior

After owned-game teardown succeeds and recovery selects an autosave, the
climb copies that exact file to the frozen segment's
`native-recovery-save/<original filename>`. It writes `manifest.json` only
after a complete, stable copy and checksum readback. The manifest records
the original path, size, SHA-256, source timestamps, root run, frozen
segment, intended continuation and last observed game turn. Run tags link
the input to the existing seat, genome, decider and recovery-chain records.
The save's numeric suffix is not interpreted as its game turn.

Each snapshot is limited to 64 MiB, with bounded reads and no truncation.
Empty, nonregular, oversized and changing inputs fail explicitly. An
existing destination is never overwritten; a failed copy removes its own
incomplete files where the filesystem permits cleanup. A reported copy
failure retains the original recovery path and does not spend another
attempt. Save selection, the six-resume budget, native settings, treatment
flags and the actual `--load-save` argument are unchanged.

The manifest identifies the selected input. It does not assert that a
continuation successfully loaded, identify the restored turn, or establish
the cause of a stall; those require the continuation's native readback.

## Validation

`PYTHONPATH=tools python3 -m unittest test_civ6_save_snapshot test_civ6_civvis_climb test_civ6_native_log_snapshot test_civ6_conquest test_ci_wiring test_docs_commands test_docs_reference_live_commands`
passed all 267 tests. `cargo test --profile ci --locked` passed 4,423 tests
with zero failures and 53 ignored. No engine code changed, so an engine soak
or paired strength screen is not applicable. The new suite is discovered by
the existing repository-tooling CI job.

The snapshot suite covers complete binary preservation after deleting the
source, provenance, overwrite refusal, size bounds, symlinks and missing
inputs, growth, truncation, same-size rewrites with restored mtime, inode
replacement, disk errors and checksum readback corruption. The climb suite
checks that the exact selected save is archived after proven cleanup and
before the continuation launches, survives simulated autosave deletion,
keeps the original load path on copy failure, and does not archive finished
games or runs whose cleanup was refused.

A read-only preflight copied a real autosave from the active native game
without loading it or interrupting play. At 2026-09-27 13:36:18 UTC,
`AutoSave_0108.Civ6Save` was 1,083,531 bytes. Its original bytes, archived
bytes and manifest independently matched SHA-256
`ca7e7a78de1a46c0ffa10b1d254402160bd3d2d66ff2952320882a005a71df39`.
This validates copying a native file, not a native restore or automatic
adoption by an already-running climb process.

Evidence lives under
`~/civvis-war-evidence-20260927/native-recovery-snapshots/`:
`preflight.json`, `read-only-native-preflight/manifest.json`, and the
complete archived save. The preflight game ran source revision
`3ec0a0780a56ea6b13237029b9b833f4e5b9eb1f` with the native seat read back
as four-player King, Gran Colombia / Simón Bolívar, Tiny Pangaea, Online,
Gathering Storm and all victories enabled. The deployed archive hook will
be used when a new climb process starts from the merged source.
