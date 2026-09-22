# Reach the industrial branch before another ground upgrade generation

Native `civvis-20260922T054400Z-cont1` ended in Scotland's Culture victory on turn 191. The runtime used `03671874f3271a5df035295ffdbfe1c59cb9109a`, binary SHA-256 `9a3db0bb042e8913b761a86f2b6fc2216ee6cc804feb818d1acab74f6a3fb396`. Ballistics was known on turn 127 and Education on turn 130. Industrialization, Flight, Radio and Advanced Flight were absent through the final state; Steel arrived on turn 179 and Refining on turn 186.

The initial hypothesis was repeated interruptions of an existing air beeline. Source and native reasoning do not support that as the main explanation: the log first appoints an air surge on turn 177, while standalone preparation requires Industrialization. Capping interruptions inside an active appointment would miss most of this game. The baseline replay reproduces the native research choices: Siege Tactics toward Steel at 130, Economics toward Advanced Ballistics at 155, and generic Refining instead of Mass Production at 179.

The candidate supplies one missing milestone, without changing the existing appointment's ground-upgrade exceptions. With air-surge-2 enabled, an explicit Domination empire that already knows Education and Ballistics can finish Industrialization before further optional modernization. It needs two cities, a safe home, no unrevealed fuel required by the standing army, and enough remaining turns for the industrial research plus the existing endgame reserve. An active air appointment retains its original policy. Knowing Advanced Flight makes this preparation unnecessary. After Industrialization, the existing readiness policy owns research and production.

This is a research-priority experiment, not a demonstrated win-rate improvement. Ballistics and Education preserve an early military and science foundation; the deadline prices the industrial milestone itself rather than assuming a bomber campaign is already appointed. The change creates no war, forces no unit purchases, and does not alter the native controller, pinned verification binary, or command bridge.

## Validation in progress

The isolated baseline is main `85744fb48b6ff59ecc27014f4ed20f2503bee143` plus the empty claim `019fa0b2a16d5d95e75dc94716c0f930a040a98a`. Its binary SHA-256 is `9121bd184e6a87ac7a2abf37c67cc446a610020645b65d584b22f3bc938cb589`.

Two independent baseline replays complete: the original segment has 299 frames (turns 1–102), and the restarted segment has 266 frames (turns 101–191). They are not concatenated into a continuous counterfactual game. Arguments are reconstructed from each native brain header. Input hashes are `3edee2ce191a4e39519d9a9eca54aae832e413ba582f758d71d2ed179a0db352` and `59acc5dfd99b2b35d3ae4efd3a0ce4ce5f2ee112295a72455a172b77a2dce1c8`. The host retains arguments, immutable input copies, binary and replay orders under `/tmp/civvis-3722-replay`.

On the unchanged baseline, both positive regression tests fail at the missing Industrialization goal and the negative-control test passes. Candidate regression tests, replay comparison, full checks and integration remain pending. No native Domination victory is established by this investigation.
