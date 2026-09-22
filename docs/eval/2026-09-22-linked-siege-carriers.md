# Keep linked support carriers in siege forces

Native `civvis-20260922T061755Z` lost to Kongo's Culture victory at turn 155 on source `85744fb48b6ff59ecc27014f4ed20f2503bee143`. Its Bombard `5439513` held at `(41,26)` from turns 123 through 134 while the siege of Wente Mapu remained in Stage. It eventually fired on turns 136 and 137. The nearby Pike and Shot `5177368` and Siege Tower `5242911` both reported formation count two and shared `(37,23)` on turn 125.

A local diagnostic build of integrated source `7e9ca63e2b65c792aee13e9acfc303027185d6d6` printed public force assignments after each decision. All 464 complete replies matched the unmodified build. The Pike and Shot was absent from every force, while the main siege force included multiple reinforcements 8–17 tiles away. Source confirms the omission: `board_pool` excludes every linked unit, and `arm_of` classifies every linked unit as Other. The mirror restores reciprocal links from the observed formation counts and co-location; the existing movement engine recognizes the military member as the linked leader.

The candidate admits a land military carrier linked to friendly, co-located support into the objective pool and siege arm classification. The link must be reciprocal. The support follower remains outside the attack roster; civilian and religious escort commitments stay excluded. Existing movement and combat legality still govern orders and keep the support with its carrier. This changes assignment, not the combat or formation rules.

A separate siege-train ablation changed 203 of 464 internal-action frames but produced no earlier Bombard shots and reduced city-targeted attack orders from 36 to 33. It does not support disabling the whole siege policy. Nor does adding one excluded carrier by itself prove an earlier capture or a win: the nearby enemy strength and other reinforcements still matter.

## Validation in progress

Baseline regressions reproduced two failures: the linked infantry was omitted from the army and the depleted-city siege step returned no action. Both negative controls passed. With the candidate, all 38 siege tests pass, including moving both linked creation orders and capturing while retaining support. Full-suite checks, candidate replay, integration and native outcome validation remain pending. Diagnostic artifacts are retained under `/tmp/civvis-force-diagnostic` and `/tmp/civvis-3722-replay/force_diagnostic061755`; the deliberate ablation is separate from the candidate experiment.
