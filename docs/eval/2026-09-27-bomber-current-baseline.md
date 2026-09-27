# Bomber baseline after public-view repairs

Registered at `2026-09-27T04:40:52.570720+00:00`, before any new games.
This report owns evaluation only; it changes no controller or deployment option.

The completed #3802 refinement used source
`4da495555e652bd6c95c2f72e6268db34197f2a7`. Several supplied two-Bomber
campaigns captured no major city. In known seed `38021006`, reasoning reports
spotting and a sortie while the corresponding authoritative action batches
contain no aircraft orders. Reasoning comes from disposable planning frames
and is not evidence that an order executed.

The old source predates the public effective-Envoy fix (#3801), public strategic
income fix (#3804), queued-Spy legality fix (#3803), and area-damage cleanup
(#3799). Its Bomber/Aluminum/escort policy is identical to the current policy.
Current main is `59dc53c6dffc7db1248c8a6b3e9816c6acf60ff3`; the empty ownership
checkpoint `439d31ed0c099f1b4b6ba221844a2c02c0639d9a` has the same tree.
This comparison measures the combined source repairs, not an isolated air
policy treatment.

## Frozen protocol

- Fresh seeds: `38090000` through `38090007`, all retained.
- Known diagnostic: `38021006`, separately reported. Reuse its completed old
  source rows and traces; run the current source on the same seed.
- Four major players, six city-states, Tiny 60×38 Pangaea, Online, 250-turn
  natural clock, all victories, Prince player and barbarian difficulty.
- Seat zero: Simón Bolívar/Gran Colombia, explicit Domination. Three adaptive
  CIVVIS live-bridge rivals; this is not native Firaxis verification.
- Same fixed 21 forced focal options in both libraries, SHA-256
  `439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`.
  `beeline-orders-by-value` remains default OFF.
- Same observer, SHA-256
  `1ee0e84f3fd9ae54b5d43d51d82b4895cd0f85826dcd47870d2f696a6412c04d`.
- Primary: compare Strike Reach ON across sources. Retain and report every
  OFF arm; no seed substitution or policy tuning during play.
- Inspect fresh outcomes only after all fresh jobs have terminal receipts.
  The known diagnostic may be inspected after its own job is terminal.
- Verify canonical applied-action records are contiguous from zero and their
  count equals the terminal outcome. Preserve reasoning ring-loss counters.

Artifacts are retained outside Git at
`/Users/martbot-mbp-m5-max-128/civvis-tactics-results/2026-09-27/bomber-current-baseline/`.
`protocol.json` contains the preregistration. Build, executable, source,
completion, and analysis receipts will accompany the terminal report.

## Native boundary

The native owner retains UI, process, runtime, force-file and orders-database
control. This work does not mutate them. No new native game, Bomber sortie, or
Domination victory is claimed. Consistent native Domination remains unproved.

## Complete fresh results

All five jobs exited zero. The last fresh job finished at
`2026-09-27T04:50:35.403541+00:00`; analysis began after the completion receipt.
There were 32 new fresh games and two new known-diagnostic games. The two old
known rows were reused from the completed #3802 block.

| Eight-game primary, Strike Reach ON | Old 4da | Current 59dc |
| --- | ---: | ---: |
| Domination wins | 0 | 0 |
| Any wins | 1 (Score) | 0 |
| Major foreign cities ever observed held | 5 | 2 |
| Foreign capitals ever observed held | 1 | 0 |
| Own original capital lost | 2 | 2 |
| Games with at least two Bomber-class aircraft | 1 | 2 |
| Bomber-class losses | 1 | 0 |
| Aluminum-shortage boundary observations | 46 | 12 |
| Applied air strikes | 92 | 59 |
| Applied air pillages | 130 | 63 |
| Applied ground pillages | 42 | 18 |

| Retained eight-game OFF control | Old 4da | Current 59dc |
| --- | ---: | ---: |
| Domination wins | 0 | 0 |
| Any wins | 0 | 1 |
| Major foreign cities ever observed held | 4 | 9 |
| Foreign capitals ever observed held | 0 | 0 |
| Own original capital lost | 1 | 1 |
| Games with at least two Bomber-class aircraft | 6 | 4 |
| Bomber-class losses | 7 | 4 |
| Aluminum-shortage boundary observations | 15 | 76 |
| Applied air strikes | 376 | 194 |
| Applied air pillages | 250 | 209 |
| Applied ground pillages | 31 | 21 |

All 16 matched source action streams differ. Every first difference belongs
to a rival (seat 1: seven; seat 2: five; seat 3: four), so this is especially
unsuited to a focal-policy causal claim. All canonical action records are
contiguous from zero and match their terminal applied-action counts.
Reasoning ring-loss counters remain in `completed-analysis.json`.

The current primary produced two planes in seed `38090002` at observed turn
167, captured two major cities, and lost to Religion at 236. Seed `38090006`
reached two planes at 202 and lost to Science at 209 without a major capture.
The other six primary games never observed a Bomber. The earlier eight-game
refinement's zero Domination wins remain separate evidence in #3802.

These results do not establish stronger or consistent Domination. Current
main is the baseline for subsequent policy work; no option is promoted by
this report.

## Known diagnostic: planning and execution diverge

Known `38021006` is excluded from fresh strength counts. ON changed from a
Science loss at 226, first two aircraft at 156, and zero major captures to a
Religion loss at 210, first two aircraft at 152, and zero major captures.
The retained OFF changed from a Science loss at 247 with two major captures
to a Science loss at 244 with none.

Separate diagnostic drivers use unchanged production `Ai::take_turn` and
clone the AI/world only to probe a first frame. The old driver exactly matches
the 28,302 canonical applied actions through observed turn 173; the current
driver exactly matches 36,578 through the first boundary observation at 210.
These probes are never counted as additional games or wins.

In old turns 173–178, the first refused order is a white-peace `ProposeDeal`
with player 1, returning `invalid diplomatic deal`. Each frame stops before
its planned Courser 612 spotting move and Bomber 777 city volley. The same
maneuver can therefore appear three times in reasoning while no sortie is
committed. The current source also loses the peace restriction: at turns
209–210 the world returns `peace_available_at(0,1) = Some(216)` while its
decision view returns `None`. The view clears `wars`; that record supplies
the minimum-duration permission check. This confirms a public-permission
mismatch, not a Bomber visibility refusal.

The current turn-210 frame stops for a different immediate reason. A wine
trade is refused as `invalid trade terms`; the executor continues because
it is an economic refusal. An attempted upgrade of aircraft 749 then fails
with `this unit cannot be upgraded here: not enough gold` and ends the frame
before the planned Cavalry 474 and aircraft 670/703 volley. Its forecast cash
never arrived. At turn 208, a refused air pillage also ends a frame; that
separate refusal is retained and not attributed to peace or upgrades.

The next repair should preserve authoritative refusal/permission information
when replanning. Repricing the city attack alone would not repair these
measured execution gaps. Existing native host upgrade observations already
provide a refusal channel; its simulator adapter needs separate verification.

## Reproduction and validation

- Frozen current executable SHA-256:
  `081766e03fcce6149fe531937a3c08f77910e589cf33a5399acadf63f63d9a5f`.
- Frozen current library SHA-256:
  `8735d8e607a3475752436d8b59900177ae7ea75282602602b8b571b08c1329c5`.
- Old executable SHA-256:
  `8133b7bcb06c7aeea2e2bd1a8a4822d31760895b0f5a8bc84d373328cad1abd7`.
- `freeze.json` records exact compiler-selected dependencies and commands.
  `run-started.json` and `run-completion.json` record process identities and
  terminal statuses; `analyze_complete.py` rejects incomplete blocks.
- `known-analysis.json` and each frame-probe directory retain diagnostic
  completions, exact-prefix validation, planned orders and authoritative
  refusal reasons. Frozen scripts and binaries remain outside Git.
- Explicit library and native CLI build passed. Full tests and final
  documentation checks are recorded before integration below.
