# Preserve a recruited person’s activation district

Native King verification run `civvis-20260920T140616Z` repeatedly reserved a Holy Site for Hildegard of Bingen, then replaced it during the same production review. At turns 84–86, Maracaibo switched to a University; later cities repeated the conflict. The native unit reported `GREAT_PERSON_INDIVIDUAL_HILDEGARD_OF_BINGEN`, `required_district=DISTRICT_HOLY_SITE`, `can_activate=false`, and no activation plots. This is a physical Scientist already recruited, not a forecast of future Prophet points.

The physical-person planner already chooses the missing district and prevents duplicate starts. The strategic governor now retains a legal queued district of that required family while the empire lacks a completed one. The reservation disappears when the live requirement disappears or a matching district exists. It follows the planner’s existing exception for wonder-activating Imhotep and Eiffel. Local siege defense and the existing upkeep-recovery override retain their priority.

The regression runs the actual reservation and production passes consecutively with zero invested production. Before the change, the governor replaced the Holy Site with a Granary. Five focused tests pass after the change, also covering no live person, an unrelated activation requirement, an already completed district, and defense of a damaged city.

Replay input is the native event prefix through turn 108, SHA-256 `545e03c096c112e2aa69c9105f2673ee6a49aa74eb44005e085a4354c6f85747`, 20,309,937 bytes. The harness emits at the first await for each frame; for a frame without an await it emits after the full tile snapshot, or after state when that frame has no tile update. This recovers frame 17/3 and matches all 313 native baseline decisions exactly. Frozen observations after a changed decision do not prove native completion of the district or activation of the person.

Both current-base (`d29b3a0c5`) and patched replays completed all 313 frames. The current base also matches the native baseline exactly. The patch changes 47 exported-order frames, of which 46 change actual actions and 35 change immediate production orders. At turn 76/frame 0, Bogotá exports `DISTRICT_HOLY_SITE` at native (44,25) instead of `UNIT_HORSEMAN`; the later repeated University replacements in Maracaibo disappear. These are frozen-observation comparisons, not claims of a completed Holy Site or a game win.

Full-suite results will be recorded before shipping.
