# Planet distance cache-order experiment

Status: **rejected; treatment reverted after the frozen cold-path gate failed**

## Evidence and hypothesis

The live 8-player Planet/Continents StrategicDeep expansion oracle was sampled
for two seconds on 2026-07-29 after it had occupied roughly four CPU cores for
2.5 hours. Across its worker threads, `Name::new` was the hottest runnable leaf
at 1,198 samples and `Sphere::distance` was second at 950. The name-allocation
path is already owned by #576. This experiment isolates the unclaimed globe
distance path; it does not modify or interrupt the running oracle.

`Sphere::distance` currently resolves a valid pair in this order:

1. equality;
2. binary search in the source tile's exact radius-six ring;
3. either endpoint's admitted full distance row;
4. admission/search for a long-distance miss.

A stock frequency-21 Planet has 4,412 tiles and the existing 64 MiB budget can
retain every exact row. A reused tactical or routing endpoint earns a full row
after eight long misses, but its later local comparisons still pay the ring
binary search and never read that O(1) row.

**Frozen hypothesis:** checking an already admitted endpoint row before the
ring will materially accelerate steady-state repeated-anchor distance queries,
without changing any distance and without materially slowing the cold local
path whose endpoints have no admitted row.

## Frozen treatment

The treatment changes only the order of the existing `cached_distance` and
radius-six ring checks. It adds no cache, allocation, memory, admission,
eviction, approximate distance, or unsafe code. Invalid positions and equal
positions retain their current precedence. Row admission and the long-distance
A* fallback remain byte-for-byte unchanged.

## Frozen benchmark

An ignored release-profile test in `src/sphere.rs` will exercise a fresh
frequency-21 globe in one process. It will:

1. choose one stable source tile, every radius-one-through-six target in its
   cached ring, and stable targets farther than six steps;
2. time seven batches of 2,000,000 cold local calls before any full row is
   admitted;
3. issue the existing eight distinct long queries required to admit the source
   row and assert that the source row is present;
4. time seven identical batches of admitted-row local calls;
5. time seven batches of 2,000,000 admitted-row long calls; and
6. report the median nanoseconds per call for all three phases, with inputs and
   an accumulated result passed through `std::hint::black_box`.

The exact command is:

```text
cargo test --release sphere_distance_cache_order_benchmark \
  -- --ignored --nocapture --test-threads=1
```

The benchmark code, frequency, source/target selection, batch count, and call
count are committed before the baseline. Baseline and treatment each receive
three fresh-process command runs; the median of the three printed phase
medians is the comparison. No other process is launched or stopped for this
test. The already running simulations remain present for both arms, so host
load is paired rather than selectively cleared.

## Frozen decision

The reordered lookup advances only if all conditions hold:

- admitted-row local calls are at least **1.50x** as fast as baseline;
- cold local calls take no more than **1.10x** baseline time;
- admitted-row long calls take no more than **1.05x** baseline time;
- every existing exact-distance, disk/ring, cache-budget, and concurrency test
  passes; and
- `cargo test --profile ci --locked` passes on the exact treatment head.

Displayed rounding does not decide a gate; calculations use the unrounded
nanoseconds derived from each command's elapsed nanoseconds and fixed call
count. A failure reverts the lookup reorder and records the negative result.
There is no tuning of the admission threshold, ring radius, row budget,
frequency, iteration count, or gate after either timing is read.

This is a throughput result only. Even a pass claims no AI strength increase;
its value is reducing the cost of the paired experiments that can establish
strength.

## Result

The three fresh-process baseline runs produced these phase medians, in
nanoseconds per call:

| run | cold local | admitted local | admitted long |
|---:|---:|---:|---:|
| 1 | 17.802521 | 17.875625 | 17.243146 |
| 2 | 17.052521 | 17.738459 | 17.956021 |
| 3 | 16.354084 | 16.428979 | 16.311729 |
| **median** | **17.052521** | **17.738459** | **17.243146** |

These baseline observations were committed before applying or benchmarking
the cache-order treatment. The three fresh-process treatment runs produced:

| run | cold local | admitted local | admitted long |
|---:|---:|---:|---:|
| 1 | 17.640875 | 3.613000 | 3.594313 |
| 2 | 20.455834 | 4.107854 | 6.212438 |
| 3 | 40.062875 | 9.770771 | 6.474708 |
| **median** | **20.455834** | **4.107854** | **6.212438** |

Using the unrounded reported values, admitted local calls were **4.318181464x**
as fast as baseline and admitted long calls took **0.360284486x** baseline
time. Both gates passed. Cold local calls, however, took **1.199578291x**
baseline time, exceeding the frozen 1.10x ceiling. The experiment therefore
failed its conjunction of gates and the production lookup reorder was
reverted. The ignored benchmark remains as a reproducible record; runtime
behavior is unchanged by this experiment.
