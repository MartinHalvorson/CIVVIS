# Pre-registration — re-measure r3 on current main before proposing the swap

Written 2026-07-31 **before any map was run on the current tip**. Agent
`claude-evolver`.

## Why this is forced, and it is not a third bite

Both passing matrices for `r3` — seed 67,000,000 and the disjoint confirmation
at 70,000,000 — were run on a binary built from `3916358`. Seventeen commits
have landed since, and one of them is decisive:

- **`065beec` — "Prevent settlers stalling beside viable island sites" (#686)**,
  which changes `src/ai.rs` and `src/ai/advanced.rs` **settler behaviour**.

The eleven genes `r3` reverts are `docs/GENOME.md`'s economy and expansion
blocks. A change to settler stalling lands in exactly the subsystem those genes
govern, so the agent I measured is not the agent that would ship. `src/ai.rs`
gained 114 lines, `src/ai/advanced.rs` 66 and `src/game.rs` 165 across that
range.

This repository has a recorded instance of the same failure — *"that eval's
binary was built in #393's worktree, not #411's"* — and the standing rule is to
measure the configuration that ships.

**This is a re-measurement compelled by a code change, not a further attempt at
a favourable read.** The candidate is unchanged, the rule is unchanged, and the
seed is one already used and already passed, so a regression here cannot be
explained away as a new sample.

## The run, fixed now

```sh
ai_eval advanced_evolved advanced --matrix --pairs 300 --jobs 5 --seed 70000000
```

built from the current `origin/main` tip, `r3` staged at `evolved/best.json`.
Seed 70,000,000 is deliberately the **same** seed the pre-#686 confirmation
passed on, so this isolates the code change: same maps, same candidate, same
rule, different agent underneath.

## The rule, fixed now

- The **unmodified** matrix rule decides. Deployment must return
  `promotion gate: PASS`; compact must not establish a regression.
- **PASS → the artifact swap is proposed**, citing three matrices: two on the
  pre-#686 agent and this one on the shipping agent.
- **Anything else → the swap is NOT proposed.** The finding is then recorded as
  "the eleven-gene reversion passed twice on the pre-#686 agent and did not
  carry across #686", which is a result about the interaction and is worth
  having. No further seed, no pooling with the pre-#686 runs.
