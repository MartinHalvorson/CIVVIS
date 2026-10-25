# Buy an affordable field defender

The current native Immortal Gran Colombia game `civvis-20261025T161417Z`
loses to Culture at turn 183 after 24 Gold Scout purchases and 21 distinct
Scout combat deaths. Those are native orders, receipts and unit identities,
not inferred captures or a policy comparison. Preserve the original archive
before further interpretation. The earlier three-game upkeep census remains
separate and does not explain this spending.

At the first frame of turn 89 the treasury has 245 Gold, seven cities, two
visible nearby barbarians including a Man-at-Arms, and a capital menu offering
Scout60/Archer120/Spearman125. The baseline soldier buyer prices units at
`spec.cost * 4`, instead of the actual purchase quote, and admits recon.
At its wartime 110-Gold floor that admits the Scout while pricing both real
field defenders out. The actual gold-purchase path can be reached through
`BasicAi::cities -> spend_gold -> buy_gold_military` in the delegated Advanced
production lane, or through its final direct baseline spending pass. Confirm
the observed caller using instrumented native-state diagnostics before making
an attribution claim about all 24 purchases.

## Behavior and validation registered before implementation

Use `Game::unit_purchase_cost` for each city/unit's actual Gold quote and
exclude promotion class recon from this defender buyer. Keep existing role
preference, strength ranking, budget reserve, host refusals, production checks,
tie ordering, ordinary exploration production and separate Scout purchases.
A rejected desired role may still fall back to a legal field defender.

First run ten new actual-purchase regression cases against unchanged purchase
code. Cases cover the quoted turn-89 affordability window, Scout-only menus,
ranged role, an expensive native quote that must preserve the floor, no money,
host refusal, unoffered ranged fallback, per-city price ties, ordinary engine
quotes and a separate legitimate Scout purchase. Genuine assertion failures
are RED; retain fixture/compiler failures separately. Use a nonoptimized dev
build with debug symbols disabled for rapid arithmetic/control-flow regression,
then repeat GREEN in the same profile and run required full CI-profile tests.

After valid RED, change only the buyer's price query and recon eligibility.
Add an actual wartime `spend_gold` integration case if the direct helper cases
reproduce as expected. Retain all outcomes and logs. Check source overlap,
changed-line quality and complete required GitHub gates, then ship and verify
the merged revision. This AI-policy correction changes no engine rule.

For simulator compatibility register complete same-seed paired games before
running: Gran Colombia Domination, Immortal and Deity, Online, 4 major Tiny
Pangaea maps, six minors, all victories, natural 250-turn cap; seeds 59010625
and59010626 at Immortal and59010627 and59010628 at Deity. Use identical
controller settings and all reported action traces for the baseline and
candidate. These four development maps are not a native trial or a claim of
improved win rate. Native execution and sustained competitive outcomes remain
required for the broader goal.

## Source overlap and retained context

The actual published AI diffs of #4081 and #3793 concern capital-landmass
traversal and exploration, thousands of lines away from this buyer. #3848's
current change is elsewhere, and #3845 has no current AI-file diff. The new
test-module declaration sits before the legacy tests, away from #4081's
end-of-file declaration. No same-line rewrite is claimed or inferred from
sharing the large AI file.

The companion native-context archive is a byte-identical copy of the original
post-hoc diagnostic. It retains all24 purchase decisions with full native
Buy actions,26 production/purchase receipts,21 distinct Scout deaths, the
first native state for each purchase turn, the terminal summary and original
file digests. It does not turn a receipt into proof of tactical usefulness.

## Frozen comparison harness

Before any game is run, fix the independent comparison source at the committed
baseline `6ddd88904`, retained in the private detached build tree
`civvis-defenders-control-3c67`. Its library is compiled with the ordinary CI
profile, without changing the development worktree. The registered harness
copies the existing fixed-profile Domination evaluator; it captures complete
initial/terminal boards, canonical actions and the drained focal journal.
It runs exactly one full game per invocation with safest-stand held ON in
both builds. There is no off/on safest-stand leg, extra adaptive treatment,
replacement map or shortened clock. The buyer correction affects adaptive
rivals too, so any two-build contrast is not isolated focal-seat strength.

