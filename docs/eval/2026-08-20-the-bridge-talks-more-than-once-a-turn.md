# The bridge talks more than once a turn, and a unit that sees something new thinks again

_2026-08-20 · `agent/mbp-m5-pro-64/claude-fable-frames` · PR #2198_

## What was asked

The operator, 2026-08-20: communicate back and forth between CIVVIS and the
Firaxis seat more times per turn; use all the information available; let a
unit make a single move and use what it revealed to make its next moves —
as new genes, optimized to be clearly positive.

## What was found before anything was built

- **One exchange per turn, and the map every 25.** The board went out at
  turn start and the orders came back once; the strike-only combat frame
  (#2132) was the one exception and was off, and the climb never forwarded
  its flag, so no ladder run ever played it. The map itself crossed only with
  the `tiles` sweep — turn 1, then every `TileExportEvery` turns (25 by
  default, 4 on the ladder). Ground a unit uncovered was planned on turns
  later; a scout ordered three hexes into the fog walked all three whatever
  its first step showed, because `coalesce_unit_paths` sends the walk as one
  `MOVE_TO` to its furthest hex.
- **Natively the engine already steps sighted — except for two things.** Every
  evaluator and the live decider run units serially
  (`advance_unit_serial`), re-deciding each step on a board that the previous
  step has already revealed into. What stays frozen is the force-group
  assignment (`force_groups_dirty` is set only by an attack: "movement cannot
  change the opposing force" — but a step that reveals one does) and, on the
  parallel CLI path only (`civvis --jobs`, the sole `WorkPool` installer), the
  blind eight-step batch plan replayed by `apply_unit_intents`.
- **The ingestion side was already cumulative.** `Snapshot::from_chunks`
  merges every `tiles` chunk ever sent, later plots winning, so a delta chunk
  needs no new reader — but it must not advance the snapshot's sweep turn,
  or the `improved` fold (events at or after the newest sweep) drops every
  improvement finished since the real one.

## What was built

`docs/LIVE_TACTICS.md` §11: the tiles delta (`CivvisTiles`, every turn and
frame), replan frames (`CivvisFrames`, `ReplanFrames` 2, opened by revealed
ground with movement left or by a strike), the brain's cut of a pure walk at
its first unrevealed hex when the seat advertises `replan_frames`, and the
gene `step-and-reassess` (live treatment + native repair): serially a step
that brings a hostile into view re-forms the force groups; on the parallel
path a blind plan stops at the step that revealed new ground.

## How it was measured

| what | instrument | result |
|---|---|---|
| cost of a frame on the brain side | `civvis_orders --mirror <run> --turn N` on the 20 MB, 241-turn journal of `live-head-rome-religious-actions-20260802T173404Z` | 0.26 s wall at turn 120, 0.81 s cold at turn 60 — whole-journal re-reads included; the host's export is what a frame costs |
| frame poll budget | `await.polls` before each `orders` on the same run | median = max = **1** of 40; the 20-poll frame budget is 20× the observed need |
| does the native gene fire | four 4p 60×38 150-turn games, repair bundle, all seats (`step_and_reassess_fires_check`, ignored test) | sightings that re-formed the groups: 288, 270, 401, 407 per game; blind cuts 0 (no pool, as expected); wall 5.5–6.1 s per game with the gene, 5.5–8.1 s without |
| first cut of the gene (parallel leg only) | `gene_screen --genes step-and-reassess`, 4p all-six, seeds 50M | **+0.0 [+0.0, +0.0]** over 204 pairs — every pair byte-identical: the gene never fired in the screen's regime. Recorded in `docs/GENE_SCREEN.md` as the inert-gene signature |
| the gene, serial leg, all six lanes | same design, 1,000 pairs (2,000 games), seeds 50000000.. | S2_RESULT |
| the gene, serial leg, war regime | `--victories domination,score`, 800 pairs (1,600 games), seeds 51000000.. | S3_RESULT |

## What it means

S_MEANING

## The second directive, the same day: the defaults are the best genome

While the screens ran the operator added: *let the defaults for the genes
reflect our best genome — only genes that provably help; unhelpful genes can
default off — so our verification games use our best genome; keep testing
and improving the less helpful genes; in the large batches a helpful gene
may be on in 90% of tests and we still compare the 90% with the 10%.*
`docs/GENE_SCREEN.md` (*The gene ledger*, *Prior-weighted screens*) records
the mechanism; the numbers:

| what | instrument | result |
|---|---|---|
| the first ledger | `tools/gene_ledger.py` over the 6p native screen (13,446 seat-pairs), the 4p war screen (3,300) and the repaired genes' war re-screen (1,064) | **10 help (on) · 11 hurt (off) · 44 unresolved (off)**; the live bundle plays the ten plus the host-only flags |
| what the ledger bought | `gene_screen --baseline best --anchor-pairs 250` — the all-on universe against the best genome, same maps, 4p all-six, seeds 52M | S4_RESULT |
| the first gene priced into the ledger | s2 above | `step-and-reassess` unresolved at +0.2 [−1.2, +1.6]: off until a screen says otherwise |

The phase-1 anchors had already said the all-on bundle was not the best
genome (7.5% wins against 27% for all-off over 200 pairs); the ledger makes
the choice per gene and from the measurements, and makes a new treatment
ship into the universe rather than into the deployment.

## Not measured

The live seat itself: this machine carries a standing operator game hold
(`gamelock.py --status`, since 2026-08-02), so no live game ran here. The
first ladder runs after merge play `--replan-frames 2` and the tiles delta by
default; §11 lists what to read per turn (`replan_frame`/`combat_frame`
against `combat_frame_timeout`, `tiles_delta.plots`, `frontier_cuts` and
re-orders on `orders.frame ≥ 1`, turn wall time against `--replan-frames 0`).
The arm that withholds everything at once:
`--replan-frames 0 --no-tile-delta --civvis-without step-and-reassess`.
