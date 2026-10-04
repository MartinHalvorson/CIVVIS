# Production ledgers and controller setup audit

The recent production probes used a stock targeted agent with the gene ledger applied, rather than the live deployment bundle. This audit preserves that evidence, identifies the setup difference, and records corrected consumed-seed diagnostics. It delivers no runtime production change. High-level production parity and actual Firaxis gains remain unverified.

## What was actually tested

`AdvancedAi::targeting(Domination)` constructs the stock production agent. Calling `apply_gene_ledger()` withholds ledger-held live/production genes and enables selected opt-ins; it does not first enable the live repairs. The deployment controller instead calls `enable_live_bridge()`, which enables the live universe and then applies the ledger (`src/ai/advanced/treatment_flags.rs`; `configure_live_bridge` in `src/bin/civvis_orders.rs`). The public native deployment helper additionally uses authoritative-board planning.

The original probe's eight consumed seeds reproduced every prior CSV checkpoint, final serialized world, and all 215,603 actions. Its city snapshots remain valid for that stock-plus-ledger configuration. The supplemental probe records government, policies, current research/civic, bankruptcy penalty, unit maintenance, active deals, contracted income, and effective `wide_map_capacity` flags. An extra stock replay again reproduces the full world and all 35,022 actions for Deity 61007100.

Every stock flag readback is false for `wide_map_capacity`, despite the deployment tag list containing `wide-map-capacity`. Every corrected live-focal readback is true for the focal seat; every all-major native-deployment readback is true for all four majors. The summarizer checks these captures before producing results. A fault-injection check confirms it rejects a stock flag mislabeled as live.

The same ledger-only initialization appears in the October 4 forecast, payback, treasury, coverage, and coverage-pilot probe sources. Those experiments are evidence about their recorded configuration, not verified live-controller treatment effects. Their original sources and results have not been rewritten. The correction does not establish that any previously rejected candidate helps deployment.

## Corrected diagnostic configurations

All matches use the original four Emperor and four Deity seeds, Tiny Pangaea, Online speed, four majors, focal Gran Colombia and explicit Domination target, a 150-turn limit, and a handicap-exempt focal seat. These are consumed-seed diagnostic replays, with zero fresh strength samples.

| Arm | Focal agent | Other majors | Planning |
| --- | --- | --- | --- |
| Stock | Targeted stock plus ledger | Stock fleet | Authoritative |
| Live | Targeted `enable_live_bridge()` | Stock fleet | Fog-honest focal |
| Native | Targeted `enable_native_deployment()` | Native deployment | Authoritative |

The live arm changes the focal bundle and information contract. The native arm also changes the opponents. Differences between these arms are not isolated production-policy effects. Neither configuration includes the private live numeric genome or a Firaxis host export.

| Difficulty | Arm | Games present at turn 75 | Mean production | Mean focal / strongest-rival production | Eliminations, all four games | Wins, all four games |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Emperor | Stock | 3 | 68.167 | 0.537 | 0 | 0 |
| Emperor | Live | 4 | 52.675 | 0.428 | 0 | 0 |
| Emperor | Native | 4 | 49.700 | 0.272 | 1 | 0 |
| Deity | Stock | 4 | 29.312 | 0.144 | 2 | 0 |
| Deity | Live | 3 | 27.833 | 0.143 | 0 | 0 |
| Deity | Native | 4 | 37.525 | 0.109 | 1 | 0 |

Missing checkpoints are retained explicitly in the summary. Stock Emperor 61007002 ends at turn 67; live Deity 61007100 ends at turn 72. Later checkpoint counts also differ. Recorded eliminated seats contribute their actual zero production when play continues. Comparing only the displayed means would confound these differences.

## Findings for the next production experiment

The government delay in the old Deity 61007100 replay is explained by the missing repair. At turn 50 the stock replay remains in Chiefdom, while the corrected live and native replays have Oligarchy. The old replay pays 20.7 contracted gold per turn and has net income −13.7; native unit maintenance is only one gold. The corrected live replay has 166.2 gold and income +1.1. The corrected native replay still owes 21.5 contracted gold per turn and has income −10.5. Thus the missing bundle explains an early-government difference; it does not remove every budget problem.

The corrected live economies have 3, 4, 3, and 7 cities at Emperor turn 50, and 5, 4, 3, and 2 at Deity turn 50. All eight have cities with housing headroom at most one, amenities below zero, or both. At Deity turn 75, the three remaining live games have respectively three, three, and one housing-bound cities without Granaries. Their total production is 28.6, 38.6, and 16.3. This supports investigating early growth capacity and establishment of productive cities using the corrected controller.

Simply increasing production appetite has little immediate headroom: adding three to the citizen production weight changes no Deity live turn-50 city total and changes turn-75 empire production by only 0, 0, and 0.9 in the remaining games. Legal non-clearing improvements on currently worked bare plots offer 3, 4, 0, and 0 raw production at Deity turn 50, then 5, 11, and 0 in the remaining turn-75 games. These are local counterfactuals, not forecasts of affordable Builder work. They cannot by themselves explain the full rival gap.

Growth appetite counterfactuals retain the existing housing-dependent food target. They do not simulate a completed Granary, altered city placement, later population, or host citizen reassignment. Any growth intervention requires a new prospective paired screen under the correct deployment initialization.

## Reproduction and validation

The original and supplemental manifests freeze probe/compiler/dependency hashes before play. The library source is `84a64d8a3cf9769afec4940f4b611608ac0851d2`; native Game/AI/rules/data/strategy source agrees with the task base, while recorded mirror/order CLI differences and Cargo metadata prevent a claim of a bit-identical current build. Exact stock replay provides the behavior-neutral observer check. The final main synchronization adds only ladder/play tools.

The supplemental audit archives 68 raw setup/snapshot/action/world files outside the repository, with every SHA-256 in the summary. The original audit archives 24 files with hashes in its equivalence receipt. To verify the setup readbacks and regenerate all summaries:

```sh
python3 docs/eval/2026-10-04-production-growth-ledger-summarize.py \
  --raw /Users/martbot-mbp-m5-max-128/civvis-production-evidence/2026-10-04/ledger/setup-audit \
  --reference /Users/martbot-mbp-m5-max-128/civvis-production-evidence/2026-10-04/ledger/diagnostic \
  --output /tmp/civvis-production-growth-ledger-v2-summary.json
```

Local full-suite and source-quality results are recorded in the validation receipt. No runtime source, difficulty bonus, yield, rule, ledger, or private controller was changed.
