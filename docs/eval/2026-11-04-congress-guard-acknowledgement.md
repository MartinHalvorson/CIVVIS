# Acknowledge the consumed Congress guard

Native Standard Immortal run `civvis-20261104T121104Z` recorded 1,021
`combat_policy_not_acknowledged` failure reports for
`CONGRESS_GUARDS_THE_LEADER`. This counts raw reports, which may repeat an
issued directive, rather than independent failed host operations.

The controller already sets `cfg.DiploVictoryGuardLeader = true` and removes
the metadata row. Unlike the survival and dialogue directives, it emits no
`combat_policy_applied` event. The decider's existing
`verify_order_with_context` contract requires that event with the issued turn
and matching policy verb. The missing acknowledgement therefore turns a
consumed policy into a verification failure.

Emit the existing acknowledgement immediately after setting the lease. Keep
the directive outside action counts. An already enabled guard without a fresh
directive emits no new receipt. This changes CIVVIS verification metadata; no
game API, ballot selection, vote budget, combat rule or default is changed.

All six recorded Diplomatic Victory ballot verdicts in the source run matched
their requested option, target and count. At the last ballot, we had zero Favor
and cast one free B vote against the leader; A won 26 to 24 and the rival won
Diplomatic Victory at turn 368. That is a separate strategic limitation. The
receipt fix does not establish that the guard changed a ballot or that the
rival victory would have been prevented.

The regression first fails exactly the two missing policy/turn acknowledgement
assertions, then passes with the fix. It also checks the no-directive case,
persistent lease without a fresh receipt, and unchanged action counts. All 115
discovered mod fixtures and syntax checks for 122 Lua files pass under Lua 5.1.5.
The optional approximate scope checker reports the same 169 findings before and
after the change; it is not claimed as a clean scope check.

Native confirmation must observe the acknowledgement and corresponding decider
verdict after the updated mod is loaded in a new game. Unit fixtures establish
the protocol fix; they do not establish a live-game or strength result.

Source summary SHA-256:
`5b8344b975b6d478e5b501fc7dd519597d61b07f1c17b763b9e2d76d28e49c94`. Native evidence and before/after test logs are retained
in the operator archive.
