# Preserve observed city ownership in ladder combat totals

## Native evidence

The completed King / Gran Colombia / Tiny Pangaea / Online / Domination run
`civvis-20260926T190901Z` used decider revision `01385ba`. It lost to Mapuche's
Culture victory on turn 188. Its own city rosters first show Egyptian Swenett
at turn 183 (new owner ID `720906`, plot `38,8`) and original capital Râ-Kedet
at turn 185 (ID `786443`, plot `34,8`). Both remained owned at the ending.
Separate `city_disposition` records request KEEP for each city. The two
`city_occupation` callbacks instead both identify Swenett.

The unmodified `events.jsonl` has SHA-256
`2751e3a479dcc8e470bf347f4b1cd4721e3a0d741be55093155700816cdad94a`.
Running both existing report paths on these exact bytes gives:

| reader | cities taken | cities lost | kills | losses |
| --- | ---: | ---: | ---: | ---: |
| `civ6_tactics_ledger.combat_section` | 2 | 0 | 66 | 24 |
| previous `civ6_ladder.combat_totals` | 1 | 0 | 66 | 24 |
| repaired `civ6_ladder.combat_totals` | 2 | 0 | 66 | 24 |

## Cause and repair

The tactical ledger already reconciles callbacks with observed ownership
rosters, preserving plot identity across city ID and name changes (#3658).
The ladder's memory-saving event projection discarded local cities, rival city
identities and founding events before calling that same code. The reconciler
therefore had only the repeated occupation callbacks to count.

Retain the city identity fields that reconciliation needs, for local and rival
rosters, and pass through `found` events. Missing rosters remain unknown;
malformed entries remain present so they cannot turn an incomplete roster into
a false claim that a city disappeared. Founding events distinguish a new city
on a razed site from recapturing the previous city.

## Validation and limits

All three new ladder regressions fail before the projection repair. They cover
the missing-capital callback, rival identity with unknown founder, a resumed
ownership baseline, missing/malformed frames, and refounding a lost site.

- `python3 -m unittest discover -s tools -p test_civ6_ladder.py`: 195 passed.
- `python3 -m unittest discover -s tools -p test_civ6_tactics_ledger.py`: 37 passed.
- The exact saved native run now agrees with the full tactical report, as above.

This changes reporting only. It neither changes the outcome of that game nor
establishes a domination win or grounds for raising difficulty. Existing saved
ladder rows retain their previous totals until explicitly recomputed; this PR
does not rewrite runtime ledgers or publish refreshed snapshots. The underlying
counter measures ownership transfers, not a separate count of capitals held.
