# Keep an air campaign aligned with its active war

Native run `civvis-20260920T114721Z` lost to the Ottoman seat on turn 204.
It was pinned to `62d89c0bf8266585b6e480d19672b8d216dda028`, fast binary
`c8160ba5126ed423354a1abf5431cbbb798f35dfb9efcb9fea6280a6ff1a1ae3`, with
King difficulty, Simon Bolivar, Tiny Pangaea, Online speed, the assigned
Domination target, and all victory types enabled. Its summary records zero
cities taken in combat. Taumutu joined from the Free Cities seat on turn 81;
that ownership transition is not a verified military capture.

The air plan appointed Ottawa during peace on turn 123, after peace with the
Maori. War with the Ottomans began on turn 128, frame 1. Once the wing was
ready on turn 167, the air overlay kept replacing the Ottoman campaign with
the old Canadian objective. Diplomacy then refused the extra war, and the
turn-179 objective board contained six recon rows and no siege.

PR #3626 corrected another cause of wrong-front planning: false elimination
when rival cities were hidden. A second replay of that fix, covering 521
frames of this recording, changed early orders on turns 75–92 but left the
late Canada/Ottoman conflict unchanged. This is a separate lifecycle defect:
existing elective appointments did not reconsider their target when a
different major war opened.

## Change

The air lifecycle redirects an unstarted war plan to the sole active major
front when the existing candidate selector can find a reachable city. It also
selects a replacement on that front if the objective changes hands. It retains
the original appointment, breakthrough, and review clocks, without imposing
an abort cooldown or throwing away the research milestone and standing wing.
A captured counterattack objective is counted once before continuing.

Trablusgarp at host (29, 9) belongs to the Ottomans through turn 132, Free
Cities from turn 133, and Canada by turn 170. At turn 133 the remaining known
Ottoman cities, Aydın and Ankara, are respectively 15 and 19 hexes from the
nearest own city. `data/units.json` gives the Bomber a range of 10. No eligible
replacement exists on this observation stream, so the final planner correctly
ends that air appointment. Radio at turn 137 and Advanced Flight at turn 142
are replaced by Combustion requests for the ground war. This is an explicit
research tradeoff, not evidence of an earlier bomber breakthrough or a better
native outcome. The continuation tests cover the case where a reachable
replacement does exist.

If no eligible counterattack objective is known, or multiple new major fronts
are active, the elective package stays in preparation. It keeps its research
and production commitment but cannot enter Strike and override the ongoing
war's strategy. A plan already fighting its target keeps that target; peace
and minor wars retain the existing elective launch behavior.

## Validation

Ten focused tests cover immediate redirection with research continuity,
preserving the breakthrough clock, an unreachable counterattack, an existing
war with a second front, two new fronts, peace, a minor war, a city changing
owner, capture accounting across continuation, and no replacement objective.
Four of the initial seven failed against the original implementation while
three controls passed. The ownership-transition cases exercise the additional
failure found by the first replay.

The comparison uses all 599 decision frames of the completed native recording,
with a baseline that already includes #3626. It excludes `order_failed`,
`order_verified`, and `turn_verified` telemetry and separately compares the
internal native action stream. Replay orders are alternate requests on fixed
observations, not proof of adoption, capture, or a native victory.

- `cargo test --profile ci --locked`: 3,715 library and 205 other tests passed;
  49 library and four documentation tests ignored. All ten lifecycle tests pass.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Paired replays both exited zero, completing all 599 frames (302.18 seconds
  baseline, 238.14 seconds final; these are not controlled speed measurements).
- Exported gameplay requests change in 171 frames and internal actions in 199,
  from turn 128/frame 1 through turn 204/frame 1. The first change coincides
  with the Ottoman war opening.
- The final replay redirects Ottawa to Trablusgarp on turn 128, then stands
  down that air appointment when Trablusgarp flips on turn 133 and no remaining
  Ottoman city meets the reach gate. At turn 179 it retains a 14-unit siege
  force for Ankara; the baseline has six recon rows and no siege while waiting
  on a Canadian war. The final replay does not emit the Canada-war delay.
- The Radio/Advanced Flight to Combustion substitution described above is the
  only research difference. Bomber rebase requests fall from 30 to 12; the
  same three Bomber `AIR_ATTACK` requests remain, none aimed at a known city.
  No improvement in city-strike count is established.
- Other order changes are broad: 598 unit requests removed / 573 added,
  59 immediate production requests removed / 145 added, and 124 next-production
  requests removed / 52 added. Policy, delegation, levy, and sale requests
  also change as the active war retains the strategy overlay. This establishes
  a coherent target, not that every alternate move is stronger.

Artifacts on the verification host are under
`/tmp/civvis-air-war-retarget-replay/` (labels `baseline` and `final`), with
`/tmp/civvis-air-war-retarget-frame-replay.py` and
`/tmp/civvis-3629-final-comparison.json`. Engine crash soak is not applicable:
only the AI's air-plan lifecycle changes.
