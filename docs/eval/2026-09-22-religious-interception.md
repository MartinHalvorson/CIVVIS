# Intercept an urgent religious threat without a known enemy city

Native `civvis-20260922T051229Z-cont2` lost to Arabia's religious victory on turn 131 on source `9d8c98737c230390a97ad7c650c55b1506d296b6`. All seven of our cities followed Islam by turn 79. Arabia's cities were still undiscovered at turn 84, but its Missionaries were visible beside our army. The ordinary conquest-denial gate requires a known enemy city.

The prototype supplies a separate, immediate religious interception: an explicit Domination seat facing a religious match point may declare a legal war and condemn a currently visible religious unit at home. A speculative declaration, optional adjacent movement and condemnation must all succeed first. It retains affordability, home-threat and single-major-war gates, requires a healthy military unit with movement, and executes the promised interception rather than merely switching plans. No distant city siege is assumed.

## Native command evidence and a corrected interpretation

`Base/Assets/Gameplay/Data/UnitCommands.xml:47` registers `UNITCOMMAND_CONDEMN_HERETIC`; the shipped command has no target parameters. The existing bridge requests it on the military unit after co-location.

Retained strings in `Base/Assets/Text/en_US/InGameText.xml:3138–3142` describe founded-faith and majority-faith refusal reasons. An initial reading treated those strings as proof that the turn-83 interception was impossible. The actual runtime contradicts that conclusion: at turn 126, Arabian Apostle `5701640` explicitly carries `RELIGION_ISLAM`, all our cities follow Islam, and our Caravel `3145731` moves to `(44,10)`. The native event stream records `condemn_removed` at `05:41:14.875Z` and confirms `CONDEMN_HERETIC` through `order_verified` on turn 127. Therefore the unused/conditional text alone does not establish an unconditional prohibition, and no speculative simulator restriction is added here. This corrects the preliminary note committed during investigation.

## Validation in progress

The base regression produces the intended failure: diplomacy does not intercept the adjacent spreader when no city objective is known. The negative-control test passes. Candidate tests and recorded-board replays remain pending. A successful native condemnation supports the command pathway, not the strategic effectiveness of this new opening; no avoided loss or Domination victory is claimed.
