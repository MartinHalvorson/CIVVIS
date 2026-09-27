# Ordinary army approach spacing experiment

Experimental and default off. No live bundle change or difficulty promotion.
This is independent of the withheld city relief policy in PR #3791.

## Problem and failed-first evidence

The coordinated movement score applies role depth only within five hexes of
the objective. Crossing from six to five can acquire the entire penalty in
one step, outweighing progress even in a clear corridor. A real advancing
Archer at distance six stays outside the ring on the original controller,
although projected counter damage at its route step is zero. The movement
regression fails at `3e0268ae5` before the implementation; its log is preserved
outside the worktree under `prince-approach-spacing/`.

The opt-in `role-spacing-continuity` prices the role-depth term at its
five-hex value for every tile beyond five. Inside the ring the term remains
unchanged. This applies to Advance, Engage and Muster on the nonlegacy
controller. Hold, Recover and the legacy controller retain prior scoring.
The separate vanguard screen term retains its existing five-hex condition;
threat, movement risk, cohesion, recovery, attack eligibility and civilian
guard paths retain their code. The field/default append is at the p-r range
end, and the registry row is appended at the final tail to preserve every
existing gene bit. The policy is off in both controller constructors.

## Registered evaluation

Before implementation or outcomes, eight fresh seed pairs
`37914000`–`37914007` were registered at
`2026-09-27T01:18:08.156096+00:00`. One frozen source runs both legs, changing
only the focal policy; rival controllers remain fixed. The target is Gran
Colombia Domination, Tiny four-player Pangaea, Online, 250 turns, with all
victories enabled and both players and barbarians explicitly at Prince.

The parent is current main `a0ca7b2e0dbb01df3286b3115a82356165194c83` and its
21 compiled forced policies, including builders-work-through-raiders. This
differs from the archived 20-policy city relief experiments. Exact source,
binary and policy hashes, all local validation and the 202-frame native
decision replay must be frozen before starting the fresh pairs. Native replay
uses fixed observed future states and cannot prove counterfactual survival.

Keep every registered pair and every reported victory, turn, winner, focal
score, applied action, home capital, foreign capital and city retention metric.
Do not tune this candidate after inspecting these fresh outcomes. A six-game
standard single-gene screen may separately establish firing for the repository
gate; its random-seat Continents regime is not the focal Domination trial.
Green tests and simulator improvements do not establish native strength or
authorize promotion. Harmed or ineffective candidates remain withheld.

The raw registration, failed-first proof and subsequent immutable artifacts
are outside task worktrees at
`~/civvis-war-evidence-20260926/prince-approach-spacing/`. The following receipts record progress from that registration; fresh efficacy
outcomes remain pending at the frozen checkpoint below.

## Frozen candidate before fresh outcomes

Eligible source `c21156422fc078404a8472809a04515c44bd4534` passes4,362
Rust tests, zero failures and53 existing ignores. All five focused tests pass,
as do changed-line Rust quality, gene generation, the regenerated evaluation
manifest and14 append-point tests. An earlier equivalent nested condition
failed changed-line quality and was collapsed before the eligible source was
frozen; that failure receipt remains preserved.

The copied binaries are immutable. SHA256:

- `civvis_orders`: `43c0ffa14ca349037273e6955ee06a9cf1d536ad54ba2661c4d62ac30145d800`
- `victory_eval`: `65ac1fbdc1b704a946516f0b1de061a1c53588c8008952fb9a166b9c461b8a0a`
- `gene_screen`: `fd8b05c913cfcdeb4218f982b1ad719ad698fc58a2883ca0b07a6ad242770af0`

The compiled21-policy force file SHA256 is `439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`;
the complete names/file are retained with candidate-build.json.

Persistent native replay completes all202 recorded frames in both legs with
exit zero. Every full decision is identical between the off and on settings. Actual
decider genome readback confirms the flag off in the control and active/forced
in the enabled arm, so this is not an omitted flag.
This record supplies no behavioral contrast or repair of the observed city
loss. It does not disprove the isolated clear-corridor fixture, and it cannot
prove counterfactual retention. Both replay arms use the current parent-main21
policies; comparing them to archived20-policy binaries would change the regime.

