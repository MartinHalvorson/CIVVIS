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
zero failed, 53 ignored. `PYTHONPATH=tools python3 -m unittest test_ci_wiring
test_docs_commands test_docs_reference_live_commands test_civ6_play
test_civ6_control_install` passed all 387 tests. The existing CI discovers
the new `*_test.lua` suite.

Before the candidate native replay, the exact source, binary and autosave
hashes were preserved in `registration.json`. The input was the archived
`AutoSave_0170.Civ6Save`, SHA-256
`98929078deac0152bd93978a406a3df55964c75cd6dc8b68eeca97ae290e3184`.
The earlier baseline continuation actually loaded those same bytes at turn
170. Candidate Lua was frozen at `bc34b9a82eae0c3f243272611c7c89642e42beaa`;
the installed script matched that source after its configuration prelude.
The unchanged Rust executable was frozen at SHA-256
`d0bd1f27f3ffb18c5e76bb6e4a4ec12f5047188e7b1f173e03fb4da74b3e9483`.
Its baseline and candidate digests match; revisions `3ec0a0780` and
`3775e9be5` differ only in recovery-save instrumentation and its records.

The replay `civvis-pr3828-save170-20260927T144218Z` actually restored turn
170. Native readback verified four-player King Gran Colombia / Simón
Bolívar, Tiny Pangaea, Online, Gathering Storm, all victories enabled and
the saved game's 250-turn limit. The genome seated all 21 force-on
treatments, air-surge-2 on and air-surge v1 off. CLI flags alone were not
used to establish the seat or restored turn.

At Congress 181, the actual bank was again 569 Favor and player 3 again
had 15 DVP. The candidate requested seventeen B votes against player 3,
costing 544. Native `wc_ballot_verdict` verified the count, option and
target; both ordinary one-vote resolutions also registered correctly.
B won 29 to A's 7, targeted player 3, and its DVP fell to 13.

At Congress 201, the actual bank was 311 Favor and the leader had 15 DVP.
The host budget afforded twelve votes costing 264. Native readback
verified all twelve B votes against player 3. B won 15 to A's 10 and the
leader remained on 15 DVP. Subsequent rival votes and banks differ from
the baseline; this is an actual changed trajectory, not the fixed-vote
counterfactual above.

The replay froze at turn 208 after GDR-versus-city combat. Its imported
outer watcher confirmed 900 seconds without turn progress, cleaned up the
owned game and preserved the selected turn-207 autosave. That reload
restored turn 207 and froze at the same combat boundary. The host watchdog
diagnosed the direct player but withheld its signal because the diagnostic
parent is outside the standard climb entry point.

Before the second recovery, an operational handoff amendment was recorded:
the operator verified the exact player fingerprint, cleanup receipt,
installed tag and source pins, repeated combat freeze, and silence beyond
240 seconds in both native and copied logs. Only that owned player was
interrupted; the existing recovery loop selected and preserved the older
turn-204 save. This changes recovery timing, not the voter, binary, source
or six-resume budget. The turn-204 reload escaped the freeze.

The native terminal event was a **Science loss at turn 221**, team 1,
`won: false`; owned cleanup and `game_stopped: true` were verified. The
retained path covers all 52 turns from 170 through 221, uses the root
through 203 and `-cont2` from 204, and excludes the rolled-back `-cont1`
observations. It held Bogotá, the two foreign major cities already present
in the input, and zero foreign original capitals. Three run segments are
one reused-save diagnosis, not three independent games.

An additional fresh baseline game, `civvis-20260927T135851Z`, reproduced
the override at Congress 201: 305 Favor, a rival on 13 DVP and twelve
verified A/self votes. That game ended in Religious loss at turn 203,
held no foreign cities or original capitals, and had two recovery segments.
It is a separate observation of the trigger, not a paired strength screen.

The native request, readback and outcome support shipping this correctness
fix. The replay avoided the baseline's turn-202 Diplomatic ending, then
lost by another victory route. No Domination win or general strength gain
is claimed. Setup failures, recovery timing amendment, every segment and
both complete endings are retained without filtering.

Evidence is retained under
`~/civvis-war-evidence-20260927/native-congress-denial-priority/`.
