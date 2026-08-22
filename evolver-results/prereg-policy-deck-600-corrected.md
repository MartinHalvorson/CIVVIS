# Pre-registration — the live-policy-deck run, corrected: a fresh 600, not an extension

Written 2026-07-31, superseding
`/Users/martin/civvis-policy-deck-extension-preregistration.md` **before any
map of it was run**. Agent `claude-evolver`.

## The correction, and it invalidates my earlier plan

The earlier pre-registration proposed *extending* the recorded 300-map prefix
at seed 22,051,000 to 600, on the ground that
`MATRIX_PROFILE_SEED_STRIDE` is constant so a prefix can be grown without
moving either profile onto different maps.

**That is true of the maps and false of the agent.** The recorded 300-map
result — compact 52.3%/+16, deployment 54.3%/+30, direction 92–51 p=0.0008,
anytime crossed at map 98 — was measured on a much older tree. Since then
`#686` changed settler behaviour, `#697` landed the Civilization VI bridge and
touched both agents, `#708` replaced the shipped champion, and the Elo protocol
advanced to **v3** precisely because `advanced_v1`'s live path changed.

Pooling maps 301–600 measured on today's agent with maps 1–300 measured on that
one would be mixing two agents inside one interval. This is the eighth check in
`civvis-measurement-discipline` applied to my own plan: *a long evaluation
outlives the tree it was launched from*.

⚠ The arm itself is unaffected by the champion swap —
`advanced_policy_live_control` is `Weights::default()` with
`PolicyDeck::Live`, and its control is `Weights::default()` with `Legacy`;
neither resolves `data/evolved/best.json`. The problem is the *engine and agent*
underneath both, not the genome.

## The run, fixed now

A **fresh, whole** 600-map matrix on the current tip — not an extension:

```sh
ai_eval advanced_policy_live_control advanced --matrix --pairs 600 --jobs 5 \
  --seed 22051000
```

Seed 22,051,000 is deliberately the recorded one, so the first 300 maps are the
same terrain and the run doubles as a re-measurement of the recorded result on
the current agent. Every map in the reported interval is measured on one agent.

## The rule, fixed now

- The **unmodified** matrix rule decides: deployment must return
  `promotion gate: PASS`, compact must not establish a regression.
- **PASS → the deliverable is a one-line change of
  `Weights::default().policy_deck` from `Legacy` to `Live`**, plus a
  measurement of what the counterfactual valuation costs a game-turn, because
  `f2d53cf` defaulted it to `Legacy` on compute grounds and that trade must be
  priced before it is reversed.
- **Anything else → recorded as a second inconclusive at higher power on the
  current agent**, and the default stays `Legacy`. No extension of this run, no
  new seed.
- The 300-map recorded numbers are **not** pooled with this and are not quoted
  alongside it; they belong to a superseded agent.
