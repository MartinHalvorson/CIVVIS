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

## Completed captured-input comparisons

The fixed selection contains 136 decision frames and 172 distinct reads,
representing 2,432 captured controller reads. All 344 off/on native-adapter
queries succeeded under each game's actual played genome, changing only the
new spotting bit. Exactly one read changes an order: game
`civvis-20261025T124842Z`, turn 174/frame 0, snapshot 517/read count 18.
Spy 9502731 changes from Gain Sources in Taruga (14,15) to travel to Istanbul
(33,24). Spy 9699340 still disrupts rockets at (32,4), and spy production
orders remain unchanged. The other 171 reads have identical proposed orders
and planned actions. The earlier development query at turn 195, which changes
nothing, is retained too.

The first complete pass omitted explanation logging. A second pass over the
same entire selection enables it: all proposed orders and planned actions
match the first pass exactly, all input/genome controls match, and the single
changed read has the candidate's decision journal entry. These fresh adapter
queries lack the historical controller's complete persisted memory. They prove
a proposed policy action, not host permission, completed travel, sight, damage,
or improved outcomes.

The 1,678-member native-query archive retains both complete passes, raw
responses, manifests, exact genomes, selection and reconstruction scripts,
frozen build metadata, the development no-op and initial CI failures.
It is 1,217,304 bytes, SHA-256
`9a16b131a148376d43772351e6765c7d8d998a68599f51c0e8b35d52aab08106`;
all members were verified. Its native binary was compiled from f029965ed and
has SHA-256
`de35ca1cf7150ff7e6d798928e036673572a4d3f531e6575f724d29ae908d5ba`.

Registration placement now passes all 14 append-point controls. Initial
GitHub Rust tests passed 6,298 tests (52 skipped), but the complete gate failed
on missing firing evidence; the initial policy gate also found registration
and generated-metadata defects. These failures are retained, not described as
a green gate. Regeneration keeps the ledger and deployed defaults identical
and lists spotting as off and unmeasured. Full validation is still pending.

## Whole-game observation status

Both registered eight-game sampled batches completed: 48 default-profile seats
and eight measured Immortal-proxy seats (32 total seats). All six victory
conditions remained enabled. The proxy is explicitly assigned Domination;
its measured seats win three games through Culture or Religion and none by
Domination. These are model outcomes, not Native verification.

The sampled design draws the spotting bit independently, so an unpaired
contrast is insufficient proof of activation or benefit. A separate matched
instrument reuses the unchanged gameplay routine, restores every original
rival genome and varies only the measured seat's spotting bit. In the first
fixed seed both initial worlds, complete action traces and terminal worlds
are byte-identical. The remaining seven fixed pairs are registered and
running; all results, including no activation, will be retained.
