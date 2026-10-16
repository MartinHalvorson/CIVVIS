# Domination Spaceport production contract

Named Domination controllers already exclude new Spaceports and launch projects. The direct physical Great Person reservation and the opening governor did not receive that exclusion. Both calls now lend and restore the same existing target context, and fresh/resumed live-person prerequisite selectors respect it. This fixes an existing production contract without adding a gene or changing rules, defaults, forcing, Native pins, target selection, or verification policy.

## Observed defect

Completed Native run `civvis-20261015T120650Z`, source `1d770be4c8e0d2b8c660a6ff648e8740c05b47da`, had physical Stephanie Kwolek needing a completed Spaceport at turn 178. Maracaibo (city 131073) had Government Culture production at 210/220, with one turn left. Its Conquest plan and assigned Domination target nevertheless proposed a 900-production Spaceport at (21,15), estimated at fourteen turns. The original host continuation built it through turn 192; the Scientist was absent by 194. These observations show the commitment, not that a different army would have won. The original game ended in a rival Science victory at turn 209.

Shipped `Base/Assets/Gameplay/Data/GreatPeople_Scientists.xml:48,88` requires a completed Spaceport for Kwolek and supplies a space-race production modifier. Rocketry research and ordinary engine legality are outside this production repair.

## Before and after evidence

The test-only baseline retains production bytes from main `1a13e3ff87a5b21f65550308c431211880d22b51`. Four intended regression failures and four positive controls completed before production edits. They cover fresh scientist/engineer prerequisites, a busy nonspace queue, a paused Spaceport foundation, the actual targeted Advanced caller, Science and untargeted controllers, a nonspace scientist, active queues, and restored temporary context.

The first candidate passed all eight controls but failed the exact Native query: after its Great Person reservation was removed, the ordinary opening governor still chose a Spaceport. That failed proposal and its source/binary hashes are retained. The corrected candidate also supplies the target context to the opening governor. No failed attempt or played game was replaced.

The corrected Domination proposal no longer opens a Spaceport in Maracaibo. Science and Culture remain identical in full to the baseline. The final integrated build repeats the same three controls; see the proposal artifact for every order and journal.

The three-target controls use the original 47,876,534-byte turn-178/frame-0 prefix (SHA-256 `90ea6d78384a84998bb1de07fa6dd03e1c6238f353cb55869b82737e746ada14`), the same 283 common Native flags, and a fresh decider process. Actual captured reads are reconstructed and verified. Full Science and Culture parsed responses must remain identical, excluding only top-level input-capture metadata. These are outbound proposals; no command is executed in Civilization VI.

An additional fixed eight-query check used captured turns 179, 180, 191, and 192, with baseline and corrected Domination binaries. Its predeclared absolute expectation that the actively built host Spaceport would never be replaced **failed on both arms**: both propose Government Culture at 179 and a Theater project at 180. At 191 both restate the existing Spaceport; at 192 both propose Spec Ops. All four full response pairs are identical, excluding only input-capture metadata. The failed absolute check is retained and is not called a passing no-cancel control. The passing unit control for an active in-memory Game queue is a separate result. How an observed host queue is imported and governed needs separate investigation; this repair introduces no difference on these four snapshots. The suffix artifact, appended to the turn-178 input, reconstructs every extra captured prefix without duplicating the initial 47.9 MB.

## Compatibility and limits

All six fixed baseline/candidate report digests and completed-turn counts match across twelve completed arms. The scope is compatibility, with no game replacement, outlier filtering, or extension.

The fixed six-world, twelve-arm Deity compatibility plan was registered before games. A source amendment before zero games retained the same seeds, commands, and order after the first candidate failed its proposal check. The original registration is preserved. Physical Great Person needs are mirror-only, so these headless games cannot prove the live reservation repair improves strength. The opening governor correction is included in the compatibility comparison. No Native win, Domination win, or measured strength improvement is claimed.

All eight focused controls, existing live controls, full serial Cargo suites, all-target developer-tools checks, changed Rust quality, and final integration checks passed. Exact counts and commands are retained in the provenance artifact and the final pull request.

## Review artifacts

- [Summary and source/binary hashes](2026-10-15-domination-gp-production.json)
- [Original turn-178 input prefix](2026-10-15-domination-gp-production.native-input.jsonl.gz)
- [Continuation suffix for the additional queue controls](2026-10-15-domination-gp-production.continuation-input-suffix.jsonl.gz)
- [All retained baseline, failed, corrected, and integrated proposals](2026-10-15-domination-gp-production.proposals.json.gz)
- [All fixed compatibility reports](2026-10-15-domination-gp-production.headless.json.gz)
- [Registrations, patches, source hashes, harnesses, validation logs, and receipts](2026-10-15-domination-gp-production.provenance.json.gz)
