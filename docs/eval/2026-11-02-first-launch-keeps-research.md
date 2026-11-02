# First launch keeps research: preregistration

Hypothesis: after a Science-committed seat owns Rocketry, optional army
modernization and a temporary Recovery posture delay Satellites enough to
harm its race. This opt-in treatment keeps Satellites and its prerequisites
as a research goal until Satellites is owned. It does not interrupt a
technology already being researched.

Evidence motivating the treatment is the native research audit in PR #4336:
13 of 30 retained runs selecting Rocketry have military-attributed selections
outside the model Satellites prerequisite path before selecting Satellites.
The Emperor Science example owns Rocketry at t171 and Satellites at t196,
then loses at t208. Those intervals are not causal savings estimates.

Immediate defensive research, appointed military breakthroughs, decisive
military windows, standing-army fuel, and named production technologies keep
their existing earlier slots. Only the optional modernization and later
economic/lane arms yield. The treatment admits available Satellites
prerequisites across the rolling era window only while it owns the forced
goal. Advanced Flight remains a required prerequisite, irrespective of its
military unlocks. No other victory lane is retargeted.

The gene is off in both controllers and the live bridge. It requires a
Science raced target, enabled Science victory, owned Rocketry, and missing
Satellites. A finished Satellites or excluded Science race releases the rule.

## Validation and experiment before any promotion

Unit checks exercise actual research selections in Recovery, the Advanced
Flight prerequisite outside the era window, modernization with a gene-off
control, an appointed war breakthrough, and continuation of an existing
research choice. Additional controls cover another target, unknown Rocketry,
disabled Science victory, and already owned Satellites.

First run the standard six-player Continents Emperor screen, 64 games on
seeds 202611020000–202611020063, Online 250-turn clock, all six target lanes,
with only this gene randomized. Use three workers to leave capacity for the
native verification run. Preserve the raw source outside Git, with its
header and final analysis receipt attached before any verdict.

```sh
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --games 64 --jobs 3 --difficulty emperor \
  --start-seed 202611020000 --genes first-launch-keeps-research --trace \
  --out /tmp/first-launch-keeps-research-screen.jsonl
```

The primary endpoint is wins with errors clustered by game; score share and
finish timing are secondary. Read technology counts, Science, first launches,
and cities lost in the trace. Check activation/exposure: an all-zero or
rarely reached first-launch stage leaves the trial inconclusive. Do not infer
success from faster research or fewer modernization choices alone.

If the initial result supports continued testing, reserve disjoint seeds
202611021000–202611021127 for 128 confirmation games. A default promotion
also needs native follow-up demonstrating earlier Satellites and first launch,
with survival/cities-lost and actual wins recorded. The earlier first-pad
placement and Satellite production treatments retain separate identities;
do not credit their effect to this research treatment.

Status: implementation and validation in progress; no outcome or win-rate
improvement established, no default promotion authorized by this evidence.

## Validation observation, 2026-11-02

All six focused tests pass with
`CARGO_PROFILE_CI_OPT_LEVEL=0 cargo test --profile ci --locked --lib first_launch_research`.
This is an unoptimized local behavior check, not the full optimized CI suite.
The Recovery fixture explicitly enables the victory portfolio and asserts
that its effective objective is Recovery before checking research choices.

The next completed native loss, `civvis-20261102T163456Z` (Emperor,
Domination target, controller `27927468d`), independently emphasizes the
launch endpoint. Its native states first show Rocketry at t166 and Satellites
at t189; the terminal t227 state reports 12 cities, 449.773 Science per turn,
Future Tech being researched, no science projects, and zero science victory
points. The ladder records a loss to Technology and no first Spaceport.
This is outside the new treatment's Science-target gate, so it is neither a
treatment result nor evidence for broadening the rule. It illustrates why
earlier Satellites alone cannot count as the goal: the lane handover and
launch-production policies must also turn that economy into a victory.
