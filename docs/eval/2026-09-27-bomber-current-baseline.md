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

## Results

Pending complete frozen runs. The earlier eight-game refinement produced zero
Domination wins in either source; that completed evidence is kept in #3802.
