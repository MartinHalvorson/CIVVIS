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

Replay evidence: `/tmp/civvis-air-expedition-replay.tRmc1V/`, containing the
script, complete decision/why streams and SHA-tagged provenance for both arms.
These are observed-history decisions: future native states stay fixed. They
do not demonstrate different survival, completed reconnaissance, a colony,
Aluminum income, Bombers or victory.

## Candidate scope

One resource-specific reconnaissance expedition uses the existing legal
purchase/production and exploration paths. It must preserve the ordinary
settlement Loyalty veto; acquiring information is not bypassing that veto.
An owned connection backlog stays a Builder job. No hidden deposit or resource
whose reveal technology is missing can demand an expedition. Civilian guards,
healing and threatened-city defense retain their existing priority.

Implementation and validation are in progress. No candidate is approved for
deployment by this document yet.
