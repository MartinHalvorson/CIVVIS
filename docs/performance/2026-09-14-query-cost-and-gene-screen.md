# Query performance and gene evidence — 2026-09-14

Status: checkpoint. Completed measurements below are final; the parallel comparison and fresh strength screen are pending. This report does not change deployment defaults.

## Completed performance comparisons

The actual tournament comparisons use `gene_screen` with its standard independent genome draw: six major players, 74×46 Continents, nine city-states, Online speed, 250-turn limit, Emperor, native competitions and all six victory lanes. Every seat independently draws the full compiled pool. This differs from the `AdvancedAi::new` controller used by native `simulate` timing in CI.

| Change | Paired games | Turns per arm | Median CPU change | Pooled CPU change | Same-binary controls |
| --- | ---: | ---: | ---: | ---: | --- |
| Regional prefilter, lazy passage, purchase-price hash map and culture memo, #3550–#3553 together | 8 | 1,584 | −7.87% | −7.77% | +0.20%, +2.05% |
| Assessment query scopes, #3557 | 8 | 1,478 | −0.80% | −0.79% | −0.54%, +0.99% |
| Settlement threat query ordering, #3558 | 4 | 793 | −1.67% | −1.81% | +1.81% |

Every candidate game was faster in these completed comparisons, and every recorded outcome field matched its baseline except the runtime field. The controls limit precision, especially for the small assessment and settlement changes. Native CI did not resolve those two gains: its final medians were +0.16% and +0.89%, within its 1% floor. Those results are retained rather than selecting an earlier favorable run.

The first row uses the optimized `ci` profile. The latter rows use `release` with thin LTO and one codegen unit. Do not add these percentages or compare their absolute CPU totals across profiles. The first baseline already includes the luxury fast paths and economic-gene changes.

### Source and method

| Comparison | Frozen baseline source | Frozen candidate source | Candidate seeds | Control seeds |
| --- | --- | --- | --- | --- |
| #3550–#3553 | `06209d711a5bd84500f128fc6ad8805b821383fe` | `7cfc65a953704b3aa14567ce7775b7079cf7483c` | 9134000–9134007 | 9133998–9133999 |
| #3557 | `cb298964ee7eacb999edfac7f3c16602ea783afb` | `5a527e118740c3b415f2d747ebbcaf3b98e7ee76` | 9136800–9136807 | 9136798–9136799 |
| #3558 | `5a527e118740c3b415f2d747ebbcaf3b98e7ee76` | `05a5a1d9964fa0144e79c12c32c8f0e0df11a642` | 9137700–9137703 | 9137699 |

The local harness interleaves baseline and candidate, measures child-process user CPU, and verifies complete seat records, matching turns, headers apart from build metadata, the compiled gene fingerprint, source commit, clean-build flag and binary SHA-256. It compares all outcome fields after excluding `secs`. No competing local compilation, simulation or profiling ran during paired timing. Equality is of the recorded outcomes; these files do not contain every intermediate action.

Source commits identify the actual build inputs before final documentation or merge commits. They are not relabeled with the later squash-merge SHA. The release sources were checked against their merged production build inputs. Raw manifests, records, logs and binaries are retained under the operator's `civvis-runs` directory, with `0913-` filenames. Detailed methods and validation are also in the corresponding PRs.

## Other shipped work

- #3258 removed obsolete requisition code while retaining relevant peace, coalition and naval behaviors.
- #3447 completed the economic-gene draft and fixed a Builder policy rule that could evict protected Liberalism. Its two experimental genes remain off by default.
- #3549 added luxury fast paths; its native improvement was modest.
- #3554 reused Great Work forecast state and avoided empty work. #3555 deferred local amenity work until needed. Neither has a resolved standalone speed claim here.
- #3556 added the reversible `culture-lane-forecast-2` treatment. It applies the engine's international religious/secular tourism modifiers per rival and excludes teammates. Version 1 retains its previous arithmetic and order. The constant-rate forecast still has the documented Film Studio and cultural-alliance limitations.

The culture treatment passed seven focused tests and full CI. Its six-game, 36-seat smoke established that the variants run; it did not establish strength. Version 2 remains off and the original 139 deployment defaults remain selected.

## Concurrency diagnostic and pending comparison

A separate 1,200-game screen was declared at 16 workers, using frozen release source `05a5a1d9964fa0144e79c12c32c8f0e0df11a642`, seeds 9139000–9140199. The machine has 18 physical cores (six Super and twelve Performance), 64 GiB RAM, and was on AC power. Actual throughput was too slow to complete both that screen and a disjoint confirmation within the session.

A ten-second profile of that run attributed 36.83% of running self CPU samples to `Name::new`, including purchase-query callers. The repeated global-interner lookup becomes costly with concurrent workers. This sample identifies a hotspot; its percentage is not an estimate of the saving from removing particular calls.

The operator-owned experiment was deliberately stopped. Its original header still declares 1,200 games; the preserved complete prefix contains **89 games and 534 seats**, seeds 9139000–9139088. The driver correctly reported a failed completion assertion after its child was terminated. This instrumented partial run is diagnostic only: it is not a completed strength batch, is not a ledger source, and makes no default decision. The previously operator-stopped tournament was not restarted.

Merged PR #3559 combines the technology and civic effect indexes behind one lookup, preserving node order and technology-before-civic floating-point accumulation. Its final CI passed 3,686 Rust tests (49 skipped), documentation checks, 71 gene-screen and 21 live-divergence tests. Its separate native CI comparison recorded −1.19% median and −1.13% pooled CPU over five matched games and 600 turns per arm, resolving approximately ±0.53 percentage points; this is not a tournament-strength result. PR #3560 reuses canonical rules names in purchase queries and menus. Eight focused tests passed with the changes combined, including direct tree-source comparisons, floating-point cancellation order, purchase prices and map replacement/removal.

The preregistered combined performance comparison uses one matched block of 16 games per arm with 16 workers, plus one same-baseline control block. Candidate seeds are 9142016–9142031; controls are 9142000–9142015. It measures aggregate process user CPU and wall time. Sixteen game rows do not constitute sixteen independent CPU estimates. All recorded outcomes must match. Results will be filled in after both blocks finish; any saving belongs to the two changes together.

## Fresh strength evidence

Pending. A new fixed-size, separately declared batch will use fresh seeds and a verified frozen binary. Its size will be chosen from observed parallel throughput before examining strength results. All declared games must complete. The screen ranks hypotheses across the full pool; a deployment change requires a disjoint single-gene or whole-family confirmation with reported uncertainty and resolving power.
