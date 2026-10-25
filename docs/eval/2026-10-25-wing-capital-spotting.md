# Candidate: a spare spy keeps a reachable capital in sight

Default-off `wing-capital-spotting` addresses the access problem measured in
PR #4143. Across the fixed eight completed Emperor games, only 30 observed
major-original-capital war turns have two ready Bombers currently in range;
only seven also have sight and a healthy land taker within eight tiles. These
counts do not prove a safe attack or a winning counterfactual.

Under an assigned Domination Conquest, one idle spy can post to the active
rival's original capital when two healthy Bombers physically reach it and
the host-observed capital is fogged. Each aircraft uses its own actual range;
a Jet Bomber does not lend range to an older aircraft. The spy then uses
Listening Post or Gain Sources, or waits when neither safe mission is offered.
An existing posting reserves the city during travel. No new spy is trained.

The hook follows the existing precise science-pad and culture-heist postings.
It requires another established agent to cover the leading space racer and
each culture-denial target, and preserves offered Disrupt Rocketry/heists,
running missions, home counterspy posts, forced targets and emergency home
defense. Existing dispatch and model legal actions choose the order; the native
adapter/host must still check permission and observe the resulting travel.
Installed `Base/Assets/Gameplay/Data/Units.xml:769` declares UNIT_SPY sight 3
and TeamVisibility. It does not prove any proposed posting occurred natively.

## Development evidence

The first build failed on helper visibility and a String/Name mismatch. The
corrected behavior-free stub, on the same real-spy dispatch regressions,
passes four control tests and fails exactly five intended behavior tests.
Candidate implementation then passes ten tests. Adding explicit home-counterspy
and forced-target controls yields **11 passed, zero failed** in the dev profile.
These are focused mechanism checks, not a full Rust gate or strength result.
The debug linker reports a large DWARF unwind section; no production speed
conclusion follows from that debug build.

The 27-member development archive preserves both baseline source snapshots,
the frozen final candidate source, raw failed/passing logs and manifests:
604,180 bytes, SHA-256
`17148dea2d3fbc7a7e422d2192ea3fe1deb6bc4d911a3bc61c77ea4e90cfefcd`.
All members were verified. The baseline claim is commit d56a31337; the initial
10-test candidate log is retained development evidence rather than a separately
frozen candidate tree. No default, ledger rating or native screen changes.

## Registered next validation

Before any result-dependent selection, use the same fixed eight material-cohort
games. Select the first actual decision each turn with two native air-bomber
units at HP >=50, plus the first and final actual decision of every game as
controls. Deduplicate by game/turn/frame. Reconstruct every distinct captured
read; retain missing or differing reads and unsupported contexts. Compare fresh
public planner and native-adapter queries with only this flag off/on under each
game's actual played genome. Record all actions, plans and journal changes,
including no-ops. Fresh queries do not reproduce the historical controller's
entire persisted memory and are not host execution or strength measurements.

Also register eight default-profile single-gene probe games at seeds
59003200–59003207, and eight Native-shape proxy games at those seeds: four
players, 60x38 Pangaea, six city-states, Online speed, 300-turn horizon,
Immortal rival handicaps and Firaxis-mix opponents. These are separate
experimental contracts; do not pool them or substitute their model outcomes
for native verification. Retain all scheduled games and any no-fire result;
do not replace inputs to obtain favorable firing evidence. Run the full local
CI Rust suite, relevant existing spy tests, format/quality and repository gates.

All broad validation, natural firing and whole-game results remain pending.
The active fixed 16-start `elimination-yields-to-a-launch-2` native study keeps
its registered treatment and lease. This candidate cannot change that trial.
