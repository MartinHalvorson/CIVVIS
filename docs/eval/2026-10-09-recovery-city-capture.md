# City capture during unit recovery

The preservation filter previously withheld every recovering unit's orders
until full health, including a survivable adjacent city capture. Finisher
admission also rejected a recovering attacker before considering the existing
survival bounds. The unchanged-production baseline executes all thirteen
registered cases: exactly those two defects fail and eleven controls pass.
The candidate preserves a narrowly verified capture while retaining recovery
memory for other orders.

## Native evidence and limits

The completed Gran Colombia four-player native game
`civvis-20261009T054512Z` lost to Culture on turn 223. It records 80 unit kills
and no city captures: 45 kills are barbarians, 18 player 3, 16 player 2 and one
player 4. All 36 friendly district combats use Bombers; native city centers
use the district event type. Fourteen target Gao, a non-capital Malian city
at native `(18,22)`. Combat readbacks reach zero city health on turns 222–223,
while state exports retain Malian ownership.

Modern Armor `15400963` requests two attacks on Gao on turn 222. Both verdicts
are `target_unharmed`, with no attributed melee combat. On turn 223 it fortifies
and moves away. The planner projects captures and then records preservation
at 73 health with an upper reply of zero. Original prefilter orders are absent,
so this does not establish the exact cause of the failed native attacks or
prove an alternative native operation would capture the city.

Native revision `a66650568cff2b2d6c4ccb2db59a1c55fb72256f` and the public
preservation module at investigation start share SHA-256
`fd3d54fdec3778abcfb629093900b0468f9e6b1ac154ba39eeeb8708c41da576`.
The rest of the controller is not source-equivalent. Original input hashes
remain unchanged in `native-054512-capture-gap-receipt.json` under the retained
goal artifacts. No native lane, pin, policy, installed mod, private source or
process is changed. No candidate adoption, native capture or win-rate gain is
credited; completed native readbacks and matched outcomes remain needed.

## Candidate behavior

One original-adjacent land melee attack, optionally followed by fortification,
may pass recovery when the observed enemy city is at war, unwalled and already
at zero or one health. A disposable legal replay must change ownership and
leave the living actor on the city. Existing conservative host retaliation
bounds remain enforced.

The wounded actor must survive the captured-position reply and an exposed
original-position probe at post-exchange health. The exposed probe retains
all originally observed enemies and their city ownership, preventing garrison
shielding or observed-world elimination from manufacturing permission. The
whole-turn fixed point rechecks ownership, occupation and survival after the
other proposed orders are filtered.

Recovery memory stays set. Ordinary movement, ranged strikes, approaches,
standing walls, uncertain captures, peace, lethal retaliation, exposed replies,
lethal hazards and exhausted attackers retain the existing recovery behavior.
A living assigned Settler guard keeps its escort role. PR #4011's shared
movement, founding and retreat handling is preserved.

The fixture uses a 73-health Modern Armor beside an unwalled one-health city
and retains the `95 versus 92` native strength preview. A second enemy city
prevents elimination from manufacturing safety. Fourteen candidate controls
check legal physical ownership and occupation, unchanged input, retained
recovery memory, both admission/filter defects and the preserved exclusions.
The city finisher case verifies its API contract; it does not establish that a
current native city planner calls that finisher API.

## Validation and retained failures

The first baseline's Arena peace fixture still forces war, and its ordinary
move accidentally matches a preservation retreat. Those fixtures are corrected
using Continents and a different legal destination. An unused trait import is
removed separately. None of those failures counts as behavioral defect proof.

The complete twelve-case local baseline on `fa8307aa5` executes all cases:
nine pass, the two intended defects fail, and an inappropriate enemy-reply
assertion fails. Existing danger modeling shields city garrisons. That fixture
is corrected to verify the exposed original position at post-exchange health;
an additional control checks a lethal 75-damage burning tile.

The complete thirteen-case local baseline on `81c70e045` exits 101 with exactly
two intended failures and eleven passes. Registered hashes remain unchanged.
Independent CI `37904606128` reproduces the erased capture and passes all eleven
controls; fail-fast stops before scheduling the finisher case. Production is
implemented only after this exact baseline. The candidate adds a fourteenth
control for the living Settler guard.

The obsolete pre-merge candidate compile is stopped intentionally with exit
130 before any tests after GitHub reports a real adjacent-insertion conflict
with merged PR #4011. Only the owned Cargo and rustc processes are interrupted.
This is not a passing test result or a timeout. Main `66b411361` is merged once
as `9643f4612`, retaining both recovery eligibility and shared departure lines,
with every escort hunk preserved.

Validation on `9643f4612`:

- Local focus: all 35 controls pass, zero failures — 14 capture plus all 21
  preservation/escort controls. All registered hashes remain unchanged.
- Complete local `cargo test --profile ci --locked -- --test-threads=1` exits
  zero: 4,739 pass, zero fail, 54 existing ignores, covering the library, CLI,
  orders, integration, protocol and documentation targets. All 525 registered
  source/test/data/Cargo file hashes remain unchanged. Both existing spectator
  timing controls pass without source changes.
- Independent CI `37907493134`: all 4,739 Rust tests pass with 50 existing
  ignores and four ignored documentation examples; all 35 preservation
  controls and overlapping 71/21/22 feature checks pass. Provenance, fidelity,
  quality, policy, overwrite, mod and publication gates pass.
- Cost CI `37907493173` passes its 8% budget: median -0.19% over five pairs,
  within the 1% noise floor, resolution ±0.43 percentage points, IQR 0.66.
  No speed gain is claimed.
- `git diff --check origin/main...` passes. This is AI policy with no engine
  mechanic change requiring an engine soak.

Receipts and complete logs remain in the goal artifacts. The final documentation
checkpoint changes only this evaluation record; the validated Rust sources,
tests, data and Cargo files remain byte-identical. Final-head CI, ready gates,
squash integration and live adoption of the public revision are tracked by the
repository workflow and integration receipt. Public spectator deployment is
separate from adoption by the private native verification controller.
