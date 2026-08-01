# Production champion promotion

## Decision

Production `advanced` is proposed to use the immutable champion embedded in
`data/evolved/best.json`, together with the already-promoted live policy deck
and envoy-production composite. `advanced_v1` remains the stock-weight legacy
anchor. This is a controller-definition change, so its tournament identity is
dated rather than appended under the old `advanced` identity.

The production constructor reads the binary-embedded snapshot only. A local
`evolved/best.json` remains useful for research arms, but cannot silently
replace the controller that a released binary calls `advanced`.

## Existing independent evidence

PR #708 produced the revised champion by reverting the eleven economy and
expansion genes that caused the prior champion to lose at deployment. The
remaining 29 genes were unchanged. Its matrix evidence against stock weights
was:

| matrix base seed | profile | paired score | Elo-equivalent | verdict |
|---:|---|---:|---:|---|
| 70,000,000 | compact Standard | 57.7% | +54 | PASS |
| 70,000,000 | deployment Online | 56.2% | +44 | PASS |
| 70,000,000 after #686 | compact Standard | 57.5% | +53 | PASS |
| 70,000,000 after #686 | deployment Online | 56.6% | +46 | PASS |

The first 300-pair seed (67,000,000) had been extended after an inconclusive
read and is therefore discovery-only. The disjoint 70,000,000 matrix is the
quoted confirmation. Across the three recorded matrices: 1,800 maps and 3,600
games.

## Current-controller confirmation — preregistered before outcome

PR #746 subsequently made policy-deck-live, envoy infrastructure, and envoy
priority shared production behavior. That wrapper is applied identically to
both weight sets, but this promotion still requires a fresh confirmation on
the exact post-#746 controller.

Before inspecting this run's outcomes, the sole confirmation is fixed as:

```sh
target/release/ai_eval advanced advanced_stock_control \
  --matrix --pairs 300 --jobs 10 --seed 74000000
```

`advanced_stock_control` uses stock numerical weights while retaining the
current live policy deck and both envoy-production mechanisms. It therefore
differs from production `advanced` on exactly one typed evaluator axis:
`weights`. Matrix mode fixes its compact Standard safety and deployment Online
strength profiles, uses disjoint fixed-stride seed streams, and rejects
degraded artifacts. Promotion requires deployment PASS and compact non-RETAIN;
any other result keeps the champion out of the production default.

No parameter, profile, sample count, or follow-up seed will be selected after
reading the outcome of this confirmation.

## Compatibility boundary

`AdvancedAi::legacy()` directly constructs stock `BasicAi` with planning
disabled. Release `ai_eval` binaries built from `ad6819d` and this candidate
produce byte-identical output for:

```sh
ai_eval advanced_v1 basic --pairs 10 --jobs 1 --seed 31337 \
  --players 4 --turns 200 --deployment-comparison
```

The source contract is deliberately re-pinned after that check; the Elo
protocol does not change.
