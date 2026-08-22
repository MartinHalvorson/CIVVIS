# Preregistration — peacetime deterrence floor (PR #1297)

Date: 2026-08-06. Agent: claude-57038a10. Branch:
`agent/martbot-mac/claude-57038a10/peacetime-deterrence-floor-20260806T142949Z-641f`

## Treatment

`peacetime_deterrence` — the strongest MET major weighs on the army target in
peacetime, clamped to `PEACETIME_DETERRENCE_CEILING = 1.5` (wartime keeps its
own 2.0 ceiling; terms combine by max, never product). Met-gated so fog stays
honest. Registered as live-bridge treatment tag `peacetime-deterrence` with
measurement arm `live_without_peacetime_deterrence`.

Motivating failure: run `civvis-20260803T220954Z` — seven cities founded, Mali
declared at t157 at 894 vs 481 military, six cities lost at loyalty 100
(sieges). Both army targets were blind until the war started. Fleet context:
59% of completed runs lose at least one city; holding vs losing is 452 vs 236
median score.

## Fires-check (before any eval)

Temporary env-gated counter (`CIVVIS_DETERRENCE_PROBE`), NOT committed:
2 pairs at each scale, count turns where the deterrence term raises the target
above what the wartime-only code would return.

- Criterion: fires in deployment-scale (6p/74×46/online/250t) games on >5% of
  seat-turns. If it does not fire there, stop — no eval.
- Eval scale (4p/24×16) recorded for the inversion table only.

## Eval

- Arms: `live` (challenger) vs `live_without_peacetime_deterrence` (incumbent)
  — controlled pair, single differing axis.
- Profile: deployment-online — `--players 6 --width 74 --height 46
  --city-states 9 --turns 250 --speed online --map continents --shape planet
  --poles poles --randomize-civs --victories science,culture,domination`.
- Discovery: `--pairs 120 --seed 520000`.
- Primary outcome: paired-map result; read the direction-resolution line
  BEFORE the paired score (50.0% with ~2/120 resolving = unmeasured).
  Secondary: terminal-score direction; cities held at end (the axis the
  treatment aims at).
- If directional at p<0.05 on any primary/secondary: CONFIRM on disjoint seed
  521000 with the same profile and pairs before quoting any number.
- Decision rule: ship enabled in `enable_live_bridge` unless the confirmed
  result is NEGATIVE on wins or terminal score (then: keep the arm registered,
  drop the `enable_live_bridge` call, record the null/negative in
  docs/EVAL.md). A null with the mechanism firing ships (the live regime —
  being declared on by a 2:1 leader — is scarce headlessly; cf. #1034's
  recorded precedent), with the null recorded honestly in docs/EVAL.md.