Only after these receipts were frozen did the registered fresh trial start.
Four independent processes run two contiguous seeds each, preserving the
single eight-pair run's alternating leg order. Same source and policy bundle,
only the focal flag differs, rivals fixed. Each segment retains its command,
raw rows, log and completion receipt. Aggregate and inspect efficacy only when
all four processes finish. No source tuning after fresh outcome inspection.
The separate standard6-game firing probe37917000–37917005 completes on the
copied binary and its unmodified analysis is committed at
`docs/gene_screens/fires/role-spacing-continuity.json`. The firing gate,
gene-generation and manifest checks pass, as do17 firing tests and21 manifest
tests. No waiver or gate weakening. This random-seat Continents regime has
Prince majors/Deity barbarians; it is not the fixed-rival focal strength trial.
Policy remains
OFF and WITHHELD pending complete evidence; no native promotion.

## Complete eight-pair pilot: mixed, withheld

All four registered segments finish with exit zero; all eight pairs change
applied actions. Same source/focal-only comparison, current parent-main21
policies and rival controllers fixed. Focal wins stay2/8 in both legs;
Domination wins rise0 to1, and home capital losses rise0 to1. Foreign major
cities ever observed held sum stays19, while foreign capitals held at end rise
0 to4. A Domination gain and a new home capital loss coexist; this is not a
consistent war improvement and does not support deployment or promotion.

| Seed | Off ending / focal score | On ending / focal score | Major cities ever, off/on | Foreign capitals at end, off/on | Home capital held, off/on |
| --- | --- | --- | --- | --- | --- |
| 37914000 | score 250 loss / 883 | domination 231 win / 1935 | 8/12 | 0/3 | yes/yes |
| 37914001 | score 250 win / 1127 | science 243 win / 1210 | 0/0 | 0/0 | yes/yes |
| 37914002 | culture 202 loss / 582 | science 229 loss / 388 | 0/0 | 0/0 | yes/no |
| 37914003 | science 234 loss / 637 | science 209 loss / 522 | 0/1 | 0/0 | yes/yes |
| 37914004 | science 244 loss / 972 | culture 156 loss / 547 | 5/0 | 0/0 | yes/yes |
| 37914005 | score 250 win / 1225 | score 250 loss / 1214 | 3/6 | 0/1 | yes/yes |
| 37914006 | culture 215 loss / 602 | culture 160 loss / 452 | 1/0 | 0/0 | yes/yes |
| 37914007 | culture 229 loss / 714 | culture 187 loss / 412 | 2/0 | 0/0 | yes/yes |

Seed37914000 switches from a Score loss at250 without a retained foreign
capital to Domination win231, retaining all three foreign capitals. Its first
foreign capital is observed at176 versus191 off. Seed37914002 introduces the
home capital loss. Seed37914005 loses the control's Score win despite more
foreign cities and one foreign capital at the end. Seed37914001 changes the
control's Score win to Science, providing no Domination win. All raw rows,
full metrics and completion receipts remain retained; combined raw SHA256 is
`b732fea74ba5f696b110627bbb68a823a6a74032fc2452953f1969ff74a3d4c7`.

After reviewing this complete mixed pilot, independently register sixteen
fresh Prince pairs37918000–37918015 on the same frozen source and binaries.
No policy or source tuning. Both the potential Domination benefit and home
capital harm must be checked, with all victories enabled. Four even-sized
four-seed segments preserve balanced alternating leg order. Complete all
sixteen and inspect only after every segment terminates; retain unfavorable
arms. This replication is new validation, not part of the original preregistered
eight pairs. Native202-frame replay remains action-identical and supplies no
native strength evidence. OFF and WITHHELD; higher native difficulty remains
unproved.
