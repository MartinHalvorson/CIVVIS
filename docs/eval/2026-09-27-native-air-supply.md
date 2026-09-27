# Native Bomber supply observations

Three completed four-player King games as Gran Colombia fielded no Bomber or
Jet Bomber. Two never observed positive Aluminum income. The third first
observed income and a Bomber queue on turn 167, with its last board on 169 and
a rival Culture victory on 170. This is a post hoc native diagnostic, not a
policy comparison or a proof of the cause of each loss.

`tools/civ6_air_supply.py` now measures this pipeline from native observations.
Completed-game recording and backfill of unrecorded runs carry `air_supply` on
the live ladder row. Publishing a run retains it in the remote summary even
after raw evidence is pruned. Raw local summaries and games remain untouched.
Already recorded historical rows are not silently rewritten; the read-only
command can inspect their retained evidence.

## Meaning of the readings

- Research selection and production proposals do not prove completion. Radio
  and Advanced Flight milestones require membership in the observed `techs`
  list; queues require the city's observed `producing` field.
- A usable Aerodrome must be observed complete and unpillaged. Missing district
  metadata stays unknown.
- The wing counts our state `units`, including native
  `PROMOTION_CLASS_AIR_BOMBER` and both standard Bomber unit types. Jet upgrades
  do not appear to destroy a wing. Rival and minor unit lists are excluded.
- Aluminum income is the exporter's gross accumulation plus imports and
  bonuses, before unit or power demand. Positive stock and positive income
  have separate milestones. A missing income key or whole stock table is
  unknown; an omitted Aluminum key in a present sparse stock table is zero.
- Deposit history uses native plot coordinates and observed owners. Major and
  minor identity comes from `rivals` and `minors`, not numeric seat ranges.
  Deposit sight changes do not count as acquisition. Ownership and a known
  unpillaged Mine are separate observations; neither is asserted to cause an
  income reading, which may include imports.
- Scope is one run segment. A resumed first-frame wing is marked present at
  the first frame, not assigned an invented build date. Stale frames are
  excluded; repeated replan frames do not become extra observed turns.
- Coverage records measured and unknown frames for income, stock, wing and
  usable bases. Missing evidence cannot be treated as an observed zero.

Export contract: `tools/civ6_control/mod/CivvisControlAgent.lua:7426` exports
`class = unitClass(name)`. The district observation at line 6800 exports
`complete`. The resource block at lines 9247–9264 exports accumulation, imports
and bonuses, while the stock block at 9268–9286 omits zero entries and returns
nil when the table has no positive entries. This change reads those fields;
it changes no controller, native action, game rule, gene or deployment force row.

## Complete native cases

All three seats read back four players, `DIFFICULTY_KING`,
`CIVILIZATION_GRAN_COLOMBIA`, `LEADER_SIMON_BOLIVAR`, Tiny Pangaea, Online,
Gathering Storm and all victory conditions enabled. Their assigned controller
target was Domination and `air-surge-2` was enabled. Actual game cap was 250.

| Native run | Radio observed | Advanced Flight observed | Usable base | First positive Aluminum income | Bomber queue | Peak wing | Last board / rival Culture outcome |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `civvis-20260927T091843Z` | 152 | 159 | 147 | 167 | 167 | 0 | 169 / 170 |
| `civvis-20260927T094432Z` | 166 | 173 | 161 | none observed | none observed | 0 | 182 / 182 |
| `civvis-20260927T100740Z` | 138 | 142 | 136 | none observed | none observed | 0 | 204 / 204 |

The observer inspected all 1,563 state frames: 479, 506 and 578 respectively.
Income, wing and usable-base coverage is complete in all three; stock is
unknown in 128, 62 and 170 early frames respectively. Last Aluminum stock is
3, 0 and 0. The first game's last income is zero despite its earlier positive
reading. The second game's five observed deposits all belong to majors. In
the third game, `(41, 11)` is observed unowned on turn 140 and owned by major
player 3 on turn 164; `(54, 31)` remains last observed unowned. None of its
observed deposits is owned by the focal seat.

The third game held one foreign major city from turn 187, one minor city and
no foreign original capital at its last board. The ownership observer in
`tools/civ6_conquest.py` supplies these separate counts; the native combat
summary's two city captures do not establish two major conquests.

These facts prioritize supply acquisition and campaign timing. They do not
establish that either unowned deposit was a safe, reachable colony. The current
colony policy requires protection, Loyalty safety and enough time to arrive.
The separate resource expedition in PR #3793 remains draft and withheld after
inconclusive or regressing pilots; this diagnostic does not approve it.
The eight-pair King `air-surge-2` ablation in PR #3823 remains unchanged:
zero Domination wins in either arm, deployed policy retained.

## Reproduction and validation

```sh
python3 tools/civ6_air_supply.py /path/to/native/run
PYTHONPATH=tools python3 -m unittest tools.test_civ6_air_supply tools.test_civ6_ladder
```

Read-only native results, event and summary hashes are retained at
`~/civvis-war-evidence-20260927/native-air-supply/native-terminal-observations.json`.
This archive preserves seat, source revision, outcome, coverage and deposit
histories, with no seed replacement or game restart. Regression tests cover
upgrade counting, missing versus zero values, queue versus completion, native
seat identity, ownership deltas, repair, resource disappearance, stale frames,
resume scope, gzip input and an incomplete live final line. Ledger tests cover
automatic recording, backfill and publication without changing raw summaries.

No engine change or strength claim; no new campaign sample is required to
validate this observation and retention change. The ongoing King native game
keeps its pinned controller until its normal completed-game boundary.
