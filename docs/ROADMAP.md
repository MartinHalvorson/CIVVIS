# Roadmap

Where the project actually is, and what it is doing next. History below is
kept for orientation; the current-state section is the part to trust, and
`docs/AI_GAPS.md` is the always-current assessment of the AI specifically.

For the standing question this roadmap exists to close — how far CIVVIS is from
Civilization VI, and why measured strength here has not transferred there —
[`docs/THE_GAP.md`](THE_GAP.md) is the synthesis across `FIDELITY.md`,
`GROUNDING.md`, `AI_GAPS.md` and the ladder record. It owns no findings of its
own; each one stays with the document that measured it.

The current evaluation inventory and live-bridge counts are generated in
[`docs/EVAL_STATUS.md`](EVAL_STATUS.md); do not duplicate those numbers in
prose. Refresh it with `python3 tools/eval_manifest.py --write`.

## Where the project is (2026-08-17)

Everything the old roadmap called planned has shipped and then some:

- **The engine** is a single pure-Rust crate (serde-only deps) at full rules
  depth — religion, governors, ages, World Congress, aircraft, alliances,
  unique units, the lot. The rules-completion pass closed 2026-07; remaining
  engine work is fidelity against real Civilization VI (`docs/FIDELITY.md`),
  not activation of dormant systems.
- **civvis.ai is live**: the WebAssembly client shipped, with a `/test` lane
  redeployed from head half-hourly, a stable front page moved by operator
  judgment (`docs/SPECTATOR_DEPLOY.md`), native/wasm build-parity gates, and
  a home page selling two products — full-game simulations and Tactics
  battles (historical scenarios on real terrain, an era rolled per battle).
- **The AI is scripted and measured**: `AdvancedAi`, whose behaviours are
  boolean genes in one registry (`src/ai/advanced/genes.rs`), priced by the
  random-genome gene screen (`docs/GENE_SCREEN.md`) and shipped by the gene
  ledger's default rule. The Glicko-2 selection league, the paired evaluator
  and the `civvis arena` Elo batches are retired (#2351, #2357,
  `docs/closed/LEAGUE.md`); a rating system for *finished* genomes is planned
  to return once the screen has settled the gene set. No learned policy
  ships; search wins offline but is not live-eligible. `docs/AI_GAPS.md`
  ranks the gaps.
