# Inspecting native Builder food alternatives

Indonesia063205 reached its first University at74 but Chemistry at136. It lost no cities; its population remained far behind an earlier native winner. In the fixed t75–150 observation window,315 of594 city-turns had0–2 Food surplus and at least2 Housing room. Only21 of those had a richer unworked Food tile observed, so broadening Food focus alone lacked support. The original executable’s readback diagnostic confirmed native Food/surplus/Housing for all7 cities at100/0. These are observations, not causal treatment evidence.

Two original-executable chronological controls through100/2 independently reproduced225 native requests and3924 actual reads each. That run’s argv/cwd were not observed live: the replay invocation was reconstructed from retained profile records. Exact bounded reproduction does not retroactively attest a live process. Local audit root: `~/civvis-tactics-results/2026-11-04/seowon-crowding/indonesia-builder-original-063205-through100`.

## Diagnostic

Use the retained event prefix in a private directory, with the same `--with`/`--without` profile as the question being investigated:

```sh
civvis_orders --mirror /path/to/private/capture --turn 100 --victory science \
  --explain-builders --builder-strategy science
```

The standalone mode emits one JSON report and returns without planning orders. It requires an explicit Builder strategy (`expansion`, `science`, `culture`, `religion`, `diplomacy`, `conquest`, or `recovery`) and refuses `--serve`, so diagnostics cannot shift the server’s one-response-per-request protocol. Startup genome metadata remains on stderr.

Every Builder report lists model-legal builder-buildable improvements, the existing selector’s worthwhile gate, intrinsic value, luxury, food/growth and worked-production premiums, existing improvement value, marginal value, the production-foundation bonus, and the existing travel scorer’s result. Ranked positions come from the production ranking helper with no reservations. Both axial and native offset job coordinates are included, with native Builder identity added at the CLI boundary.

The API reads the supplied Game and configured AI, without changing gameplay rules, selectors, gene defaults or force profiles. The CLI constructs a fresh observed board. It does not restore historical AI memory, infer the historical strategy, test route reachability/safety, predict the next emitted action, or certify Firaxis legality. Legal model candidates can still be rejected by the host; absent candidates can reflect incomplete observations. Running this diagnostic on a whole append-only log may select a later frame than an actual historical request; use an exact captured prefix when frame identity matters.

## Validation and findings

Focused tests cover legal Food/production alternatives, score components, native refusal/district exclusions, repeatability, unchanged next Builder behavior, invalid unit/owner/charge requests, explicit CLI strategy, server-protocol exclusion and missing-board failure. Required optimized CI is separate from local unoptimized focused checks.

The tool enables a scored counterfactual investigation. It does not itself improve the policy or establish a native win-rate benefit. No native game, harness, lock waiter, continuation or installed configuration was used.

### Retained turn100/0 query

Four standalone queries (explicit Science/Conquest, two repeats each) exited0, reported4 Builders and104 legal model candidate rows per query, and repeated byte-for-byte. Startup genomes matched the native profile in every field except the source-revision label. Source inputs were the exact retained100/0 bytes; no later feedback was substituted.

The capital Farm at native(64,32) is legal in the model and passes the existing worthwhile-improvement gate. With explicit Science strategy its intrinsic value is1.4, food/housing premium3.6, future-citizen Food premium0, and total job value5.0. For Builder3145729 it is2 hexes away and scores2.5 after travel; an unreserved Ternate Lumber Mill scores3.3007 and the farther Mataram Plantation4.3751. The Farm is therefore not absent from the legal/scoring surface. This is a fresh-board score comparison, not a claim that the persistent native planner chose either top entry: reservations, local work, safety and memory are not replayed by this mode.

The evidence identifies a concrete valuation/travel tradeoff to examine, without proving that raising the Farm value improves Science wins. It argues against a missing-legality repair or a blind Food-focus extension. Local query root: `~/civvis-tactics-results/2026-11-04/seowon-crowding/indonesia-builder-scored-t100`.
