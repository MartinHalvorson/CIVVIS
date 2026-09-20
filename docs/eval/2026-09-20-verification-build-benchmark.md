# Native verification build tradeoff, 2026-09-20

A separate build with 16 codegen units, no LTO, and incremental compilation
built the native worker and map executable in 81.53 seconds, versus
354.21 seconds with the standard release settings. Both builds produced
identical native orders for all 479 frozen decision frames in each of three
replays. The candidate's median replay took 92.34 seconds versus
83.90 seconds: 10.06% slower.

This is evidence for an opt-in native verification build setting, not a
production release-profile change or a proven improvement in live game time.
No launcher, Cargo profile, running game, or production setting was changed by
this experiment. Warm rebuild times and end-to-end native game times were not
measured. These were sequential observations on a busy host, not a controlled
idle-machine benchmark.

## Source and host

- Same immutable measured revision for both builds: `bb0bfee91b7c434d4714ca84022b29e658876e18`
  (a documentation claim on `460ebd6eb8abd9aee59161eae87bbe1f14573912`).
- Apple M5 Max, `aarch64-apple-darwin`.
- Rust `1.97.1 (8bab26f4f 2026-07-14)`, LLVM 22.1.6;
  Cargo `1.97.1 (c980f4866 2026-06-30)`.
- Native King game `civvis-20260920T101201Z`, a production spectator build,
  and desktop services remained active. Load averages sampled during the
  experiment were approximately 16–20 for the one-minute average.
- Replay input: native `civvis-20260920T085522Z`, turns 1–168,
  33,840,417 bytes, SHA-256
  `15cc70be101f2364d7367d4e5e92934d34a501d0ef456b2a26b015628bec345b`.

## Build method

Inherited `CARGO_PROFILE_RELEASE_*`, `CARGO_TARGET_DIR`, and
`CARGO_INCREMENTAL` variables were removed before setting each experiment's
values. The two builds ran sequentially, standard first, with separate empty
Cargo target directories. The source HEAD was checked before and after the
build pair. Each built both binaries and exited successfully:

```sh
cargo build --release --locked --bin civvis_orders --bin civvis
```

| Setting | Standard | Candidate |
| --- | --- | --- |
| `CARGO_PROFILE_RELEASE_CODEGEN_UNITS` | `1` | `16` |
| `CARGO_PROFILE_RELEASE_LTO` | `thin` | `false` |
| `CARGO_INCREMENTAL` | `0` | `1` |
| `CARGO_TARGET_DIR` | `target/bench-standard` | `target/bench-fast` |
| Elapsed seconds | 354.21 | 81.53 |

The observed cold-build saving was 272.68 seconds (76.98%).
There was only one cold build per setting. Shared dependency downloads, OS
caches, fixed build order, and background load limit causal timing claims.
The experiment changes three compiler settings together; it does not isolate
the contribution of any one setting.

## Decision parity and replay timing

Replay timing began only after both builds completed. Each worker used the
same frozen event stream, a fresh private mirror directory, `--serve`,
`--fresh-board`, `--explain`, `--victory domination`, Gran Colombia, and the
same 19 forced genes from the native run. Events were appended in original
order. On the first `await` for each distinct state `(turn, frame)`, the harness
flushed the mirror, requested that turn over stdin, and read one JSON reply.
It replayed observations; it did not execute returned orders in Civ VI.

The harness compared the canonical JSON of each ordered list of
`turn`, `replay_frame`, and `orders`, with sorted object keys and compact
separators. Explanation text and timing metadata were excluded. All six
workers exited zero, returned 479 frames, and had the same order digest:

`5e23bf35e1c48f1e069d3a089226698d9a48c6b2990675fbf07b4f52fdc07a8c`

| Sequential trial | Setting | Seconds | Frames | Exact orders match |
| --- | --- | --- | --- | --- |
| standard1 | standard | 99.59 | 479 | yes |
| fast1 | fast | 110.73 | 479 | yes |
| fast2 | fast | 92.34 | 479 | yes |
| standard2 | standard | 83.90 | 479 | yes |
| standard3 | standard | 82.17 | 479 | yes |
| fast3 | fast | 90.86 | 479 | yes |

Median standard: 83.90s. Median candidate: 92.34s.
The median difference was 8.44s, or 10.06%.
The first run of each binary was slower than subsequent runs; the repeated
order was standard, candidate, candidate, standard, standard, candidate.
These observations are consistent with a decision-throughput cost, but do not
establish its exact size under other host loads, maps, or game stages.

The measured cold-build saving equals about 32.3 times this
479-frame median replay penalty. That arithmetic motivates a separate native
launcher experiment; it is not an end-to-end break-even measurement. The
native game also spends time inside Civ VI and its UI, and future rebuilds
need not be cold. Profile adoption should retain the normal release default,
record the selected build settings, and validate inherited settings in both
the supervisor build and the worker's refresh build.

## Artifacts and limits

Local experiment artifacts are under
`/tmp/civvis-verification-build-benchmark/`: `build.py`, `replay.py`,
`frame-replay.py`, build/replay/environment JSON results, compiler logs,
per-trial order streams and explanations, and `artifact-hashes.json`.
The temporary directory is a local diagnostic, not durable repository data.
The frozen input remains in the native run archive identified above.

| Worker binary | SHA-256 |
| --- | --- |
| Standard | `793781484577ac2dda67f8276bebe5d018302b9dbd13c38dadaa6b0c2855a44f` |
| Candidate | `d5639fc679852219aa4e1469171f310f80bdfa076df7a083c2e5ad86d70c4822` |

Order parity covers this one frozen game only. It does not prove universal
compiler-profile equivalence, headless simulation performance, a capture,
a victory, or the performance of an actual candidate-built native game.
The production release profile and the active King game's binary were left
unchanged.
