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
control also passes. Every captured read in the two protocol-selected examples also matches: Rio
turn 157/frame 0 has one distinct read across eighteen calls; Uruk turn
165/frame 0 has two distinct reads across eighteen calls. All three exact input
prefixes end in the unique actual Native state, and every candidate tile record
is identical in the census and captured input. Explicit capture-directory
remapping reproduces all read digests, states, decisions and tile records from
disposable relocated archives. It leaves the original descriptors untouched.

The installed Gathering Storm rule additionally allows volcanic soil without
clearance (`DLC/Expansion2/Data/Expansion2_Improvements.xml:201`). Amendment
commit edcb79565 registers this correction before the complete second run.
The first report/witness and source remain retained. Both full reports have
identical data rows and source-run hashes; the correction changes no observed
count. Result 02 and the final witness are authoritative. All twenty-four
actual/requested/seat receipts identify `RULESET_EXPANSION_2`.

Final Git/CI integration is pending. No gameplay, genome or runtime setting is
changed by this audit.

The final archive preserves 31 byte-verified members, including
both filters/reports, original witnesses, relocation control, source identities,
primary searches and controls: **152,156 bytes**, SHA-256
`8426a2038fed0bf2283e782144df4d4859b6dd124d32a47812fd6edfe881e75d`. The original sixteen-member archive remains
separate (65,848 bytes, SHA-256
`03f9c932e716f74e6853dd1e716ee9a5ddc4cf3fb50fc86bda3e624fef8811eb`).

To reproduce, run the companion tests and census script with `--control` and
`--air-state` pointing to the restored fixed runs and final sixteen-start
assignment state. Use a new output path. The witness script takes that census,
`--control`, `--tools` and a new `--out`. On another machine, `--capture-map`
accepts a JSON map from original capture-directory strings to restored absolute
directories; only an in-memory descriptor copy changes. Raw Native events,
decisions and captured bytes/indexes are already committed in the eight
material and sixteen Culture-air cohort archives. Compare their retained
file hashes before interpretation. Neither terrain filter proves a safe
Engineer route, legal future improvement, free air slot or a winning campaign.
