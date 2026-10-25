# Forward airstrip terrain and infrastructure census

The complete fixed cohorts remain separate: eight Emperor material starts and
sixteen Immortal Culture-air starts. All have known zero-win outcomes. This is
a descriptive audit, not a new strength comparison. Protocol commit 804be9502
precedes aggregation; all source file hashes and excluded unknown-owner sites
are retained in the first result.

| Cohort | War-capital turns | Two ready Bombers with no current base in range | Neutral flat empty terrain in range | Owned flat empty terrain in range |
|---|---:|---:|---:|---:|
| Emperor material (8 games) | 629 | 70 | 38 | 0 |
| Immortal Culture air (16 games) | 1,011 | 184 | 54 | 0 |

These are observed capital-turns, with a fresh city roster every state and
whole-plot replacement. An original major capital is classified by founder,
not current holder. Candidate terrain must cover at least two ready Bombers'
individual native ranges. Visibility is the last exported plot reading.
Terrain is not host build permission, safe access or a legal rebase.

None of these blocked states has an observed Engineer, Armory or actual
Native Engineer offer. In the existing Rio turn-157 example, nine neutral
empty sites satisfy terrain/range filters although no owned empty site does.
Flight and Military Engineering are already known there. The installed primary
row at `Base/Assets/Gameplay/Data/Improvements.xml:50` permits Airstrips outside
territory with Flight and three air slots; `Units.xml:932` requires an Armory
for an Engineer. The retained search includes scenario overrides; these
verification runs use `RULESET_EXPANSION_2`, not those scenarios.

Six new census controls and six existing capital-audit controls pass, including
unknown ownership, feature removal, roster replacement, city-state founders,
wrapped geometry and authoritative Engineer offers. The old 629/70 whole-cohort
control also passes. Captured-read witness verification and final archival/
integration checks remain pending. No gameplay, genome or runtime setting is
changed by this audit.
