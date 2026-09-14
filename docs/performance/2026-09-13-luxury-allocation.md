# Luxury amenity allocation performance

Measured on mbp-m5-pro-64’s MacBook Pro, 2026-09-14 UTC (2026-09-13 local).
Baseline: `af4fadba8f71a97cb3435b7b31479dc357702af3`. Candidate implementation:
`14971c7fb`, compiled with the same locked `ci` profile.

A 20-second sample of the six-player economic-timing probe put city amenities
at 19.3% inclusive on the busiest thread, with luxury allocation beneath it.
That inclusive cost alone does not imply redundant work; the optimization
removes two specific operations:

- Cached requests read one city's integer allocation without cloning the
  empire-wide B-tree; cache misses move the computed map into the memo.
- If every available luxury reaches every city, allocations are uniform.
  This includes the no-luxury case, ordinary empires of up to four cities,
  and six-city reach from the Aztec ability or Zanzibar's special luxuries.
  Those cases skip the local-amenity valuations and need ranking entirely.
  Larger mixed-reach allocations retain the original ordering and reuse their
  city-ID scratch vector between luxuries.

## Paired results

`tools/speed_ab.py`, six majors, 74x46 Continents, nine city-states, Online,
flat topology, one job. The supplied turn count is a cap; games can end earlier.

| Cap / seeds | Pairs | Completed turns per arm | Median CPU/turn change | Resolution | Pooled change |
| --- | ---: | ---: | ---: | ---: | ---: |
| 120 / 9132000–9132007 | 8 | 960 | -0.66% | ±0.47% | -0.75% |
| 250 / 9132100–9132103 | 4 | 861 | -0.56% | ±0.56% | -0.64% |

All twelve pairs produced identical game reports and each candidate reading was
faster. This is a modest local improvement; the full-clock window is at its own
resolution boundary. It is not a cross-machine or production speed guarantee.
The first window's load fell from 7.79 to 2.36; the second stayed at 2.36–2.51.

## Correctness

A retained version of the original ranked algorithm is the test oracle. It is
compared against the optimized allocator and cached city lookup for 0, 1, 4, 5,
6, 7, and 10 cities, ordinary and Aztec civilizations, connected resources,
Congress bans and multiple-copy effects, and Zanzibar's synthetic luxuries.
The separate cache test verifies that uniform allocations do not populate the
local-amenity memo and that a later scope observes newly connected luxuries.
The existing Zanzibar supply test remains active.

The full locked Rust suite passed after integrating main at `90a9c3886`:
3,462 library tests, 49 ignored, plus all binary, integration, protocol, and
documentation targets. Raw samples and benchmark logs remain in the local run
directory, with names beginning `0913-`.
