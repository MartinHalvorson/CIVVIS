# Slinger first: a bounded opening experiment

The recent Standard-speed Immortal/Deity Native cohort opens with a Scout in all 15 sampled games. In six, the first soldier queue interrupts positive Scout production; nine show a completed Scout in the opening prefix. These are descriptive observations through turn 20, including later-abandoned games and several civilizations and source revisions. They are not a treatment comparison.

In `civvis-20261104T015124Z`, the Scout is queued on turn 1, a Slinger replaces it on turn 7, and later menus show 18 production banked in the unfinished Scout. The Slinger appears on turn 17; Vietnam attacks that turn and the capital falls on turn 18. The starting Warrior is far away. Another game, `civvis-20261104T002534Z`, loses its capital on turn 37 with several defenders near home. Earlier support might help some starts; it does not explain every early defeat.

The fixed cohort record is bound by SHA-256 `60671ed4f3919a4cb43ba479b05c07af2a48c8574049ce8f26f5ba9903b2fb7f`. The source run's first-production captured input is `fbd291b00f1d284600b6c36231d60964202be7739469b09bdebdbcf0fffa9630`. No future state or outcome is consulted by the policy.

At turn 1/frame 2, the host quotes `UNIT_SCOUT` at 30 production/10 turns, `UNIT_WARRIOR` at 40/14 and `UNIT_SLINGER` at 35/12, from a capital making 3 production. These are observed menu quotes, not counterfactual completion times.

## Why a separate flag is necessary

A repeated 2x2 query on the exact Native executable from `015124Z` (`a21e4648769c924b954f1eb8f28873c49c77a4a8921ae07072e2296bb6036c04`) gives:

| scout-first-opening | recon-replacement | first production |
| --- | --- | --- |
| on | on | Scout |
| off | on | Scout |
| on | off | Scout |
| off | off | Warrior |

All four planning-payload repeats match, and the both-on orders match the actual Native first-production request. These are eight fresh-board queries on one captured position, not persistent-controller qualification or survival tests. The ordinary first book slot is Warrior; `recon-replacement` can substitute Scout even when `scout-first-opening` is disabled. Turning both off for a whole game would also remove later recon replacement, so it cannot isolate an opening-only intervention.

## Candidate behavior

`opening-ranged-support` is an independent opt-in, off by default. It replaces exactly the first capital opening slot with a legal Slinger when the empire has one city, a non-embarked land melee defender stands within five tiles, and no ranged military unit is fielded. It leaves occupied queues, minors, barbarians, later book slots, established empires and unsupported capitals alone. It runs before both existing Scout substitutions. Production legality remains the normal engine/host gate.

The dispatcher then advances the book by one, just as the existing Scout substitution does. The recon flags and subsequent recon rules are unchanged. **The opening Scout itself is displaced:** later exploration can be delayed. This is the experiment's cost, alongside possible delays in contact, settlement discovery and city-state rewards. Keeping a flag enabled does not mean its downstream decisions or timing remain identical after the opening changes.

## Captured first-turn control

The published [fixture](../../src/ai/slinger_first/fixture/manifest.json) includes the exact 69,162-byte input archive, its snapshot index, and the three signed Native decision records for turn 1/frames 0, 1 and 2. The original Native executable reproduces the full planning payloads and ordered reads twice. Candidate-OFF does the same twice. Candidate-ON repeats twice and first changes a host order at turn 1/frame 2: the capital starts `UNIT_SLINGER` rather than `UNIT_SCOUT`; all other host orders stay identical. No recorded inputs are followed beyond that first changed host order.

Those candidate controls use source `a88bdf08ff9e0937ef96b4ffda8048095a3de4f4`, frozen unoptimized CLI SHA-256 `50ddde2bd792db87ed3d8f9470bf6e0bef0db4d2156eb2274d4f9d703e1027d1`. The manifest binds the historical Native executable and inputs separately. This qualifies the recorded first turn, including founding and the preceding frames; it does not qualify a later counterfactual world or measure survival.

Reproduce the candidate controls from the repository root with an explicitly supplied CLI and a new output directory:

```sh
python3 src/ai/slinger_first/replay_opening.py \
  --binary target/debug/civvis_orders --out /tmp/civvis-opening-prefix
```

The manual reproducer checks archive hashes and Native record signatures, both OFF planning/read controls, both ON played profiles, and the single host-order change. It runs the planner in isolated directories and does not launch or control Civilization VI. If a future baseline intentionally changes this frame, retain this historical fixture and report the mismatch; do not silently refresh its Native expectations.

## Validation and limits

Seven focused behavioral tests cover first-slot precedence, release to the Settler after completion, off behavior under both existing Scout routes, existing ranged support, occupied/played/non-capital queues, missing or distant defenders, and multi-city exclusion. The PR records execution results and captured-board controls. These checks do not establish survival or win-rate gains.

No Native arm is enabled by this change. The earlier Anvil pilot stopped when its baseline changed, and the host's System Events suspension remains unresolved. No claim of improved survival or win rate follows from a different production order.

Before a Native trial, recheck the actual published binary against this prefix and confirm the played profiles differ by exactly this treatment. Any headless whole-game screen is a separate instrument: starts already carrying ranged units cannot exercise this opening, and passive rival openings cannot establish rush resistance. A later bounded Native pilot must retain failed starts separately, record actual played genomes and source revisions, keep speeds and difficulties distinct, and retain all assigned overhang. Delivery, earlier support, and the defender positions relative to the capital when the first rival attacks are mechanism endpoints; higher-level wins remain the strength endpoint. No default promotion is justified yet.
