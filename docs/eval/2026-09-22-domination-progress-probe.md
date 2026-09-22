# Gran Colombia siege-progress probe — 2026-09-22

The `siege-is-progress-3` policy produced **zero focal wins in either arm across
32 matched pairs**. Only one pair changed its applied-action history. This is
insufficient evidence to enable the policy; the deployment bundle is unchanged.

## Method and provenance

The new `victory_eval --domination-pair POLICY` mode holds one Gran Colombia
seat to Domination and pits it against three adaptive CIVVIS live-bridge rivals.
These are simulator opponents, not Firaxis AI. The focal player is exempt from
AI difficulty bonuses; the three rivals receive King bonuses. The profile is
44×26 Pangaea, Online speed, six city-states, barbarians, all victory conditions,
a natural 250-turn clock and no mercy rule. Barbarians retain the repository's
separate **Emperor** difficulty, per the existing training setting; this is
recorded in every row and is not an exact native-game reproduction.

Each seed initializes fresh worlds and controllers for both arms. Only the
focal seat's named policy changes. The focal seat receives the compiled native
live-force-on bundle; rivals retain their adaptive deployed controllers.
Execution alternates off/on and on/off. Complete serialized applied-action
histories are compared byte for byte before their summaries are written.
Foreign-city counts mean unique foreign cities observed held at turn boundaries
or the end, not every capture or razing event.

The plan was registered at 02:26:40 UTC: 32 pairs, seeds 37140000–37140031,
four eight-pair blocks, complete every pair regardless of the result. The
initial run on `7c572e5a2` crashed at seed 37140009 with an unchecked city lookup
in settlement validation. It completed 25 other pairs, all identical and all
focal losses. Those interrupted-run observations are **excluded** below.

A timestamped amendment retained the policy, sample and seeds and required a
complete rerun after fixing the panic (#3716). All 32 final pairs completed on
source `d11e4efec02ee74794d4eb7d04e1a5dc55c8ead7`, integrating main
`453093af81ea2986843474f2fd421e11be309dc4`. That integration also includes the
airfield reservation fix (#3707). The release binary SHA-256 was
`5292acbcb196b2d049f7e6ff0883c11a7a8d55e9425e3346bcffb603081dcc59`.
The rerun began at 04:41:54 UTC. Later strategy changes on main are not measured
by this frozen sample.

Build and run at the recorded source:

```sh
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_INCREMENTAL=true cargo build --release --locked --features developer-tools --bin victory_eval
target/release/victory_eval --domination-pair siege-is-progress-3 --games 8 --start-seed 37140000 --out /tmp/block0.jsonl
```

Repeat with start seeds 37140008, 37140016 and 37140024 and distinct new output
files. Output files are created exclusively, never overwritten. A block with
no changed action histories exits 2 **after writing every pair**, explicitly
reporting lack of behavioral contrast. The other block exits successfully.

## Results

| Measure | Off | On |
|---|---:|---:|
| Focal wins | 0/32 | 0/32 |
| Focal Domination wins | 0/32 | 0/32 |
| Mean final focal score | 244.500 | 247.219 |
| Foreign cities observed held, summed across games | 5 | 5 |
| Foreign cities held at the end, summed across games | 4 | 4 |
| Rival Religious victories | 17 | 18 |
| Rival Science victories | 8 | 7 |
| Rival Culture victories | 7 | 7 |

Exactly one pair changed: seed **37140013**, against Aztec, Sumeria and China.
Off ended at turn 204 with an Aztec Science victory and focal score 193. On
ended at turn 147 with an Aztec Religious victory and focal score 280. Neither
arm held a foreign city in that pair. The score increase occurred in an earlier
loss to a different victory condition; it is not evidence of stronger conquest.
The formerly crashing seed 37140009 completed in both arms.

The [32 machine-readable pairs](2026-09-22-domination-progress-probe.jsonl)
include complete setup axes, actual civilizations, execution order, policy
bundle, outcomes, action contrast and source/binary provenance.

## Interpretation

The native replay that motivated this test showed the policy suppressing a
peace offer when a siege was finally staged. That established a decision
change, not a victory benefit. This probe adds one exposed pair and no wins:
31 pairs have identical actions, so they cannot distinguish the policy's
consequences. Zero wins in both arms does not establish equivalence or safety.
No promotion-ledger entry or King-to-Emperor progression is credited. The next
optimization must address demonstrated conquest bottlenecks; extending wars
by itself has not earned deployment here.
