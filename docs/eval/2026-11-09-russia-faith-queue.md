# Russia Faith building queue regression

Native run: `civvis-20261109T140800Z`, Russia/Peter, Deity, six-player Small
Continents, Online speed, 250-turn cap. The `seat` event reports
`RULESET_EXPANSION_2`; this is live ruleset evidence, unlike the cached
DebugGameplay database discussed in #4839. The seat founded Buddhism with
Work Ethic and Pilgrimage at t24.

## Observed production conflict

The capital's Lavra was pillaged repeatedly. The repair path actually worked:
its pillaged flag clears at t50, t79 and t101 after repair proposals. Thus a
single permanently broken repair is not the explanation for weak Faith.

At t50-54, `why.log` repeatedly reports the
`religion-seat-builds-holy-sites` Shrine claim. Native state at t50, t53 and
t54 lists `BUILDING_SHRINE` buildable, cost 35, zero invested production and
four turns to finish. Nevertheless the subsequent capital queue is:

| Turn | Native queue | Lavra pillaged |
| --- | --- | --- |
| 50 | idle | false |
| 51 | Granary, 12 progress | false |
| 52 | Granary, 30 progress | false |
| 53 | idle | false |
| 54 | idle | false |
| 55 | Settler, 9 progress | false |
| 58 | Settler, 36 progress | true |
| 81 | idle, Shrine still buildable | false |
| 82 | Shrine, 10 progress | false |

These are first exported states per turn; they do not claim that every
intermediate action is represented. The source explains how the claim can
be lost: `claim_religion_seat_faith_economy` runs before ordinary production,
`advanced_production` lacks a reservation for its Faith buildings, and
`preview_live_production` omits that claim unless an unrelated preview
claim path happens to provide it. A journaled choice is not final production.

## Intended fix and limits

The existing opt-in `religion-seat-builds-holy-sites` treatment should carry
its Shrine/Temple decision through ordinary strategic rescoring and the
production preview. Preserve its Religion-target, founded-faith, Holy Site,
opening-book, legal-production, threatened-city and recent-attack gates.
Civilization-specific Temple replacements must use their building family.
Emergency military decisions and insolvency retain their current precedence.
No new global preference or unconditional Russia pantheon choice is implied.

A full-turn regression and production-hint regression must fail on the old
code, then pass for both Shrine and Temple. Test the hint with and without
`hint-runs-the-claims`: religious queue correctness must not depend on that
separate opt-in treatment. Include another victory target, the treatment off,
a newly attacked city, a threatened city, an unavailable building, an absent
Holy Site and a Temple replacement in the reservation tests.

This regression verifies production policy. It does not prove a native
Shrine/Temple completion, additional Apostles, a converted rival majority or
an improved religious win rate. Repeated pillaging remains a separate
military/economic problem to measure.

## Sources

Source functions are in `src/ai/advanced.rs` and
`src/ai/advanced/religion_seat_fields_apostles.rs`. Native evidence comes from
`events.jsonl` and `why.log` in the named control run. Firaxis
`Base/Assets/Gameplay/Data/Buildings.xml` defines `BUILDING_SHRINE` with
`PrereqDistrict="DISTRICT_HOLY_SITE"` and defines `BUILDING_TEMPLE` with
`PrereqCivic="CIVIC_THEOLOGY"`; the policy relies on the existing full legality
gate rather than bypassing those requirements.
