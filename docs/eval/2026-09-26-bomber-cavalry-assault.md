# Cavalry spotting and bomber city assaults

The operator requested cavalry to reveal cities for Bombers, then use its remaining movement to capture or retreat after the sorties. Existing tactics finished one unit at a time and scheduled siege/air units before cavalry. The native per-unit queue cannot guarantee settlement between different units.

This candidate extends the existing opt-in air surge. A bounded prepass tries up to four healthy cavalry and four nearby Bombers against the active city objective. A spotting move must leave a legal return to a position whose modeled incoming damage is below half the cavalry health. Sorties must have positive modeled value. Capture is selected only after the volley succeeds on the trial board, retains at least 30 cavalry HP, passes the existing loyalty screen, and survives the modeled reply. Otherwise the spotter returns; a subsequent observed failed breach prompts withdrawal. Selected units are reserved from the ordinary unit loop. Other aircraft retain their existing strike, pillage and rebase policy.

For the native bridge the host advertises its exact `ReplanFrames` cap. The AI reads current host visibility, not remembered coordinates or inferred simulator visibility alone. An unseen-city opening needs two remaining observations; a visible-city volley needs one. Older hosts without a numeric budget do not start this maneuver. The bridge stops the speculative batch at the required observation: first the spotting move, then the bomber volley, then the cavalry decision from the fresh board. Later orders are recomputed after that observation so they cannot rely on a city captured only in speculation. A bounded `observe/AIR_ASSAULT` batch directive requests the next existing replan even when the host refuses the move or every sortie; it cannot exceed the advertised cap. Native refusal cooldowns enter the planning board before the maneuver is selected.

The installed game uses `Base/Assets/UI/WorldInput.lua:2077–2078` to check and request `UnitOperationTypes.AIR_ATTACK` at the target coordinates. The existing control mod includes `PlayersVisibility[pid]:IsVisible(x, y)` in tile signatures, so regaining sight of already explored ground opens the same replan path as new exploration. A Lua 5.1 test exercises the actual exported functions: regain sight, frame 1, strike, frame 2, cap exhausted, and numeric seat readback.

## Evidence status

Implementation and local validation are in progress. All eight focused policy tests pass, covering successful capture, affordable return, failed observed breach, refused sorties, explicit host frame limits, and reservation from the ordinary unit loop. All 63 discovered Lua 5.1 control-mod suites pass. Rust quality checks pass. The complete Rust suite and integrated bridge tests are still running. No native game has run this candidate, and no win-rate improvement is claimed.

The completed `civvis-20260926T190901Z` baseline at source `01385ba836d0749d4391bf64c83f14a0c1624179` held Egyptian Swenett by turn 183 and the original capital Râ-Kedet by 185; owned-city snapshots show both at turn 188. The game still lost to Mapuche Culture. This is evidence of existing capture capability, not of this candidate. Native runtime adoption belongs to the verification agent at completed-game boundaries; this task never drives the active game tab or writes its order database.

## Preregistered simulator diagnostic

Compare source `5725e0ce77669bd3a87445e7bd51b414d5f8c83c` with this candidate on four complete fixed-profile pairs, seeds 37730000–37730003, using the existing `air-surge-2` evaluator toggle. Each source runs both toggle arms; compare matching arms across sources. Finish all games without early stopping. This is a small diagnostic for regressions and whether behavior changes, not a native win-rate claim or a promotion screen. The opponents are CIVVIS controllers, not Firaxis AI.

Build both with `cargo build --profile ci --locked --features developer-tools --bin victory_eval`, freeze the binaries, and record their SHA-256 hashes before running:

```sh
victory_eval --domination-pair air-surge-2 --games 4 --start-seed 37730000 --out <fresh-file>
```

The runner fixes Gran Colombia seat zero, King, four players, 60×38 Pangaea, six city-states, Online, barbarians, all victories, and the natural 250-turn horizon. Both sources carry the identical compiled live-force-on bundle. Primary checks are focal wins, major cities and original capitals held, and completion; score and turn are secondary diagnostics. Native replay proposals and actual native outcomes will be reported separately.

The first immutable opening-frame replays (190901Z turns 166, 182, 183, 184) all select a combined volley. All four objectives are already visible; they do not prove native spotting. Three trial boards forecast capture, but exported orders contain the sorties plus an observation request and withhold capture. These are fresh-AI, one-frame proposals with `replan_frame_limit=2` explicitly assumed, not native executed outcomes or reconstructions of persistent agent state.

Those replays exposed a prepass integration defect: after a simulated capture, the city-disposition prompt blocked later unit attacks. Candidate `38eb4b15f` invokes the existing disposition policy before returning to the ordinary unit loop; the capture regression now asserts no pending disposition remains. All eight policy tests pass after that correction. The initial frozen pilot at `b7a02984a` will finish unchanged, and the corrected candidate will run the same complete seed set against the same frozen baseline. No outcome-driven stopping or seed substitution is used.
