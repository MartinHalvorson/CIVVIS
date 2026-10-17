# Native replay frame boundary — 2026-10-17

## Result and scope

The replay requested a decision immediately after `state`, before the first
terrain export. It now requests once at the frame's first matching `await` or
`orders_peek_wake`. This repairs an offline input-boundary defect, **not** an AI
conquest policy. No Native game, brain, database, policy pin, difficulty,
verification rule, or running process was changed or restarted.

The fixed full-history A/A control still **fails**. Matching increases from
276/394 to 372/394, but the remaining 22 mismatches prohibit an A/B interpretation.
There is no survival, speed, strength, Domination-win, or highest-difficulty-win
claim. No A/B was run, and no frame was excluded to obtain a passing control.

## Fixed workload and provenance

The workload was registered at **2026-10-17 08:58:14 UTC**, before either control
and before the repair. The original run's abandonment and combat results were
already known: this is a diagnostic case chosen for its preserved exact binary,
not a representative or outcome-blind game sample.

- Run: `civvis-20261017T072826Z`, all 394 frames through turn 150, including replans.
- Native difficulty: Emperor; configured Domination, stock `AdvancedAi::new`,
  Gran Colombia. This is not a Deity verification game.
- Original revision: `712a48e638dcdd2464b321f9fff9acbb4ee4eb23`.
- Both arms of both controls use the exact original attempt-snapshot binary:
  `d66c8d1a3950592633e1a3d4ba3c1b996dbe6b722ea92dfca0d82605c30a0164`.
- All **282 original force flags**, in original order, match `brain.log` and
  `summary.json`; no withheld flags. No current-bundle flag was silently added.
- Both arms use `--serve --fresh-board --explain --victory domination --civ
  CIVILIZATION_GRAN_COLOMBIA`, with persistent agent memory and rebuilt boards.
- Frozen event history, recorded Native replies, flags, binary, and metadata are
  hash-guarded. The original source files also remain byte-identical.
- Registration SHA-256:
  `790c72d725c8aabab07dc76cf707bebd771c6b479896685ba0fb00e601e3aa49`.

`2026-10-17-native-replay-frame-boundary.registration.json` records the fixed
workload, timing, input hashes, selection caveat, boundary plan, and strict gate.
`2026-10-17-native-replay-frame-boundary.result.json` retains every mismatch key.

## Boundary contract

The initial export is `state` at source line 67, then `export_timing`,
`revealed_api`, `tiles`, `tiles_done`, and `await` at line 72. The old replay asked
at line 67 and obtained an empty, no-terrain reply instead of Native's decision.
`tiles_done` is not a universal frame boundary: only 38/394 state segments have
one; 356/394 do not. Same-turn replans generally use existing terrain, including
deltas that precede the next state.

All 394 segments have a controller polling marker before order execution:
137 first reach `await`, and 257 first reach `orders_peek_wake`. The repaired
replay includes that marker and the preceding export, but never advances to a
later state or admits current order-execution receipts while awaiting a boundary.
A newer state, order execution, mismatched poll identity, cutoff, or EOF with an
unclosed frame fails closed. Later polls do not create duplicate decisions.

`frame-prefixes.jsonl` records each requested turn/frame, marker, source line,
mirrored byte count, and SHA-256. All 394 digests were independently checked
against the corresponding exact frozen source prefixes. Both persistent arms
receive identical prefixes; prior frames' execution history remains available.

This is **not** proof of the live brain's exact read timing. `civ6_brain.py`
asks when it observes a state in a polled append-only file; it does not wait on
an export-ready handshake. The immutable replay boundary removes a concrete
missing-terrain defect but cannot retroactively capture an asynchronous read.
The existing full-history A/A gate remains necessary and unchanged.

## Complete control results

| Measurement | Original replay | Repaired replay |
| --- | ---: | ---: |
| Frames replayed | 394 | 394 |
| Exact Native orders **and** complete decision matches | 276 | 372 |
| Mismatches retained | 118 | 22 |
| First mismatch, turn/frame | 1/0 | 52/0 |
| Empty no-board replies | 1 | 0 |
| Missing Native records | 0 | 0 |
| Arm-to-arm complete-reply differences | 0 | 0 |
| Invalid request identities | 0 | 0 |
| Baseline/candidate child exit | 0/0 | 0/0 |
| All input hash guards | pass | pass |
| Full-history gate | **fail** | **fail** |
| Replay command exit | **2** | **2** |

The repaired first mismatch is index 126, turn 52/frame 0. Native moves unit
196608 to `(47,26)` while replay moves it to `(47,27)`; the native-action list
also differs in length (23 versus 24). The full terrain refresh follows that
state within roughly 52 ms in the journal. Asynchronous input visibility is a
plausible explanation, not an established cause. There was no boundary tuning
or rerun to erase this failure, and no substitution of a more convenient run.

The remaining mismatches are at 52/0, 71/0, 72/0, 76/0, 77/0, 80/0, 81/0,
126/0, 127/0, 138/0, 138/1, 139/0, 144/0, 145/0, 146/0, 147/0, 147/1,
148/0, 149/0, 149/1, 149/2, and 150/0. Neither agreement between identical arms
nor 372 matching frames establishes the missing full-history control.

## Validation and evidence

- Regression-first run: 36 tests, nine expected assertion failures against the
  unchanged tool (eight added test methods, with two receipt subcases).
- Repaired focused suite: **36 passed**. Event-sensitive persistent-decider
  fixtures check complete initial terrain, same-turn replans, prior history,
  exact prefix hashes, no current-receipt/future-frame leakage, and fail-closed
  incomplete/mismatched boundaries. Existing Native comparison, identity,
  input-integrity, and process-failure tests remain in the suite.
- Full local tools discovery: **3,415 run, one failure, one skip**. The failure
  is the unrelated live-roadmap hotspot check: `src/ai/advanced/tests.rs` is
  targeted by `docs/ROADMAP.md` despite only 2% of the last 200 merges touching
  it, below the 5% floor. The exact failing test also fails on clean main
  `b2aa14580f74786fccbf6fc3b19442346c33a1ec`. No roadmap, gate, or unrelated test
  was changed to hide this result. Both full and clean-main failure logs remain.
- `cargo test --profile ci --locked`: **6,169 passed, zero failed, 56 ignored**.
- `git diff --check`: pass. Rust quality reports no changed Rust files and
  correctly skips Rust formatting/clippy; no Rust-quality exemption was added.

The evidence bundle retains the fixed registration, complete frozen inputs
including the original executable, both harness snapshots, regression sources,
all four persistent-arm replies/journals, all Native mismatches, every repaired
request prefix, command exits, and local validation logs. Shared file contents
are stored once by SHA-256 without dropping any file identity or bytes.

Archive: `2026-10-17-native-replay-frame-boundary.provenance.json.gz`,
**53 files, 40 unique contents, 16,299,914 bytes**. Every archived blob's SHA-256
was read back and verified, including the original executable. Archive SHA-256:
`87047d25e93798fe1af630f3c017161490623a57df0f9586296f09c69445217d`.

## Next decision

Do not use this replay as a policy-effect or win-rate evaluator yet. Faithful
captured live read inputs, or another demonstrated explanation of the 22
remaining mismatches, are required before Native counterfactual route changes
can be trusted. Independent simulator/mechanics tests may continue, but their
results must not be relabelled as Native conquest victories.
