# Native Gran Colombia conquest accounting — 2026-09-27

Native game records now preserve original-capital control separately from
foreign major cities and foreign minor cities. The live `record_summary` path
and missing-summary backfill both populate the ledger's `conquest` field.
This is read-only measurement; it does not change the controller, game setup,
victory conditions or existing ladder promotion rules.

The shipped `Base/Assets/UI/PartialScreens/WorldRankings.lua:1844` counts a
foreign original capital using:

```lua
if(playerID ~= originalOwnerID and pOriginalOwner:IsMajor() and city:IsOriginalCapital()) then
```

The native control exporter reads `city:IsOriginalCapital()` and
`city:GetOriginalOwner()` into each city, builds `rivals` from major players,
and builds `minors` from minor players. The observer remembers those player
identities after elimination and uses city coordinates because IDs are local
to each owner and can change on transfer. Unknown identities, missing plots
and absent original-capital metadata remain explicit evidence gaps. A current
capital is not assumed to be an original capital.

Counts mean unique city plots observed held in a recorded run segment. They
do not count military captures: a city can arrive through loyalty or transfer,
and a capture and loss between frames can be missed. First milestones are
observation dates; `present_at_first_frame` identifies already-held cities in
continuations. Final control uses the last ownership frame, not an inferred
terminal board. It distinguishes holding a capital at some point from keeping
it, and records whether the seat retains its own original capital when that
capital has been identified in the segment.

Declaration requests and observed war exposure are separate. The mod's `war`
event is emitted after a request succeeds without throwing, so it proves a
request was issued, not that the host applied it. A rival's `at_war` state
proves observed war exposure, including defensive wars. Neither a kill ratio
nor these ownership metrics is itself a victory. The standalone CLI joins the
terminal victory index to the run's own exported victory table; a Religious
win under a Domination target is not labelled a Domination win.

## Retained native observations

Six terminal King / four-player / Gran Colombia / Tiny Pangaea records from
2026-09-26 retain raw events on `MacBook Pro`. All six ended in rival victories.
These records span different deployed revisions and include two continuations;
they are diagnostic observations, not a paired experiment or a win-rate estimate.

| Run suffix | Ending | Major cities ever held | Major original capitals ever held | Minor cities ever held | First ownership frame |
|---|---|---:|---:|---:|---:|
| 173411Z | Science | 0 | 0 | 1 | 1 |
| 185025Z | Religion | 0 | 0 | 0 | 1 |
| 190901Z | Culture | 2 | 1 | 0 | 1 |
| 194755Z-cont2 | Science | 0 | 0 | 0 | 228 |
| 205122Z | Religion | 0 | 0 | 0 | 1 |
| 211003Z-cont3 | Diplomacy | 0 | 0 | 0 | 175 |

Run `civvis-20260926T190901Z` first observed a foreign major city held at turn
183 and a major original capital at 185, retained both foreign cities and the
capital, and lost to Culture at 188. This isolates late conquest as a concrete
follow-up question. Run `173411Z` held a minor city even though its combat
summary counted no military city captures, illustrating why ownership must
not be relabelled capture events.

The separate Prince run `civvis-20260927T073508Z` issued its first major war
request at 76, first observed major war at 77, and first held a foreign major
city at 224. At 238 it held one major city, one minor city and zero foreign
major original capitals. The native wedge watchdog interrupted it and resumed
the autosave; the segment is not a completed game or King evidence.

Raw derived output remains outside Git at
`~/civvis-war-evidence-20260927/native-conquest-accounting/native-observations.json`.
Reproduce any retained segment with:

```sh
python3 tools/civ6_conquest.py ~/civvis-civ6-runs/control/civvis-20260926T190901Z
```

## Process for subsequent optimization

Verify the native seat's difficulty, civilization, player count, map, speed,
ruleset and victory target before comparing results. Keep King and Prince
observations separate. Group continuations by the existing `game_id` when
counting games; never sum their segment ownership counts as distinct captures.
Preserve the exact binary revision, hash and forced/withheld genome already
carried by each ledger row.

Use major war exposure, first major city held, first major original capital
held, final original-capital control and the actual victory type to locate the
next bottleneck. Register a single treatment, seed range and full-length paired
sample before evaluation, preserve every result, and check that applied actions
actually differ. Keep simulator opponents distinct from native Firaxis games.
Score, unit kills, extra time survived and minor-city holdings do not establish
a Domination benefit or justify policy promotion.

Focused checks cover minor-capital exclusion, replacement capitals, changing
city IDs, loss and recapture, repeated frames, continuation scope, late or
missing metadata, stale frames, gzip/torn-tail input, actual victory types,
automatic recording and backfill. No engine soak applies to this observer-only
change.
