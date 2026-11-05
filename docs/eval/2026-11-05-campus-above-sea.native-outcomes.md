# Native Campus placement and bounded outcomes

All four predeclared cold continuations completed in OFF/ON/ON/OFF order on the same preserved native turn-4 save. Initial full mechanical states were equal. Actual loaded city identity and seat, native argv, whole configuration, effective genome, installed Lua reconstruction, source and binary hashes were checked. Only the Campus opt-in differs between the planned profiles. No arm was replaced after its outcome, and no operator retirement was observed.

| Role | End turn | Native result | Decision receipts | Registered reads | Retargets matched to a planned action | Techs at150: ours/rival |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| 01-off | 153 | Loss: Culture | 387 | 6728 | 0 | 54/70 |
| 02-on | 164 | Loss: Technology | 430 | 7470 | 0 | 57/70 |
| 03-on | 166 | Loss: Culture | 433 | 7523 | 1 | 64/66 |
| 04-off | 156 | Loss: Culture | 413 | 7173 | 0 | 58/64 |

These are four retained losses, not evidence of a win-rate gain. The first treatment was nonfiring; the repeat treatment contained one verified direct Campus retarget. The complete bounded summary is in `2026-11-05-campus-above-sea.native-outcomes.json`.

## Native actuation

At turn 117/frame 0 in the repeat treatment, Lampung (native city 458758) proposed a Campus at (10,33), native lowland band 2. The emitted direct production order selected (8,30), native nonlowland -1. All three actual input frontiers (367 once, 368 once, 369 sixteen times: eighteen reads) agree that both sites were host-offered, the selected site was unflooded and unsubmerged, and no Campus already existed.

The actual host exported a new foundation at (8,30) on 117/frame 1 and a completed Campus on 130/frame 0. Library, University and Research Lab first appear at 133, 139 and 150. The final 166/frame 2 export retains the completed, unpillaged Campus and all three unpillaged buildings. No other Campus site appears in that city's later states.

The self-contained archive `2026-11-05-campus-above-sea.native-actuation.tar.xz` is 437936 bytes, SHA-256 `084f9d0be9fba7440bbd0bdc1ffab6578a0757fef4d4ac6bc6f8511d330e00bc`. It contains the original hashed decision receipt, all eighteen actual read frontiers deduplicated losslessly as one base plus append chunks, observed runtime context, preflight, binary binding, independent audit, and six full host-state witnesses. Run `python3 docs/eval/2026-11-05-campus-above-sea.native-actuation.verify.py docs/eval/2026-11-05-campus-above-sea.native-actuation.tar.xz` to verify it.

Neither the planned nor selected site has a flood/submergence observation between 117 and 166. This establishes placement, completion and retained science buildings; it establishes no avoided flood, saved production, improved yield or strength benefit.

## Scope and comparison limits

The native pilot used frozen source `d66b7cf2dd20abb7fea2e9ebc5de58a0e25b6118`, native executable SHA-256 `27d98df06b4b6193f93d6dee6f62a38a292e95007f939a79515ccfdf9400751b`, and preserved save SHA-256 `bd74dabc8ec8a8d9e49fb0a1785c251504492d2ea973457881816edbc44976f4`. Later branch merges introduce other background source changes. The current candidate searches all legal host-constrained district sites rather than the ordinary two-site shortlist, so a safe third offer is not hidden by two higher-yield lowlands. It also adds an integration change: when both `campus-above-the-sea` and `nusantara-reads-the-coast` are enabled for Indonesia, it uses the shared coast-adjacency ranking on a private analysis board, including on a fresh native mirror. With Nusantara off it retains the ordinary base-yield ranking. The pilot did not execute this combined-opt-in change, the later whole-source revisions, or the separate production-hint and board-publication repairs. Its native actuation evidence applies to its frozen source; it is not native validation of the combined opt-ins.

This is a cold continuation, not reproduction of the original controller's private history. Although initial states match, actual captured inputs already differ at 5/frame 0: the baseline's reads end after its new state, while the other arm includes the following terrain delta. Movement orders differ before any Campus order, so the later outcomes are not deterministic Campus-only counterfactuals. That race is separately reproduced and repaired in PR4616; the frozen pilot was not changed.

The earlier v1 matrix is retained as an instrumentation failure: its sole OFF launch loaded turn 4, then failed strict argv qualification because preflight used `Indonesia` while the actual worker used `CIVILIZATION_INDONESIA`. The corrected v2 records actual native argv before checking it. The prelaunch missing-helper-path failure, v1 logs and all v2 raw events, decisions, read archives and outcomes remain preserved; no failed launch was silently relabeled a qualified game.

The original source game's Ternate Campus still supplies genuine flood-damage evidence. This later cold pilot supplies native activation and completion, not reproduction of that flood trajectory. The opt-in remains off by default, and this PR changes no deployment genome.
