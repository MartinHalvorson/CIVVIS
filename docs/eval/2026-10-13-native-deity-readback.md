# Native Deity setup read-back

Attempt `civvis-20261013T095333Z` requested Deity but never emitted a native
seat or state record. Setup retried sixteen times and repeatedly refused to
start because the selected difficulty could not be read back. Its partial
summary ended `reason=killed`, turn -1, with no game outcome.

The saved Create Game and closed-dropdown captures visibly select Deity.
The actual macOS Vision window and enlarged-crop passes read the closed value
as `Delty`, both with confidence 1. The exact allowed-value lookup rejected
that text. Re-running the same saved closed capture against the unchanged
reader reproduces `None`; its heading and screen bounds remain readable.

Normalize this observed i/l confusion for menu observations in both the
closed-field read-back and option-label selection. The wanted label remains
canonical Deity. Other short misspellings stay unverified, and text outside
the game window cannot license a click. Existing heading-based row ownership,
observed screen positions and native seat verification remain in place.

Two regressions using the actual native OCR boxes failed on the unchanged
reader. Candidate validation passed all 279 launcher tests, including the
negative spelling and out-of-window controls. A real Vision read of the saved
capture now returns `DIFFICULTY_DEITY` at `(431,233)` in both setup read-back
and option selection, with no click sent and no game launched. This proves
that the recorded startup failure is repaired; it does not prove that a new
native Deity game is configured or that strategy strength improves.

The current native publisher retains the military stacking and refused-shot
fixes and is running a separate Immortal A/B trial with four Prophet-race
policies removed. Keep that trial's settings and strategy source intact.
Native adoption of this startup correction and in-game Deity read-back remain
pending. Runtime captures, actual OCR output, baseline failures, candidate
results and the partial setup summary are retained under
`~/civvis-tactics-results/2026-10-13/native-movement-adoption/`.
