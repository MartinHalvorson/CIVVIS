# Policy/envoy production promotion

## Decision

On 2026-08-01, production `advanced` adopted the exact composite that was
pre-registered and independently confirmed as
`advanced_policy_envoy_priority`:

- `policy-deck-live`
- `envoy-infrastructure`
- `envoy-priority`

The promotion intentionally excludes the separate `pol_influence` numerical
weight. The experiment therefore supports this particular causal bundle, not a
general claim that every envoy valuation adjustment is beneficial.

The evidence is recorded in [the composite record](POLICY_ENVOY_COMPOSITE.md).
Its independent confirmation used 300 mirrored compact maps and 300 mirrored
deployment maps. Compact met the safety condition at 52.2% paired score
(Elo-equivalent +15; 95% interval −24 to +54); deployment passed at 57.2%
(+51; +11 to +90), with 115 favorable and 52 unfavorable map directions and an
anytime-valid crossing at map 42.

## Construction boundary

`AdvancedAi::new`, `targeting`, `with_weights`, and
`with_weights_and_target` all force `PolicyDeck::Live` and enable the two envoy
flags. `reweight` preserves that live non-gene deck for a production Advanced
controller, while `advanced_v1` retains the weight it is given.

`AdvancedAi::pre_policy_envoy` and its weighted counterpart are crate-private
evaluator controls. They retain the old Live/Legacy and envoy-flag combinations
without exposing a second production API. The public historical name
`advanced_policy_envoy_priority` resolves to `advanced`; evaluator provenance,
typed specifications, and self-play detection all use that same canonical
resolution.

The remaining policy/envoy evaluator arms are deliberate reversion controls:

| arm | difference from production `advanced` |
|---|---|
| `advanced_policy_live_control` | turns off infrastructure and priority |
| `advanced_envoy_policy` | adds `pol_influence`; turns off infrastructure and priority |
| `advanced_envoy_infrastructure` | restores Legacy policy deck; turns off priority |
| `advanced_envoy_priority` | restores Legacy policy deck |
| `advanced_envoy_economy` | adds `pol_influence`; turns off priority |

## Longitudinal protection

The frozen `advanced_v1` factory remains `AdvancedAi::legacy()`. It does not
call any promoted constructor. CI hashes the full shared `ai.rs` and
`ai/advanced.rs` sources, so this change requires a conscious contract re-pin
only after a release-mode before/after comparison proves the legacy evaluator
output is identical.

Because production `advanced` changed, the default tournament roster now uses
the fresh immutable identity `advanced-20260801-policy-envoy=advanced`. The
unchanged `advanced_v1` entry remains the 1500-point longitudinal anchor; its
old ledger row is never blended with the promoted controller.

## Verification record

Before changing the production constructors, a release `ai_eval` binary was
built from `e46d1b7`. A second release binary was built from this promotion in a
separate target directory. Their output for

```sh
ai_eval advanced_v1 basic --pairs 10 --jobs 1 --seed 31337 \
  --players 4 --turns 200 --deployment-comparison
```

was byte-identical under `diff -u`. The fixed comparison exercises 20 mirrored
games without routing either side through the promoted `advanced` constructor.
The CI source-contract fingerprint was consequently re-pinned to
`fnv1a64:b71feabfd699cd32`; no Elo protocol or ledger change is warranted.
