# A Scout for the known Bomber-supply frontier

Target: Prince (level 4), Simón / Gran Colombia, four-player Tiny Pangaea,
Domination. Other victory conditions remain enabled. This is a supply-path
repair, not evidence of a Domination win.

## Native observation

Run `civvis-20260926T222220Z` used source
`3d8d41e43c982b92bd703c1d503c6e86e1a081e5` and decision binary SHA-256
`4e77629863cd59acc2de6201cd241f03fcf69fd4d221a2e05ed011b5e851ad93`.
It ended at turn 218 with a Culture loss and score 1025.

The immutable turn-162/frame-0 observation prefix is at
`~/civvis-tactics-results/2026-09-26/air-readiness/222220-turn162-frame0/`:
16,936 lines, SHA-256
`f411b837a4bb39b0aa423ff0cacb65709899ff879fcabba6ce48d46ba40cbf28`.
Flight, Radio and Advanced Flight were researched; Cali had a completed,
unpillaged Aerodrome. Aluminum stock and income were both zero. The ten-city
empire had no Scout or Settler, seven Builders, 530 Gold and +86.75 Gold/turn.
Its observed purchase menus legally offered a Skirmisher for 300 Gold.

The known unclaimed Aluminum center at offset `(6,23)` had 83 uncharted
plots within the nine-tile Loyalty neighborhood. A diagnostic speculative
city had +17 Loyalty/turn, but that forecast cannot certify unseen rivals.
The normal remote-frontier settlement veto therefore correctly rejected it.
The other known deposit at `(9,22)` had a negative speculative Loyalty rate;
scouting must not be treated as permission to found there.

## Existing mechanism checked first

A one-shot replay resets production ownership and therefore is not the native
controller's persistent decision. Both one-shot arms suggested a Settler.
That is not evidence that the observed controller would build one.

The stronger diagnostic streamed all 436 observed state frames through the
original native binary with `--serve --fresh-board --explain --victory
domination --civ CIVILIZATION_GRAN_COLOMBIA` and the exact twenty deployment
force-on rows. The comparison added only `--with recon-replacement`.
The default persistent turn-162 answer produced only Cali's Shipyard and no
recon unit. The forced-recon answer reserved Maracaibo's Skirmisher and also
produced a Skirmisher at Bogotá; neither arm produced a Settler at that frame.
Thus generic replacement does work when enabled, but its ledger-held global
quota is not necessary to address this specific known supply frontier.

Replay evidence is archived at
`~/civvis-tactics-results/2026-09-26/air-resource-expedition-pr3793/`, containing
the script, complete decision/why streams and SHA-tagged provenance for both
arms. Historical commands retain their original temporary paths.
These are observed-history decisions: future native states stay fixed. They
do not demonstrate different survival, completed reconnaissance, a colony,
Aluminum income, Bombers or victory.

## Candidate scope

One resource-specific reconnaissance expedition uses the existing legal
purchase/production and exploration paths. It must preserve the ordinary
settlement Loyalty veto; acquiring information is not bypassing that veto.
An owned connection backlog stays a Builder job. A healthy connected source
whose income cannot support the committed wing no longer suppresses the
expedition. No hidden deposit or resource whose reveal technology is missing
can demand it. Civilian guards, healing and threatened-city defense retain
their existing priority. No resource-colony Settler is added.

## Validation and limits

Nine focused tests cover legal acquisition, reveal/knowledge gating, owned
connection backlogs, an insufficient healthy source, known-fog targeting,
treasury and queue reservations, guard retention, real exploration movement,
and clearing an all-grassland remote neighborhood within 60 ticks. Acquisition
and the insufficient-healthy-source case were observed red before repair.
The grassland fixture does not prove that a Scout can reveal every coastal or
ocean plot in the native nine-tile neighborhood.

After merging `98a1749f6c0cbe6bcb6d3d72e11c9a98b1cd1bbb` once, the integrated
suite passed 4,352 tests with 53 ignored; incremental changed-line quality
passed. This is correctness evidence, not a strength result.

The parent source was `57850ab0603b46a379d13a976981bbb2e6689949`. Replaying the
same 436 native observations changed six Scout purchase frames at turns 153,
157 and 160. The first change was Cartagena's legal Skirmisher purchase at
turn 153/frame 409. The reviewed candidate produced exactly the same 436
decision objects as the initial candidate on this archive: its new
healthy-owned-supply case is not exercised by these observations. Archived
refusals and future states remain fixed, so purchase orders are not proof of
a completed native acquisition.

The pre-registered pilot used seeds `37930000`–`37930003`, with no substitution,
and the frozen external harness at `9dda6319d413cb11bc5fdda55dfa487a6c1f4c11`.
Its primary contrast is the same `strike-reach`-on rows across source
libraries, not the within-binary off/on contrast. Profile: Prince player and
barbarians, Gran Colombia focal Domination controller, four majors, 60×38 Tiny
Pangaea, six city-states, Online, all victories enabled and a 250-turn cap.
Both adaptive opponents and the focal seat consume each library change.

The initial candidate `8f1f363240dab8591cb7c0ee0955b4cc32bceeee` preserved all
four primary reported outcome records after excluding applied-action counts,
with zero Domination wins. Its secondary off arm changed seed `37930003`.
The reviewed production repair, checkpoint `d3d2f2df97a2f8a155dfa44b2738dbb97a2370b2`,
had these primary results:

| Seed suffix | Parent outcome / score / final foreign capitals | Reviewed outcome / score / final foreign capitals |
| --- | --- | --- |
| 000 | Religious loss t162 / 612 / 0 | unchanged |
| 001 | Score win t250 / 1189 / 1 | Score loss t250 / 1001 / 0 |
| 002 | Science loss t239 / 773 / 0 | unchanged |
| 003 | Science loss t241 / 1348 / 1 | unchanged |

Both versions had zero Domination wins. At seed `37930001`, observed foreign
major cities fell from six to five and the final foreign capital was lost.
Secondary off-arm outcomes changed seeds `37930000` and `37930003`; all are
retained. The harness records outcomes/action counts, not complete action
transcripts. The source pilot froze the original twenty force-on rows; the
integrated main has a twenty-first `builders-work-through-raiders` row, so it
is not an end-to-end test of that newer deployment bundle.

## Decision

PR #3793 remains draft and withheld. The healthy-source extension has a
reported campaign regression whose cause is not yet established. Neither
successful information acquisition nor a completed colony, Aluminum income,
Bomber or Domination win has been demonstrated. Do not deploy this candidate
on test-suite success or the unchanged initial pilot alone.
