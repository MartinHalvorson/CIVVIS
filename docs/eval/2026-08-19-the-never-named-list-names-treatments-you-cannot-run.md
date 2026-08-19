# The never-named list names treatments you cannot run

_2026-08-19 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

Roadmap objective 3 records **35 shipped live treatments never named in any
round**. A treatment that never *fires* is dead code removable without a
strength gate, because removing something provably inert is behaviour-
preserving. So: which of them fire at all?

## How it was measured

All 33 `live_without_*` arms against `live`, 10 pairs each, at a deliberately
cheap profile — 4p 40x26, 150 turns, seed 62000000 — as a **candidate filter**.
An arm that fires there is certainly not dead; one that does not needs
confirmation at the deployment shape before anything is concluded.

## What it measured

**Eight arms report #2003's "nothing differed" on the cheap profile:**
`district-coverage`, `escort-unstick`, `housing-buildings`,
`housing-districts`, `religion-sues-peace`, `score-horizon`,
`slot-kind-tiebreak`, `war-patience`.

⚠ **That is a candidate list and nothing more.** 150 turns on a small map does
not reach housing pressure, a stuck escort, or a religion suing for peace, so
"did not fire here" is exactly the weak reading #2042 warned about. Confirmation
at the deployment shape is the next step and is not in this round.

⚠ **Two attempts to strengthen it with counters failed and are recorded rather
than used.** The first used `thread_local!` counters and read zero for every
branch — `ai_eval` runs its games on worker threads, so the main thread saw
nothing. A census that reports zero is a broken census, which is this
repository's own phrase. The second, on atomics, read `enter=1` across four
250-turn games, which is not credible for a per-city production valuation, and
no conclusion is drawn from either.

## The defect the search itself turned up

Chasing one candidate, `ai_eval` refused the name the published list gives:
`live_without_ranged_line_of_sight` is not an arm.

`docs/EVAL_STATUS.md` publishes the never-named list as the work objective 3
asks the fleet to do, and the arm name was **derived** as
`live_without_{tag with underscores}`. There is no such rule, and both obvious
ones are wrong somewhere:

| tag | its arm | derived from |
|---|---|---|
| `ranged-line-of-sight` | `live_without_ranged_needs_line_of_sight` | the **flag** |
| `army-target-weighs-enemy` | `live_without_army_target_weighs_enemy` | the **tag** (its flag is `army_target_weighs_the_enemy`) |

The derivation was doing two jobs and getting both wrong for those tags: it
printed a name nobody can run, and the evidence search uses the arm name as one
of its spellings — so a round that used the real one was invisible, over-counting
the debt.

**The arm is now looked up in `EVAL_ONLY_AIS` rather than guessed**, and a
withholdable treatment with no arm raises instead of producing a plausible
string. `docs/EVAL_STATUS.md` now prints `` `tag` (`arm`) `` so the list is
directly runnable.

## And the guard I got wrong first

Scoped to every row of `LIVE_TREATMENTS`, the new check raised on
`strategic-wonders` — apparently a withholdable treatment with no arm, which
would have been a real find. **It is not.** `strategic-wonders` is not in
`LIVE_BRIDGE_TREATMENTS`, so it is not withholdable and correctly has no
`live_without_*` arm.

I registered one anyway, and three existing tests refused it —
`each_live_without_arm_holds_exactly_one_treatment_off` failing with
*"strategic-wonders is not a live-bridge treatment"*. The arm is reverted and
the check is scoped to `LIVE_BRIDGE_TREATMENTS` minus `FIRAXIS_ONLY_TREATMENTS`,
which is what "withholdable" means everywhere else in the file.

Recorded because the near-miss is the useful part: a guard written from one
example generalised past its evidence, and the thing that caught it was a test
somebody else wrote for the same invariant from the other side.
