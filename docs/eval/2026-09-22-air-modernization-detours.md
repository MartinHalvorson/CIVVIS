# Reach the industrial branch before another ground upgrade generation

Native `civvis-20260922T054400Z-cont1` ended in Scotland's Culture victory on turn 191. The runtime used `03671874f3271a5df035295ffdbfe1c59cb9109a`, binary SHA-256 `9a3db0bb042e8913b761a86f2b6fc2216ee6cc804feb818d1acab74f6a3fb396`. Ballistics was known on turn 127 and Education on turn 130. Industrialization, Flight, Radio and Advanced Flight were absent through the final state; Steel arrived on turn 179 and Refining on turn 186.

The initial hypothesis was repeated interruptions of an existing air beeline. Source and native reasoning do not support that as the main explanation: the log first appoints an air surge on turn 177, while standalone preparation requires Industrialization. Capping interruptions inside an active appointment would miss most of this game. The baseline replay reproduces the native research choices: Siege Tactics toward Steel at 130, Economics toward Advanced Ballistics at 155, and generic Refining instead of Mass Production at 179.

The candidate supplies one missing milestone, without changing the existing appointment's ground-upgrade exceptions. With air-surge-2 enabled, an explicit Domination empire that already knows Education and Ballistics can finish Industrialization before further optional modernization. It needs two cities, a safe home, no unrevealed fuel required by the standing army, and enough remaining turns for the industrial research plus the existing endgame reserve. An active air appointment retains its original policy. Knowing Advanced Flight makes this preparation unnecessary. After Industrialization, the existing readiness policy owns research and production.

This is a research-priority experiment, not a demonstrated win-rate improvement. Ballistics and Education preserve an early military and science foundation; the deadline prices the industrial milestone itself rather than assuming a bomber campaign is already appointed. The change creates no war, forces no unit purchases, and does not alter the native controller, pinned verification binary, or command bridge.

## Validation

The unchanged baseline fails both positive regression tests at the missing Industrialization goal; its negative control passes. The candidate passes 62 focused air-policy tests (one ignored), including home-threat, fuel-emergency, early-development, other-lane and deadline controls. On integrated source `7e9ca63e2b65c792aee13e9acfc303027185d6d6`, the full CI-profile suite passes 4189 tests with 53 ignored. Changed-line Rust quality, both release builds, 14 treatment append checks and eight four-player, 180-turn stability games from seed 372200 pass. The soak checks stability, not this policy's win rate.

Isolated replay compares main `85744fb48b6ff59ecc27014f4ed20f2503bee143` plus empty claim `019fa0b2a16d5d95e75dc94716c0f930a040a98a` against prototype `1eba742f9598642876468081d6d40bcf4806de38`:

- Original 054400 segment: all 299 frames (turns 1–102) retain identical exported orders and internal actions.
- Independent 054400 restart: 266 frames (turns 101–191), with 11 research decisions changed. The first is turn 130, replacing Siege Tactics with Buttress toward Industrialization. Later choices take Military Tactics and Mass Production along the same branch. All other physical orders remain unchanged. Eleven following receipt frames also differ because the recorded boards still execute the original research; those replay verification mismatches are not native command failures.
- Independent 061755 control: all 464 frames retain identical exported orders and internal actions. This game already researched Industrialization at 117, before Ballistics at 121, so the candidate correctly leaves it alone. It nevertheless lost to Kongo's Culture victory at 155 with no cities captured. This is evidence that research priority alone does not solve the larger domination problem.

All three segments match their native effective forced flags, treatments and withheld genes for both binaries. Overlapping segments are not concatenated into a counterfactual game. Repeated Buttress choices are proposals against restored recorded states, not evidence that the candidate repeatedly researches a completed technology. No earlier Industrialization arrival, avoided loss, or Domination victory is established by replay. Concurrent timings are not performance evidence.

SHA-256 provenance:

- Baseline binary: `9121bd184e6a87ac7a2abf37c67cc446a610020645b65d584b22f3bc938cb589`
- Candidate binary: `04bc1be9b553e7132ef97338c36023be35f2d808d8865e558b785d4dd7fbae4e`
- Original input: `3edee2ce191a4e39519d9a9eca54aae832e413ba582f758d71d2ed179a0db352`
- Restart input: `59acc5dfd99b2b35d3ae4efd3a0ce4ce5f2ee112295a72455a172b77a2dce1c8`
- Control input: `5990a98535070db433545ca7995b6f0f1a719367bc03b472fe6fd9e19b702991`

Arguments are reconstructed from each native brain header. The host retains immutable inputs, arguments, binaries, comparisons and genome checks under `/tmp/civvis-3722-replay`. Integrated validation is separate from the isolated replay revision pair above.
