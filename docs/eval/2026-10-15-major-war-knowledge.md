# A living major war whose cities have not been located

Native Emperor attempt `civvis-20261015T041838Z` declared its faith-defense
war on Sumeria at turn 47. The turn-47/frame-1 host state reported three
Sumerian cities through `public_stats.city_count`, with no city locations
visible. Cahokia joined its suzerain's war. The plan selected Cahokia as its
primary campaign, and its siege force began marching toward that minor.
This establishes a targeting diversion, not its effect on the game's outcome.

The mirror already preserves the public empire count and derives
`Game::unseen_city_owners` when that count exceeds its located city records.
The old active-war filter required a located city and discarded the major
despite that evidence. The repair also admits a living at-war major with the
existing unseen-city fact. It keeps the major-first priority and does not
manufacture a city objective. Missing public evidence, zero reported cities,
peace, elimination and a located major city are covered by controls.

Checkpoint `217fe4827` ran before the repair in GitHub run `38692575247`.
After its fixture preconditions passed, the assessment named player 2 (the
minor) instead of player 1 (the publicly surviving major). Four other
controls passed before fail-fast; the located-city control was not reached.
The repaired checkpoint `2afb7e162` passed all six focused controls and the
complete GitHub Rust suite: 5,946 passed, 51 existing skips and four ignored
documentation examples, in run `38694131697`.

Final integration `3753190b3` includes air/land handoff merge `317127e25`.
Its complete local serial suite (`cargo test --profile ci --locked -j 4 --
--test-threads=1`) passed 5,953 tests, with 51 existing ignored tests and
four ignored documentation examples. The same integrated revision passed
all GitHub checks and 5,953 Rust tests in run `38697733201`. The final
validation checkpoint changes only this evidence note.

The complete private native source `5c5fed372` carries the same repair on
parent `048be52e8`, preserving all 652 gene positions and the existing
native deployment list. It is backed at
`refs/civvis/wip/codex-goal-1014-native-major-war-knowledge`. Hosted Apple
Silicon run `38693904822` passed 113 focused controls across nine filters,
verified 2,521 tracked source files and packaged both native executables.
The downloaded executables passed local simulator and orders smoke runs.
The orders executable SHA-256 is
`44c7e5bb91ceae14ab7c412f4338c17b0acc5325307cf925cfbcf3ad177f9045`.

The baseline and repaired orders executables were then given the same
recorded turn-48/frame-0 input from attempt `041838Z`, snapshot 119: 6,038,677 bytes, SHA-256
`490d32cb747899887f23f62daa9b9fde6cbe4aaab41ca7a6ff67d8f1aad9d307`.
Both captured input archives matched those exact bytes. Both commands used
the same 280 policy flags and `--explain`. Source `048be52e8` named modeled
player 4 and journaled "Campaign aimed at Cahokia" with Cahokia as its city
objective. Source `5c5fed372` named player 1 and journaled "Campaign aimed at
Sumeria" with "no city objective yet". The first harness attempt omitted
`--explain`: the repaired binary selected Sumeria, but the journal assertion
could not run. Those outputs were retained, and both binaries were rerun
with identical journal settings; no AI source or artifact was changed.

This is a fresh one-shot mechanism comparison on identical native bytes.
It does not reconstruct persistent AI history or establish a counterfactual
outcome. Fresh native attempt `civvis-20261015T072322Z` then verified the same
orders executable, Emperor/Tiny Pangaea/Online/Gathering Storm, Simon
Bolivar, four major players, six city-states, all victories and a 650-turn
limit. Its 271 effective forced treatments match the preceding three-game
`048be52e8` cohort. All 55 captured decisions reported complete recording.
The CIVVIS page's New Game control operator-retired it at turn 24, before a
war-targeting case. It is neither a completed result nor a loss. The
replacement owner's game was preserved; this attempt was not requeued.

No native conquest, win-rate improvement or highest-level victory is
established by this repair.
