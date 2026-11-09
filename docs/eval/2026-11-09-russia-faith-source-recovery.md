# Russia restores its lost Faith source

Native evidence: `civvis-20261109T154422Z`, Russia/Peter, six-player Deity,
Online speed, Gathering Storm. The frozen decider revision was
`4a05dd664c00023e5a35cab1574a2ad21e03352d`; its binary SHA-256 was verified
against the attempt provenance in the preceding queue-retention evaluation.

The seat founded Buddhism at t19 and finished its first Shrine at t32.
`city_lost` records St. Petersburg's loss at t115. After the original capital
fell, no completed Holy Site remained in the surviving empire. The religion
itself remained founded, and surviving cities later followed Buddhism.

At t157, the native export lists two own cities and offers Smolensk a Lavra:
46 Production, seven turns, four legal placement plots. Smolensk has two
citizens, follows Buddhism, and its queue is idle. The decision portfolio
still names Religious as the primary target, and the native bank is 1,257
Faith. At t189, Novgorod also offers a Lavra (58 Production, four turns) in
an idle queue. These are legal native opportunities, not inferred placements.

The source explains a policy gap. The opt-in
`religion-seat-builds-holy-sites` claim returns below three cities, including
when a founder has lost its only completed district. The separate
`founder-keeps-two-sources` sanctuary path is restricted to Domination seats.
Neither the Shrine/Temple retention fix nor buying another Apostle can
restore a district which no surviving city owns.

## Recovery behavior

Retain the normal three-city expansion floor. Under the existing Holy Site
construction treatment, a founded Religion-target seat with no completed
Holy Site instead restores one legal site in a city following its faith.
A completed pillaged district stays on the repair path. Units, Walls,
repairs (including defensive repair projects), threatened cities and recently
attacked cities retain their queues.
Ordinary buildings and projects can wait for the missing source. Native
legality decides whether a one-citizen city can place its first district.

Choose the fastest legal completion using native turn quotes where available,
breaking ties by Faith adjacency and
stable city/plot order. Resume a legal foundation in an eligible city before
placing a new one. An unsafe, defended or refused foundation does not block
recovery in another eligible city.
The recovery investment must survive ordinary strategic rescoring, including
its first frame, and production hints must restrict recovery to the requested
city. After completion, the existing Shrine/Temple claim resumes.

## Acceptance evidence

Test one- and two-city founders, Russia's Lavra, treatment off, other victory
targets, absent founded religion, another completed or pillaged Holy Site,
foreign-faith cities, threatened/recently hit cities, military/Wall/repair
queues, blocked placements, paused foundations, complete turns and production
hints. A helper proposing a site without retaining it through the final
production decision is insufficient.

The local focused test command uses explicit compile-speed overrides:
`CARGO_PROFILE_CI_OPT_LEVEL=0 CARGO_PROFILE_CI_LTO=false cargo test --profile ci --locked ...`.
Optimized required CI remains authoritative for integration. No native
recovery completion or religious win-rate improvement is claimed by a source
change or a passing model test.

Native rule: `Base/Assets/Gameplay/Data/Districts.xml` defines
`DISTRICT_LAVRA` with `PrereqTech="TECH_ASTROLOGY"` and
`DistrictReplaces` maps it to `DISTRICT_HOLY_SITE`. Existing producibility
checks enforce ownership, placement, capacity and technology requirements.

The local pre-change test compile was intentionally stopped before its test
executed, to free resources for the native seat. It is not recorded as a
passing or failing model test. The native missed opportunities above provide
the observed control; the added model regressions require optimized CI.

Recovery reuses the controller's defensive-queue classification for repair
projects as well as ordinary `Item::Repair` queues. Its reservation and hint
coverage includes a threatened foundation with another safe city available;
ordinary projects remain deferrable. The district fixture uses the engine's
`Districts` API and provides Pottery for its legal Granary queue.