The four immutable assignments remain Immortal59010625/26 and
Deity59010627/28. Both profiles retain250 natural turns,4 majors,60×38
Pangaea,6 minors,Online,all victories and the existing focal handicap exemption.
The correction does not acquire a native strength claim from these games.

The first harness preparation ran from the control directory and therefore
wrote only its new diagnostic files there. Before any game, those exact files
move to the owned feature tree, the registration's candidate path is corrected,
and the control's documentation returns byte-for-byte to its committed text.
No AI, rules, Cargo or evaluator source in the control is changed. The retained
preparation receipt makes the correction explicit; the control build continues.

## First baseline fixture failure

The first baseline completes compilation, then all10 cases stop during city
setup: axial(18,12) lies outside a24-wide map. The original source, diagnostics
and source digests remain in the fixture-error archive. No policy assertion
is reached, so this is not RED. Only the rival coordinate changes to(14,12),
with explicit map membership, before rerunning all ten cases on unchanged
purchase code. The strict controller rejects the invalid attempt and changes
no production code.

A separate post-hoc all-day census includes83 completed-summary run segments
at its fixed start cutoff,39 with a requested Gold Scout purchase and112 such
requests in total. Continuations are separate segments, not independent games,
and requests are not executions. Its complete included-path list and outcomes
are retained; the original Immortal native execution witness remains separate.

## Valid regression result and implementation

After only the fixture coordinate is corrected, the complete unchanged-code
suite reports4 passes and exactly the6 registered logical failures. Every
failure reaches a purchase, chosen unit/city, treasury floor or legal fallback
assertion. All four controls pass, including separate legitimate Scout buying.
The complete genuine RED output and source hashes are retained in`red.json.gz`.

The implementation now excludes recon from the defender shortlist and obtains
each candidate's Gold price from`unit_purchase_cost`, using the same query as
actual purchase execution. Existing build eligibility, purchase refusals,
role preference and deterministic strength/price/city ties remain. A real
wartime`spend_gold` integration test is added after the valid direct regressions.
The corrected11-case suite and complete CI-profile suite remain required;
results are not inferred from the fact that a runner was launched.

After integration, passively retain the first8 newly launched native attempt
stems starting with confirmed delivery of the merged correction. Failed starts,
continuations, source changes and missing outcomes remain visible; no attempt
is replaced and no game is interrupted. Read back actual compiled source,
Gold unit quotes/orders, veteran upgrades, city captures, economies and native
terminal outcomes. This observational delivery cohort does not estimate a
paired causal win-rate gain, and does not take the active screen lease or
change the host's difficulty, lane, map, force bundle, pin or setup.

## Full hosted failure and compatibility repair

The first full hosted candidate runs6,281 tests:6,280 pass, and the frozen
`advanced_v1` stream fails with19,250 decisions against its unchanged19,252
anchor. This correction is for current controllers. Add a private BasicAi
compatibility bit, false in both ordinary constructors and true only in
`AdvancedAi::legacy`, restoring the exact pre-repair recon/price/predicate path
there. Current BasicAi, stock AdvancedAi, targeted/native controllers retain
the complete quote/recon repair. Do not change the historical expected stream
or its fingerprint. Re-run all eleven actual-action tests and the existing
full five-profile anchor, then the complete CI-profile suite. The interrupted
local duplicate and complete hosted logs remain explicit, not GREEN.

The standalone harness's copied cfg(test) module is unreachable in the actual
probe but makes rustfmt look for a test file at its new location. Remove that
unused declaration and format only the owned harness. Before candidate
execution, run an additional same-map baseline instrumentation control and
require exact initial/terminal boards, canonical actions, outcomes and focal
journal equality with the already completed baseline59010625. Both executions
remain retained; it is a diagnostic repetition, not another map or a replacement.

The compatibility-scoped correction again passes all 11 actual-action tests.
The existing historical anchor and full CI-profile tests remain required.
Whole-file rustfmt checks also show pre-existing formatting suggestions in
unmodified Advanced sections and a child module; those lines are unchanged.
The owned standalone harness and purchase test file pass exact rustfmt checks.
The changed-line quality gate remains required before integration.
