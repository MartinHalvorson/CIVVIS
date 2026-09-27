# Keep native Congress denial ahead of speculative self-claims

## Native trigger

The four-player King Gran Colombia game `civvis-20260927T132350Z`
recovered from turn 171 under `-cont1`, then lost to player 3's Diplomatic
victory at turn 202. This is one terminal game with one recovery, not two
independent measurements. It retained Yokohama and Vancouver, held Bogotá,
and held no foreign original capitals. No Bomber or Jet Bomber appeared.

At the turn-201 Congress, the controller observed leader 3 on 17 DVP,
314 Favor, and an affordable 13-vote ballot costing 312. The existing
threshold selected denial, then the unconditional large-bank self-claim
replaced it with option A targeting player 0. Native readback verified all
13 votes. The other two rivals cast nine votes each for B against player 3;
player 3 cast fourteen for A targeting itself. Option A won 27 to 18, with
player 3's fourteen-vote target block beating our thirteen. The rival's DVP
rose from 17 to 21 and the native victory event identifies Diplomatic
victory, team 3, `won: false`.

The same override selected seventeen self-votes at turn 181 against a
leader on 15 DVP. A historical twelve-vote claim threshold does not prove
that the current rival block is smaller.

## Candidate

The self-claim condition now also requires the leading rival to be below
`DiploVictoryVoteFloor`. At or above that configured threshold, the existing
full-budget denial keeps its option, target and vote count. The default
threshold remains 12. Below it, the existing claim and probe decisions
remain available. Host prices, Favor limits, submission, ordinary
resolutions, leader selection and explicit threshold overrides are unchanged.

The offline regression invokes the production ballot handler, including all
three resolutions and the final submission. It reproduced the old native
turn-201 self-claim before the fix, then verifies denial, the threshold
boundary, turn-181 inputs, small and empty banks, claims below the floor and
an explicit later threshold. With the *other recorded votes held fixed*,
our thirteen B votes give B 31 votes against A's 14. This is a ballot
counterfactual, not proof of a changed native trajectory or Domination win.

## Shipped authority

The actual installed `DLC/Expansion2/Data/Expansion2_Congress.xml:135`
defines `WC_RES_DIPLOVICTORY`, `WhichEffect="1"`,
`ModifierId="APPLY_INCREASED_DIPLO_VP_TO_PLAYER"`; line 136 defines
`WhichEffect="2"`, `ModifierId="APPLY_DECREASED_DIPLO_VP_TO_PLAYER"`.
Lines 378–380 bind `ADD_DIPLOMATIC_VICTORY_POINTS` to `Amount` 2, and
393–395 bind `SUBTRACT_DIPLOMATIC_VICTORY_POINTS` to `Amount` -2.
The recorded native ballot and outcome establish the option/target mapping
and the applied thirteen-vote request for this case.

## Validation and registered native preflight

All 69 discovered control-mod Lua suites passed under Lua 5.1 via the
installed Lupa runtime. Lua 5.1 compiled all 76 scripts in the control-mod
and legacy-mod roots. `cargo test --profile ci --locked` passed 4,423 tests,
zero failed, 53 ignored. Python validation is recorded with the final
preflight results before integration. The existing CI discovers the new
`*_test.lua` suite.

Before a candidate native replay, preserve the exact source, binary and
autosave hashes. The input is the already archived
`AutoSave_0170.Civ6Save`, SHA-256
`98929078deac0152bd93978a406a3df55964c75cd6dc8b68eeca97ae290e3184`.
The earlier baseline continuation actually loaded those same bytes at turn
170. Keep the Rust binary, all 21 force-on treatments, air-surge-2 on,
air-surge v1 off, four-player King Gran Colombia / Simón Bolívar, Tiny
Pangaea, Online, Gathering Storm and all victories enabled. Read the native
seat and restored turn back again; CLI flags alone do not prove them.

After the current fresh game finishes, run the candidate from the preserved
input. Verify the production request and native readback at the first
eligible Congress, retain every subsequent outcome, and play to the native
terminal event. A rejected or mismatched ballot must not be called an
applied fix. Native outcomes from a reused save are a diagnosis, not a
fresh-game strength screen; no general strength claim follows from one
trajectory. Candidate native preflight is pending at this checkpoint.

Evidence is retained under
`~/civvis-war-evidence-20260927/native-congress-denial-priority/`.
