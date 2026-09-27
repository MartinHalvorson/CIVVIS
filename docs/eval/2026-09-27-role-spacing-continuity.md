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

## Complete sixteen-pair replication: Domination gain does not repeat

All four replication segments complete with exit zero and all sixteen pairs
change applied actions. The source, copied binaries, current parent-main21
policies and rivals remain fixed. Focal wins rise2 to3, but Domination wins
fall1 to0. Home capital losses are1 in both legs. Major cities ever observed
held sum falls49 to29, and foreign capitals held at end fall5 to3. The policy
does not establish a consistent improvement in wars.

| Seed | Off ending / focal score | On ending / focal score | Major cities ever, off/on | Foreign capitals at end, off/on | Home capital held, off/on |
| --- | --- | --- | --- | --- | --- |
| 37918000 | culture 223 loss / 795 | religious 191 loss / 593 | 0/0 | 0/0 | yes/yes |
| 37918001 | science 225 loss / 861 | score 250 win / 1327 | 0/3 | 0/1 | yes/yes |
| 37918002 | score 250 loss / 1013 | score 250 win / 1975 | 3/18 | 0/1 | yes/yes |
| 37918003 | culture 163 loss / 527 | culture 159 loss / 532 | 0/0 | 0/0 | yes/yes |
| 37918004 | diplomatic 214 loss / 604 | culture 214 loss / 561 | 1/0 | 0/0 | yes/yes |
| 37918005 | domination 242 win / 2094 | score 250 win / 1162 | 19/2 | 3/0 | yes/yes |
| 37918006 | score 250 win / 1513 | science 244 loss / 1096 | 11/0 | 1/0 | yes/yes |
| 37918007 | science 200 loss / 550 | religious 167 loss / 558 | 0/0 | 0/0 | yes/yes |
| 37918008 | diplomatic 225 loss / 1005 | science 250 loss / 986 | 1/3 | 0/0 | yes/yes |
| 37918009 | score 250 loss / 792 | culture 189 loss / 531 | 0/0 | 0/0 | yes/yes |
| 37918010 | science 240 loss / 891 | score 250 loss / 940 | 3/0 | 0/0 | yes/yes |
| 37918011 | religious 166 loss / 154 | religious 161 loss / 154 | 0/0 | 0/0 | no/no |
| 37918012 | science 241 loss / 1244 | culture 196 loss / 817 | 4/0 | 0/0 | yes/yes |
| 37918013 | science 243 loss / 1147 | culture 196 loss / 614 | 7/0 | 1/0 | yes/yes |
| 37918014 | religious 213 loss / 1139 | science 237 loss / 1216 | 0/3 | 0/1 | yes/yes |
| 37918015 | science 230 loss / 758 | culture 180 loss / 591 | 0/0 | 0/0 | yes/yes |

Seed37918005 switches the control's Domination win242 with all three foreign
capitals to a Score win250 with none. Seed37918006 loses the control Score win
and foreign capital. Seeds37918001/2 gain Score wins, and37918014 gains one
foreign capital despite losing. Every arm and full metric is retained in the
replication folder; combined raw SHA256 is
`d4a6300fae404e56177f6232d4a68c9ed4508bbae28f5c921d6f5a625181a203`.

Across the original8 and separately registered16, wins are4/24 off versus5/24
on; Domination is1/24 each; home capital losses1 versus2; major cities ever
held sum68 versus48; foreign capitals held at end5 versus7. This descriptive
total does not treat24 as preregistered together, and it does not convert
Score/Science wins into Domination. The original single Domination gain is
matched by a lost Domination win in fresh replication. OFF and WITHHELD.
The score-boundary stall remains reproduced, but this repair has not solved
the broader war outcome defect. Native strength and higher-level promotion
remain unproved; no policy/source tuning follows these outcomes.

## Read-only diagnosis of the pilot benefit and capital harm

An observer-only harness compiled against the immutable candidate library
replays37914000 and37914002. It reproduces every originally reported field
and action count. Original pilot full action sequences were not retained;
these comparisons are field/count proof, not sequence identity with the pilot.

On37914000, control captures Hanoi observed191, then loses it to Free Cities
observed239: HP200, Loyalty7.14 before the flip, no target attack in that
interval and last attacked190. Enabled captures Hanoi176, Gyeongju207 and
Aachen231 and retains all three through its Domination ending. Its first
applied difference is a Warrior155 move at index5255, observed60, with both
snapshots HP100. That early difference alone does not establish the entire
later campaign's cause.

On37914002, the enabled home capital Bogota changes0 to3 observed196, after
ranged unit775 and melee unit85 attack at195. Loyalty is100 before capture,
50 after; HP59 to100 and walls37 to0. This is military capture, not Loyalty
rebellion. The first applied difference is Horseman410 fortifying versus
moving at index15749, observed114; both snapshots are HP100. That event alone
does not prove the later capital loss mechanism or identify a safe fix.

All snapshot/action records survive, and thought-ID continuity has zero gaps
in the four traces. The positive enabled trace reports11 civilization turns
truncated by the reasoning journal's per-turn budget; its reasoning content
is therefore incomplete despite contiguous IDs. The other three report zero
truncated turns. Cumulative dropped/truncated counters are not summed across
snapshots. Owner/HP/actions and ending fields come from full state/action
records, independent of those reasoning limits. Full raw traces, summaries,
field comparisons, first differences and harness/library hashes remain under
`prince-approach-spacing/causal-37914000/` and `causal-37914002/`.
