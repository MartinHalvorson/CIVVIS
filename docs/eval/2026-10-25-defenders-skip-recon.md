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
