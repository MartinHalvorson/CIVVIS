# Preserve air preparation once the denial war is underway

Native run `civvis-20260920T080404Z` on `83886cee5` repeatedly appointed an air surge against Thebes and immediately stood it down with “victory denial superseded the surge”. At turn 176 Egypt was already at war with Gran Colombia. The plan still needed Advanced Flight; cancellation removes its research and production reservations.

`air_surge_opening` checked urgent victory pressure before asking whether a diplomatic opening was needed. Urgent pressure should bypass peacetime readiness, but during an existing war there is no declaration to accelerate. Returning to the ordinary wartime controller can retain the air package. This also applies after a surge has made its own declaration, not just one appointed during an existing war.

Three regressions cover an appointment during war, a surge that already declared, and urgent peacetime override. The two wartime regressions fail before the fix; the urgent peacetime control passes. Afterwards all 11 focused air-surge tests pass (one ignored), and the full suite passes 3,661 library plus 204 binary tests (49 library and four doc tests ignored). No native capture or win-rate effect is established by this finding.

The same-base replay uses parent `f1c0da515` in both arms, with only this fix changing production code. Both complete 515 decision frames through turn 184 with exit zero. Actionable orders differ on 44 frames, beginning at turn 167 frame 0. There the baseline researches Sanitation while the fix researches Advanced Flight; turn 172 similarly changes Replaceable Parts to Advanced Flight. The recorded host states do not execute replayed orders, so this demonstrates retained preparation and changed decisions, not technology completion, aircraft delivery, captures, or win rate. Artifacts: `/tmp/civvis-wartime-air-replay/`.

No game rules or attack thresholds change. The existing wartime tactical controller still acts, and urgent peacetime denial still bypasses the air-readiness wait. The native match remains pinned to its launch revision until its outcome or recovery budget ends.
