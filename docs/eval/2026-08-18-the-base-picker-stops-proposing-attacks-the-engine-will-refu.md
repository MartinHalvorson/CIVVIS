# The base picker stops proposing attacks the engine will refuse

_2026-08-18 · `agent/mbp-m5-max-128/claude-fable-multistep`_

## What was asked

The same-turn multi-step contract — a unit steps, the `0..8` loop re-invokes
its step function, and it shoots / settles / improves from the tile the step
opened — was audited end to end. The settler and builder halves are clean: a
refusal census of one deployment game recorded **zero** authoritative
`FoundCity` and zero `Improve` refusals, and three new contract tests pin the
step-then-act turn for archer, settler, and builder. The military half was
not. `BasicAi::military_step` proposes one attack candidate per enemy tile in
reach on distance alone, scores candidates **statically** (no speculative
clone), and applies the argmax winner. So an order the engine refuses —
invisible target, blocked line of sight, wrong melee domain, unpayable entry —
can win the argmax outright, and when it does the unit takes no offensive
action at all: the legal shot that scored second is shadowed, not merely
delayed. The sibling round
(`2026-08-18-the-tactical-picker-stops-paying-for-attacks-the-engine-will.md`)
fixed exactly this in `AdvancedAi`'s clone-scored picker, where a refused
candidate could never win and the change was answer-identical. In the base
picker the refused candidate *does* win, so this is a behavior change and was
treated as one.

How large is the defect, who does it reach, and what does fixing it cost?

## How it was measured

1. **A refusal census** — instrumenting the `Err` arm of `Game::apply` on the
   authoritative board (speculative clones excluded via
   `visibility_suppressed`), with a backtrace naming the proposing call site.
   One 150-turn six-player game at the deployment shape (74x46, 6
   city-states, Online), seed 7700000, before and after; seed 8800000 after,
   as the independent confirmation.
2. **The frozen-anchor behavior test.** The fix is gated behind
   `BasicAi::legal_tactical_candidates`, off in both `BasicAi` constructors
   and never set by `AdvancedAi::legacy()`;
   `advanced_v1_plays_the_same_game_it_always_did` replays the anchor's five
   profiles and must return the pinned 18,572 decisions and fingerprint.
3. **The seat-local price**, by withholding: `advanced_without_legal_candidates`
   (production minus the screen) against `advanced`, 24 pairs / 48 games per
   seed at 6p 74x46, 9 city-states, Online, 250 turns, all six victories,
   prince, two disjoint seed streams (31150000, 31250000). The environment
   cannot express the treatment in `ai_eval` — minor seats are driven by the
   `basic` arm there, identically in both arms — so this prices exactly the
   major seat's own delegation to the base picker.
4. **Contract tests**: `a_refused_shot_no_longer_shadows_the_legal_one` pins
   both halves (the frozen identities keep the historical shadowing; the
   screened picker takes the legal shot), with engine-level preconditions
   asserted through the new `Game::ranged_order_is_legal` /
   `melee_order_is_legal` predicates — `legal_actions_within`'s own checks,
   exposed rather than re-derived. Three further tests pin the same-turn
   multi-step contract itself: `an_archer_steps_once_and_shoots_in_the_same_turn`,
   `a_settler_steps_once_and_founds_in_the_same_turn`,
   `a_builder_steps_once_and_improves_in_the_same_turn`.

No speed claim is made in either direction: the fleet was at load ~80 all
session, which the measurement doctrine rules out for timing. The vision
frames are hoisted and lazy per unit, the pattern whose absence the sibling
round measured at +6.43%.

## What it measured

**The census.** Seed 7700000, authoritative board, `military_step` call sites:

| | before | after |
|---|---:|---:|
| `Ranged` "target is not visible" | 281 | 0 |
| `Ranged` "line of sight blocked" | 195 | 0 |
| `Ranged` "nothing to attack" | 26 | 0 |
| `Attack` (domain / movement / no target) | 17 | 0 |
| refused `Move` from `tactical_step` (pre-existing, out of scope) | 9 | 17 |

519 refused combat orders per game, **every one of them from
`BasicAi::military_step`** — majors contributed 10 (pids 0 and 4), city-states
and the barbarian seat the other 509. After the screen: zero, on both seeds
(seed 8800000 after: zero refused `Ranged`/`Attack`; its only two
`military_step` refusals are pre-existing `CondemnHeretic` ones). The residual
refused `Move`s are `tactical_step` stacking races, a different and
self-healing class.

**The anchor**: `advanced_v1_plays_the_same_game_it_always_did` passes on this
branch — 18,572 decisions, `0x3bda_c2f2_b84d_30fc` — so every frozen identity
(the anchor's legacy base, the dated `basic` entrant, the evaluator controls)
still plays the recorded game.

**The seat-local price** (withholding arm vs production, 24 pairs / 48 games
per seed):

| seed stream | paired-map score | outcomes | terminal-score direction | gate |
|---|---|---|---|---|
| 31150000 | 50.0% (betting CI 32.1%..67.9%), Elo +0 (CI −130..+130) | 0 sweeps / **24 neutral** / 0 | 1 for, 21 neutral, 2 against, p=1.0000 | INCONCLUSIVE |
| 31250000 | 50.0% (betting CI 32.1%..67.9%), Elo +0 (CI −130..+130) | 0 sweeps / **24 neutral** / 0 | 1 for, 23 neutral, 0 against, p=1.0000 | INCONCLUSIVE |

Twenty-four neutral splits on twenty-four maps, both seeds, is the signature
of two agents playing essentially the same games — consistent with the census:
majors reach this candidate loop about ten times a game, and dropping a
refused winner changes play only when a legal runner-up cleared the threshold.
The major seat's own delegation is priced at nothing; the change's payload is
the environment, which a paired arm cannot express by construction.

## What was decided

**Shipped as a correctness and fidelity change, priced by withholding.** The
screen ships on in `AdvancedAi::promoted_policy_envoy`, so production fleets —
which drive every seat, city-states and barbarians included — stop skipping
the legal shots their refused argmax winners were shadowing. Civilization VI's
minors shoot what they can see; ours were declining to, several hundred times
a game. The dominant effect is environmental (minors and barbarians defend
themselves properly against every major symmetrically), which no paired arm
can price by construction; the seat-local component is priced above and the
axis stays measurable through `advanced_without_legal_candidates`.

This deliberately does not touch the two arm-only blind-apply sites
(`timed_war_objective_step`'s breach shot, `rush_siege_step`'s wall attack) —
they tolerate refusal by design and run only under their arms — but the two
new `Game` predicates are the one-line fix if their owners want it. The
`strike_opening` / `ranged_needs_line_of_sight` / `joint_tactics` quarantine
(live-bridge-only, measured in `docs/TACTICS.md`) is likewise untouched: those
are about *seeking* the tile a shot needs; this is about not proposing orders
`Game::apply` refuses from the tile the unit already holds.
