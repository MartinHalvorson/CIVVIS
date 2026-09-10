# Domination: turn military advantage into original-capital control

This is a code-grounded strategy audit and implementation record, not proof
of an optimal policy or a measured live win-rate improvement.

## What must be true to win

In a non-team world game, `Game::check_domination` requires the winner
to control every major civilization's original capital, including its own.
Eliminating every rival city is unnecessary. A capital conquered by a third
party remains an objective against its current owner. Captured capitals must
remain held while the army takes the others. The engine separately handles
teams, arenas, and civilizations defeated before founding a capital; those
rules should not be inferred from the ordinary world-game condition.

A useful campaign therefore needs all of the following: an original-capital
objective, a reachable approach, enough wall damage, a surviving capture unit,
sustainable replacements and upgrades, and a way to hold the captured city.
Military power alone establishes none of those conditions. An army's size is
an input to a campaign; capital control is the outcome.

## A path through the game

1. **Opening:** establish production, income, research and a defensible core;
   scout rivals and approaches. Use a nearby conquest window when the existing
   early-conquest checks can assemble a viable force before enemy defenses
   close it. A fixed wait until late specialization forfeits that window, but
   an unconditional rush can sacrifice the economic base needed for later
   capitals. The existing early-conquest opening and expansion schedule are
   the mechanisms to test against these alternatives.
2. **Preparation:** select the next missing original capital and its current
   owner. Budget an army against that city and its defenders, reserve a capture
   unit and the appropriate wall-breaking capability, and stage the force
   before declaring. The domination mobilization exception already allows an
   understrength army to name a preparation objective while preserving the
   declaration gate. Income recovery must precede discretionary upkeep when
   the empire cannot fund its existing troops.
3. **Campaign:** concentrate on one executable front, preserve a useful siege
   commitment while marching, and protect the capture unit. Take an ordinary
   city when it is a necessary approach or occupation-support step; do not
   confuse convenient conquests with progress toward the final condition.
   Wall technology and land/sea access determine which units can do the job.
4. **Consolidation:** hold original capitals, restore damaged income and
   production, heal and upgrade survivors, then move the next campaign toward
   another missing capital. The current one-war policy can continue a
   profitable war after its original-capital objective is gone; pricing peace
   and redeployment against further conquest remains a separate hypothesis.
5. **Completion:** focus the remaining objectives, including recapture of our
   own original capital. Maintain enough defense to avoid trading away an
   earlier capital while the main force travels. Rival victory clocks impose
   a real deadline; urgent denial may still need to override the preferred
   capital sequence.

The best sequence depends on distance, defenses, technology, terrain, income,
loyalty and rival clocks. The implementation uses existing campaign costs and
legality checks; this audit does not claim that their weights are optimal.

## Routing defect repaired in this change

The previous planner computed the cheapest missing capital across every rival
but selected its target player independently. It used that capital only if
its owner happened to match the chosen rival. Otherwise it ranked ordinary
cities. In a multi-rival game, an easier unengaged rival could therefore erase
the capital priority inside an active war. At peace, generic opponent value
could send the next campaign after a capital's former owner who retained only
ordinary cities.

When a war or other priority names a rival, capital ranking now runs within
that rival's current holdings. An easier capital belonging to an unrelated
rival cannot erase this front's capital priority. Emergency, rush, denial,
city-campaign and existing siege-commitment overrides retain their precedence.
The change applies to the explicit Domination contract and adds no deployment
promotion or new numerical weight.

An initial version also made the cheapest missing capital choose the next
opponent. That broader policy was rejected: four paired three-player seeds
completed Domination once on the baseline and zero times with the initial
change, and the exposed-city-state test demonstrated that it removed a useful
intermediate conquest. This small sample is not a statistical verdict, but it
provides no basis to impose an unconditional opponent priority. The retained
fix repairs capital ranking inside the selected opponent; opponent selection
keeps its existing cost, feasibility and staging tradeoffs.

## Validation

Pending focused regression tests, full Rust tests and an end-to-end native
Domination probe. A native probe establishes execution and observed outcomes;
it does not establish live-game strength or isolate a win-rate effect without
a paired baseline.
