# Keep an urgent score counter in the Domination campaign

Native `civvis-20260922T041757Z`, pinned to `453093af81ea2986843474f2fd421e11be309dc4`, took Germany's original capital Munich on turn 225. It held Munich at the end but lost on score to China at turn 251. After peace with Germany, the explicit Domination seat assessed Expansion in response to a rival's score threat, despite holding an army much larger than any rival. The generic counter-in-lane policy was answering score with expansion.

An exploratory 700-frame replay through turn 237 disabled counter-in-lane. It changed 15 exported and internal-action frames, beginning at 232/2, and kept the late plan in Conquest. The army still failed the staging gate for China. Enabling the existing capital-focus option on top of that experiment changed no frames, so that option is not promoted here. These recorded-board experiments demonstrate policy routing, not new native captures or a prevented loss.

The proposed change is narrower than the exploratory switch: only an urgent score threat against an explicitly assigned Domination seat chooses Conquest instead of Expansion. Existing pressure eligibility, optional stand-down, target legality and declaration readiness still apply. Other target contracts, Science counters and eligible but nonurgent score alarms retain their policies.

## Validation

The score-leader regression failed on the base because actionable denial was Expansion rather than Conquest; its other-contract control passed. Both candidate tests pass, including the nonurgent score control. `cargo test --profile ci --locked` passes 4,182 tests with 53 ignored. Changed-line Rust quality passes, both release binaries build, the treatment append checks pass 14/14, and eight four-player, 180-turn soak games complete from seed 372000.

An isolated current-source replay compares base `03671874f3271a5df035295ffdbfe1c59cb9109a` against candidate `963fdd9332879cbfc1902c49c5f0f2ce071b02fe`, using the same native input through turn 237 and the same forced genome. This comparison does not disable counter-in-lane. Both binaries complete all 700 frames. Exactly 15 frames change exported orders and internal actions, beginning at turn 232/frame 2; the earlier 685 frames are identical. The late plan changes from Expansion to Conquest against the same rival, China. The existing staging gate still holds the declaration, so this is evidence of corrected policy and orders, not evidence of a new capture or avoided score loss.

Reproduction artifacts are in `/tmp/civvis-3720-score-replay` on the verification host. SHA-256:

- Input: `24c7bc174db2966430a775cb1fa7e4e2f621d4c244b9ba4528bfc1b88794af04`
- Shared arguments: `287ab0f3f0b0cc8526129597075b7dc77ed02aa3f72faddb8f1331d6ad30bdd9`
- Baseline binary: `55e5666dace04fce95297a2755a82ffdc2763ebadf0a203a7672f87ba05eeea0`
- Candidate binary: `cd31a5855489c374e388e01ff7507653f267dfdc3ef9957e95f09620042fbf3a`

Concurrent replay timings are not performance evidence. No native victory improvement is claimed.
