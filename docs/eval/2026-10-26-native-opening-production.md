# Fixed Native opening-Production census

The fixed sixteen-game Immortal Domination cohort has a median turn-50 queue-Production reading of **29.7% of the reported global leader’s**. Replaying the current one-third rule over exact turns 50–52 would reach three below-threshold readings in **9/16 games**. This is retrospective description, not a treatment effect or an actual host defeat. No gameplay or live-driver setting changes.

All **80 registered samples and 83 distinct captured reads** reconstruct successfully. Every captured last state exactly matches an original Native event at its selected turn/frame. All repeated read variants share that state. The corrected full extraction repeats byte-identically. Nine semantic controls pass.

## Fixed samples and Production sources

Use the entire completed Slinger cohort, including the turn-41 continuation, at exact frame-zero turns 10, 20, 30, 40 and 50. The selection and conservative rules were committed before measurement; no game, target or missing field was replaced. Repeated same-frame decisions and captured-read variants remain retained. This does not compare Slinger treatment effects.

The Native turn map reads build-queue Production with a city-yield fallback. The local city `production` field also reads the queue. Public empire statistics and `city.yields.production` read City:GetYield directly. Keep those quantities separate: the same-state city/population factorization is not a decomposition of a different queue or post-action turn reading.

The turn map covers reported alive majors, including unmet ones. Rival state records include only met civilizations. Decompose the actual global leader only when its public empire statistics are available; never substitute the strongest met rival or count only visible rival cities. The map is complete against each turn’s reported major count at all eighty samples.

| Turn | Known queue standings | Median queue ratio | Below one-third | Same-state leader decomposition available |
| ---: | ---: | ---: | ---: | ---: |
| 10 | 16 | 31.6% | 10/16 | 1/16 |
| 20 | 16 | 26.9% | 11/16 | 3/16 |
| 30 | 16 | 29.8% | 10/16 | 5/16 |
| 40 | 16 | 31.4% | 9/16 | 8/16 |
| 50 | 16 | 29.7% | 10/16 | 9/16 |

## What the current readings support

The turn-50 median is **3.5 cities and 14.5 population**. For the nine leaders whose same-state public statistics are available, the median city-count ratio is 0.75, median population-per-city ratio 0.7143 and median city-yield Production-per-population ratio 0.5246. These are medians of separate per-game factors; do not multiply them to explain the overall queue median. Each retained game’s factorization independently reproduces its own same-state city-yield ratio. Seven global leaders lack this public decomposition.

The Production-per-population factor includes terrain, improvements, infrastructure, city modifiers and difficulty effects. This census does not identify how much each contributes. The observations justify investigating growth, expansion and yield modifiers. They do not establish that citizen focus alone closes the gap.

## Conservative allocation bounds

Quotes must be explicit same-turn plots in the actual captured prefix, visible, owned by the local player and city, passable, non-district and within three wrapped hexes. Metadata-only deltas cannot renew old fields. The free centre is not an extra worker; staffed districts are excluded from terrain-worker totals. Missing, stale and unreadable quotes remain unknown.

One turn-40 case, `civvis-20261026T032104Z`, has identical Native state but different map availability: snapshot 86 has no same-turn explicit plots, while 87 has 126. Both are retained. Its allocation views differ, so the summary withholds that case from aggregated allocation bounds; its city and Production metrics remain usable. No favorable read is selected.

| Turn | Native city samples | Unambiguous allocation samples | With current unworked quote | Pareto-bound city samples | Food-buffer tradeoff city samples |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 10 | 16 | 16 | 2 | 0 | 0 |
| 20 | 30 | 30 | 20 | 0 | 5 |
| 30 | 44 | 44 | 3 | 0 | 0 |
| 40 | 54 | 51 | 21 | 1 | 5 |
| 50 | 63 | 63 | 3 | 0 | 2 |

The sole unambiguous Pareto quote is **+1 raw Production in Quito at turn 40** of `civvis-20261026T014409Z`, preserving the other five quoted yields. Twelve city-turn samples have at least one one-worker tradeoff that leaves the raw Food surplus at least two; those exchanges may spend Food or other yields. Thirty raw candidate pairs are independently checked against their actual worked and unworked quotes. Multiple pairs are alternatives, not additive yields.

These are local terrain-yield bounds. They are not Native allocation permissions, actual citizen moves, city-multiplier forecasts or free yields. Existing locks and allocator behavior can prevent an exchange. Sparse fresh quote coverage prevents a broad Production-focus recommendation. No focus order or policy is introduced.

## All turn-50 cases

| Start suffix | Queue ratio | Cities | Population | Three below by 52 in replay | Leader public stats |
| --- | ---: | ---: | ---: | --- | --- |
| `000859Z` | 19.4% | 3 | 9 | Yes | Known |
| `001624Z` | 46.0% | 6 | 25 | No | Known |
| `002652Z` | 53.2% | 6 | 20 | No | Unknown |
| `004001Z` | 45.6% | 5 | 19 | No | Known |
| `005035Z` | 19.4% | 3 | 15 | Yes | Known |
| `005810Z` | 54.2% | 6 | 22 | No | Known |
| `011557Z` | 36.3% | 4 | 17 | No | Unknown |
| `013313Z` | 30.6% | 3 | 14 | Yes | Known |
| `014409Z` | 51.2% | 5 | 21 | No | Unknown |
| `021028Z` | 22.2% | 2 | 10 | Yes | Known |
| `022518Z` | 18.8% | 3 | 12 | Yes | Unknown |
| `023847Z` | 28.7% | 4 | 12 | No | Unknown |
| `024315Z` | 22.1% | 3 | 12 | Yes | Unknown |
| `025544Z` | 31.3% | 5 | 18 | Yes | Known |
| `031004Z` | 12.3% | 2 | 4 | Yes | Known |
| `032104Z` | 14.5% | 3 | 13 | Yes | Unknown |

The replay uses the existing production_share_reading/run helpers at exactly one-third over readable turns 50, 51 and 52. All sixteen probes are complete: nine reach three below-threshold readings and seven do not. Repeated records cannot count as extra turns. This does not say that the seven would never trigger later or that the nine could not recover if allowed to play on.

## Validation and retained failure

The initial full extraction selected all eighty samples, but readback validation rejected tuple coordinates inside unknown-data annotations after JSON converted them to lists. A new semantic control reproduced that failure before the representation fix. Both original source/output and failure log remain retained. Replacing only the annotation representation changes no computed value; the corrected result matches the first output after excluding analyzer-source identity. A separate corrected repeat is byte-identical.

The final result is 460,061 bytes of deterministic gzip JSON, SHA-256 `bb042a99cbcd6bbda3ff6781b690791896815b310ae36e6252397e750c759f87`. All 83 whole-state cross-checks match actual Native events. The artifact preserves every missing field, alternate read, source identity, raw quoted-yield witness, rule probe, semantic control and source revision.

The sealed evidence archive has **26 byte-verified members, 917,928 bytes**, SHA-256 `37603ed89e23691234c3100937f65fe92c95ddc78ebc1544b23fa190428fef04`. Original non-image cohort archives already published with the Slinger trial remain the raw source; their full prefixes are not duplicated here.

No Native command, runtime baseline, retirement rule, default or ledger promotion follows from this census.
