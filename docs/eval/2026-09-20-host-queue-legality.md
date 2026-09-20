# Host-confirmed active production after resource payment

Native run `civvis-20260920T065948Z`, King, Gran Colombia, four-player Tiny Pangaea, domination target, revision `28471afa6121621e6722f37d381834cb8cbfba00`.

Guayaquil (host city 131073) starts a bombard at turn 118 frame 0 with 21 niter. Frame 1 reports the bombard as active with 11 niter; frame 2 reports it active with 1 niter. At turn 119 frame 0 it remains active with 3 niter and is explicitly present in the host buildable menu. Nevertheless the AI journals “replaces unavailable bombard with cuirassier” and transmits the cuirassier order. Frame 1 confirms the replacement.

The production-commitment repair calls `Game::can_produce`, which also checks local start costs. The host menu is a negative gate there, so a positive menu entry does not override the local resource check. Native reconstruction does not populate the local strategic-resource commitment for this already started unit.

The correction should preserve a current queue item explicitly allowed by the host menu, unless the host has explicitly refused it. It must not change new-start legality, ordinary simulation boards, or the same-turn production guard. A separate turn-102 trebuchet proposal was suppressed by that guard after an industrial-zone order; that is distinct from this turn-119 replacement.

Evidence: `events.jsonl`, `decisions.jsonl`, and `why.log` in the run directory; extracted trace in `army-approach-review.json`. This observation establishes a production cancellation, not proof that preserving this bombard wins the siege or game.

Focused validation: the paid-resource regression fails on the original implementation once the fixture includes a legal alternative build. All 14 production-commitment tests pass after the fix, including explicit-refusal precedence and absence of a host menu. Full-suite and frozen native replay validation are pending.
