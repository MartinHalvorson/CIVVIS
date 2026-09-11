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

## What the next experiments must distinguish

A victory count alone hides the stalled stage. Record the turn of first
Conquest plan, first declaration, first foreign original-capital capture,
subsequent capital captures and any capital losses. Also record whether the
last required capital was reachable and whether the army had a surviving
capture unit and a wall-breaking unit. Those observations distinguish an
opening that never hands over from a campaign that starts but cannot finish.

The first follow-up is the opening handover: the native probe failures include
worlds ending with four cities per major. `domination-lane-hands-over`, now on
trunk, is an independent opt-in for leaving that expansion plateau. Test a
capability change with the handover held equal in both arms; changing both at
once cannot identify which mechanism contributed.

The next hypothesis is conditional redeployment after a capital capture.
`one_war_prizes_in_reach` counts ordinary city damage and reachable pillage,
while `one_war_presses` can use those prizes to extend a profitable war. A
redeployment experiment must retain urgent defense, rival-victory denial and
necessary loyalty-support captures. A captured capital that promptly flips
back is not completed progress, and a peace treaty may constrain a later
recapture. Measure retained capital control and completion time, not only
cities taken or units killed.

Neither proposal justifies an unconditional capital rush. A future default
promotion needs a predeclared matched batch under the deployment profile,
showing fewer stalled campaigns without more home-capital losses or occupation
failures. Native symmetric-lane probes and standard screen reach are useful
checks, but they do not substitute for that comparison.

## The capital-focus gene

The previous planner computed the cheapest missing capital across every rival
but selected its target player independently. It used that capital only if
its owner happened to match the chosen rival. Otherwise it ranked ordinary
cities. In a multi-rival game, an easier unengaged rival could therefore erase
the capital priority inside an active war. At peace, generic opponent value
could send the next campaign after a capital's former owner who retained only
ordinary cities.

`domination-capital-focus` ranks required capitals within the selected
opponent's current holdings. An easier capital belonging to an unrelated
rival cannot erase this front's capital priority. Emergency, rush, denial,
city-campaign and existing siege-commitment overrides retain their precedence.
The gene applies to the explicit Domination contract. It is default-off and
independently reversible; the existing global-ranking fallback remains the
control. No deployment promotion is part of this change.

An initial version also made the cheapest missing capital choose the next
opponent. That broader policy was rejected: four paired three-player seeds
completed Domination once on the baseline and zero times with the initial
change, and the exposed-city-state test demonstrated that it removed a useful
intermediate conquest. A second probe retaining only focus within the chosen
front also completed zero of those four games. These samples are not a
statistical verdict, but they provide no basis to change the default. The
within-front behavior is therefore an opt-in hypothesis for the normal screen,
not a claim that always taking a capital first is optimal. Opponent selection
keeps its existing cost, feasibility and staging tradeoffs.

Main subsequently added capital-owner priority as a separate change. The
final gene preserves that opponent choice: it still computes the global
capital for opponent selection, then ranks within the chosen front only when
this option is enabled. A regression checks that enabling focus cannot remove
main's capital-owner priority. The earlier broad-policy probes above describe
their historical base, not the current integrated policy.

## Validation

The full Rust suite passed 3,576 tests (52 ignored), including the focused
capital-routing and reversible-default checks. The changed Rust passed Clippy
without diagnostics. All 14 append-point checks passed, as did the generated
gene and evaluation-manifest consistency checks.

The four matched native seeds used three players, a 36×22 map and Online speed
with a 250-turn limit. Baseline completed one Domination victory; each explored
capital-priority policy completed zero. A separate six-player deployment-profile
probe completed zero of four on the baseline and broad policy, and zero of two
on the within-front policy. These are completed non-Domination outcomes, not
engine crashes. They do not establish a benefit.

A predeclared 24-game standard fieldless screen (six players, 74×46 Continents,
Online 250, Emperor, nine city-states; seeds 109103000–109103023) completed
all 24 games / 144 seats. The committed
[screen artifact](../gene_screens/fires/domination-capital-focus.json)
preserves its original clean source `1c28388674e5` and binary provenance;
it predates the integration described below. Its 35 on / 109 off seats
show a win contrast of +15.73 percentage points (SE 8.40) and share contrast
+2.54 points (SE 1.74). This small mixed-target, independent-seat screen
passes the repository reach ratchet; it does not establish a Domination
win-rate benefit for the integrated code or justify deployment promotion.
After final main integration, `cargo check --profile ci --locked`,
generated-metadata checks, and the gene-fires ratchet and unit suite pass.

After integrating current main and fixing that interaction, the full suite
passed **3,593 tests** (52 ignored), including the capital-owner preservation
regression. A new matched eight-seed Prince native probe used seeds
109108000–109108007, three players, 36×22, Online 250 and
`domination-lane-hands-over` on in both arms. Capital focus completed Domination
in **1/8** worlds versus **2/8** with the option off. Both arms used source
`584c2d573`; current main's opponent-selection and terminal-capture rules were
held equal. This also provides no basis for a default promotion.
