# Naval demand follows the exposed coast

Native Emperor run `civvis-20261009T143233Z`, revision `46fa42f32163160d977320b0fb21b5642bf2869a`, lost to Culture at turn 186. At turn 120 it was at peace with every major, held four ships, and started another Ironclad. The mirrored fleet budget was nine because eight cities could launch ships and distant barbarians on water activated the full naval-war budget. All four visible barbarian ships were 21–23 tiles from the nearest launch site.

The current controller’s open-water fleet mode uses the existing six-tile home-threat radius to count launchable cities exposed to barbarians on water, with one spare ship. Distant raiders leave the exploration/settler budget in place. Wars against civilizations retain the existing empire-wide fleet budget, including coastal enemy cities. Harbor and lake launch rules remain shared with both production choosers. The frozen `advanced_v1` controller and explicit open-water withholding keep the historical fleet budget; the anchor’s recorded behavior must continue to match.

A read-only reconstruction of six captured frame-zero boards projected demand 9 → 2 at turns 105, 120 and 127; 9 → 3 at turn 132; 9 → 4 at turn 137; and 9 → 9 at turn 150 after civilization wars began. These prefixes are recorded board reconstructions, not exact native decision-input captures or executed counterfactual games.

Financial evidence is diagnostic: treasury Gold was 91 at turn 150, net income 6.40234, and the host refused Catapult upgrades quoted at 165–330 Gold. At turn 140 ships summed to 41 Gold **before policy discounts**; the treasury reported 62 Gold of total unit maintenance **after discounts**. They are not comparable shares. No savings or additional affordable upgrade is claimed from that subtraction.

Nine focused cases cover distant raiders, local raids, multiple exposed coasts, duplicate raiders, civilization wars, missing Sailing/launch sites, exploration/settler budgets, harbors, lake withholding and the frozen launch mode. Results and complete repository validation are recorded in the PR after they finish.

No native win-rate gain has been established. Five completed games on the observed revision produced zero wins, four production retirements and one Culture defeat. The challenge settings and retirement denominator are unchanged.
