# The empire's city target and the governor that decides

⚠ `ai_eval` was removed in #2351 (2026-08-23): the paired evaluator and its arm registry were retired in favour of the gene screen (`docs/GENE_SCREEN.md`). Every `ai_eval` command in this document is kept as the record of how a result was measured — it does not run against this tree.

## Production policy

Production `AdvancedAi` now treats six cities as the opening floor rather than
the finish. Its strategic target rises by roughly one city per era, is bounded
by the existing land-capacity estimate, and can reach nine. A weighted agent
with a learned target above six keeps that larger target.

The delegated `BasicAi` governor receives the larger of its own city-target
gene and `plan.desired_cities` for that call. It also receives the strategy's
speed-aware expansion deadline instead of being cut off by the raw turn-150
gene, plus at least 0.75 Builders per city. All three numerical genes are
restored before the call returns, so evolved and frozen genomes remain exact.

This is an expansion policy, not permission to throw Settlers away:

- the practical-site gate still refuses a Settler when there is nowhere legal
  and reachable to found;
- Advanced settlement ranking still combines early growth, production,
  freshwater/coast, district adjacency, travel cost, visible attack envelopes,
  barbarian pressure, support distance, and isolation;
- repairs, a besieged city's walls/defender, and the one-unit-per-city military
  floor all rank ahead of a Settler;
- production Advanced enables siege muster, home-defense assignments and the
  bounded Recovery posture, so the larger empire raises and uses defenders;
- Builder coverage rises from 0.5 to 0.75 per city, while `has_builder_work`
  stops Builder production after useful improvements and repairs are exhausted.

Configured historical controls retain a three-city ramp, a six-city planning
ceiling, the flat four-city production gene, 0.5 Builders per city, and the
default-off live-defense adapters. `AdvancedAi::legacy()` therefore remains the
frozen rating anchor.

## Why the governor had to change

`AdvancedAi::assess` has always computed a land-aware, speed-aware target, but
an untargeted adaptive empire delegates ordinary queues to `BasicAi::cities`.
That governor used only this flat gate:

```rust
((n_cities + settlers) as f64) < self.w.city_target
```

The production default was 4.0. The plan and the queue therefore disagreed,
and the governor that actually produced Settlers knew the least about the map.
The live six-player league had already demonstrated that a target near ten was
viable, while the historical expansion-leverage probe was the one gene block
where drawing larger values helped.

## Why this is not the rejected 2026-07-31 treatment

The original `advanced_plan_city_target` experiment replaced the gene with the
plan verbatim. At the time the plan opened at three while the production gene
was four, so the treatment made the compounding opening *smaller*. Its original
fires check correctly rejected it:

```text
[eval 4p 24x16]       plan=false  cities 2.17 / target 3.83
[eval 4p 24x16]       plan=true   cities 1.50 / target 3.67
[deployment 6p 74x46] plan=false  cities 4.83 / target 5.00
[deployment 6p 74x46] plan=true   cities 4.33 / target 5.00
```

That null remains useful evidence. The production policy fixes its mechanism:
the opening floor is six and delegation takes `max(gene, plan)` so a strategic
target can widen the governor but can never narrow it. It also scales Builder
and defense capacity alongside the target rather than changing one number in
isolation.

## Current validation

The focused contract tests cover four properties:

- production constructors get the six-to-nine target, higher Builder coverage,
  smart settlement, siege/home-defense, and bounded Recovery together;
- historical controls retain their old values and flags;
- a higher learned city target or Builder ratio is preserved;
- after the historical stop turn, a wide plan builds a defender first when the
  army is short, then a Settler once the defense floor is filled, and restores
  all three temporary genes afterward.

The existing six-seed fires census on the production candidate reported:

```text
[eval 4p 24x16]       target 4  cities 3.83  score 260
[deployment 6p 74x46] target 6  cities 6.83  score 384
[deployment 6p 74x46] target 8  cities 6.83  score 365
```

Compact maps are site-limited. On the deployment map the six-city production
floor fires materially; raising the reported late target from six to eight did
not add another city in this small census, so nine is a continuing ceiling,
not a claim that every empire reaches nine.

For an end-to-end check, candidate and base `ai_eval` binaries were run on the
same six Online Continents maps (seeds `88620000..88620005`, 6 players,
74x46, 200 turns), each against the same frozen `advanced_v1` anchor. The
production candidate moved the `advanced` averages as follows:

| measure | base | candidate |
|---|---:|---:|
| cities | 5.31 | **8.28** |
| population | 63.8 | **90.4** |
| districts | 21.0 | **29.8** |
| buildings | 73.6 | **100.9** |
| military power | 791.9 | **1311.8** |
| Builders | 2.36 | **5.67** |
| food yield | 163.5 | **233.4** |
| production yield | 283.1 | **373.6** |
| science yield | 148.2 | **175.4** |
| culture yield | 115.2 | **129.3** |
| terminal score | 563.8 | **666.0** |

All six maps favored the candidate on terminal score. Six maps are mechanism
evidence, not a powered win-rate claim: no game in this short 200-turn profile
ended in a victory, and the promotion gate correctly called the sample
insufficient. The durable claim is narrower and directly observed: the AI
built and developed substantially more cities while fielding a larger army and
raising every terminal city-yield column the evaluator reports.
