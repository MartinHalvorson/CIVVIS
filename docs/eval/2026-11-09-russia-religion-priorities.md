# Russia religious strategy audit, 2026-11-09

Audited revision: `a73dbe3a11`. This is a source and native-rule audit,
not a measured win-rate improvement. The ongoing objective remains open.

## Verified decision gap

`src/ai/empire_turn.rs::pantheon_effect_reach` explicitly returns zero for
Holy Site terrain adjacency effects. This includes
`holy_site_tundra_faith`, Dance of the Aurora's only effect in
`data/beliefs.json`. Thus the terrain board score for Dance is zero even
when a Russian Lavra could stand beside six tundra tiles. The board-scoring
flag does not fix this: it calls the same function. This is a source-level
fact, not a claim that a recorded Russian game chose the wrong pantheon.

The chooser adds a standing-list prior of one point per rank
(`src/ai.rs::PANTHEON_PRIOR_STEP`). Reproducing its list from the catalogue
with the expansion/Prophet-race prefix gives 23 pantheons:

| Candidate | Rank | Prior | Dance terrain board score |
| --- | ---: | ---: | ---: |
| Religious Settlements | 1 | 23 | n/a |
| Divine Spark | 2 | 22 | n/a |
| Dance of the Aurora | 8 | 16 | 0 |

Even pricing one six-tundra site at +6 Faith would only tie Divine Spark's
prior, and the existing tie break favors Divine Spark. Therefore adding a
tundra tile count alone does not establish an effective Russia policy.
Simply counting all owned tundra would also overstate the payout: only
neighbors of a completed Holy Site pay this adjacency.

## Native mechanics checked

These are installed Firaxis files, relative to `Civ6.app/Contents/Assets`:

- `Base/Assets/Gameplay/Data/Beliefs.xml:736-781`:
  `DANCE_OF_THE_AURORA_FAITHTUNDRAADJACENCY` and its hills counterpart
  each target `DISTRICT_HOLY_SITE`, grant `YIELD_FAITH`, and have Amount 1;
  terrain requirements are respectively `TERRAIN_TUNDRA` and
  `TERRAIN_TUNDRA_HILLS`.
- `Base/Assets/Gameplay/Data/Districts.xml:60`: Lavra has base Cost 27
  and requires Astrology. Line 154 assigns it 2 Prophet points per turn.
  These are database base values, not a quoted Online-speed build cost.
- Native `DistrictReplaces` maps `DISTRICT_LAVRA` to `DISTRICT_HOLY_SITE`.
  The policy must use the district family and civilization variant.
- `DLC/Expansion2/Data/Expansion2_Beliefs.xml:831-839`:
  `WORK_ETHIC_ADJACENCY_PRODUCTION_2` mirrors `YIELD_FAITH` to
  `YIELD_PRODUCTION`. This benefit depends on the expansion rules being loaded.

The local cached `DebugGameplay.sqlite` at audit time instead attached
`BELIEF_WORK_ETHIC` to `WORK_ETHIC_FOLLOWER_PRODUCTION`, with production
Amount 1 in its child modifier. The cache alone does **not** prove the active
run's ruleset. Before evaluating the adjacency/Work Ethic combination, read
back the run's reported ruleset and capture its loaded belief rows. Do not
substitute the cache for that live observation.

## Existing religious work to retain and verify

The Russia process `civvis-20261109T140020Z` was observed alive with
`--leader LEADER_PETER_GREAT`, `--difficulty DIFFICULTY_DEITY`, six-player
Small Continents, Online speed, `--civvis-victory religious`, and a 250-turn
cap. Its brain log records revision `a73dbe3a11` and these forced treatments:
`religion-seat-races-the-prophet`, `religion-seat-earns-its-prophet`,
`religion-seat-builds-holy-sites`, `religion-seat-fields-apostles`,
`religion-seat-takes-the-faith-beliefs`, `religion-seat-presses-the-campaign`,
and `religion-finishes-majorities`. Process launch and forced flags do not
prove successful native actions or a victory. No outcome is claimed here.

The source already prefers Work Ethic among follower beliefs. Its religious
package already reserves Apostle savings, builds Faith infrastructure, and
prioritizes unconverted civilization majorities. Reimplementing these would
not address the newly identified terrain choice gap. A separate open PR,
#4836, owns religious corps movement; this audit does not alter those hunks.

## Next implementation and acceptance evidence

1. Forecast the best legal Holy Site/Lavra site per owned city under each
   candidate adjacency pantheon. Use the existing district calculator, which
   plans before tech unlocks, and compare marginal yields to the same board
   without that pantheon. Include completed sites, avoid counting alternative
   sites as simultaneous income, and leave unknown land unvalued.
2. Make the religion-target choice explicitly compare that payoff against
   Prophet timing and the free Settler. A fixed Russia preference regardless
   of terrain is not supported by this audit. Keep the candidate opt-in until
   measured. Tests must include tundra hills, a green Russian start, occupied
   and illegal sites, a taken Dance, a pre-Astrology choice, and a Lavra.
3. Replay captured pantheon-choice boards with the same binary inputs and
   record changed proposals, native acceptance, actual Lavra placement and
   completed adjacency. Test the full chooser: a helper returning +6 alone
   misses the standing-list prior demonstrated above.
4. Evaluate paired Russia Deity games on the same seeds/settings. Record
   founding success and turn, Faith income, first Temple/Apostle, religious
   unit losses, rival civilization majorities and actual religious wins.
   Compare against the current package, not an older baseline missing its
   recent Prophet and Apostle fixes. No winner is established by this audit.

## Reproduction

Run `python3 tools/civ6_scripts.py grep` on the modifier identifiers above
to locate the shipped rows. Read the native database with SQLite read-only:

```sql
SELECT * FROM BeliefModifiers WHERE BeliefType = 'BELIEF_WORK_ETHIC';
SELECT * FROM ModifierArguments WHERE ModifierId LIKE 'WORK_ETHIC%';
SELECT * FROM ModifierArguments WHERE ModifierId LIKE 'DANCE_OF_THE_AURORA%';
SELECT * FROM DistrictReplaces WHERE CivUniqueDistrictType = 'DISTRICT_LAVRA';
SELECT * FROM District_GreatPersonPoints WHERE DistrictType = 'DISTRICT_LAVRA';
```

The prior table was recomputed from the six-element prefix in
`research_with_government`, followed by sorted unlisted keys of
`data/beliefs.json["pantheon"]`, using `len(order) - rank_zero_based`.
Source inspection, that arithmetic, the installed XML and the read-only SQL
queries were performed during this audit. No engine code changed; engine
soaks and Rust tests are not evidence for this document's source observations.