- **The live bridge plays real Civilization VI**: a Lua control mod + macOS
  harness drives full Settler-difficulty games end to end, self-records every
  attempt on the difficulty ladder (`docs/CIV6_LADDER.md`), and carries its
  bridge health (orders-applied rate, ~97%) on the ledger. **Rung 1, Settler,
  was claimed on 2026-08-16** by run `civvis-20260816T054344Z` — a victory
  event naming our own team at turn 251 of a configured 250-turn game, score
  1021. A second Settler win followed the same day (`civvis-20260816T223457Z`,
  1121 against a best rival's 1031). Winning is not yet reliable; the current
  attempt, terminal, and win counts are generated in `docs/EVAL_STATUS.md`,
  rather than repeated here. This is the project's front line.

## Active objectives (ranked 2026-08-17)

1. **Make Settler repeatable, then take Chieftain.** The rung is claimed; two
   wins in 119 attempts is a result, not a capability. The installed supervisor
   now runs a three-game pinned batch by default and asks the read-only
   `tools/civ6_ladder_policy.py` gate which rung to target. The rule
   (operator, 2026-08-23): **play the highest rung the controller has claimed
   until it has three configured wins there, then move up** — wins over the
   whole fleet record, no trailing window, losses are not evidence against a
   rung. Settler (16 wins) and Chieftain (2) are claimed, Warlord was won on
   2026-08-22 on the other seat and waits on `civ6_ladder.py publish` to reach
   the record; the seat plays Warlord once it does.
2. **Close the actuation gap.** ✅ The bridge now carries a host-timed
   `produce_next` lease instead of letting the built-in ladder answer a queue
   completion unseen by CIVVIS; the lease is preserved across slow frames and
   its consume/expiry counts are recorded. Optional envoy spending now has a
   next-frame host readback (`envoy_reconcile`) rather than treating an issued
   request as proof. Batch runs still measure the resulting applied-rate and
   ladder-share change before the next objective is reprioritized.
3. **Price every gene, and only through the screen.** ✅ (re-cut 2026-08-23)
   The paired evaluator and its 228 arms — one flag per arm, priced against a
   fixed background — were retired in favour of the one random-genome
   instrument: `gene_screen` draws every seat's genome independently and
   reads each gene as seats-on against seats-off (`docs/GENE_SCREEN.md`;
   `HEURISTIC_GENE_RANKING.md` is the ledger). What that instrument cannot
   see is now the objective: the host-only treatments only the live seat can
   price (`civvis_orders --without` over ladder games), and a contested board
   that produces the diplomatic and culture endings the live seat loses to —
   which no screen plays yet (`docs/eval/2026-08-23-the-arms-that-were-pre-registered-and-never-run.md`).
4. **A tactics-grade controller for the arena.** ✅ The existing bounded
   portfolio search now auto-activates for promoted `AdvancedAi` on the 20×20
   Battlefield, is measured on the skirmish benchmark, and leaves native-world
   and frozen-anchor identities unchanged.
5. **Relieve the measured conflict hotspots**, which are not the three this
   objective used to name. Measured over the 200 merges preceding 2026-08-18
   (`tools/conflict_hotspots.py`; CI checks the targets below are real).

   ⚠ Two targets were dropped on 2026-08-25 because the measurement moved out
   from under them and the check was failing on its own floor: `src/elo.rs`,
   which no longer exists in the tree at all, and
   `tools/civ6_control/mod/CivvisControlAgent.lua` at 4% — under the 5% floor,
   where the tool's own words are that "splitting a file nobody edits costs a
   large diff and buys nothing". What the last 200 merges actually contend over
   is the gene registry and its toggle table (`src/ai/advanced/genes.rs` 30%,
   `src/ai/advanced/treatment_flags.rs` 28%, `tools/test_genes.py` 18%), and
   those are answered by the per-letter append markers rather than by a split.
   Naming them as new split targets is a decision for whoever owns this
   objective; this edit only removes the two the check refuses.


   | file | merges touching it | why it is contended |
   |---|---:|---|
   | `src/ai/advanced.rs` | 23% | size — one 23.3k-line impl block |
   | `src/game.rs` | 17% | size |
   | `src/ai/advanced/tests.rs` | 24% | size — 31.7k lines, cut out of `advanced.rs` by #1918 and now longer than it |
   | `src/ai.rs` | 10% | size |
   | `src/bin/civvis_orders.rs` | 10% | one shared list: the `--without` treatments |

   ⚠ The Lua row was invisible until 2026-08-18. `conflict_hotspots.py` ranked
   `(rs|js|py|sh)` only, so the fifth-most-contended file in the repository
   could not appear in the ranking however contended it became — and an absent
   file prints exactly like an uncontended one. The tool now ranks every
   hand-written source suffix, and a test pins that rather than the rank.

   The old list was built from file size, and size is not the tax. `elo.rs` is
   a seventh of `game.rs`'s length and is contended more often. And
   `web/assets/app.js` — the third-largest file in the repository, and one of
   the three this objective used to name — is touched by **one merge in fifty**;
   it is off the list, because splitting a file nobody edits costs a large diff
   and buys nothing.

   **Two problems, two remedies.** Splitting along seams answers the ones
   contended for their size — `advanced.rs`, then `game.rs`. It does nothing
   for `main.rs`, `elo.rs` and `civvis_orders.rs`, where every treatment PR
   appends to *one shared line or list*: two such PRs conflict whatever the
   file's length, and the fix is to move that data out of source, the way
   `docs/eval/` did it for `docs/EVAL.md`'s single append point.

   **Which problem each file has, measured (2026-08-23).** The table above is
   touch rate, and touch rate scores a file contended for its size and a file
   contended for one shared list identically. `conflict_hotspots.py --modes`
   separates them: for each file it takes every pair of *consecutive* merges
   that touch it, undoes the earlier one and merges the later one in with
   git's own three-way merge, then splits the conflicts into **two pull
   requests appending to one shared list** (both sides only inserted lines, at
   a place where collisions repeat) and **two pull requests editing the same
   code**. Over the 200 merges ending at `2c570f4f`:

   ```
   file                                touch  collide  anchored  verdict
   src/ai/advanced.rs                    24%     8/47      8/16  BOTH
   src/ai/advanced/tests.rs              24%    10/46      0/10  SPREAD
   src/ai/advanced/treatment_flags.rs    22%     7/42       0/7  SPREAD
   src/ai/advanced/treatments.rs         19%    10/37     10/10  ANCHOR
   src/ai.rs                             12%     7/24      2/21  SPREAD
   src/elo.rs                            11%     4/21     15/18  ANCHOR
   src/game.rs                           11%     7/21      2/25  SPREAD
   web/assets/app.js                     10%     1/20       0/3  SPREAD
   ```

   ⚠⚠ **Half of the 2026-08-18 relocation worked and half did not, and only
   this reading can tell which.** `treatment_flags.rs` and `treatments.rs`
   came out of the same effort. The 182 toggles now collide at 182 *different*
   places, which is no anchor at all; the two tables still collide at exactly
   two lines. Moving a list to another file relieves it only if the appends
   stop landing on one line — for the toggles they did, for the tables they
   did not. `tests.rs`, second in the ranking and never on this table before,
   is pure size (0 of 10): splitting is its remedy and moving data is not. And
   `advanced.rs`, recorded below as having had its shared-anchor half done,
   still holds the two largest single anchors in the repository — the flag
   field on `pub struct AdvancedAi` (3 pairs) and its `flag: false,` twin in
   `fn configured` (5), which no list of hotspots had ever named.

   **What the anchor relocation cost, and what replaced it (2026-08-23).**
   #2022 and #2029 moved two anchors out of `advanced.rs` and bought two new
   hotspots at 22% and 19%; `advanced.rs` did not fall. So the second attempt
   does not move anything. Each anchor a treatment pull request appends to —
   the struct field, the `configured` initialiser, the `enable_*`/`disable_*`
   pair, and both tables — now carries a run of markers, one per range of
   first letters, and a treatment is filed under the range its own name falls
   in. Git's merge conflicts only when two insertions land on the same line,
   so two treatments whose names start in different ranges no longer collide
   anywhere. On the 156 existing names the eight ranges hold
   19/20/23/17/24/18/23/12, so two new treatments share a range **13% of the
   time — 7.7x fewer collisions, dividing the rate rather than removing it.**
   Cost: 122 added comment lines, no row re-ordered, no line deleted, no
   consumer touched. `tools/test_treatment_append_points.py` builds two
   synthetic treatment pull requests and *merges* them rather than asserting
   that they would, and pins the same-range case as a control so the suite
   cannot pass by testing nothing.

   ⚠ **Why nothing was re-ordered, and why a data file is not the answer.**
   `gene_screen`'s `draw_genome` walks `ENGINE_REPAIR_TREATMENTS ++
   PRODUCTION_TREATMENTS ++ PRODUCTION_OPT_INS` **positionally**, taking the
   i-th value of one seeded stream for the i-th gene, so re-ordering those
   tables re-assigns every gene's drawn bit. That is why every row has always
   been appended at the very end — the only edit that leaves the existing
   genes' bits alone. A JSON or TOML table would relocate the anchor a third
   time for the reason `treatments.rs` already demonstrates: two pull requests
   appending a row to one file conflict wherever that file lives, and sorting
   it only narrows the collision to neighbouring names.

   **Still open, in the order the measurement ranks them:** removing the
   `fn configured` anchor outright rather than dividing it — its 143
   `flag: false,` lines are all the derived default, but `impl Default for
   AdvancedAi` already means `Self::new()`, so the derive is a semantic change
   to a public trait impl and 83 of those lines are less than a week old;
   `src/elo.rs`'s four registries and two `ArmKind` match tables (ANCHOR, 15
   of 18); and the size half of `advanced.rs` and `tests.rs`, 31.7k lines
   each, which is the only one of these that splitting answers.

   **`advanced.rs` had both problems, and its shared-anchor half is done
   (2026-08-18).** It is the most contended file in the repository *and* the
   one every live-treatment PR appends to. Both of its append anchors have now left it:
   the `LIVE_TREATMENTS` table to `advanced/treatments.rs` (#2022), and the 182
   `enable_*`/`disable_*` toggles — the anchor whose collisions had already
   swallowed a function's closing brace twice — to
   `advanced/treatment_flags.rs`. A guard in the new file fails when a toggle
   is defined in `advanced.rs` again, so the move does not quietly reverse.
   The size half is untouched: `advanced.rs` is still 27k lines and still
   first on the table above.

   ⚠ Touch rate is exposure, not pain — two PRs editing distant parts of one
   file do not collide. Real conflict counts are not recoverable from `main`,
   because a squash merge records the resolution and never the collision. The
   ranking is what the available evidence supports; a conflict count is not.
6. **Delete measured-null code.** ✅ The 2026-08-17 cleanup removes the
   confirmed-null `bounded_recovery` and `envoy_infrastructure` arms from
   production while retaining explicit evaluator/live-bridge controls and
   their negative records. The remaining off-flags and netless experiment arms
   are still queued for their own evidence-backed cleanup.
7. **wasm/native viewer parity.** Panels that read native-only state are
   silently dead on civvis.ai; implement or hide, and gate the contract.
8. **Headless empire actuation repairs** (housing/loyalty cards, eureka
   asks) — screen first, then gate at the deployment shape.
9. **Drain the stranded-work queue** (`tools/stranded_work_report.py`).
10. **Keep the paper trail true** — this file, retired docs to
    `docs/closed/`, generated ledgers current.

The measurement doctrine behind that ordering, in one line each: actuation
repairs pay and valuation tunes do not; a composite gate licenses the
composite, never its parts; gate on the deployment shape; one seed is never
a result; `audit` detects defects but does not estimate value.

## History (shipped)

### v0.1 — headless engine
Hex map + mapgen, cities/growth/borders, districts with adjacency,
buildings/improvements, tech + civic trees, melee/ranged/city combat,
war/peace, three victory types, fog of war, JSON saves, gym-style env,
scripted AIs, CLI, tests.

### v0.2 — soak
City-states (pre-founded defensive minors, conquerable, excluded from
victory); `soak` playing many full AI games across seeds with anomaly flags.

### v0.3 — Rust performance core
Ported the Python engine to Rust: ~16x single-core (36k vs 2.3k turns/sec),
parallel across cores. (The once-planned PyO3 bindings eventually became
unnecessary: the Python engine was removed instead.)

### v0.4 — rules depth + browser GUI
Housing/amenities, eurekas & inspirations, unit XP/fortify, city strikes,
barbarians, governments, medieval/renaissance content, and `civvis play` —
a zero-dep local web GUI over the JSON action protocol.

### v0.5 — content and systems breadth
Religion, Great People, trade routes, envoys, expanded diplomacy, per-civ
uniques, era score; ruleset data pass.

### v0.6 — pure Rust, rules completion
Python reference implementation removed (2026-07-21); the crate moved to the
repo root with GUI server, observation builder, and Elo harness all in Rust.
The completion pass activated every deferred tactical and world system:
pillaging/repairs, coastal raids, cliffs, aircraft, named Great People and
Governors, belief categories and Apostle promotions, Ages and Dedications,
Quick Deals, grievances, formal wars, alliances, Diplomatic Favor, World
Congress, conquest decisions.

### The browser client (shipped)
What an earlier revision of this file scoped as "planned" is the live site:
a `wasm32-unknown-unknown` build of the engine behind a Worker shim
(`beta/shim.js`), deterministic native/wasm parity checks in CI
(`docs/FLOAT_DETERMINISM.md`), immutable content-hashed static artifacts,
and Cloudflare Pages serving `/` (stable tag) and `/test` (head).
`docs/SPECTATOR_DEPLOY.md` owns the deploy contract. The acceptance gates
that section demanded — parity across seeds, green-only deployment, explicit
lane provenance (`build.json`) — are the shipped `published-build` +
`to-test-auto-30` machinery.

### The live Civilization VI bridge (shipped, climbing)
`tools/civ6_control` (a Lua mod + macOS input/vision harness) configures and
plays real Civ 6 games unattended: `docs/CIV6_COMPUTER_CONTROL.md` is the
contract, `docs/CIV6_LADDER.md` the record, and a supervisor loop plays one
game per fresh build of head.
