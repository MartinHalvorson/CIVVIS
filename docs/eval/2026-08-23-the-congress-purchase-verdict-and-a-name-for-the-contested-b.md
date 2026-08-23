# The congress purchase verdict, and a name for the contested board

_2026-08-23 · `agent/mbp-m5-max-128/claude-fable-denial`_

## What was asked

Three questions, all downstream of the same hole. Of 232 terminal live games,
**83 were lost to a rival's victory before the clock — diplomatic 47, culture
27** (`docs/EVAL_STATUS.md`), four of them while our own score was the highest
on the board. Diplomatic Victory Points are the currency of the largest bucket,
and the World Congress is where they are spent.

1. **Does a multi-vote World Congress purchase land yet?** #2108 shipped a
   mispricing theory — a core charging the Standard vote curve while the
   accessor reports the Online one would refuse every ask the seat had ever
   made — and pre-registered its own falsifier: *"watch the first
   `wc_ballot_verdict` with `asked = 3`; `recorded 1` kills the affordability
   theory too."* The probe shipped. **Nobody read it.**
2. **Should the contested board have a name?** `deployment-contested` exists
   only inside `--matrix`. Every single-arm round measured on it retyped the
   world by hand.
3. **What is `advanced_congress_banks_decided` worth?** It ships OFF, fires on
   13.5% of ballot decisions, and has never been gated.

## How it was measured

**The verdict, from the refusal ledger and not from a source field.** Every
`wc_ballot_verdict` row under `~/civvis-civ6-runs/control/*/events.jsonl` —
**802 rows across 28 runs** — parsed directly, plus the 2,446 `wc_vote` rows
beside them. `registered` is computed host-side as `recorded >= asked`
(`CivvisControlAgent.lua:13297`); the `source` field says only which trigger
fired, which is the discriminator this project has been burned by.

**The board, from the code and the recorded rounds.** `PROMOTION_PROFILES` in
`src/bin/ai_eval.rs` against the four contested rounds already in `docs/eval/`.

**The arm, on both boards, on one seed stream.**
`advanced_congress_banks_decided` against `advanced`, **60 pairs / 120 games**,
seed 36000000, run twice:

- `--profile deployment-contested` — 6 players, 74×46, 9 city-states, Online,
  250 turns, all six victories, continents/planet/poles/randomized, the four
  non-entrant chairs seated `live_target_diplomatic, live_target_culture`;
- `--profile deployment-online` — the identical world, fieldless.

Same seed prefix `36000000..=36000059`, so these are the same sixty maps. One
axis differs between the arms (`congress-banks-a-decided-vote`), so neither run
needed `--deployment-comparison`.

## What it measured

### 1. The purchase is still refused whole, and #2108's theory is dead

| asked | recorded | rows |
|---:|---:|---:|
| 1 | 1 | 663 |
| **3** (the probe) | **1** | **23** |
| 6–21 | 1 | 116 |

**139 multi-vote asks. Zero registered. The split is perfect in both
directions**, over four times the corpus #2108 was written on.

The probe is the part that settles it. Twenty-three `asked = 3` ballots across
**nine separate post-#2108 runs**, every one `recorded 1`. Three votes cost
**12 Favor on all 23** against banks of **169–427**, with `MaxVotes` 9–15 and
the verdict's own dual walk reading `host` 9–15 and `standard` 6–9 — so three
votes were inside **both** price tables on **every** probe. That is precisely
the ask no ballot had ever made when the mispricing theory was written, and it
is the ask a Standard-charging core would have honoured. **It was refused.**

Two more explanations die beside it:

- **The moment.** 14 of the 31 probe ballots were cast with
  `in_congress_segment = true`, from inside `TURNSEG_WORLDCONGRESS_*` — the
  window the shipped popup votes in. Same result.
- **The ballot.** `option_asked == option_recorded` on **82.8%** of one-vote
  rows and **73.4%** of multi-vote rows. The ballot registers and flips its
  option at essentially the same rate either way; only the **count** is
  clamped.

And the parameter path is not the difference. Firaxis' own `OnAccept` in the
installed game — `…/Expansion2/UI/Additions/WorldCongressPopup.lua:2239-2270` —
sends `PARAM_RESOLUTION_TYPE`, `PARAM_WORLD_CONGRESS_VOTES`,
`PARAM_RESOLUTION_OPTION`, `PARAM_RESOLUTION_SELECTION` through
`WORLD_CONGRESS_RESOLUTION_VOTE` and then `WORLD_CONGRESS_SUBMIT_TURN`. The mod
sends the same four parameters through the same operation and the same submit
(`CivvisControlAgent.lua:14040-14093`).

**Verdict: still refused whole. `PARAM_WORLD_CONGRESS_VOTES > 1` is not
honoured through `UI.RequestPlayerOperation` for this seat, and every
explanation the mod controls is now eliminated with data rather than argued.**

### 2. And the ledger has been reporting a spend that never happened

`wc_vote.spent` is the mod's **model** of the stake: it walks the host's cost
table and adds the charge for every vote it *asked* for. The host charges for
the votes it *records*, and the first vote on any ballot is free.

**620 `wc_vote` rows report a spend above zero, totalling 333,840 Favor.** For
69 of those ballots a verdict was later readable: **20,520 Favor reported
spent, 884 votes asked, 205 recorded — and not one multi-vote ask registered.**
Every recorded vote was a first vote, at `costs[0] = 0`.

⚠ This is the shape `AGENTS.md` names as *"a setting you send is read back from
wherever it lands"*, and it is why the 961-resolution drought took twenty-nine
runs to see: the instrument that should have shouted was quietly agreeing with
the plan.

### 3. The hand-typed contested board was never the contested board

Three rounds in `docs/eval/` measured congress arms on
`--field live_target_diplomatic,live_target_culture`, and each records its world
as **`pangaea`/`flat`/fixed civilizations**:
`2026-08-18-the-screen-can-seat-the-rivals-firaxis-actually-plays`,
`2026-08-18-the-promotion-matrix-can-see-the-lanes-the-front-line-loses-`, and
`2026-08-18-the-vote-weight-and-the-ballot-agree-on-who-the-counter-oppo`
(which reuses the second's world explicitly: *"the identical run … same maps,
same field"*).

`deployment-contested`, the profile the promotion gate actually runs, has been
**`continents`/`planet`/`poles`/randomized** since #658 —
`matrix_child_args` hard-coded `--map`, `--shape`, `--poles` and
`--randomize-civs` *outside* the profile struct, where nothing could compare
them to anything. Eleven world flags typed by hand, agreeing with the gate on
eight.

⚠ **And a fourth round on the same field got all eleven right.**
`2026-08-19-the-suzerainty-prize-was-retired-on-a-board-with-diplomacy-s`
prints its command line in full and it is the gate's world exactly. That is the
finding, not an exoneration: whether a hand-typed contested round was on the
contested board was a property of the person typing, and nothing in the
repository could tell the two apart afterwards. There was no second expansion
of the profile for a test to disagree with.

### 4. The arm, on both boards

PLACEHOLDER_RESULTS

## What was decided

PLACEHOLDER_DECIDED
